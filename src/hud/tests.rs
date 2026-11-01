use super::{
    CROSSHAIR, FLY_CONTROLS, HEADLAMP_HINT, HudInfo, HudVertex, MAX_TEXT_CHARS, MUTED, Painter,
    SWIM_CONTROLS, TEXT, WALK_CONTROLS, biome_label, build_hud, controls_bar, coordinate_label,
    count_label, fit_text, fps_label, glyph, readout_rows, text_width, truncate_text,
};

fn info(captured: bool, flying: bool) -> HudInfo<'static> {
    HudInfo {
        captured,
        flying,
        swimming: false,
        phasing: false,
        headlamp: true,
        underground: false,
        biome: "Temperate Forest",
        position: [12.5, 64.0, -173.25],
        fps: 59.7,
        loaded_chunks: 289,
        pending_chunks: 12,
        speed: crate::player::SpeedMultiplier::Normal,
        time: crate::game::WorldTime::default(),
    }
}

fn assert_valid(vertices: &[HudVertex]) {
    assert_eq!(vertices.len() % 6, 0);
    for vertex in vertices {
        for value in vertex.position {
            assert!(value.is_finite() && (-1.0..=1.0).contains(&value));
        }
        for value in vertex.color {
            assert!(value.is_finite() && (0.0..=1.0).contains(&value));
        }
    }
}

#[test]
fn empty_and_tiny_viewports_have_no_geometry() {
    for (width, height) in [(0, 0), (0, 720), (1280, 0), (1, 1), (159, 96), (160, 95)] {
        for captured in [false, true] {
            for underground in [false, true] {
                for headlamp in [false, true] {
                    let info = HudInfo {
                        underground,
                        headlamp,
                        ..info(captured, false)
                    };
                    assert!(build_hud(width, height, &info).is_empty());
                }
            }
        }
    }
}

#[test]
fn compact_and_standard_layouts_are_finite_clipped_triangle_lists() {
    for (width, height) in [
        (160, 96),
        (200, 128),
        (320, 180),
        (479, 720),
        (640, 279),
        (480, 280),
        (640, 360),
        (1200, 760),
        (1280, 720),
        (1920, 1080),
        (3840, 2160),
        (u32::MAX, u32::MAX),
    ] {
        for captured in [false, true] {
            for flying in [false, true] {
                for underground in [false, true] {
                    for headlamp in [false, true] {
                        let info = HudInfo {
                            underground,
                            headlamp,
                            ..info(captured, flying)
                        };
                        let vertices = build_hud(width, height, &info);
                        assert!(!vertices.is_empty());
                        assert_valid(&vertices);
                        assert!(vertices.len() < 40_000);
                    }
                }
            }
        }
    }
}

#[test]
fn only_a_captured_mouse_has_a_centered_crosshair() {
    for (width, height) in [(160, 96), (1280, 720)] {
        let captured = build_hud(width, height, &info(true, false));
        let paused = build_hud(width, height, &info(false, false));
        let crosshair: Vec<_> = captured
            .iter()
            .filter(|vertex| vertex.color == CROSSHAIR)
            .collect();
        assert_eq!(crosshair.len(), 24);
        assert!(
            crosshair.iter().all(|vertex| {
                vertex.position[0].abs() < 0.10 && vertex.position[1].abs() < 0.15
            })
        );
        assert!(!paused.iter().any(|vertex| vertex.color == CROSSHAIR));
    }
}

#[test]
fn ui_pixels_scale_readably_across_standard_and_resized_hidpi_windows() {
    for (width, height, expected_scale) in [
        (640, 360, 1.0),
        (1200, 760, 2.0),
        (1280, 720, 2.0),
        (1920, 1080, 3.0),
        (2560, 1440, 4.0),
    ] {
        let vertices = build_hud(width, height, &info(true, false));
        let horizontal_stroke: Vec<_> = vertices
            .iter()
            .filter(|vertex| vertex.color == CROSSHAIR)
            .take(6)
            .collect();
        let thickness = (horizontal_stroke[0].position[1] - horizontal_stroke[1].position[1])
            * height as f32
            * 0.5;
        assert!((thickness - expected_scale).abs() < 0.001);
    }
}

#[test]
fn uppercase_lowercase_digits_and_all_hud_punctuation_have_glyphs() {
    let punctuation = ".,:-+/\\()[]%!|_=<>?";
    for character in ('A'..='Z').chain('0'..='9').chain(punctuation.chars()) {
        let rows = glyph(character);
        assert!(rows.iter().any(|row| *row != 0), "{character}");
        assert!(rows.iter().all(|row| *row <= 0x1f), "{character}");
        if character != '?' {
            assert_ne!(rows, glyph('?'), "{character} uses the fallback glyph");
        }
        assert_eq!(glyph(character.to_ascii_lowercase()), rows);
    }
    for character in WALK_CONTROLS
        .chars()
        .chain(FLY_CONTROLS.chars())
        .chain(SWIM_CONTROLS.chars())
        .chain(HEADLAMP_HINT.chars())
        .chain("LAMP ON (H) OFF (H)".chars())
    {
        assert!(character == ' ' || glyph(character) != glyph('?'));
    }
    assert_eq!(glyph(' '), [0; 7]);
    assert_eq!(glyph('é'), glyph('?'));
}

#[test]
fn underground_readouts_keep_the_cave_biome_and_report_both_lamp_states() {
    let mut info = HudInfo {
        underground: true,
        biome: "Dripstone Cavern",
        ..info(true, false)
    };
    let on = readout_rows(&info);
    assert_eq!(on.len(), 7);
    assert_eq!(on[0], ("BIOME", "DRIPSTONE CAVERN".to_owned()));
    assert_eq!(on[6], ("LAMP", "ON (H)".to_owned()));

    info.headlamp = false;
    let off = readout_rows(&info);
    assert_eq!(off[6], ("LAMP", "OFF (H)".to_owned()));
    assert_eq!(&on[..6], &off[..6]);

    info.underground = false;
    let surface = readout_rows(&info);
    assert_eq!(surface, off[..6]);
    info.headlamp = true;
    assert_eq!(readout_rows(&info), surface);
}

#[test]
fn phasing_readout_requires_flight_and_keeps_the_overlay_readable() {
    let mut view = HudInfo {
        phasing: true,
        underground: true,
        ..info(true, true)
    };
    let rows = readout_rows(&view);
    assert_eq!(rows[1], ("MODE", "PHASING".to_owned()));
    assert_eq!(rows.last().unwrap(), &("VIEW", "ROCK CUTAWAY".to_owned()));
    assert_valid(&build_hud(1200, 760, &view));
    assert_valid(&build_hud(320, 180, &view));
    view.flying = false;
    assert_eq!(readout_rows(&view)[1], ("MODE", "ON FOOT".to_owned()));
    assert!(!readout_rows(&view).iter().any(|row| row.0 == "VIEW"));
}

#[test]
fn swimming_readouts_use_dive_controls_and_flight_keeps_priority() {
    let mut view = HudInfo {
        swimming: true,
        biome: "Freshwater River",
        ..info(true, false)
    };
    assert_eq!(readout_rows(&view)[1], ("MODE", "SWIMMING".to_owned()));
    for (width, height) in [(320, 180), (1200, 760), (2400, 1520)] {
        assert_valid(&build_hud(width, height, &view));
    }
    view.flying = true;
    assert_eq!(readout_rows(&view)[1], ("MODE", "FLIGHT".to_owned()));
    assert!(SWIM_CONTROLS.contains("SPACE UP/SHIFT DIVE"));
}

#[test]
fn controls_include_the_complete_headlamp_hint_at_scale_two() {
    for (flying, controls) in [(false, WALK_CONTROLS), (true, FLY_CONTROLS)] {
        assert!(controls.contains("H LAMP"));
        assert!(controls.chars().count() <= MAX_TEXT_CHARS);
        assert!(text_width(controls, 2.0) <= 1200.0 - 2.0 * 16.0 - 12.0 * 2.0);
        let mut painter = Painter {
            width: 1200.0,
            height: 760.0,
            vertices: Vec::new(),
        };
        controls_bar(&mut painter, flying, false, 16.0, 2.0);
        assert_valid(&painter.vertices);
        let expected_text_vertices: usize = controls
            .chars()
            .flat_map(glyph)
            .map(|row| row.count_ones() as usize * 6)
            .sum();
        assert_eq!(
            painter
                .vertices
                .iter()
                .filter(|vertex| vertex.color == MUTED)
                .count(),
            expected_text_vertices,
        );
    }
}

#[test]
fn text_measurement_and_truncation_keep_labels_inside_their_budget() {
    assert_eq!(text_width("", 2.0), 0.0);
    assert_eq!(text_width("AB", 2.0), 22.0);
    assert_eq!(fit_text("AB", 22.0, 2.0), "AB");
    assert_eq!(fit_text("ABCDE", 22.0, 2.0), "..");
    assert_eq!(fit_text("ABCDE", 0.0, 1.0), "");
    assert_eq!(truncate_text("ABCDEF", 5), "AB...");
    assert_eq!(truncate_text("éééééé", 5), "éé...");
    assert_eq!(truncate_text("ABC", 0), "");
    assert_eq!(biome_label("  "), "UNKNOWN");
    assert_eq!(biome_label("Snow Plains"), "SNOW PLAINS");
    assert_eq!(biome_label("é\n"), "??");
    let long_label = biome_label(&"forest".repeat(10_000));
    assert_eq!(long_label.len(), 18);
    assert!(long_label.ends_with("..."));
}

#[test]
fn numeric_readouts_handle_invalid_and_extreme_inputs() {
    assert_eq!(coordinate_label(-0.1), "0");
    assert_eq!(coordinate_label(-12.7), "-13");
    assert_eq!(coordinate_label(64.0), "64");
    assert!(coordinate_label(f32::MAX).len() <= 8);
    assert!(coordinate_label(-f32::MAX).len() <= 8);
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(coordinate_label(value), "--");
        assert_eq!(fps_label(value), "--");
    }
    assert_eq!(fps_label(-1.0), "--");
    assert_eq!(fps_label(59.7), "60");
    assert_eq!(fps_label(f32::MAX), "9999");
    assert_eq!(count_label(0), "0");
    assert_eq!(count_label(9999), "9999");
    assert_eq!(count_label(12_345), "12K");
    assert_eq!(count_label(2_000_000), "2M");
    assert_eq!(count_label(usize::MAX), "999M+");
}

#[test]
fn hostile_readout_values_cannot_expand_the_hud() {
    let long_biome = "W".repeat(100_000);
    let info = HudInfo {
        biome: &long_biome,
        position: [f32::NAN, -f32::MAX, f32::INFINITY],
        fps: f32::NAN,
        loaded_chunks: usize::MAX,
        pending_chunks: usize::MAX,
        underground: true,
        headlamp: false,
        ..info(false, true)
    };
    let vertices = build_hud(1280, 720, &info);
    assert_valid(&vertices);
    assert!(vertices.len() < 40_000);
}

#[test]
fn typical_exploration_and_pause_overlays_stay_lightweight() {
    for flying in [false, true] {
        let captured = build_hud(1280, 720, &info(true, flying));
        let paused = build_hud(1280, 720, &info(false, flying));
        assert!((5_000..24_000).contains(&captured.len()));
        assert!(paused.len() > captured.len());
        assert!(paused.len() < 40_000);
    }
}

#[test]
fn rectangles_clip_to_the_viewport_and_use_y_up_ndc() {
    let mut painter = Painter {
        width: 100.0,
        height: 100.0,
        vertices: Vec::new(),
    };
    painter.rect([-20.0, -10.0, 200.0, 150.0], TEXT);
    assert_eq!(painter.vertices.len(), 6);
    assert_eq!(painter.vertices[0].position, [-1.0, 1.0]);
    assert_eq!(painter.vertices[1].position, [-1.0, -1.0]);
    assert_eq!(painter.vertices[2].position, [1.0, -1.0]);
    assert_valid(&painter.vertices);
    for rect in [
        [-20.0, 0.0, 10.0, 10.0],
        [0.0, -20.0, 10.0, 10.0],
        [100.0, 0.0, 10.0, 10.0],
        [0.0, 100.0, 10.0, 10.0],
        [0.0, 0.0, 0.0, 10.0],
        [0.0, 0.0, 10.0, -1.0],
    ] {
        painter.rect(rect, TEXT);
    }
    assert_eq!(painter.vertices.len(), 6);
}

#[test]
fn vertex_layout_is_packed_and_bytemuck_compatible() {
    let vertex = HudVertex {
        position: [0.0, 1.0],
        color: TEXT,
    };
    assert_eq!(std::mem::size_of::<HudVertex>(), 24);
    assert_eq!(std::mem::align_of::<HudVertex>(), 4);
    assert_eq!(bytemuck::bytes_of(&vertex).len(), 24);
}

#[test]
fn speed_readouts_and_controls_cover_every_multiplier_without_clipping() {
    use crate::player::SpeedMultiplier;
    for speed in [
        SpeedMultiplier::Normal,
        SpeedMultiplier::Double,
        SpeedMultiplier::Quadruple,
        SpeedMultiplier::Octuple,
        SpeedMultiplier::Sixteen,
    ] {
        let view = HudInfo {
            speed,
            ..info(true, true)
        };
        let rows = readout_rows(&view);
        assert!(
            rows.iter()
                .any(|(name, label)| *name == "SPEED"
                    && label == &format!("{}X (CTRL)", speed as u8))
        );
        for (width, height) in [(320, 180), (1200, 760), (1920, 1080)] {
            assert_valid(&build_hud(width, height, &view));
        }
    }
    assert!(FLY_CONTROLS.contains("CTRL SPEED"));
    assert!(WALK_CONTROLS.contains("CTRL SPEED"));
    assert!(SWIM_CONTROLS.contains("CTRL SPEED"));
}
