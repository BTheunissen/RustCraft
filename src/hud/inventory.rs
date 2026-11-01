//! Survival hotbar, whole-stack inventory, and safely separated icon/count cells.

use crate::game::{Inventory, Item, ItemStack};
use crate::world::Block;

use super::icons::{block_icon, tool_icon};
use super::interface_layout::{GRID_WIDTH, Rect, UiLayout};
use super::readouts::{biome_label, fps_label};
use super::{
    GREEN, HudInfo, MUTED, PANEL, Painter, Screen, TEXT, UiHit, UiState, crosshair, fit_text,
    text_width,
};

pub(super) const SELECTION: [f32; 4] = [0.87, 0.94, 0.72, 1.0];
pub(super) const HOVER: [f32; 4] = [0.39, 0.56, 0.55, 1.0];
const MINE_CONTROLS: &str = "HOLD LEFT MINE  RIGHT PLACE  I INVENTORY";
const WALK_CONTROLS: &str = "WASD MOVE  SPACE JUMP  1-9/WHEEL SELECT  H LAMP  ESC PAUSE";
const SWIM_CONTROLS: &str = "WASD SWIM  SPACE/SHIFT  1-9/WHEEL SELECT  H LAMP  ESC PAUSE";

pub(super) fn survival_controls(swimming: bool) -> [&'static str; 2] {
    [
        MINE_CONTROLS,
        if swimming {
            SWIM_CONTROLS
        } else {
            WALK_CONTROLS
        },
    ]
}

pub(super) fn survival(
    painter: &mut Painter,
    layout: &UiLayout,
    info: &HudInfo<'_>,
    ui: &UiState<'_>,
) {
    let scale = layout.scale;
    let margin = 8.0 * scale;
    painter.rect([margin, margin, 220.0 * scale, 40.0 * scale], PANEL);
    painter.text(
        margin + 6.0 * scale,
        margin + 5.0 * scale,
        "RUSTCRAFT",
        2.0 * scale,
        TEXT,
    );
    painter.text(
        margin + 132.0 * scale,
        margin + 9.0 * scale,
        "SURVIVAL",
        scale,
        GREEN,
    );
    painter.text(
        margin + 6.0 * scale,
        margin + 26.0 * scale,
        &format!("BIOME {}", biome_label(info.biome)),
        scale,
        MUTED,
    );
    let right = layout.width - margin - 210.0 * scale;
    painter.rect(
        [
            right,
            margin,
            210.0 * scale,
            if info.swimming { 43.0 } else { 30.0 } * scale,
        ],
        PANEL,
    );
    painter.text(
        right + 6.0 * scale,
        margin + 5.0 * scale,
        &format!(
            "{} FPS  H LAMP {}",
            fps_label(info.fps),
            if info.headlamp { "ON" } else { "OFF" }
        ),
        scale,
        MUTED,
    );
    painter.text(
        right + 6.0 * scale,
        margin + 18.0 * scale,
        &format!("TIME {}  F6", info.time.label()),
        scale,
        GREEN,
    );
    if info.swimming {
        painter.text(
            right + 6.0 * scale,
            margin + 31.0 * scale,
            "SWIMMING - SPACE UP/SHIFT DIVE",
            scale,
            GREEN,
        );
    }
    crosshair(painter, scale);

    let selected = ui.inventory.map_or(0, Inventory::selected).min(8);
    painter.rect(
        [
            (layout.width - GRID_WIDTH * scale) * 0.5 - 4.0 * scale,
            layout.height - 86.0 * scale,
            (GRID_WIDTH + 8.0) * scale,
            44.0 * scale,
        ],
        PANEL,
    );
    for index in 0..9 {
        if let Some(rect) = layout.hotbar_slot(index) {
            slot(
                painter,
                rect,
                ui.inventory
                    .and_then(|inventory| inventory.slots()[index].as_ref()),
                index == selected,
                false,
                Some(index + 1),
            );
        }
    }
    let stack = ui.inventory.and_then(Inventory::selected_stack);
    let name = fit_text(
        stack
            .filter(|stack| stack.count > 0 && stack.item != Item::Block(Block::Air))
            .map_or("EMPTY HAND", |stack| stack.item.name()),
        layout.width - 32.0 * scale,
        scale,
    );
    let width = text_width(&name, scale) + 12.0 * scale;
    painter.rect(
        [
            (layout.width - width) * 0.5,
            layout.height - 107.0 * scale,
            width,
            17.0 * scale,
        ],
        PANEL,
    );
    painter.centered_text(
        layout.width * 0.5,
        layout.height - 102.0 * scale,
        &name,
        scale,
        TEXT,
    );
    let controls = survival_controls(info.swimming);
    if let Some(notice) = ui.interaction_notice {
        painter.centered_text(
            layout.width * 0.5,
            layout.height * 0.55,
            notice,
            scale,
            [0.98, 0.66, 0.34, 1.0],
        );
    }
    let width = controls
        .iter()
        .map(|text| text_width(text, scale))
        .fold(0.0_f32, f32::max)
        + 16.0 * scale;
    painter.rect(
        [
            (layout.width - width) * 0.5,
            layout.height - 36.0 * scale,
            width,
            28.0 * scale,
        ],
        PANEL,
    );
    for (index, text) in controls.into_iter().enumerate() {
        painter.centered_text(
            layout.width * 0.5,
            layout.height - (29.0 - index as f32 * 13.0) * scale,
            text,
            scale,
            MUTED,
        );
    }
}

pub(super) fn inventory(painter: &mut Painter, layout: &UiLayout, ui: &UiState<'_>) {
    let scale = layout.scale;
    let panel = layout.inventory_panel();
    let center = panel.center()[0];
    painter.rect(
        [0.0, 0.0, layout.width, layout.height],
        [0.009, 0.015, 0.024, 0.74],
    );
    painter.rect(
        [
            panel.x + 4.0 * scale,
            panel.y + 6.0 * scale,
            panel.width,
            panel.height,
        ],
        [0.0, 0.0, 0.0, 0.40],
    );
    painter.rect(panel.pixels(), [0.065, 0.085, 0.092, 0.98]);
    painter.frame(panel.pixels(), scale, [0.28, 0.37, 0.34, 1.0]);
    painter.centered_text(
        center,
        panel.y + 13.0 * scale,
        "INVENTORY",
        2.0 * scale,
        TEXT,
    );
    painter.text(
        panel.x + 22.0 * scale,
        panel.y + 35.0 * scale,
        "STORAGE",
        scale,
        MUTED,
    );
    painter.text(
        panel.x + 22.0 * scale,
        panel.y + 176.0 * scale,
        "HOTBAR",
        scale,
        MUTED,
    );
    let hovered = ui
        .cursor
        .and_then(|cursor| layout.hit(Screen::Inventory, cursor));
    let selected = ui.inventory.map_or(0, Inventory::selected).min(8);
    for index in 0..36 {
        if let Some(rect) = layout.inventory_slot(index) {
            slot(
                painter,
                rect,
                ui.inventory
                    .and_then(|inventory| inventory.slots()[index].as_ref()),
                index == selected,
                hovered == Some(UiHit::InventorySlot(index)),
                (index < 9).then_some(index + 1),
            );
        }
    }
    painter.centered_text(
        center,
        panel.y + 241.0 * scale,
        "CLICK: PICK UP / PLACE / SWAP WHOLE STACKS",
        scale,
        TEXT,
    );
    let craft = super::crafting::toggle(layout);
    painter.rect(craft.pixels(), [0.13, 0.23, 0.16, 1.0]);
    painter.centered_text(
        craft.center()[0],
        craft.y + 3.0 * scale,
        "CRAFTING (C)",
        scale,
        GREEN,
    );
    painter.centered_text(
        center,
        panel.y + 256.0 * scale,
        "RIGHT CLICK: HALF STACK / PLACE ONE",
        scale,
        MUTED,
    );
    painter.centered_text(
        center,
        panel.y + 271.0 * scale,
        "I / ESC CLOSE - WORLD PAUSED",
        scale,
        GREEN,
    );
    if let Some(stack) = ui.inventory.and_then(Inventory::cursor_stack)
        && let Some(rect) = layout.held_stack(ui.cursor)
    {
        painter.rect(rect.pixels(), [0.014, 0.023, 0.029, 0.84]);
        draw_stack(painter, rect, &stack);
    }
}

pub(super) fn slot(
    painter: &mut Painter,
    rect: Rect,
    stack: Option<&ItemStack>,
    selected: bool,
    hovered: bool,
    key: Option<usize>,
) {
    let scale = rect.width / 36.0;
    painter.rect(rect.pixels(), [0.031, 0.044, 0.054, 0.97]);
    painter.frame(rect.pixels(), scale, [0.15, 0.19, 0.20, 1.0]);
    painter.rect(
        [
            rect.x + scale,
            rect.y + scale,
            rect.width - 2.0 * scale,
            scale,
        ],
        [0.009, 0.014, 0.020, 1.0],
    );
    if selected {
        painter.frame(rect.pixels(), 2.0 * scale, SELECTION);
    } else if hovered {
        painter.frame(rect.pixels(), scale, HOVER);
    }
    if let Some(key) = key {
        painter.text(
            rect.x + 3.0 * scale,
            rect.y + 3.0 * scale,
            &key.to_string(),
            scale,
            MUTED,
        );
    }
    if let Some(stack) = stack {
        draw_stack(painter, rect, stack);
    }
}

/// Icon and count occupy disjoint bands, including the widest possible u16 count.
pub(super) fn stack_rects(rect: Rect, count: u16) -> (Rect, Rect) {
    let scale = rect.width / 36.0;
    let width = text_width(&count.to_string(), scale);
    (
        Rect {
            x: rect.x + 8.0 * scale,
            y: rect.y + 3.0 * scale,
            width: 20.0 * scale,
            height: 20.0 * scale,
        },
        Rect {
            x: rect.x + rect.width - 3.0 * scale - width,
            y: rect.y + rect.height - 10.0 * scale,
            width,
            height: 7.0 * scale,
        },
    )
}

pub(super) fn draw_stack(painter: &mut Painter, rect: Rect, stack: &ItemStack) {
    if stack.count == 0 || stack.item == Item::Block(Block::Air) {
        return;
    }
    let scale = rect.width / 36.0;
    let (icon, count) = stack_rects(rect, stack.count);
    match stack.item {
        Item::Block(block) => block_icon(painter, block, icon.x, icon.y, icon.width),
        Item::Tool(tool) => tool_icon(painter, tool, icon.x, icon.y, icon.width),
    }
    painter.rect(
        [
            count.x - scale,
            count.y - scale,
            count.width + 2.0 * scale,
            count.height + 2.0 * scale,
        ],
        [0.01, 0.015, 0.018, 0.86],
    );
    let label = stack.count.to_string();
    painter.text(
        count.x + scale,
        count.y + scale,
        &label,
        scale,
        [0.0, 0.0, 0.0, 0.95],
    );
    painter.text(count.x, count.y, &label, scale, TEXT);
}
