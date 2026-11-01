//! Bounded time-command input, with the same pause and capture rules as inventory.

use super::App;
use crate::hud::Screen;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

const MAX_COMMAND_CHARS: usize = 60;

pub(super) struct TimeConsole {
    pub(super) line: String,
    pub(super) notice: Option<String>,
}

impl Default for TimeConsole {
    fn default() -> Self {
        Self {
            line: "/time ".into(),
            notice: None,
        }
    }
}

impl TimeConsole {
    fn push_text(&mut self, text: &str) {
        for character in text.chars() {
            if self.line.len() >= MAX_COMMAND_CHARS {
                break;
            }
            if character.is_ascii_graphic() || character == ' ' {
                self.line.push(character);
            }
        }
    }
}

impl App {
    pub(super) fn open_time_console(&mut self) {
        if !self.focused || self.options.smoke_test {
            return;
        }
        let authorized = self.screen == Screen::Playing && self.captured;
        if !authorized && self.screen != Screen::Paused {
            return;
        }
        self.release_pointer();
        self.inventory_resume_authorized = authorized;
        self.time_console = TimeConsole::default();
        self.screen = Screen::TimeConsole;
    }

    pub(super) fn close_time_console(&mut self) {
        if self.screen != Screen::TimeConsole {
            return;
        }
        self.pointer_authorized = std::mem::take(&mut self.inventory_resume_authorized);
        self.screen = Screen::Paused;
        self.resume_play();
    }

    pub(super) fn submit_time_command(&mut self) {
        if self.screen != Screen::TimeConsole || !self.focused {
            return;
        }
        match self.game.time_mut().command(&self.time_console.line) {
            Ok(result) => {
                println!("Debug: {}", result.message);
                self.time_console.notice = Some(result.message);
                if result.changed {
                    self.close_time_console();
                }
            }
            Err(error) => self.time_console.notice = Some(error.into()),
        }
    }

    pub(super) fn console_keyboard(&mut self, event: &KeyEvent) {
        self.console_key(
            &event.logical_key,
            event.text.as_deref(),
            event.state == ElementState::Pressed,
            event.repeat,
        );
    }

    fn console_key(&mut self, key: &Key, text: Option<&str>, pressed: bool, repeat: bool) {
        if self.screen != Screen::TimeConsole || !self.focused || !pressed {
            return;
        }
        match key {
            Key::Named(NamedKey::Escape) if !repeat => self.close_time_console(),
            Key::Named(NamedKey::Enter) if !repeat => self.submit_time_command(),
            Key::Named(NamedKey::Backspace) => {
                self.time_console.line.pop();
                self.time_console.notice = None;
            }
            Key::Character(_) => {
                if let Some(text) = text {
                    self.time_console.push_text(text);
                }
                self.time_console.notice = None;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
