//! Composition of gameplay, streaming, input, and native presentation adapters.

mod audio;
mod native;
mod pacing;
mod persistence;
mod screens;
mod session;
mod smoke;
mod time_console;

use std::error::Error;
use std::io;
use std::sync::Arc;
use std::time::Instant;

use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop};
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::window::{Fullscreen, Window};

use crate::game::{Game, GameMode};
use crate::hud::Screen;
use crate::input::{Action, InputState};
use crate::options::LaunchOptions;
use crate::renderer::Renderer;
use crate::streaming::ChunkStreamer;
use audio::Audio;
use pacing::FramePacing;
use persistence::Persistence;
use smoke::{SMOKE_TIMEOUT, SmokeState, size_changed, smoke_timed_out};

mod frame;

/// Boots the native adapter. A smoke launch exercises real window presentation.
pub fn run(options: LaunchOptions) -> Result<(), Box<dyn Error>> {
    options.validate_time().map_err(io::Error::other)?;
    if options.audio_check {
        return audio::listening_check().map_err(|error| io::Error::other(error).into());
    }
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "macos")]
    builder.with_activation_policy(ActivationPolicy::Regular);
    let event_loop = builder.build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.listen_device_events(DeviceEvents::WhenFocused);
    let mut app = App::new(options);
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error.take() {
        return Err(io::Error::other(error).into());
    }
    Ok(())
}

struct App {
    options: LaunchOptions,
    game: Game,
    persistence: Persistence,
    audio: Audio,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    streamer: Option<ChunkStreamer>,
    input: InputState,
    screen: Screen,
    cursor: Option<[f32; 2]>,
    focused_button: usize,
    resume_requested: bool,
    pointer_authorized: bool,
    inventory_resume_authorized: bool,
    scroll_remainder: f64,
    session_time: f32,
    time_console: time_console::TimeConsole,
    captured: bool,
    focused: bool,
    occluded: bool,
    zero_sized: bool,
    frame_deadline: Option<Instant>,
    frame_pacing: FramePacing,
    activate_on_next_event: bool,
    started: Instant,
    last_frame: Instant,
    fps_started: Instant,
    fps_frames: u32,
    fps: f32,
    presented_frames: u32,
    offscreen_frames: u32,
    smoke: SmokeState,
    finished: bool,
    error: Option<String>,
}

impl App {
    fn new(options: LaunchOptions) -> Self {
        let mut game = Game::new(options.seed);
        let _ = game.time_mut().set_hour(options.time_of_day);
        let _ = game.time_mut().set_speed(options.time_speed);
        let screen = if options.smoke_test {
            Screen::Playing
        } else {
            game.player.position.y += 8.0;
            game.player.pitch = -0.42;
            Screen::MainMenu
        };
        let smoke = SmokeState::new(game.player.position);
        let now = Instant::now();
        let persistence = if options.smoke_test || cfg!(test) {
            Persistence::disabled()
        } else {
            Persistence::local()
        };
        let focused_button = usize::from(!persistence.can_continue());
        let mut audio = Audio::new(!options.smoke_test && !options.audio_disabled && !cfg!(test));
        audio.load_preferences(!options.smoke_test && !cfg!(test));
        Self {
            options,
            game,
            persistence,
            audio,
            window: None,
            renderer: None,
            streamer: None,
            input: InputState::default(),
            screen,
            cursor: None,
            focused_button,
            resume_requested: false,
            pointer_authorized: false,
            inventory_resume_authorized: false,
            scroll_remainder: 0.0,
            session_time: 0.0,
            time_console: time_console::TimeConsole::default(),
            captured: false,
            focused: false,
            occluded: false,
            zero_sized: false,
            frame_deadline: None,
            frame_pacing: FramePacing::new(now),
            activate_on_next_event: false,
            started: now,
            last_frame: now,
            fps_started: now,
            fps_frames: 0,
            fps: 0.0,
            presented_frames: 0,
            offscreen_frames: 0,
            smoke,
            finished: false,
            error: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl std::fmt::Display) {
        self.error = Some(error.to_string());
        self.finished = true;
        self.release_pointer();
        event_loop.exit();
    }

    fn record_exit(&mut self) {
        self.audio.shutdown();
        // macOS native Quit can reach exiting directly, without a close request or Drop.
        match self.persistence.save_active(&self.game, Instant::now()) {
            Ok(()) => self.persistence.end_world(),
            Err(error) if self.error.is_none() => {
                self.error = Some(format!("Could not save the world on exit: {error}"));
            }
            Err(_) => {}
        }
        if self.options.smoke_test && !self.finished && self.error.is_none() {
            self.error = Some("smoke test exited before completing verification".into());
        }
    }

    fn gameplay_action(&mut self, action: &Action) {
        if self.screen != Screen::Playing || !self.captured || !self.focused {
            return;
        }
        match action {
            Action::ToggleFlight => self.game.toggle_flight(),
            Action::ToggleHeadlamp => self.game.toggle_headlamp(),
            Action::CycleSpeed => self.game.cycle_speed(),
            Action::CycleTimeSpeed => {
                self.game.time_mut().cycle_speed();
                println!("Debug: world time {}", self.game.time().label());
            }
            Action::Respawn => {
                self.game.respawn();
                self.input.clear();
            }
            Action::SelectSlot(slot) if self.game.mode() == GameMode::Survival => {
                self.game.inventory_mut().select(*slot);
            }
            _ => {}
        }
    }

    fn action(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        if self.options.smoke_test && action != Action::Fullscreen {
            return;
        }
        self.gameplay_action(&action);
        match action {
            Action::Pause => self.escape(),
            Action::Inventory => self.toggle_inventory(),
            Action::Crafting => self.open_crafting(),
            Action::TimeConsole => self.open_time_console(),
            Action::Navigate(direction) => self.navigate(direction),
            Action::Activate => self.activate_button(self.focused_button, event_loop),
            Action::Fullscreen => {
                if let Some(window) = &self.window {
                    let fullscreen = window.fullscreen().is_none();
                    window.set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
                }
                self.refresh_window_pacing(Instant::now(), true);
            }
            Action::Quit
                if !self.captured
                    && !matches!(
                        self.screen,
                        Screen::Inventory | Screen::Crafting | Screen::Storage
                    )
                    && !matches!(self.screen, Screen::Audio(_))
                    && self.screen != Screen::TimeConsole
                    && self.save_before_exit() =>
            {
                event_loop.exit();
            }
            _ => {}
        }
    }

    fn simulate(&mut self, dt: f32) {
        if self.options.smoke_test {
            self.simulate_smoke();
            self.session_time += 1.0 / 60.0;
        } else if self.screen.is_menu_preview() {
            if dt.is_finite() && dt > 0.0 {
                self.game.player.yaw =
                    (self.game.player.yaw + dt.min(0.1) * 0.025).rem_euclid(std::f32::consts::TAU);
            }
        } else if self.screen == Screen::Playing && self.captured && self.focused {
            if dt.is_finite() && dt > 0.0 {
                self.session_time += dt.min(0.25);
            }
            if self.input.take_jump_release() {
                self.game.player.clear_input();
            }
            let input = self.input.movement(self.game.player.flying);
            self.game.update(input, dt);
            if self.input.take_mining_release() {
                self.game.cancel_mining();
            }
            let mining = self.input.mining_held();
            let place = self.input.take_place_press();
            self.game
                .interact_with_modifier(mining, place, self.input.place_modifier(), dt);
            if self.game.storage_view().is_some() {
                self.release_pointer();
                self.inventory_resume_authorized = true;
                self.screen = Screen::Storage;
            }
        }
    }
}

#[cfg(test)]
mod tests;
