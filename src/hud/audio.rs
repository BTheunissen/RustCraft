//! Compact pause controls; all rectangles are shared by drawing and hit testing.

use super::interface_layout::{Rect, UiLayout};
use super::{MUTED, Painter, Screen, TEXT, UiState};

/// Bounded UI readout, not a native device or preferences handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioControls {
    pub muted: bool,
    pub master: u8,
    pub actions: u8,
    pub ambience: u8,
    pub unavailable: bool,
    pub preferences_unavailable: bool,
}

pub(super) fn rectangle(layout: &UiLayout, index: usize) -> Option<Rect> {
    let (left, top, width) = match index {
        0 => (-140.0, -90.0, 280.0),
        1..=6 => (
            if index % 2 == 1 { -140.0 } else { 96.0 },
            -42.0 + ((index - 1) / 2) as f32 * 42.0,
            44.0,
        ),
        7 => (-140.0, 84.0, 280.0),
        _ => return None,
    };
    Some(Rect {
        x: layout.width * 0.5 + left * layout.scale,
        y: layout.height * 0.5 + top * layout.scale,
        width: width * layout.scale,
        height: 34.0 * layout.scale,
    })
}

pub(super) fn draw(
    painter: &mut Painter,
    layout: &UiLayout,
    ui: &UiState<'_>,
    controls: AudioControls,
) {
    let scale = layout.scale;
    let center = layout.width * 0.5;
    let middle = layout.height * 0.5;
    painter.rect(
        [0.0, 0.0, layout.width, layout.height],
        [0.012, 0.023, 0.031, 0.67],
    );
    super::menus::menu_panel(painter, layout.centered(360.0, 308.0), scale);
    painter.centered_text(center, middle - 130.0 * scale, "AUDIO", 3.0 * scale, TEXT);
    let focus = super::menus::highlighted_button(layout, ui);
    for index in 0..8 {
        if let Some(rect) = rectangle(layout, index) {
            let label = match index {
                0 => {
                    if controls.muted {
                        "SOUND: MUTED"
                    } else {
                        "SOUND: ON"
                    }
                }
                1 | 3 | 5 => "-",
                2 | 4 | 6 => "+",
                _ => "BACK TO PAUSE",
            };
            super::menus::button(painter, rect, label, scale, focus == Some(index), true);
        }
    }
    for (row, (label, gain)) in [
        ("MASTER", controls.master),
        ("ACTIONS", controls.actions),
        ("AMBIENCE", controls.ambience),
    ]
    .into_iter()
    .enumerate()
    {
        painter.centered_text(
            center,
            middle + (-31.0 + row as f32 * 42.0) * scale,
            &format!("{label} {}%", gain.min(100)),
            2.0 * scale,
            TEXT,
        );
    }
    let note = if controls.unavailable {
        "NO AUDIO DEVICE - PLAYING SILENTLY"
    } else if controls.preferences_unavailable {
        "SETTINGS NOT SAVED - CHECK STORAGE"
    } else {
        "UP/DOWN SELECT  ENTER/CLICK  ESC BACK"
    };
    painter.centered_text(center, middle + 135.0 * scale, note, scale, MUTED);
    debug_assert!(matches!(ui.screen, Screen::Audio(_)));
}

#[cfg(test)]
mod tests;
