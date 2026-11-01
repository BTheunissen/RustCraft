//! Keyboard state and edge-triggered actions, independent of window callbacks.

use winit::event::MouseButton;
use winit::keyboard::KeyCode;

use crate::player::MovementInput;

/// One-shot action emitted by a nonrepeated key press.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Release the pointer and pause.
    Pause,
    /// Switch between walking and creative flight.
    ToggleFlight,
    /// Switch the exploration headlamp.
    ToggleHeadlamp,
    /// Cycle the persistent exploration speed, from 1× through 16×.
    CycleSpeed,
    /// Cycle day-night debug speed without affecting movement.
    CycleTimeSpeed,
    /// Open the paused, time-only debug command console.
    TimeConsole,
    /// Return to a safe spawn.
    Respawn,
    /// Switch borderless fullscreen.
    Fullscreen,
    /// Exit only while the pointer is released.
    Quit,
    /// Open or close the survival inventory.
    Inventory,
    /// Select a hotbar slot, numbered from zero.
    SelectSlot(usize),
    /// Move keyboard focus through menu buttons.
    Navigate(i32),
    /// Activate the focused menu button.
    Activate,
    /// Open crafting from paused survival inventory; never a world action.
    Crafting,
}

/// Tracks independent left/right modifier keys and a jump edge between frames.
#[derive(Default)]
pub struct InputState {
    movement: MovementInput,
    left_shift: bool,
    right_shift: bool,
    left_control: bool,
    right_control: bool,
    jump_pressed: bool,
    jump_released: bool,
    space_held: bool,
    last_space_press: Option<f64>,
    mining_held: bool,
    mining_released: bool,
    place_held: bool,
    place_pressed: bool,
    place_against: bool,
}

impl InputState {
    /// Clears all transient/held state on focus loss, pause, and pointer capture.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Applies physical keys so WASD is stable across keyboard layouts.
    /// `seconds` is a monotonic timestamp used only for double-Space flight.
    /// The application gates exploration actions by pointer capture.
    #[cfg(test)]
    pub fn key(
        &mut self,
        key: KeyCode,
        pressed: bool,
        repeat: bool,
        seconds: f64,
    ) -> Option<Action> {
        self.key_for_mode(key, pressed, repeat, seconds, true)
    }

    /// Survival preserves every jump press instead of recognizing double-Space flight.
    pub fn key_for_mode(
        &mut self,
        key: KeyCode,
        pressed: bool,
        repeat: bool,
        seconds: f64,
        creative: bool,
    ) -> Option<Action> {
        if repeat {
            return None;
        }
        match key {
            KeyCode::KeyW => self.movement.forward = pressed,
            KeyCode::KeyS => self.movement.backward = pressed,
            KeyCode::KeyA => self.movement.left = pressed,
            KeyCode::KeyD => self.movement.right = pressed,
            KeyCode::ShiftLeft => self.left_shift = pressed,
            KeyCode::ShiftRight => self.right_shift = pressed,
            KeyCode::ControlLeft | KeyCode::ControlRight => {
                let was_held = self.left_control || self.right_control;
                if key == KeyCode::ControlLeft {
                    self.left_control = pressed;
                } else {
                    self.right_control = pressed;
                }
                if creative && pressed && !was_held {
                    return Some(Action::CycleSpeed);
                }
            }
            KeyCode::Space => {
                if !pressed {
                    self.jump_released = true;
                }
                if pressed && !self.space_held {
                    self.jump_pressed = true;
                    if creative
                        && self
                            .last_space_press
                            .is_some_and(|last| (0.0..=0.28).contains(&(seconds - last)))
                    {
                        self.last_space_press = None;
                        self.space_held = true;
                        self.jump_pressed = false;
                        return Some(Action::ToggleFlight);
                    }
                    self.last_space_press = creative.then_some(seconds);
                }
                self.space_held = pressed;
            }
            KeyCode::Escape if pressed => return Some(Action::Pause),
            KeyCode::KeyF if pressed && creative => return Some(Action::ToggleFlight),
            KeyCode::KeyH if pressed => return Some(Action::ToggleHeadlamp),
            KeyCode::KeyR if pressed => return Some(Action::Respawn),
            KeyCode::F11 if pressed => return Some(Action::Fullscreen),
            KeyCode::F6 if pressed => return Some(Action::CycleTimeSpeed),
            KeyCode::Backquote | KeyCode::Slash if pressed => return Some(Action::TimeConsole),
            KeyCode::KeyQ if pressed => return Some(Action::Quit),
            KeyCode::KeyI if pressed => return Some(Action::Inventory),
            KeyCode::KeyC if pressed => return Some(Action::Crafting),
            KeyCode::Digit1 if pressed => return Some(Action::SelectSlot(0)),
            KeyCode::Digit2 if pressed => return Some(Action::SelectSlot(1)),
            KeyCode::Digit3 if pressed => return Some(Action::SelectSlot(2)),
            KeyCode::Digit4 if pressed => return Some(Action::SelectSlot(3)),
            KeyCode::Digit5 if pressed => return Some(Action::SelectSlot(4)),
            KeyCode::Digit6 if pressed => return Some(Action::SelectSlot(5)),
            KeyCode::Digit7 if pressed => return Some(Action::SelectSlot(6)),
            KeyCode::Digit8 if pressed => return Some(Action::SelectSlot(7)),
            KeyCode::Digit9 if pressed => return Some(Action::SelectSlot(8)),
            KeyCode::ArrowUp if pressed => return Some(Action::Navigate(-1)),
            KeyCode::ArrowDown if pressed => return Some(Action::Navigate(1)),
            KeyCode::Tab if pressed => {
                return Some(Action::Navigate(if self.left_shift || self.right_shift {
                    -1
                } else {
                    1
                }));
            }
            KeyCode::Enter | KeyCode::NumpadEnter if pressed => return Some(Action::Activate),
            _ => {}
        }
        None
    }

    /// Tracks a held mining button and one placement edge per physical right-click.
    pub fn mouse_button(&mut self, button: MouseButton, pressed: bool) {
        match button {
            MouseButton::Left => {
                self.mining_released |= !pressed && self.mining_held;
                self.mining_held = pressed;
            }
            MouseButton::Right => {
                if pressed && !self.place_held {
                    self.place_against = self.left_shift || self.right_shift;
                }
                self.place_pressed |= pressed && !self.place_held;
                self.place_held = pressed;
            }
            _ => {}
        }
    }

    /// Final held state for this update; release edges are consumed separately.
    pub const fn mining_held(&self) -> bool {
        self.mining_held
    }

    /// Preserves release/repress pairs that arrive between simulation updates.
    pub fn take_mining_release(&mut self) -> bool {
        std::mem::take(&mut self.mining_released)
    }

    /// Consumes one queued placement intent, even after a quick press/release.
    pub fn take_place_press(&mut self) -> bool {
        std::mem::take(&mut self.place_pressed)
    }

    /// Modifier at the physical right-press edge, not the final held Shift state.
    pub const fn place_modifier(&self) -> bool {
        self.place_against
    }

    /// Reports a release even when a second Space press arrived before simulation.
    /// The application uses it to rearm the player's independently tracked jump edge.
    pub fn take_jump_release(&mut self) -> bool {
        std::mem::take(&mut self.jump_released)
    }

    /// Consumes a pending jump edge once; flight and swimming use held Space for ascent.
    pub fn movement(&mut self, flying: bool) -> MovementInput {
        let mut input = self.movement;
        input.jump = if flying {
            self.space_held
        } else {
            self.space_held || self.jump_pressed
        };
        input.descend = self.left_shift || self.right_shift;
        self.jump_pressed = false;
        input
    }
}

#[cfg(test)]
mod survival_tests;
#[cfg(test)]
mod tests;
