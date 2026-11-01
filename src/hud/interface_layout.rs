//! Shared physical-pixel rectangles for rendering and pointer hit tests.

use super::{Screen, UiHit, button_count};

pub(super) const SLOT_SIZE: f32 = 36.0;
pub(super) const SLOT_GAP: f32 = 4.0;
pub(super) const GRID_WIDTH: f32 = 9.0 * SLOT_SIZE + 8.0 * SLOT_GAP;

#[derive(Copy, Clone, Debug)]
pub(super) struct Rect {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
}

impl Rect {
    pub(super) fn pixels(self) -> [f32; 4] {
        [self.x, self.y, self.width, self.height]
    }

    pub(super) fn contains(self, [x, y]: [f32; 2]) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    pub(super) fn center(self) -> [f32; 2] {
        [self.x + self.width * 0.5, self.y + self.height * 0.5]
    }
}

pub(super) struct UiLayout {
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) scale: f32,
}

impl UiLayout {
    pub(super) fn new(width: u32, height: u32) -> Option<Self> {
        if width < 160 || height < 96 {
            return None;
        }
        let width = width as f32;
        let height = height as f32;
        let ratio = (width / 480.0).min(height / 360.0);
        Some(Self {
            width,
            height,
            scale: if ratio < 1.0 {
                ratio
            } else {
                ratio.floor().min(8.0)
            },
        })
    }

    pub(super) fn centered(&self, width: f32, height: f32) -> Rect {
        Rect {
            x: (self.width - width * self.scale) * 0.5,
            y: (self.height - height * self.scale) * 0.5,
            width: width * self.scale,
            height: height * self.scale,
        }
    }

    pub(super) fn button(&self, screen: Screen, index: usize) -> Option<Rect> {
        if index >= button_count(screen) {
            return None;
        }
        if matches!(screen, Screen::Audio(_)) {
            return super::audio::rectangle(self, index);
        }
        if screen == Screen::Crafting {
            return if index < crate::game::RECIPES.len() {
                super::crafting::recipe(self, index)
            } else {
                Some(super::crafting::back(self))
            };
        }
        let offset = match screen {
            Screen::MainMenu => -24.0,
            Screen::Paused => -48.0,
            Screen::ConfirmReplace(_) => 16.0,
            _ => return None,
        };
        Some(Rect {
            x: (self.width - 280.0 * self.scale) * 0.5,
            y: self.height * 0.5 + (offset + index as f32 * 42.0) * self.scale,
            width: 280.0 * self.scale,
            height: 34.0 * self.scale,
        })
    }

    pub(super) fn inventory_panel(&self) -> Rect {
        self.centered(400.0, 284.0)
    }

    pub(super) fn inventory_slot(&self, index: usize) -> Option<Rect> {
        if index >= 36 {
            return None;
        }
        let panel = self.inventory_panel();
        let (column, top) = if index < 9 {
            (index, 190.0)
        } else {
            let storage = index - 9;
            (
                storage % 9,
                50.0 + (storage / 9) as f32 * (SLOT_SIZE + SLOT_GAP),
            )
        };
        Some(Rect {
            x: panel.x + (22.0 + column as f32 * (SLOT_SIZE + SLOT_GAP)) * self.scale,
            y: panel.y + top * self.scale,
            width: SLOT_SIZE * self.scale,
            height: SLOT_SIZE * self.scale,
        })
    }

    pub(super) fn hotbar_slot(&self, index: usize) -> Option<Rect> {
        (index < 9).then_some(Rect {
            x: (self.width - GRID_WIDTH * self.scale) * 0.5
                + index as f32 * (SLOT_SIZE + SLOT_GAP) * self.scale,
            y: self.height - 82.0 * self.scale,
            width: SLOT_SIZE * self.scale,
            height: SLOT_SIZE * self.scale,
        })
    }

    pub(super) fn held_stack(&self, cursor: Option<[f32; 2]>) -> Option<Rect> {
        let panel = self.inventory_panel();
        let [x, y] = cursor.unwrap_or([
            panel.x + panel.width - 48.0 * self.scale,
            panel.y + 2.0 * self.scale,
        ]);
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let size = SLOT_SIZE * self.scale;
        Some(Rect {
            x: (x + 8.0 * self.scale).clamp(0.0, self.width - size),
            y: (y + 8.0 * self.scale).clamp(0.0, self.height - size),
            width: size,
            height: size,
        })
    }

    pub(super) fn hit(&self, screen: Screen, cursor: [f32; 2]) -> Option<UiHit> {
        if !cursor.into_iter().all(f32::is_finite)
            || !(Rect {
                x: 0.0,
                y: 0.0,
                width: self.width,
                height: self.height,
            })
            .contains(cursor)
        {
            return None;
        }
        for index in 0..button_count(screen) {
            if self.button(screen, index)?.contains(cursor) {
                if screen == Screen::Crafting {
                    return Some(if index < crate::game::RECIPES.len() {
                        UiHit::Recipe(index)
                    } else {
                        UiHit::CraftBack
                    });
                }
                return Some(UiHit::Button(index));
            }
        }
        if screen == Screen::Inventory {
            if super::crafting::toggle(self).contains(cursor) {
                return Some(UiHit::Crafting);
            }
            for index in 0..36 {
                if self.inventory_slot(index)?.contains(cursor) {
                    return Some(UiHit::InventorySlot(index));
                }
            }
        }
        if screen == Screen::Storage {
            for index in 0..crate::game::CRATE_SLOTS {
                if super::storage::crate_slot(self, index)?.contains(cursor) {
                    return Some(UiHit::CrateSlot(index));
                }
            }
            for index in 0..36 {
                if super::storage::player_slot(self, index)?.contains(cursor) {
                    return Some(UiHit::InventorySlot(index));
                }
            }
        }
        None
    }
}
