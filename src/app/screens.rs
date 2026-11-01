//! Screen transitions and menu routing, independent of native event details.

use winit::event::{MouseButton, MouseScrollDelta};
use winit::event_loop::ActiveEventLoop;

use super::App;
use crate::game::GameMode;
use crate::hud::{Screen, UiHit, button_count, hit_test};
use crate::world::ChunkCoord;

#[derive(Debug, PartialEq, Eq)]
enum ButtonAction {
    Start(GameMode),
    Continue,
    Resume,
    Leave,
    Replace(GameMode),
    CancelReplace,
    Quit,
    Audio,
    AudioControl(usize),
    Craft(usize),
    CraftBack,
}

fn button_action(screen: Screen, index: usize) -> Option<ButtonAction> {
    match (screen, index) {
        (Screen::MainMenu, 0) => Some(ButtonAction::Continue),
        (Screen::MainMenu, 1) => Some(ButtonAction::Start(GameMode::Survival)),
        (Screen::MainMenu, 2) => Some(ButtonAction::Start(GameMode::Creative)),
        (Screen::MainMenu, 3) | (Screen::Paused, 2) => Some(ButtonAction::Quit),
        (Screen::Paused, 0) => Some(ButtonAction::Resume),
        (Screen::Paused, 1) => Some(ButtonAction::Leave),
        (Screen::Paused, 3) => Some(ButtonAction::Audio),
        (Screen::Audio(_), index) if index < 8 => Some(ButtonAction::AudioControl(index)),
        (Screen::Crafting, index) if index < crate::game::RECIPES.len() => {
            Some(ButtonAction::Craft(index))
        }
        (Screen::Crafting, index) if index == crate::game::RECIPES.len() => {
            Some(ButtonAction::CraftBack)
        }
        (Screen::ConfirmReplace(mode), 0) => Some(ButtonAction::Replace(mode)),
        (Screen::ConfirmReplace(_), 1) => Some(ButtonAction::CancelReplace),
        _ => None,
    }
}

fn clicked_capture(screen: Screen, index: usize) -> bool {
    matches!(
        button_action(screen, index),
        Some(
            ButtonAction::Start(_)
                | ButtonAction::Continue
                | ButtonAction::Replace(_)
                | ButtonAction::Resume
        )
    )
}

fn hovered_button(screen: Screen, hit: UiHit) -> Option<usize> {
    match hit {
        UiHit::Button(index) => (index < button_count(screen)).then_some(index),
        UiHit::Recipe(index) if screen == Screen::Crafting => {
            (index < crate::game::RECIPES.len()).then_some(index)
        }
        UiHit::CraftBack if screen == Screen::Crafting => Some(crate::game::RECIPES.len()),
        _ => None,
    }
}

impl App {
    pub(super) fn finish_loading(&mut self) {
        if self.screen != Screen::Loading {
            return;
        }
        let center =
            ChunkCoord::from_world(self.game.player.position.x, self.game.player.position.z);
        let ready = (-1..=1).all(|z| {
            (-1..=1).all(|x| {
                let coord = ChunkCoord::new(center.x + x, center.z + z);
                self.game.world.chunk(coord).is_some()
            })
        });
        if ready {
            self.screen = Screen::Paused;
            if self.resume_requested && self.focused && !self.occluded && !self.zero_sized {
                self.resume_play();
            }
            self.resume_requested = false;
            self.pointer_authorized = false;
            if !self.captured
                && self.focused
                && !self.occluded
                && !self.zero_sized
                && let Some(window) = &self.window
            {
                let size = window.inner_size();
                let center = winit::dpi::PhysicalPosition::new(size.width / 2, size.height / 2);
                if window.set_cursor_position(center).is_ok() {
                    self.cursor = Some([center.x as f32, center.y as f32]);
                }
            }
        }
    }

    pub(super) fn resume_play(&mut self) {
        if !self.focused || self.occluded || self.zero_sized {
            return;
        }
        self.input.clear();
        self.game.clear_input();
        self.scroll_remainder = 0.0;
        self.capture_pointer();
        if self.captured {
            self.screen = Screen::Playing;
            self.cursor = None;
        }
    }

    pub(super) fn toggle_inventory(&mut self) {
        if self.game.mode() != GameMode::Survival {
            return;
        }
        match self.screen {
            Screen::Playing if self.captured && self.focused => {
                self.release_pointer();
                self.inventory_resume_authorized = true;
                self.screen = Screen::Inventory;
            }
            Screen::Inventory | Screen::Crafting => {
                self.game.inventory_mut().close_cursor();
                self.pointer_authorized = std::mem::take(&mut self.inventory_resume_authorized);
                self.screen = Screen::Paused;
                self.resume_play();
            }
            Screen::Storage => {
                self.game.close_storage();
                self.pointer_authorized = std::mem::take(&mut self.inventory_resume_authorized);
                self.screen = Screen::Paused;
                self.resume_play();
            }
            _ => {}
        }
    }

    pub(super) fn escape(&mut self) {
        match self.screen {
            Screen::TimeConsole => self.close_time_console(),
            Screen::Inventory => self.toggle_inventory(),
            Screen::Storage => self.toggle_inventory(),
            Screen::Crafting => {
                self.screen = Screen::Inventory;
                self.focused_button = 0;
                self.cursor = None;
            }
            Screen::Playing => self.release_pointer(),
            Screen::Paused => self.resume_play(),
            Screen::Audio(_) => self.close_audio_controls(),
            Screen::ConfirmReplace(_) => {
                self.screen = Screen::MainMenu;
                self.focused_button = 1;
                self.cursor = None;
            }
            _ => {}
        }
    }

    pub(super) fn navigate(&mut self, direction: i32) {
        let count = button_count(self.screen);
        if count > 0 {
            self.focused_button = (self.focused_button as i64 + i64::from(direction))
                .rem_euclid(count as i64) as usize;
            if self.screen == Screen::MainMenu
                && self.focused_button == 0
                && !self.persistence.can_continue()
            {
                self.focused_button = if direction < 0 { count - 1 } else { 1 };
            }
            self.cursor = None;
        }
    }

    pub(super) fn activate_button(&mut self, index: usize, event_loop: &ActiveEventLoop) {
        match self.apply_button(index) {
            Ok(true) => event_loop.exit(),
            Ok(false) => {}
            Err(error) => self.scene_error(&error),
        }
    }

    /// Menu effects are testable without a native event loop; only a successful
    /// save may request exit. Pointer authorization still comes from `click_hit`.
    fn apply_button(&mut self, index: usize) -> std::io::Result<bool> {
        match button_action(self.screen, index) {
            Some(ButtonAction::Start(mode)) => self.request_new_world(mode).map(|()| false),
            Some(ButtonAction::Continue) if self.persistence.can_continue() => {
                self.continue_world().map(|()| false)
            }
            Some(ButtonAction::Replace(mode)) => self.create_world(mode).map(|()| false),
            Some(ButtonAction::Leave) => self.leave_world().map(|()| false),
            Some(ButtonAction::Quit) => Ok(self.save_before_exit()),
            Some(ButtonAction::Audio) => {
                self.release_pointer();
                self.screen = Screen::Audio(self.audio.controls());
                self.focused_button = 0;
                Ok(false)
            }
            Some(ButtonAction::AudioControl(index)) => {
                self.adjust_audio(index);
                Ok(false)
            }
            Some(ButtonAction::Craft(index)) => {
                let _ = self.game.inventory_mut().craft(index);
                Ok(false)
            }
            Some(ButtonAction::CraftBack) => {
                self.screen = Screen::Inventory;
                self.focused_button = 0;
                Ok(false)
            }
            Some(ButtonAction::Resume) => {
                self.resume_play();
                Ok(false)
            }
            Some(ButtonAction::CancelReplace) => {
                self.screen = Screen::MainMenu;
                self.focused_button = 1;
                self.cursor = None;
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn close_audio_controls(&mut self) {
        self.input.clear();
        self.game.clear_input();
        self.pointer_authorized = false;
        self.screen = Screen::Paused;
        self.focused_button = 3;
        self.cursor = None;
    }

    pub(super) fn open_crafting(&mut self) {
        if self.screen == Screen::Inventory && self.game.mode() == GameMode::Survival {
            self.input.clear();
            self.game.clear_input();
            self.pointer_authorized = false;
            self.screen = Screen::Crafting;
            self.focused_button = 0;
            self.cursor = None;
        }
    }

    fn adjust_audio(&mut self, index: usize) {
        if index == 7 {
            self.close_audio_controls();
            return;
        }
        let mut settings = self.audio.settings();
        if index == 0 {
            settings.muted = !settings.muted;
        } else {
            let (gain, direction) = match index {
                1 => (&mut settings.master, -1.0),
                2 => (&mut settings.master, 1.0),
                3 => (&mut settings.actions, -1.0),
                4 => (&mut settings.actions, 1.0),
                5 => (&mut settings.ambience, -1.0),
                6 => (&mut settings.ambience, 1.0),
                _ => return,
            };
            *gain = ((*gain * 100.0).round() + direction * 5.0).clamp(0.0, 100.0) / 100.0;
        }
        self.audio.set_settings(settings);
        self.audio.save_preferences();
        self.screen = Screen::Audio(self.audio.controls());
    }

    pub(super) fn cursor_moved(&mut self, position: [f32; 2]) {
        if self.captured || !position.into_iter().all(f32::is_finite) {
            return;
        }
        if self.cursor == Some(position) {
            return;
        }
        self.cursor = Some(position);
        if let Some(renderer) = &self.renderer {
            let (width, height) = renderer.size();
            if let Some(index) = hit_test(width, height, self.screen, position)
                .and_then(|hit| hovered_button(self.screen, hit))
                && (self.screen != Screen::MainMenu
                    || index != 0
                    || self.persistence.can_continue())
            {
                self.focused_button = index;
            }
        }
    }

    pub(super) fn mouse_button(
        &mut self,
        button: MouseButton,
        pressed: bool,
        event_loop: &ActiveEventLoop,
    ) {
        if self.options.smoke_test || !self.focused {
            return;
        }
        if self.screen == Screen::TimeConsole {
            return;
        }
        if self.screen == Screen::Playing && self.captured {
            self.input.mouse_button(button, pressed);
            return;
        }
        if !pressed || !matches!(button, MouseButton::Left | MouseButton::Right) {
            return;
        }
        let hit = self.renderer.as_ref().and_then(|renderer| {
            self.cursor.and_then(|cursor| {
                let (width, height) = renderer.size();
                hit_test(width, height, self.screen, cursor)
            })
        });
        if button == MouseButton::Right {
            self.right_click_hit(hit);
            return;
        }
        if let Some(index) = self.click_hit(hit) {
            self.activate_button(index, event_loop);
        }
    }

    fn right_click_hit(&mut self, hit: Option<UiHit>) {
        self.pointer_authorized = false;
        if self.screen == Screen::Storage {
            match hit {
                Some(UiHit::InventorySlot(index)) => self
                    .game
                    .storage_click(crate::game::StorageSlot::Player(index), true),
                Some(UiHit::CrateSlot(index)) => self
                    .game
                    .storage_click(crate::game::StorageSlot::Crate(index), true),
                _ => {}
            }
            return;
        }
        if self.screen == Screen::Inventory
            && let Some(UiHit::InventorySlot(index)) = hit
        {
            self.game.inventory_mut().right_click_slot(index);
        }
    }

    fn click_hit(&mut self, hit: Option<UiHit>) -> Option<usize> {
        self.pointer_authorized = false;
        match hit {
            Some(UiHit::InventorySlot(index)) if self.screen == Screen::Storage => {
                self.game
                    .storage_click(crate::game::StorageSlot::Player(index), false);
                None
            }
            Some(UiHit::CrateSlot(index)) if self.screen == Screen::Storage => {
                self.game
                    .storage_click(crate::game::StorageSlot::Crate(index), false);
                None
            }
            Some(UiHit::Crafting) if self.screen == Screen::Inventory => {
                self.open_crafting();
                None
            }
            Some(UiHit::Recipe(index))
                if self.screen == Screen::Crafting && index < crate::game::RECIPES.len() =>
            {
                Some(index)
            }
            Some(UiHit::CraftBack) if self.screen == Screen::Crafting => {
                Some(crate::game::RECIPES.len())
            }
            Some(UiHit::Button(index))
                if index < button_count(self.screen)
                    && (self.screen != Screen::MainMenu
                        || index != 0
                        || self.persistence.can_continue()) =>
            {
                self.pointer_authorized = clicked_capture(self.screen, index);
                Some(index)
            }
            Some(UiHit::InventorySlot(index)) if self.screen == Screen::Inventory => {
                self.game.inventory_mut().click_slot(index);
                None
            }
            _ => None,
        }
    }

    pub(super) fn mouse_wheel(&mut self, delta: MouseScrollDelta) {
        if self.screen != Screen::Playing
            || !self.captured
            || !self.focused
            || self.game.mode() != GameMode::Survival
        {
            return;
        }
        let amount = match delta {
            MouseScrollDelta::LineDelta(_, y) => f64::from(y),
            MouseScrollDelta::PixelDelta(position) => position.y / 40.0,
        };
        if !amount.is_finite() {
            return;
        }
        self.scroll_remainder += amount.clamp(-18.0, 18.0);
        let steps = self.scroll_remainder.trunc() as i32;
        self.scroll_remainder -= f64::from(steps);
        self.game.inventory_mut().cycle(-steps);
    }
}

#[cfg(test)]
mod audio_tests;
#[cfg(test)]
mod crafting_tests;
#[cfg(test)]
mod item_tests;
#[cfg(test)]
mod persistence_tests;
#[cfg(test)]
mod storage_tests;
#[cfg(test)]
mod tests;
