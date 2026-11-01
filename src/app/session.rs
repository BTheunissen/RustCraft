//! Scene construction and durable new/continue/leave boundaries.

use std::io;
use std::time::Instant;

use super::App;
use crate::game::{Game, GameMode};
use crate::hud::{SaveNotice, Screen};
use crate::streaming::ChunkStreamer;
use crate::world::ChunkCoord;

impl App {
    fn prepare_scene(&self, game: &Game, active: bool) -> io::Result<ChunkStreamer> {
        let center = ChunkCoord::from_world(game.player.position.x, game.player.position.z);
        let radius = if active {
            self.options.render_distance
        } else {
            3
        };
        let mut streamer = ChunkStreamer::new(game.terrain(), center, radius)?;
        // Restored overlays must be in the worker before its first mesh request.
        streamer.sync_edits(game.world.edits());
        Ok(streamer)
    }

    fn install_scene(&mut self, mut game: Game, streamer: ChunkStreamer, active: bool) {
        let _ = game.time_mut().set_speed(self.options.time_speed);
        let authorized = active && self.pointer_authorized;
        self.release_pointer();
        self.audio.clear_scene();
        self.pointer_authorized = authorized;
        // Joining the old worker drains its bounded results before the new scene exists.
        self.streamer.take();
        if let Some(renderer) = &mut self.renderer {
            renderer.clear_chunks();
        }
        self.game = game;
        self.streamer = Some(streamer);
        self.resume_requested = active;
        self.screen = if active {
            Screen::Loading
        } else {
            Screen::MainMenu
        };
        self.cursor = None;
        self.focused_button = if !active && !self.persistence.can_continue() {
            1
        } else {
            0
        };
        self.session_time = 0.0;
        self.time_console = super::time_console::TimeConsole::default();
        self.last_frame = Instant::now();
        if active {
            self.persistence.begin_world(self.last_frame);
        } else {
            self.persistence.end_world();
        }
        if let Some(window) = &self.window {
            window.set_title(if active {
                match self.game.mode() {
                    GameMode::Survival => "RustCraft — Survival",
                    GameMode::Creative => "RustCraft — Creative Explorer",
                }
            } else {
                "RustCraft"
            });
        }
    }

    pub(super) fn show_main_menu(&mut self) -> io::Result<()> {
        let mut game = Game::new(self.options.seed);
        let _ = game.time_mut().set_hour(self.options.time_of_day);
        game.player.position.y += 8.0;
        game.player.pitch = -0.42;
        let streamer = self.prepare_scene(&game, false)?;
        self.install_scene(game, streamer, false);
        Ok(())
    }

    pub(super) fn request_new_world(&mut self, mode: GameMode) -> io::Result<()> {
        if self.persistence.check_occupied()? {
            self.release_pointer();
            self.screen = Screen::ConfirmReplace(mode);
            self.focused_button = 1;
            self.cursor = None;
            Ok(())
        } else {
            self.create_world(mode)
        }
    }

    pub(super) fn create_world(&mut self, mode: GameMode) -> io::Result<()> {
        let mut game = Game::with_mode(self.options.seed, mode);
        game.time_mut()
            .set_hour(self.options.time_of_day)
            .map_err(io::Error::other)?;
        let streamer = self.prepare_scene(&game, true)?;
        self.persistence.write(&game.snapshot(), Instant::now())?;
        self.install_scene(game, streamer, true);
        Ok(())
    }

    pub(super) fn continue_world(&mut self) -> io::Result<()> {
        let snapshot = self.persistence.load()?;
        let game = Game::from_snapshot(snapshot)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let streamer = self.prepare_scene(&game, true)?;
        self.install_scene(game, streamer, true);
        Ok(())
    }

    pub(super) fn leave_world(&mut self) -> io::Result<()> {
        self.game.inventory_mut().close_cursor();
        self.persistence.save_active(&self.game, Instant::now())?;
        self.show_main_menu()
    }

    pub(super) fn autosave(&mut self, now: Instant) {
        if self.persistence.due(now) && self.persistence.save_active(&self.game, now).is_err() {
            self.pause_for_save_error();
        }
    }

    /// Returns false on failure so user-requested quit/close can be retried.
    pub(super) fn save_before_exit(&mut self) -> bool {
        self.pause_for_save_error();
        match self.persistence.save_active(&self.game, Instant::now()) {
            Ok(()) => {
                self.persistence.end_world();
                true
            }
            Err(_) => false,
        }
    }

    pub(super) fn scene_error(&mut self, error: &io::Error) {
        self.release_pointer();
        if !matches!(
            self.persistence.notice,
            Some(SaveNotice::SaveFailed | SaveNotice::Unavailable | SaveNotice::Incompatible)
        ) {
            self.persistence.warn(SaveNotice::LoadFailed, error);
        }
    }

    fn pause_for_save_error(&mut self) {
        self.release_pointer();
        if matches!(
            self.screen,
            Screen::Inventory | Screen::Crafting | Screen::Storage
        ) {
            self.game.close_storage();
            self.game.inventory_mut().close_cursor();
            self.screen = Screen::Paused;
        } else if matches!(self.screen, Screen::Audio(_) | Screen::TimeConsole) {
            self.screen = Screen::Paused;
            self.focused_button = 0;
        }
    }
}

#[cfg(test)]
mod tests;
