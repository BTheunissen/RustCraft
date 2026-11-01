//! A fixed 18-slot crate above the 36-slot player inventory; no native or save resources.

use super::interface_layout::{Rect, SLOT_GAP, SLOT_SIZE, UiLayout};
use super::{GREEN, MUTED, Painter, Screen, TEXT, UiHit, UiState};

pub(super) fn panel(layout: &UiLayout) -> Rect {
    layout.centered(400.0, 338.0)
}

pub(super) fn crate_slot(layout: &UiLayout, index: usize) -> Option<Rect> {
    if index >= crate::game::CRATE_SLOTS {
        return None;
    }
    let panel = panel(layout);
    Some(Rect {
        x: panel.x + (22.0 + (index % 9) as f32 * (SLOT_SIZE + SLOT_GAP)) * layout.scale,
        y: panel.y + (35.0 + (index / 9) as f32 * (SLOT_SIZE + SLOT_GAP)) * layout.scale,
        width: SLOT_SIZE * layout.scale,
        height: SLOT_SIZE * layout.scale,
    })
}

pub(super) fn player_slot(layout: &UiLayout, index: usize) -> Option<Rect> {
    if index >= 36 {
        return None;
    }
    let panel = panel(layout);
    let (column, row) = if index < 9 {
        (index, 3)
    } else {
        ((index - 9) % 9, (index - 9) / 9)
    };
    Some(Rect {
        x: panel.x + (22.0 + column as f32 * (SLOT_SIZE + SLOT_GAP)) * layout.scale,
        y: panel.y + (134.0 + row as f32 * (SLOT_SIZE + SLOT_GAP)) * layout.scale,
        width: SLOT_SIZE * layout.scale,
        height: SLOT_SIZE * layout.scale,
    })
}

pub(super) fn draw(painter: &mut Painter, layout: &UiLayout, ui: &UiState<'_>) {
    let panel = panel(layout);
    let scale = layout.scale;
    painter.rect(
        [0.0, 0.0, layout.width, layout.height],
        [0.009, 0.015, 0.024, 0.74],
    );
    super::menus::menu_panel(painter, panel, scale);
    painter.centered_text(
        panel.center()[0],
        panel.y + 12.0 * scale,
        "STORAGE CRATE",
        2.0 * scale,
        TEXT,
    );
    let hovered = ui
        .cursor
        .and_then(|cursor| layout.hit(Screen::Storage, cursor));
    for index in 0..crate::game::CRATE_SLOTS {
        if let Some(rect) = crate_slot(layout, index) {
            super::inventory::slot(
                painter,
                rect,
                ui.storage.and_then(|storage| storage.slots[index].as_ref()),
                false,
                hovered == Some(UiHit::CrateSlot(index)),
                None,
            );
        }
    }
    painter.text(
        panel.x + 22.0 * scale,
        panel.y + 119.0 * scale,
        "PLAYER INVENTORY / HOTBAR",
        scale,
        MUTED,
    );
    for index in 0..36 {
        if let Some(rect) = player_slot(layout, index) {
            super::inventory::slot(
                painter,
                rect,
                ui.inventory
                    .and_then(|inventory| inventory.slots()[index].as_ref()),
                false,
                hovered == Some(UiHit::InventorySlot(index)),
                (index < 9).then_some(index + 1),
            );
        }
    }
    painter.centered_text(
        panel.center()[0],
        panel.y + 297.0 * scale,
        "LEFT MOVE/SWAP  RIGHT HALF/ONE",
        scale,
        TEXT,
    );
    painter.centered_text(
        panel.center()[0],
        panel.y + 315.0 * scale,
        "I / ESC CLOSE - WORLD PAUSED",
        scale,
        GREEN,
    );
    if let Some(held) = ui.storage.and_then(|storage| storage.cursor)
        && let Some(rect) = layout.held_stack(ui.cursor)
    {
        painter.rect(rect.pixels(), [0.014, 0.023, 0.029, 0.84]);
        super::inventory::draw_stack(painter, rect, &held);
    }
}

#[cfg(test)]
mod tests;
