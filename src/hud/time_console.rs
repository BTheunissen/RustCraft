//! A time-only command overlay; no world actions or pointer hit regions.

use super::{GREEN, HudVertex, MUTED, PANEL, Painter, TEXT, fit_text};

/// Builds a paused time-command overlay. The application owns text editing and execution.
pub fn build_time_console(
    width: u32,
    height: u32,
    line: &str,
    notice: Option<&str>,
    clock: &str,
) -> Vec<HudVertex> {
    if width < 160 || height < 96 {
        return Vec::new();
    }
    let mut painter = Painter {
        width: width as f32,
        height: height as f32,
        vertices: Vec::with_capacity(16_000),
    };
    let scale = (painter.width / 640.0)
        .min(painter.height / 400.0)
        .floor()
        .clamp(1.0, 4.0);
    let margin = 8.0 * scale;
    let panel_height = (112.0 * scale).min(painter.height - margin * 2.0);
    let top = painter.height - panel_height - margin;
    painter.rect(
        [margin, top, painter.width - margin * 2.0, panel_height],
        PANEL,
    );
    let x = margin + 8.0 * scale;
    let available = painter.width - x * 2.0;
    for (row, text, color) in [
        (0.0, format!("TIME DEBUG - WORLD PAUSED - {clock}"), GREEN),
        (
            17.0,
            "/time speed 60 (20S CYCLE)  /time speed 1 (NORMAL)".into(),
            MUTED,
        ),
        (
            30.0,
            "/time set dawn|day|noon|sunset|night|HH:MM".into(),
            MUTED,
        ),
        (
            47.0,
            notice
                .unwrap_or("ENTER RUNS - ESC CANCELS - F6 CYCLES SPEED IN PLAY")
                .into(),
            TEXT,
        ),
        (72.0, format!("{line}_"), GREEN),
    ] {
        painter.text(
            x,
            top + (8.0 + row) * scale,
            &fit_text(&text, available, scale),
            scale,
            color,
        );
    }
    painter.vertices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_overlay_is_bounded_and_clipped_with_oversized_or_unicode_input() {
        for (width, height) in [(160, 96), (640, 400), (1280, 720), (2400, 1520)] {
            let vertices = build_time_console(
                width,
                height,
                &"/time é".repeat(1000),
                Some(&"error".repeat(1000)),
                "23:59 120X",
            );
            assert!(!vertices.is_empty() && vertices.len() < 30_000);
            assert_eq!(vertices.len() % 6, 0);
            assert!(vertices.iter().all(|vertex| {
                vertex
                    .position
                    .into_iter()
                    .all(|value| value.is_finite() && (-1.0..=1.0).contains(&value))
            }));
        }
        assert!(build_time_console(0, 720, "", None, "").is_empty());
        assert!(build_time_console(160, 95, "", None, "").is_empty());
    }
}
