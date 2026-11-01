//! Paused inventory recipe list with shared physical-pixel hit regions.

use super::interface_layout::{Rect, UiLayout};
use super::{GREEN, MUTED, Painter, TEXT, UiState};
use crate::game::{CraftError, RECIPES};

pub(super) fn toggle(layout: &UiLayout) -> Rect {
    let panel = layout.inventory_panel();
    Rect {
        x: panel.x + 260.0 * layout.scale,
        y: panel.y + 33.0 * layout.scale,
        width: 118.0 * layout.scale,
        height: 14.0 * layout.scale,
    }
}

pub(super) fn recipe(layout: &UiLayout, index: usize) -> Option<Rect> {
    if index >= RECIPES.len() {
        return None;
    }
    let panel = layout.centered(464.0, 336.0);
    Some(Rect {
        x: panel.x + (16.0 + (index % 2) as f32 * 220.0) * layout.scale,
        y: panel.y + (48.0 + (index / 2) as f32 * 74.0) * layout.scale,
        width: 212.0 * layout.scale,
        height: 66.0 * layout.scale,
    })
}

pub(super) fn back(layout: &UiLayout) -> Rect {
    let panel = layout.centered(464.0, 336.0);
    Rect {
        x: panel.x + 18.0 * layout.scale,
        y: panel.y + 274.0 * layout.scale,
        width: 428.0 * layout.scale,
        height: 30.0 * layout.scale,
    }
}

pub(super) fn draw(painter: &mut Painter, layout: &UiLayout, ui: &UiState<'_>) {
    let panel = layout.centered(464.0, 336.0);
    let scale = layout.scale;
    painter.rect(
        [0.0, 0.0, layout.width, layout.height],
        [0.009, 0.015, 0.024, 0.74],
    );
    super::menus::menu_panel(painter, panel, scale);
    painter.centered_text(
        panel.center()[0],
        panel.y + 15.0 * scale,
        "CRAFTING",
        2.0 * scale,
        TEXT,
    );
    for (index, recipe_value) in RECIPES.iter().enumerate() {
        let rect = recipe(layout, index).unwrap_or(panel);
        let result = ui
            .inventory
            .map_or(Err(CraftError::MissingIngredients), |inventory| {
                inventory.can_craft(index)
            });
        let hovered = ui.cursor.is_some_and(|cursor| rect.contains(cursor))
            || ui.cursor.is_none() && ui.focused_button == index;
        painter.rect(
            rect.pixels(),
            if hovered {
                [0.12, 0.21, 0.15, 1.0]
            } else {
                [0.07, 0.095, 0.10, 1.0]
            },
        );
        painter.frame(
            rect.pixels(),
            scale,
            if result.is_ok() { GREEN } else { MUTED },
        );
        let inputs = recipe_value
            .inputs
            .iter()
            .map(|stack| format!("{} {}", stack.count, stack.item.name().to_uppercase()))
            .collect::<Vec<_>>()
            .join(" + ");
        painter.text(
            rect.x + 8.0 * scale,
            rect.y + 8.0 * scale,
            &format!(
                "{} {}",
                recipe_value.output.count,
                recipe_value.output.item.name().to_uppercase()
            ),
            1.5 * scale,
            TEXT,
        );
        painter.text(
            rect.x + 8.0 * scale,
            rect.y + 30.0 * scale,
            &inputs,
            scale,
            MUTED,
        );
        let status = match result {
            Ok(()) => "CLICK / ENTER: CRAFT ONE",
            Err(CraftError::NoCapacity) => "NO OUTPUT SPACE",
            _ => "NEED INGREDIENTS",
        };
        painter.text(
            rect.x + 8.0 * scale,
            rect.y + 48.0 * scale,
            status,
            scale,
            if result.is_ok() { GREEN } else { MUTED },
        );
    }
    let rect = back(layout);
    super::menus::button(
        painter,
        rect,
        "BACK TO INVENTORY",
        scale,
        ui.cursor
            .map_or(ui.focused_button == RECIPES.len(), |cursor| {
                rect.contains(cursor)
            }),
        true,
    );
    painter.centered_text(
        panel.center()[0],
        panel.y + 313.0 * scale,
        "AVAILABLE SLOTS ONLY - CURSOR EXCLUDED",
        scale,
        MUTED,
    );
    painter.centered_text(
        panel.center()[0],
        panel.y + 326.0 * scale,
        "ESC BACK  I CLOSE - WORLD PAUSED",
        scale,
        MUTED,
    );
}

#[cfg(test)]
mod tests;
