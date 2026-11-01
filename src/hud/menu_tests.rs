use crate::game::GameMode;

use super::interface_layout::UiLayout;
use super::{HudInfo, MUTED, Painter, SaveNotice, Screen, TEXT, UiState, build_interface, glyph};

fn view() -> HudInfo<'static> {
    HudInfo {
        captured: false,
        flying: false,
        swimming: false,
        phasing: false,
        headlamp: true,
        underground: false,
        biome: "Plains",
        position: [0.0, 64.0, 0.0],
        fps: 60.0,
        loaded_chunks: 9,
        pending_chunks: 0,
        speed: crate::player::SpeedMultiplier::Normal,
        time: crate::game::WorldTime::default(),
    }
}

#[test]
fn menu_label_order_matches_the_public_button_contract() {
    let layout = UiLayout::new(640, 400).unwrap();
    for (screen, labels) in [
        (
            Screen::MainMenu,
            &["CONTINUE", "NEW SURVIVAL", "NEW CREATIVE", "QUIT"][..],
        ),
        (
            Screen::Paused,
            &["RESUME", "SAVE / MAIN MENU", "SAVE / QUIT"][..],
        ),
        (
            Screen::ConfirmReplace(GameMode::Survival),
            &["REPLACE WORLD", "CANCEL"][..],
        ),
    ] {
        let vertices = build_interface(
            640,
            400,
            &view(),
            &UiState {
                screen,
                mode: GameMode::Survival,
                inventory: None,
                storage: None,
                interaction_notice: None,
                cursor: None,
                focused_button: 0,
                can_continue: true,
                save_notice: None,
            },
        );
        for (index, label) in labels.iter().enumerate() {
            let rect = layout.button(screen, index).unwrap();
            let mut painter = Painter {
                width: 640.0,
                height: 400.0,
                vertices: Vec::new(),
            };
            painter.centered_text(
                rect.center()[0],
                rect.y + 10.0 * layout.scale,
                label,
                2.0 * layout.scale,
                TEXT,
            );
            assert!(vertices.windows(painter.vertices.len()).any(|actual| {
                actual
                    .iter()
                    .zip(&painter.vertices)
                    .all(|(a, b)| a.position == b.position && a.color == b.color)
            }));
        }
    }
}

#[test]
fn loading_modes_are_distinct_without_faking_completion_progress() {
    let mut renders = Vec::new();
    for mode in [GameMode::Creative, GameMode::Survival] {
        renders.push(build_interface(
            640,
            400,
            &view(),
            &UiState {
                screen: Screen::Loading,
                mode,
                inventory: None,
                storage: None,
                interaction_notice: None,
                cursor: None,
                focused_button: 0,
                can_continue: false,
                save_notice: None,
            },
        ));
    }
    assert_ne!(
        bytemuck::cast_slice::<_, u8>(&renders[0]),
        bytemuck::cast_slice::<_, u8>(&renders[1])
    );
}

#[test]
fn menu_and_inventory_instructions_use_only_original_font_glyphs() {
    for text in [
        "CONTINUE NEW SURVIVAL NEW CREATIVE QUIT RESUME MAIN MENU REPLACE WORLD CANCEL",
        "A WORLD OF YOUR OWN ONE LOCAL WORLD - SAVED AUTOMATICALLY",
        "UP/DOWN OR TAB SELECT ENTER OR CLICK",
        "THIS REPLACES YOUR ONLY LOCAL SAVE.",
        "YOUR PREVIOUS BUILD CANNOT BE CONTINUED.",
        "SAVE / MAIN MENU SAVE / QUIT WORLD SAVED LOCALLY",
        "SAVE FAILED - CHECK STORAGE AND RETRY.",
        "CLICK: PICK UP / PLACE / SWAP WHOLE STACKS",
        "RIGHT CLICK: HALF STACK / PLACE ONE",
        "I / ESC CLOSE - WORLD PAUSED",
    ] {
        assert!(
            text.chars()
                .all(|character| character == ' ' || glyph(character) != glyph('?'))
        );
    }
}

#[test]
fn unavailable_continue_is_muted_and_never_highlighted_by_mouse_or_keyboard() {
    let layout = UiLayout::new(640, 400).unwrap();
    let rect = layout.button(Screen::MainMenu, 0).unwrap();
    for cursor in [None, Some(rect.center())] {
        let ui = UiState {
            screen: Screen::MainMenu,
            mode: GameMode::Survival,
            inventory: None,
            storage: None,
            interaction_notice: None,
            cursor,
            focused_button: 0,
            can_continue: false,
            save_notice: None,
        };
        assert_eq!(super::menus::highlighted_button(&layout, &ui), None);
        let actual = build_interface(640, 400, &view(), &ui);
        let mut expected = Painter {
            width: 640.0,
            height: 400.0,
            vertices: Vec::new(),
        };
        expected.centered_text(rect.center()[0], rect.y + 10.0, "CONTINUE", 2.0, MUTED);
        assert!(actual.windows(expected.vertices.len()).any(|vertices| {
            vertices
                .iter()
                .zip(&expected.vertices)
                .all(|(a, b)| a.position == b.position && a.color == b.color)
        }));
        assert!(
            !actual
                .iter()
                .any(|vertex| vertex.color == super::menus::BUTTON_FOCUS)
        );
    }
}

#[test]
fn persistence_notices_are_distinct_visible_finite_and_bounded_on_each_menu() {
    for screen in [
        Screen::MainMenu,
        Screen::Paused,
        Screen::ConfirmReplace(GameMode::Creative),
    ] {
        for (width, height) in [(640, 400), (1280, 720), (1920, 1080)] {
            let ui = UiState {
                screen,
                mode: GameMode::Creative,
                inventory: None,
                storage: None,
                interaction_notice: None,
                cursor: None,
                focused_button: 1,
                can_continue: true,
                save_notice: None,
            };
            let plain = build_interface(width, height, &view(), &ui);
            let mut previous = Vec::new();
            for notice in [
                SaveNotice::Saved,
                SaveNotice::SaveFailed,
                SaveNotice::LoadFailed,
                SaveNotice::Incompatible,
                SaveNotice::Unavailable,
            ] {
                let vertices = build_interface(
                    width,
                    height,
                    &view(),
                    &UiState {
                        save_notice: Some(notice),
                        ..ui
                    },
                );
                assert!(vertices.len() > plain.len());
                assert!(vertices.len() < 40_000);
                assert!(vertices.iter().all(|vertex| {
                    vertex
                        .position
                        .iter()
                        .all(|value| value.is_finite() && (-1.0..=1.0).contains(value))
                }));
                let bytes = bytemuck::cast_slice::<_, u8>(&vertices).to_vec();
                assert!(!previous.contains(&bytes));
                previous.push(bytes);
            }
        }
    }
}
