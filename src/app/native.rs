//! Native window/input lifecycle; gameplay remains renderer-independent.

use super::{App, SMOKE_TIMEOUT, size_changed, smoke_timed_out};
use crate::renderer::Renderer;
use crate::streaming::ChunkStreamer;
use crate::world::ChunkCoord;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{DeviceEvent, DeviceId, ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::PhysicalKey;
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActiveEventLoopExtMacOS, WindowAttributesExtMacOS};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId, WindowLevel};

impl App {
    pub(super) fn resize(&mut self, size: PhysicalSize<u32>) {
        self.zero_sized = size.width == 0 || size.height == 0;
        if self.zero_sized {
            self.release_pointer();
        }
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(size);
        }
        self.refresh_window_pacing(Instant::now(), true);
        if self.smoke.step.is_some_and(|step| step >= 30)
            && size_changed(self.smoke.size_before, size)
        {
            self.smoke.resized = true;
        }
    }

    pub(super) fn refresh_window_pacing(&mut self, now: Instant, force: bool) {
        if !force && !self.frame_pacing.refresh_due(now) {
            return;
        }
        let refresh = self
            .window
            .as_ref()
            .and_then(|window| match window.fullscreen() {
                Some(Fullscreen::Exclusive(mode)) => Some(mode.refresh_rate_millihertz()),
                _ => window
                    .current_monitor()
                    .or_else(|| window.primary_monitor())
                    .and_then(|monitor| monitor.refresh_rate_millihertz()),
            });
        if self.frame_pacing.update(refresh, now) {
            self.frame_deadline = self
                .frame_deadline
                .map(|_| now + self.frame_pacing.interval());
            println!(
                "Render limit: {:.3} Hz (current display, VSync enabled)",
                f64::from(self.frame_pacing.refresh_millihertz) / 1000.0
            );
        }
    }

    pub(super) fn release_pointer(&mut self) {
        let was_captured = self.captured;
        self.cursor = None;
        if let Some(window) = &self.window {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
            if was_captured && self.focused && !self.occluded && !self.zero_sized {
                let size = window.inner_size();
                let center = PhysicalPosition::new(size.width / 2, size.height / 2);
                if window.set_cursor_position(center).is_ok() {
                    self.cursor = Some([center.x as f32, center.y as f32]);
                }
            }
        }
        self.captured = false;
        if self.screen == crate::hud::Screen::Playing {
            self.screen = crate::hud::Screen::Paused;
            self.focused_button = 0;
        }
        self.resume_requested = false;
        self.pointer_authorized = false;
        self.inventory_resume_authorized = false;
        self.scroll_remainder = 0.0;
        self.input.clear();
        self.game.clear_input();
        if matches!(
            self.screen,
            crate::hud::Screen::Inventory | crate::hud::Screen::Crafting
        ) {
            self.game.inventory_mut().close_cursor();
        }
        self.audio.suspend();
        self.frame_deadline = None;
        self.last_frame = Instant::now();
    }

    pub(super) fn capture_pointer(&mut self) {
        if !std::mem::take(&mut self.pointer_authorized) {
            return;
        }
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        // On macOS, warping after locking re-associates the cursor. Center first.
        let _ = window.set_cursor_position(PhysicalPosition::new(size.width / 2, size.height / 2));
        let grab = window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
        match grab {
            Ok(()) => {
                window.set_cursor_visible(false);
                self.captured = true;
                self.input.clear();
                self.game.clear_input();
                self.last_frame = Instant::now();
            }
            Err(error) => {
                eprintln!("Cannot capture the mouse: {error}. Click the window to retry.");
                window.set_cursor_visible(true);
                self.captured = false;
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("RustCraft")
            .with_active(!self.options.offscreen)
            .with_visible(!self.options.offscreen)
            .with_window_level(if self.options.smoke_test && !self.options.offscreen {
                // Automated launches can stay behind the harness without desktop activation permission.
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            })
            .with_inner_size(LogicalSize::new(1280.0, 720.0))
            .with_min_inner_size(LogicalSize::new(640.0, 400.0));
        #[cfg(target_os = "macos")]
        let attributes = {
            event_loop.set_allows_automatic_window_tabbing(false);
            attributes
                .with_accepts_first_mouse(true)
                .with_borderless_game(true)
        };
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };
        self.focused = window.has_focus();
        self.window = Some(Arc::clone(&window));
        self.refresh_window_pacing(Instant::now(), true);
        if !self.options.offscreen {
            window.focus_window();
        }
        let renderer = match pollster::block_on(Renderer::new(
            Arc::clone(&window),
            event_loop.owned_display_handle(),
        )) {
            Ok(renderer) => renderer,
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };
        let center =
            ChunkCoord::from_world(self.game.player.position.x, self.game.player.position.z);
        self.streamer = match ChunkStreamer::new(
            self.game.terrain,
            center,
            if self.options.smoke_test {
                self.options.render_distance
            } else {
                3
            },
        ) {
            Ok(streamer) => Some(streamer),
            Err(error) => {
                self.fail(event_loop, error);
                return;
            }
        };
        self.renderer = Some(renderer);
        self.audio.start();
        if self.options.smoke_test
            && let Some(renderer) = &mut self.renderer
        {
            renderer.set_diagnostic_deadline(self.started + SMOKE_TIMEOUT);
        }
        self.activate_on_next_event = !self.options.offscreen;
        self.last_frame = Instant::now();
        println!(
            "RustCraft ready — new-world seed {}. Continue or start a world from the main menu.",
            self.options.seed
        );
        window.request_redraw();
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.release_pointer();
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.release_pointer();
        self.record_exit();
        self.streamer.take();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self
            .window
            .as_ref()
            .is_none_or(|window| window.id() != window_id)
        {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                if self.save_before_exit() {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                self.resize(size);
            }
            WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.refresh_window_pacing(Instant::now(), true);
            }
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                if self.options.smoke_test {
                    println!("Smoke: window focus {focused}");
                }
                if !focused {
                    self.release_pointer();
                } else {
                    self.refresh_window_pacing(Instant::now(), true);
                }
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                if self.options.smoke_test {
                    println!("Smoke: window occluded {occluded}");
                }
                if occluded {
                    self.release_pointer();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_moved([position.x as f32, position.y as f32]);
            }
            WindowEvent::CursorLeft { .. } => self.cursor = None,
            WindowEvent::MouseInput { state, button, .. } => {
                self.mouse_button(button, state == ElementState::Pressed, event_loop);
            }
            WindowEvent::MouseWheel { delta, .. } => self.mouse_wheel(delta),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => {
                if self.screen == crate::hud::Screen::TimeConsole {
                    self.console_keyboard(&event);
                    return;
                }
                if self.focused
                    && let PhysicalKey::Code(code) = event.physical_key
                {
                    let pressed = event.state == ElementState::Pressed;
                    if let Some(action) = self.input.key_for_mode(
                        code,
                        pressed,
                        event.repeat,
                        self.started.elapsed().as_secs_f64(),
                        self.game.mode() == crate::game::GameMode::Creative,
                    ) {
                        self.action(action, event_loop);
                    }
                }
            }
            WindowEvent::RedrawRequested
                if !self.zero_sized
                    && (!self.occluded || self.options.smoke_test)
                    && self
                        .frame_deadline
                        .is_none_or(|deadline| Instant::now() >= deadline) =>
            {
                self.redraw(event_loop);
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if self.screen == crate::hud::Screen::Playing
            && self.captured
            && self.focused
            && let DeviceEvent::MouseMotion { delta: (x, y) } = event
        {
            self.game.look(x, y);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.finished {
            self.autosave(Instant::now());
        }
        if smoke_timed_out(
            self.options.smoke_test,
            self.finished,
            self.started,
            Instant::now(),
        ) {
            self.fail(
                event_loop,
                "smoke test timed out while waiting for the window or GPU",
            );
            return;
        }
        if self.activate_on_next_event
            && let Some(window) = &self.window
        {
            window.set_visible(true);
            window.focus_window();
            self.activate_on_next_event = false;
        }
        if !self.finished {
            self.refresh_window_pacing(Instant::now(), false);
        }
        if self.finished {
            event_loop.set_control_flow(ControlFlow::Wait);
        } else if self.zero_sized || (self.occluded && !self.options.smoke_test) {
            event_loop.set_control_flow(if self.options.smoke_test {
                ControlFlow::WaitUntil(self.started + SMOKE_TIMEOUT)
            } else {
                self.persistence
                    .wake_deadline()
                    .map_or(ControlFlow::Wait, ControlFlow::WaitUntil)
            });
        } else if let Some(deadline) = self.frame_deadline
            && Instant::now() < deadline
        {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else if let Some(window) = &self.window {
            window.request_redraw();
            // The persistent deadline above paces both successful and failed acquisitions.
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + self.frame_pacing.interval(),
            ));
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.release_pointer();
    }
}
