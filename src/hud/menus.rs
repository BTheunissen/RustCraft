//! Live-terrain menu overlays, original pixel title, and blocky button styling.

use crate::game::GameMode;

use super::interface_layout::{Rect, UiLayout};
use super::readouts::count_label;
use super::{
    GREEN, HudInfo, MUTED, Painter, SaveNotice, Screen, TEXT, UiHit, UiState, button_count, glyph,
};

pub(super) const BUTTON_FOCUS: [f32; 4] = [0.77, 0.92, 0.66, 1.0];
const NAVIGATION: &str = "UP/DOWN OR TAB SELECT  ENTER OR CLICK";

pub(super) fn highlighted_button(layout: &UiLayout, ui: &UiState<'_>) -> Option<usize> {
    let index = if let Some(UiHit::Button(index)) =
        ui.cursor.and_then(|cursor| layout.hit(ui.screen, cursor))
    {
        index
    } else {
        ui.focused_button
    };
    (index < button_count(ui.screen)
        && (ui.screen != Screen::MainMenu || index != 0 || ui.can_continue))
        .then_some(index)
}

pub(super) fn menu(painter: &mut Painter, layout: &UiLayout, ui: &UiState<'_>) {
    let scale = layout.scale;
    let center = layout.width * 0.5;
    let middle = layout.height * 0.5;
    painter.rect(
        [0.0, 0.0, layout.width, layout.height],
        [
            0.012,
            0.023,
            0.031,
            if ui.screen.is_menu_preview() {
                0.30
            } else {
                0.67
            },
        ],
    );
    let labels: &[&str] = match ui.screen {
        Screen::MainMenu => {
            pixel_title(painter, center, middle - 166.0 * scale, scale);
            painter.centered_text(
                center,
                middle - 100.0 * scale,
                "A WORLD OF YOUR OWN",
                2.0 * scale,
                TEXT,
            );
            painter.centered_text(
                center + scale,
                middle - 61.0 * scale,
                "ONE LOCAL WORLD - SAVED AUTOMATICALLY",
                scale,
                [0.0, 0.0, 0.0, 0.72],
            );
            painter.centered_text(
                center,
                middle - 62.0 * scale,
                "ONE LOCAL WORLD - SAVED AUTOMATICALLY",
                scale,
                MUTED,
            );
            &["CONTINUE", "NEW SURVIVAL", "NEW CREATIVE", "QUIT"]
        }
        Screen::Paused => {
            menu_panel(painter, layout.centered(360.0, 336.0), scale);
            painter.centered_text(center, middle - 126.0 * scale, "PAUSED", 3.0 * scale, TEXT);
            painter.centered_text(
                center,
                middle - 84.0 * scale,
                "YOUR WORLD IS PAUSED",
                scale,
                MUTED,
            );
            &["RESUME", "SAVE / MAIN MENU", "SAVE / QUIT", "AUDIO"]
        }
        Screen::ConfirmReplace(_) => {
            menu_panel(painter, layout.centered(440.0, 284.0), scale);
            painter.centered_text(
                center,
                middle - 103.0 * scale,
                "REPLACE SAVED WORLD?",
                2.0 * scale,
                TEXT,
            );
            painter.centered_text(
                center,
                middle - 62.0 * scale,
                "THIS REPLACES YOUR ONLY LOCAL SAVE.",
                scale,
                [0.98, 0.66, 0.34, 1.0],
            );
            painter.centered_text(
                center,
                middle - 43.0 * scale,
                "YOUR PREVIOUS BUILD CANNOT BE CONTINUED.",
                scale,
                MUTED,
            );
            &["REPLACE WORLD", "CANCEL"]
        }
        _ => return,
    };
    let highlighted = highlighted_button(layout, ui);
    for (index, label) in labels.iter().enumerate() {
        if let Some(rect) = layout.button(ui.screen, index) {
            let enabled = ui.screen != Screen::MainMenu || index != 0 || ui.can_continue;
            button(
                painter,
                rect,
                label,
                scale,
                highlighted == Some(index),
                enabled,
            );
        }
    }
    let (footer_y, footer) = match ui.screen {
        Screen::MainMenu => (middle + 146.0 * scale, NAVIGATION),
        Screen::Paused => (middle + 126.0 * scale, NAVIGATION),
        _ => (middle + 108.0 * scale, "ENTER / CLICK SELECT  ESC CANCEL"),
    };
    if ui.screen == Screen::MainMenu {
        painter.centered_text(
            center + scale,
            footer_y + scale,
            footer,
            scale,
            [0.0, 0.0, 0.0, 0.72],
        );
    }
    painter.centered_text(center, footer_y, footer, scale, MUTED);
    if ui.screen == Screen::Paused {
        painter.centered_text(
            center,
            footer_y + 16.0 * scale,
            "CLICK RESUME TO CAPTURE MOUSE",
            scale,
            GREEN,
        );
    }
    if let Some(notice) = ui.save_notice {
        let (message, color) = match notice {
            SaveNotice::Saved => ("WORLD SAVED LOCALLY", GREEN),
            SaveNotice::SaveFailed => (
                "SAVE FAILED - CHECK STORAGE AND RETRY.",
                [0.98, 0.66, 0.34, 1.0],
            ),
            SaveNotice::LoadFailed => (
                "CANNOT CONTINUE - SAVED FILE WAS NOT CHANGED.",
                [0.98, 0.66, 0.34, 1.0],
            ),
            SaveNotice::Incompatible => (
                "NEW DEVELOPMENT WORLD REQUIRED - BACK UP SAVE FIRST.",
                [0.98, 0.66, 0.34, 1.0],
            ),
            SaveNotice::Unavailable => (
                "LOCAL SAVE UNAVAILABLE - CHECK TERMINAL ERROR.",
                [0.98, 0.66, 0.34, 1.0],
            ),
        };
        painter.centered_text(center, layout.height - 18.0 * scale, message, scale, color);
    }
}

pub(super) fn menu_panel(painter: &mut Painter, rect: Rect, scale: f32) {
    painter.rect(
        [
            rect.x + 4.0 * scale,
            rect.y + 6.0 * scale,
            rect.width,
            rect.height,
        ],
        [0.0, 0.0, 0.0, 0.35],
    );
    painter.rect(rect.pixels(), [0.040, 0.059, 0.066, 0.95]);
    painter.frame(rect.pixels(), scale, [0.25, 0.34, 0.31, 1.0]);
    painter.rect([rect.x, rect.y, 52.0 * scale, 3.0 * scale], GREEN);
}

pub(super) fn button(
    painter: &mut Painter,
    rect: Rect,
    label: &str,
    scale: f32,
    highlighted: bool,
    enabled: bool,
) {
    painter.rect(
        [
            rect.x + 2.0 * scale,
            rect.y + 3.0 * scale,
            rect.width,
            rect.height,
        ],
        [0.0, 0.0, 0.0, 0.55],
    );
    painter.rect(
        rect.pixels(),
        if highlighted {
            [0.14, 0.25, 0.17, 1.0]
        } else {
            [0.105, 0.132, 0.145, 0.98]
        },
    );
    painter.frame(rect.pixels(), scale, [0.024, 0.029, 0.031, 1.0]);
    painter.rect(
        [
            rect.x + scale,
            rect.y + scale,
            rect.width - 2.0 * scale,
            2.0 * scale,
        ],
        [0.36, 0.43, 0.39, 1.0],
    );
    painter.rect(
        [
            rect.x + scale,
            rect.y + rect.height - 3.0 * scale,
            rect.width - 2.0 * scale,
            2.0 * scale,
        ],
        [0.055, 0.075, 0.069, 1.0],
    );
    if highlighted {
        painter.frame(rect.pixels(), 2.0 * scale, BUTTON_FOCUS);
    }
    painter.centered_text(
        rect.center()[0] + scale,
        rect.y + 11.0 * scale,
        label,
        2.0 * scale,
        [0.012, 0.018, 0.018, 0.90],
    );
    painter.centered_text(
        rect.center()[0],
        rect.y + 10.0 * scale,
        label,
        2.0 * scale,
        if enabled { TEXT } else { MUTED },
    );
}

fn pixel_title(painter: &mut Painter, center: f32, top: f32, scale: f32) {
    let pixel = 6.0 * scale;
    let left = center - super::text_width("RUSTCRAFT", pixel) * 0.5;
    painter.text(
        left + 4.0 * scale,
        top + 7.0 * scale,
        "RUSTCRAFT",
        pixel,
        [0.0, 0.0, 0.0, 0.50],
    );
    painter.text(
        left + 2.0 * scale,
        top + 4.0 * scale,
        "RUSTCRAFT",
        pixel,
        [0.11, 0.16, 0.14, 1.0],
    );
    for (index, character) in "RUSTCRAFT".chars().enumerate() {
        for (row, bits) in glyph(character).into_iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                let grain = ((index * 13 + row * 7 + column * 3) % 5) as f32 * 0.025;
                let color = if row == 0 {
                    [0.46 + grain, 0.65 + grain, 0.35 + grain, 1.0]
                } else {
                    [0.68 + grain, 0.73 + grain, 0.65 + grain, 1.0]
                };
                painter.rect(
                    [
                        left + (index * 6 + column) as f32 * pixel,
                        top + row as f32 * pixel,
                        pixel,
                        pixel,
                    ],
                    color,
                );
            }
        }
    }
}

pub(super) fn loading(
    painter: &mut Painter,
    layout: &UiLayout,
    info: &HudInfo<'_>,
    mode: GameMode,
) {
    let scale = layout.scale;
    let center = layout.width * 0.5;
    let panel = layout.centered(380.0, 164.0);
    painter.rect(
        [0.0, 0.0, layout.width, layout.height],
        [0.01, 0.02, 0.03, 0.52],
    );
    menu_panel(painter, panel, scale);
    painter.centered_text(
        center,
        panel.y + 17.0 * scale,
        "RUSTCRAFT",
        3.0 * scale,
        TEXT,
    );
    painter.centered_text(
        center,
        panel.y + 55.0 * scale,
        match mode {
            GameMode::Creative => "LOADING CREATIVE",
            GameMode::Survival => "LOADING SURVIVAL",
        },
        2.0 * scale,
        GREEN,
    );
    painter.centered_text(
        center,
        panel.y + 86.0 * scale,
        "PREPARING TERRAIN...",
        scale,
        TEXT,
    );
    painter.centered_text(
        center,
        panel.y + 107.0 * scale,
        &format!(
            "{} CHUNKS READY  {} PENDING",
            count_label(info.loaded_chunks),
            count_label(info.pending_chunks),
        ),
        scale,
        MUTED,
    );
    painter.centered_text(
        center,
        panel.y + 139.0 * scale,
        "PLEASE WAIT - YOUR WORLD IS BEING BUILT",
        scale,
        MUTED,
    );
}
