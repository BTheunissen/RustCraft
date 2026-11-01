//! Standard/compact exploration overlays and pause controls.

use super::{
    BORDER, CROSSHAIR, FLY_CONTROLS, GREEN, HEADLAMP_HINT, HudInfo, MUTED, PANEL, Painter,
    SWIM_CONTROLS, TEXT, WALK_CONTROLS, fit_text, text_width,
};

pub(super) fn brand_badge(painter: &mut Painter, margin: f32, scale: f32) -> f32 {
    let title_scale = (scale * 1.5).round();
    let padding = 6.0 * scale;
    let width = text_width("RUSTCRAFT", title_scale) + padding * 2.0 + 5.0 * scale;
    let height = padding * 2.0 + 7.0 * title_scale + 11.0 * scale;
    painter.rect([margin, margin, width, height], PANEL);
    painter.rect(
        [
            margin + padding,
            margin + padding,
            2.0 * scale,
            height - padding * 2.0,
        ],
        GREEN,
    );
    let text_x = margin + padding + 5.0 * scale;
    painter.text(text_x, margin + padding, "RUSTCRAFT", title_scale, TEXT);
    painter.text(
        text_x,
        margin + padding + 7.0 * title_scale + 4.0 * scale,
        "CREATIVE",
        scale,
        GREEN,
    );
    width
}

pub(super) fn controls_bar(
    painter: &mut Painter,
    flying: bool,
    swimming: bool,
    margin: f32,
    scale: f32,
) {
    let full = if flying {
        FLY_CONTROLS
    } else if swimming {
        SWIM_CONTROLS
    } else {
        WALK_CONTROLS
    };
    let compact = if flying {
        "WASD  SPACE/SHIFT  F FLY  H LAMP  ESC PAUSE"
    } else if swimming {
        "WASD SWIM  SPACE/SHIFT  F FLY  H LAMP  ESC"
    } else {
        "WASD  SPACE JUMP  F FLY  H LAMP  ESC PAUSE"
    };
    let max_width = painter.width - margin * 2.0 - 12.0 * scale;
    let text = if text_width(full, scale) <= max_width {
        full.to_owned()
    } else {
        fit_text(compact, max_width, scale)
    };
    let width = text_width(&text, scale) + 12.0 * scale;
    let height = 17.0 * scale;
    let left = (painter.width - width) * 0.5;
    let top = painter.height - margin - height;
    painter.rect([left, top, width, height], PANEL);
    painter.centered_text(painter.width * 0.5, top + 5.0 * scale, &text, scale, MUTED);
}

pub(super) fn crosshair(painter: &mut Painter, scale: f32) {
    let center_x = (painter.width * 0.5).floor();
    let center_y = (painter.height * 0.5).floor();
    let stroke_x = (center_x - scale * 0.5).floor();
    let stroke_y = (center_y - scale * 0.5).floor();
    for [x, y, width, height] in [
        [center_x - 6.0 * scale, stroke_y, 4.0 * scale, scale],
        [center_x + 2.0 * scale, stroke_y, 4.0 * scale, scale],
        [stroke_x, center_y - 6.0 * scale, scale, 4.0 * scale],
        [stroke_x, center_y + 2.0 * scale, scale, 4.0 * scale],
    ] {
        painter.rect(
            [x - 1.0, y - 1.0, width + 2.0, height + 2.0],
            [0.02, 0.03, 0.04, 0.60],
        );
        painter.rect([x, y, width, height], CROSSHAIR);
    }
}

pub(super) fn pause_card(painter: &mut Painter, flying: bool, swimming: bool, scale: f32) {
    painter.rect(
        [0.0, 0.0, painter.width, painter.height],
        [0.015, 0.03, 0.04, 0.19],
    );
    let width = 256.0 * scale;
    let height = 144.0 * scale;
    let left = (painter.width - width) * 0.5;
    let top = (painter.height - height) * 0.5;
    let center = painter.width * 0.5;
    painter.rect(
        [left + 4.0 * scale, top + 6.0 * scale, width, height],
        [0.0, 0.0, 0.0, 0.24],
    );
    painter.rect([left, top, width, height], [0.035, 0.059, 0.080, 0.94]);
    painter.frame([left, top, width, height], scale, BORDER);
    painter.rect([left, top, 48.0 * scale, 2.0 * scale], GREEN);
    painter.centered_text(center, top + 16.0 * scale, "RUSTCRAFT", 2.0 * scale, TEXT);
    painter.centered_text(
        center,
        top + 35.0 * scale,
        "CREATIVE EXPLORER",
        scale,
        GREEN,
    );
    painter.rect(
        [
            left + 16.0 * scale,
            top + 51.0 * scale,
            width - 32.0 * scale,
            scale,
        ],
        [0.34, 0.49, 0.42, 0.25],
    );
    let button = [
        left + 22.0 * scale,
        top + 62.0 * scale,
        width - 44.0 * scale,
        22.0 * scale,
    ];
    painter.rect(button, [0.15, 0.26, 0.21, 0.94]);
    painter.frame(button, scale, BORDER);
    painter.centered_text(center, top + 69.0 * scale, "CLICK TO EXPLORE", scale, TEXT);
    painter.centered_text(
        center,
        top + 96.0 * scale,
        if flying {
            "WASD MOVE  SPACE UP / SHIFT DOWN"
        } else if swimming {
            "WASD SWIM  SPACE UP / SHIFT DIVE"
        } else {
            "WASD MOVE  SPACE JUMP"
        },
        scale,
        MUTED,
    );
    painter.centered_text(
        center,
        top + 108.0 * scale,
        "F FLIGHT  CTRL SPEED 1X-16X",
        scale,
        MUTED,
    );
    painter.centered_text(center, top + 120.0 * scale, HEADLAMP_HINT, scale, MUTED);
    painter.centered_text(center, top + 132.0 * scale, "Q TO QUIT", scale, GREEN);
}

pub(super) fn compact_hud(painter: &mut Painter, info: &HudInfo<'_>) {
    if info.captured {
        painter.text(8.0, 8.0, "RUSTCRAFT", 1.0, TEXT);
        painter.text(
            8.0,
            46.0,
            &format!("SPEED {}X (CTRL)", info.speed as u8),
            1.0,
            GREEN,
        );
        painter.text(
            8.0,
            58.0,
            &format!("TIME {} F6", info.time.label()),
            1.0,
            GREEN,
        );
        if info.underground {
            painter.text(
                8.0,
                22.0,
                if info.headlamp {
                    "LAMP ON (H)"
                } else {
                    "LAMP OFF (H)"
                },
                1.0,
                TEXT,
            );
        }
        if info.flying && info.phasing {
            painter.text(8.0, 34.0, "PHASING - ROCK CUTAWAY", 1.0, GREEN);
        }
        crosshair(painter, 1.0);
        let controls = if info.flying {
            "WASD  SPACE/SHIFT  F FLY  H LAMP  ESC"
        } else if info.swimming {
            "WASD SWIM  SPACE/SHIFT  F FLY  ESC"
        } else {
            "WASD  SPACE JUMP  F FLY  H LAMP  ESC"
        };
        let text = fit_text(controls, painter.width - 16.0, 1.0);
        painter.rect([0.0, painter.height - 25.0, painter.width, 25.0], PANEL);
        painter.centered_text(
            painter.width * 0.5,
            painter.height - 16.0,
            &text,
            1.0,
            MUTED,
        );
        return;
    }

    let width = (painter.width - 16.0).min(288.0);
    let height = (painter.height - 16.0).min(144.0);
    let left = (painter.width - width) * 0.5;
    let top = (painter.height - height) * 0.5;
    let center = painter.width * 0.5;
    painter.rect([left, top, width, height], PANEL);
    painter.frame([left, top, width, height], 1.0, BORDER);
    painter.rect([left, top, 40.0, 2.0], GREEN);
    painter.centered_text(center, top + 10.0, "RUSTCRAFT", 2.0, TEXT);
    let click_y = if height >= 128.0 {
        painter.centered_text(center, top + 32.0, "CREATIVE EXPLORER", 1.0, GREEN);
        top + 56.0
    } else {
        top + 40.0
    };
    painter.rect(
        [left + 8.0, click_y - 6.0, width - 16.0, 19.0],
        [0.15, 0.26, 0.21, 0.94],
    );
    painter.centered_text(center, click_y, "CLICK TO EXPLORE", 1.0, TEXT);
    if height >= 128.0 {
        let text = fit_text(
            if info.flying {
                "WASD MOVE  SPACE/SHIFT"
            } else {
                "WASD MOVE  SPACE JUMP"
            },
            width - 16.0,
            1.0,
        );
        painter.centered_text(center, top + 80.0, &text, 1.0, MUTED);
        painter.centered_text(center, top + 94.0, "H LAMP - DARK CAVES", 1.0, MUTED);
    }
    painter.centered_text(
        center,
        top + height - 17.0,
        "F FLY  H LAMP  Q QUIT",
        1.0,
        GREEN,
    );
}
