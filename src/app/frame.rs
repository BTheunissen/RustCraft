//! One frame of streaming, presentation, and optional bounded diagnostics.

use std::time::{Duration, Instant};

use winit::event_loop::ActiveEventLoop;

use super::pacing::next_frame_deadline;
use super::{App, SMOKE_TIMEOUT};
use crate::game::GameMode;
use crate::hud::{HudInfo, Screen, UiState, build_interface};
use crate::phase::PhaseCache;
use crate::renderer::{FrameState, SurvivalFrame};
use crate::world::ChunkCoord;

impl App {
    pub(super) fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        if self.finished {
            return;
        }
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.simulate(dt);
        let Some(streamer) = &mut self.streamer else {
            return;
        };
        let center =
            ChunkCoord::from_world(self.game.player.position.x, self.game.player.position.z);
        streamer.sync_edits(self.game.world.edits());
        let evicted = streamer.update_center(center);
        let ready = streamer.poll_ready(4);
        if streamer.worker_failed() {
            self.fail(event_loop, "terrain worker stopped unexpectedly");
            return;
        }
        let loaded = streamer.loaded_count();
        let pending = streamer.pending_count();
        if let Some(renderer) = &mut self.renderer {
            for coord in evicted {
                renderer.remove_chunk(coord);
                self.game.world.remove(coord);
            }
            for prepared in ready {
                let coord = prepared.chunk.coord();
                renderer.upload_chunk(coord, &prepared.mesh);
                self.game.world.insert(prepared.chunk);
            }
        }
        self.finish_loading();
        let environment = self.game.environment();
        let sound_environment = self.game.sound_environment(environment, dt);
        self.audio.update(
            sound_environment,
            self.game.player.eye_position(),
            self.game.player.yaw,
            self.game.take_sound_events().into_iter().flatten(),
            self.screen == Screen::Playing && self.captured && self.focused,
            now,
        );
        let elapsed = self.started.elapsed().as_secs_f32();
        if matches!(self.screen, Screen::Audio(_)) {
            self.screen = Screen::Audio(self.audio.controls());
        }
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let (width, height) = renderer.size();
        let hud_info = HudInfo {
            captured: self.captured || self.options.smoke_test,
            flying: self.game.player.flying,
            swimming: self.game.player.is_swimming(),
            phasing: PhaseCache::phasing(
                &self.game.world,
                self.game.player.eye_position(),
                self.game.player.flying,
            ),
            biome: environment.biome,
            headlamp: self.game.headlamp,
            underground: environment.underground,
            position: self.game.player.position.to_array(),
            fps: self.fps,
            loaded_chunks: loaded,
            pending_chunks: pending,
            speed: self.game.speed(),
            time: self.game.time(),
        };
        let ui = UiState {
            screen: if self.options.smoke_test {
                Screen::Playing
            } else {
                self.screen
            },
            mode: self.game.mode(),
            inventory: (self.game.mode() == GameMode::Survival).then_some(self.game.inventory()),
            storage: self.game.storage_view(),
            interaction_notice: self.game.interaction_notice(),
            cursor: self.cursor,
            focused_button: self.focused_button,
            can_continue: self.persistence.can_continue(),
            save_notice: self.persistence.notice,
        };
        let mut hud = build_interface(width, height, &hud_info, &ui);
        if self.screen == Screen::TimeConsole {
            hud.extend(crate::hud::build_time_console(
                width,
                height,
                &self.time_console.line,
                self.time_console.notice.as_deref(),
                &self.game.time().label(),
            ));
        }
        let playing = self.screen == Screen::Playing && self.captured;
        let survival = (self.game.mode() == GameMode::Survival).then(|| SurvivalFrame {
            pickups: self.game.pickups(),
            held: self
                .game
                .inventory()
                .selected_stack()
                .map(|stack| stack.item),
            target: if playing { self.game.target() } else { None },
            mining_progress: if playing {
                self.game.mining_progress()
            } else {
                0.0
            },
            show_hand: playing,
        });
        let frame = FrameState {
            player: &self.game.player,
            world: &self.game.world,
            hud: &hud,
            elapsed: if self.screen.is_menu_preview() || self.options.smoke_test {
                elapsed
            } else {
                self.session_time
            },
            water_time: None,
            time_of_day: self.game.time().hour(),
            render_distance: if self.screen.is_menu_preview() {
                3
            } else {
                self.options.render_distance
            },
            underwater: environment.underwater,
            water: environment.water,
            sky_exposure: environment.sky_exposure,
            headlamp: self.game.headlamp,
            in_lava: environment.in_lava,
            survival,
        };
        let presented = if self.options.offscreen {
            if let Err(error) = renderer.render_offscreen(&frame) {
                self.fail(event_loop, error);
                return;
            }
            self.offscreen_frames += 1;
            false
        } else {
            match renderer.render(&frame) {
                Ok(presented) => presented,
                Err(error) => {
                    self.fail(event_loop, error);
                    return;
                }
            }
        };
        if presented {
            self.presented_frames += 1;
        }
        let rendered = presented || self.options.offscreen;
        if rendered {
            self.fps_frames += 1;
            if self.smoke.resized {
                self.smoke.resize_rendered = true;
            }
        }
        self.frame_deadline = Some(next_frame_deadline(
            now,
            Instant::now(),
            rendered,
            self.frame_pacing.interval(),
        ));
        let fps_elapsed = now - self.fps_started;
        if fps_elapsed >= Duration::from_millis(500) {
            self.fps = self.fps_frames as f32 / fps_elapsed.as_secs_f32();
            self.fps_frames = 0;
            self.fps_started = now;
        }
        if self.options.smoke_test
            && self.smoke.step == Some(240)
            && pending == 0
            && (self.presented_frames >= 120 || self.offscreen_frames >= 120)
            && rendered
        {
            if let Err(error) = self.smoke.verify_motion(&self.game.player) {
                self.fail(event_loop, error);
                return;
            }
            let deadline = self.started + SMOKE_TIMEOUT;
            let result = crate::playtest::surface::verify_pause_menu(
                renderer,
                &frame,
                hud_info,
                self.options.screenshot.as_deref(),
                !self.options.offscreen,
            )
            .and_then(|()| {
                crate::playtest::verify_surface(
                    renderer,
                    self.game.terrain,
                    &self.options.screenshot,
                    &frame,
                    hud_info,
                    deadline,
                )
            })
            .and_then(|()| {
                crate::playtest::verify_underground(
                    renderer,
                    self.game.terrain,
                    self.options.screenshot.as_deref(),
                    deadline,
                )
            })
            .and_then(|()| {
                crate::playtest::verify_water(
                    renderer,
                    self.game.terrain,
                    self.options.screenshot.as_deref(),
                    deadline,
                )
            })
            .and_then(|()| {
                crate::playtest::verify_coastal(
                    renderer,
                    self.game.terrain,
                    self.options.screenshot.as_deref(),
                    !self.options.offscreen,
                    deadline,
                )
            })
            .and_then(|()| {
                crate::playtest::verify_lighting(
                    renderer,
                    self.game.terrain,
                    self.options.screenshot.as_deref(),
                    !self.options.offscreen,
                    deadline,
                )
            })
            .and_then(|()| {
                crate::playtest::verify_volumetrics(
                    renderer,
                    self.game.terrain,
                    self.options.screenshot.as_deref(),
                    !self.options.offscreen,
                    deadline,
                )
            })
            .and_then(|()| {
                crate::playtest::verify_survival(
                    renderer,
                    self.game.terrain,
                    self.options.screenshot.as_deref(),
                    !self.options.offscreen,
                    deadline,
                )
            })
            .and_then(|()| {
                crate::playtest::verify_lanterns(
                    renderer,
                    self.game.terrain,
                    self.options.screenshot.as_deref(),
                    !self.options.offscreen,
                    deadline,
                )
            })
            .and_then(|()| {
                super::audio::verify_silent_output()
                    .map_err(|error| std::io::Error::other(error).into())
            })
            .and_then(|()| crate::playtest::check_deadline(deadline).map_err(Into::into));
            if let Err(error) = result {
                self.fail(event_loop, error);
                return;
            }
            if self.options.offscreen {
                println!(
                    "Offscreen smoke passed: {} GPU frames, {loaded} chunks, resize verified; native window presentation NOT tested",
                    self.offscreen_frames
                );
            } else {
                println!(
                    "Smoke passed: {} presented frames, {loaded} chunks, {:.1} FPS, position {:?}, resize verified",
                    self.presented_frames,
                    self.fps,
                    self.game.player.position.to_array()
                );
            }
            self.finished = true;
            event_loop.exit();
        } else if self.options.smoke_test && elapsed > SMOKE_TIMEOUT.as_secs_f32() {
            self.fail(
                event_loop,
                format!(
                    "smoke test timed out: {loaded} loaded, {pending} pending, {} frames",
                    self.presented_frames
                ),
            );
        }
    }
}
