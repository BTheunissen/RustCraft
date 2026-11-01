use super::*;
use crate::game::GameMode;
use crate::hud::{HudInfo, UiHit, UiState, build_interface, button_count, hit_test};

fn controls() -> AudioControls {
    AudioControls {
        muted: false,
        master: 55,
        actions: 70,
        ambience: 50,
        unavailable: false,
        preferences_unavailable: false,
    }
}

#[test]
fn compact_audio_buttons_share_nonoverlapping_draw_hit_regions() {
    for (width, height) in [(640, 400), (1200, 760), (1920, 1080), (160, 96)] {
        let layout = UiLayout::new(width, height).unwrap();
        let screen = Screen::Audio(controls());
        assert_eq!(button_count(screen), 8);
        for index in 0..8 {
            let rect = rectangle(&layout, index).unwrap();
            assert_eq!(
                hit_test(width, height, screen, rect.center()),
                Some(UiHit::Button(index))
            );
            for other in index + 1..8 {
                let other = rectangle(&layout, other).unwrap();
                assert!(
                    !(rect.x < other.x + other.width
                        && other.x < rect.x + rect.width
                        && rect.y < other.y + other.height
                        && other.y < rect.y + rect.height)
                );
            }
        }
        assert!(rectangle(&layout, 8).is_none());
        assert_eq!(
            hit_test(
                width,
                height,
                screen,
                [width as f32 * 0.5, height as f32 * 0.5]
            ),
            None
        );
    }
}

#[test]
fn audio_readouts_and_failure_states_draw_finite_clipped_geometry() {
    let info = HudInfo {
        captured: false,
        flying: false,
        swimming: false,
        phasing: false,
        headlamp: true,
        underground: false,
        biome: "",
        position: [0.0; 3],
        fps: 60.0,
        loaded_chunks: 0,
        pending_chunks: 0,
        speed: crate::player::SpeedMultiplier::Normal,
        time: crate::game::WorldTime::default(),
    };
    for controls in [
        controls(),
        AudioControls {
            muted: true,
            master: u8::MAX,
            unavailable: true,
            ..controls()
        },
        AudioControls {
            preferences_unavailable: true,
            ..controls()
        },
    ] {
        for (width, height) in [(160, 96), (640, 400), (1920, 1080), (u32::MAX, u32::MAX)] {
            let ui = UiState {
                screen: Screen::Audio(controls),
                mode: GameMode::Survival,
                inventory: None,
                storage: None,
                interaction_notice: None,
                cursor: None,
                focused_button: 0,
                can_continue: false,
                save_notice: None,
            };
            let vertices = build_interface(width, height, &info, &ui);
            assert!(!vertices.is_empty() && vertices.len() < 120_000);
            assert!(vertices.iter().all(|vertex| {
                vertex
                    .position
                    .into_iter()
                    .all(|value| value.is_finite() && (-1.0..=1.0).contains(&value))
            }));
        }
    }
}
