//! Bounded exploration labels and readout layout.

use super::{GREEN, HudInfo, PANEL, Painter, TEXT, fit_text, text_width, truncate_text};

pub(super) fn biome_label(biome: &str) -> String {
    let label: String = biome
        .chars()
        .take(19)
        .map(|character| {
            if character.is_ascii_graphic() || character == ' ' {
                character.to_ascii_uppercase()
            } else {
                '?'
            }
        })
        .collect();
    if label.trim().is_empty() {
        "UNKNOWN".to_owned()
    } else {
        truncate_text(label.trim(), 18)
    }
}

pub(super) fn coordinate_label(value: f32) -> String {
    if !value.is_finite() {
        "--".to_owned()
    } else if value.abs() >= 100_000.0 {
        format!("{value:.1e}")
    } else {
        format!("{:.0}", if value.abs() < 0.5 { 0.0 } else { value })
    }
}

pub(super) fn fps_label(fps: f32) -> String {
    if fps.is_finite() && fps >= 0.0 {
        format!("{:.0}", fps.min(9999.0))
    } else {
        "--".to_owned()
    }
}

pub(super) fn count_label(count: usize) -> String {
    match count {
        0..10_000 => count.to_string(),
        10_000..1_000_000 => format!("{}K", count / 1000),
        1_000_000..1_000_000_000 => format!("{}M", count / 1_000_000),
        _ => "999M+".to_owned(),
    }
}

pub(super) fn readout_rows(info: &HudInfo<'_>) -> Vec<(&'static str, String)> {
    let [x, y, z] = info.position.map(coordinate_label);
    let mut rows = vec![
        ("BIOME", biome_label(info.biome)),
        (
            "MODE",
            if info.flying && info.phasing {
                "PHASING"
            } else if info.flying {
                "FLIGHT"
            } else if info.swimming {
                "SWIMMING"
            } else {
                "ON FOOT"
            }
            .to_owned(),
        ),
        ("SPEED", format!("{}X (CTRL)", info.speed as u8)),
        ("TIME", format!("{} (F6)", info.time.label())),
        ("XYZ", format!("{x} / {y} / {z}")),
        (
            "STATS",
            format!(
                "{} FPS  {}+{} CHUNKS",
                fps_label(info.fps),
                count_label(info.loaded_chunks),
                count_label(info.pending_chunks),
            ),
        ),
    ];
    if info.underground || info.time.hour() < 6.0 || info.time.hour() >= 18.0 {
        rows.push((
            "LAMP",
            if info.headlamp { "ON (H)" } else { "OFF (H)" }.to_owned(),
        ));
    }
    if info.flying && info.phasing {
        rows.push(("VIEW", "ROCK CUTAWAY".to_owned()));
    }
    rows
}

pub(super) fn readouts(
    painter: &mut Painter,
    info: &HudInfo<'_>,
    margin: f32,
    scale: f32,
    badge_width: f32,
) {
    let rows = readout_rows(info);
    let padding = 6.0 * scale;
    let label_width = text_width("STATS", scale) + 4.0 * scale;
    let value_width = rows
        .iter()
        .map(|(_, value)| text_width(value, scale))
        .fold(0.0_f32, f32::max);
    let width =
        (padding * 2.0 + label_width + value_width).min(painter.width - badge_width - margin * 3.0);
    let height = padding * 2.0 + (rows.len() as f32 * 10.0 - 3.0) * scale;
    let left = painter.width - margin - width;
    painter.rect([left, margin, width, height], PANEL);
    for (index, (label, value)) in rows.iter().enumerate() {
        let y = margin + padding + index as f32 * 10.0 * scale;
        painter.text(left + padding, y, label, scale, GREEN);
        painter.text(
            left + padding + label_width,
            y,
            &fit_text(value, width - padding * 2.0 - label_width, scale),
            scale,
            TEXT,
        );
    }
}
