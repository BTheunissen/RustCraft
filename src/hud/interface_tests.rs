use crate::game::{GameMode, ItemStack};
use crate::world::Block;

use super::interface_layout::{Rect, UiLayout};
use super::inventory::{SELECTION, draw_stack, stack_rects, survival_controls};
use super::menus::{BUTTON_FOCUS, highlighted_button};
use super::{
    CROSSHAIR, HudInfo, HudVertex, Painter, Screen, UiHit, UiState, build_hud, build_interface,
    button_count, hit_test,
};

const SCREENS: [Screen; 8] = [
    Screen::MainMenu,
    Screen::Loading,
    Screen::Playing,
    Screen::Inventory,
    Screen::Crafting,
    Screen::Storage,
    Screen::Paused,
    Screen::ConfirmReplace(GameMode::Survival),
];

fn info(captured: bool) -> HudInfo<'static> {
    HudInfo {
        captured,
        flying: false,
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

fn state(screen: Screen, mode: GameMode) -> UiState<'static> {
    UiState {
        screen,
        mode,
        inventory: None,
        storage: None,
        interaction_notice: None,
        cursor: None,
        focused_button: 0,
        can_continue: true,
        save_notice: None,
    }
}

fn assert_valid(vertices: &[HudVertex]) {
    assert!(vertices.len().is_multiple_of(6));
    for vertex in vertices {
        assert!(
            vertex
                .position
                .into_iter()
                .all(|value| value.is_finite() && (-1.0..=1.0).contains(&value))
        );
        assert!(
            vertex
                .color
                .into_iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
        );
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

fn quad_bounds(quad: &[HudVertex], width: f32, height: f32) -> Rect {
    let xs: Vec<_> = quad
        .iter()
        .map(|vertex| (vertex.position[0] + 1.0) * 0.5 * width)
        .collect();
    let ys: Vec<_> = quad
        .iter()
        .map(|vertex| (1.0 - vertex.position[1]) * 0.5 * height)
        .collect();
    let x = xs.iter().copied().fold(f32::INFINITY, f32::min);
    let y = ys.iter().copied().fold(f32::INFINITY, f32::min);
    Rect {
        x,
        y,
        width: xs.into_iter().fold(f32::NEG_INFINITY, f32::max) - x,
        height: ys.into_iter().fold(f32::NEG_INFINITY, f32::max) - y,
    }
}

#[test]
fn explicit_screens_are_safe_for_empty_compact_retina_and_extreme_viewports() {
    for (width, height) in [(0, 0), (0, 720), (1280, 0), (159, 96), (160, 95)] {
        for screen in SCREENS {
            assert!(
                build_interface(
                    width,
                    height,
                    &info(false),
                    &state(screen, GameMode::Survival)
                )
                .is_empty()
            );
            assert_eq!(hit_test(width, height, screen, [0.0, 0.0]), None);
        }
    }
    for (width, height) in [
        (160, 96),
        (320, 200),
        (640, 400),
        (1200, 760),
        (1280, 800),
        (1920, 1080),
        (3840, 2160),
        (u32::MAX, u32::MAX),
    ] {
        for screen in SCREENS {
            for mode in [GameMode::Creative, GameMode::Survival] {
                let vertices = build_interface(width, height, &info(false), &state(screen, mode));
                assert!(!vertices.is_empty(), "{screen:?} {width}x{height}");
                assert_valid(&vertices);
                assert!(vertices.len() < 120_000, "{screen:?}: {}", vertices.len());
            }
        }
    }
}

#[test]
fn creative_playing_is_identical_to_the_existing_captured_hud() {
    for (width, height) in [(160, 96), (640, 400), (1280, 720), (1920, 1080)] {
        for swimming in [false, true] {
            for flying in [false, true] {
                let view = HudInfo {
                    swimming,
                    flying,
                    underground: true,
                    phasing: true,
                    ..info(true)
                };
                let expected = build_hud(width, height, &view);
                for captured in [false, true] {
                    let actual = build_interface(
                        width,
                        height,
                        &HudInfo { captured, ..view },
                        &state(Screen::Playing, GameMode::Creative),
                    );
                    assert_eq!(
                        bytemuck::cast_slice::<_, u8>(&expected),
                        bytemuck::cast_slice::<_, u8>(&actual)
                    );
                }
            }
        }
    }
}

#[test]
fn new_screens_have_only_their_intended_interactions_and_playing_crosshair() {
    for screen in SCREENS {
        let vertices = build_interface(640, 400, &info(false), &state(screen, GameMode::Survival));
        assert_eq!(
            vertices
                .iter()
                .filter(|vertex| vertex.color == CROSSHAIR)
                .count(),
            if screen == Screen::Playing { 24 } else { 0 }
        );
        if matches!(screen, Screen::Loading | Screen::Playing) {
            let legacy = build_hud(640, 400, &info(false));
            assert_ne!(
                bytemuck::cast_slice::<_, u8>(&vertices),
                bytemuck::cast_slice::<_, u8>(&legacy)
            );
            for cursor in [[320.0, 200.0], [320.0, 300.0], [0.0, 0.0]] {
                assert_eq!(hit_test(640, 400, screen, cursor), None);
            }
        }
    }
    assert_eq!(button_count(Screen::MainMenu), 4);
    assert_eq!(button_count(Screen::Paused), 4);
    assert_eq!(button_count(Screen::ConfirmReplace(GameMode::Survival)), 2);
    for screen in [Screen::Loading, Screen::Playing, Screen::Inventory] {
        assert_eq!(button_count(screen), 0);
    }
}

#[test]
fn menu_buttons_draw_and_hit_the_same_nonoverlapping_physical_rectangles() {
    for (width, height, scale) in [
        (640, 400, 1.0),
        (1280, 800, 2.0),
        (1920, 1200, 3.0),
        (2560, 1600, 4.0),
        (1200, 760, 2.0),
        (3840, 2160, 6.0),
    ] {
        let layout = UiLayout::new(width, height).unwrap();
        assert_eq!(layout.scale, scale);
        for screen in [
            Screen::MainMenu,
            Screen::Paused,
            Screen::ConfirmReplace(GameMode::Survival),
        ] {
            let mut previous = None;
            for index in 0..button_count(screen) {
                let rect = layout.button(screen, index).unwrap();
                assert!(rect.x >= 0.0 && rect.y >= 0.0);
                assert!(
                    rect.x + rect.width <= width as f32 && rect.y + rect.height <= height as f32
                );
                assert_eq!(
                    hit_test(width, height, screen, rect.center()),
                    Some(UiHit::Button(index))
                );
                assert_eq!(
                    hit_test(width, height, screen, [rect.x, rect.y]),
                    Some(UiHit::Button(index))
                );
                assert_eq!(
                    hit_test(width, height, screen, [rect.x + rect.width, rect.y]),
                    None
                );
                assert_eq!(
                    hit_test(width, height, screen, [rect.x, rect.y + rect.height]),
                    None
                );
                if let Some(previous) = previous {
                    assert!(!overlaps(previous, rect));
                }
                previous = Some(rect);
                let ui = UiState {
                    focused_button: index,
                    ..state(screen, GameMode::Survival)
                };
                let vertices = build_interface(width, height, &info(false), &ui);
                let focus: Vec<_> = vertices
                    .iter()
                    .copied()
                    .filter(|vertex| vertex.color == BUTTON_FOCUS)
                    .collect();
                assert_eq!(focus.len(), 24);
                let focus_top = quad_bounds(&focus[..6], width as f32, height as f32);
                assert!((focus_top.x - rect.x).abs() < 0.01);
                assert!((focus_top.y - rect.y).abs() < 0.01);
                assert!((focus_top.width - rect.width).abs() < 0.01);
            }
            assert!(layout.button(screen, button_count(screen)).is_none());
        }
    }
}

#[test]
fn hover_overrides_keyboard_focus_but_gaps_and_invalid_cursors_do_not() {
    let layout = UiLayout::new(1280, 800).unwrap();
    for screen in [
        Screen::MainMenu,
        Screen::Paused,
        Screen::ConfirmReplace(GameMode::Survival),
    ] {
        let mut ui = state(screen, GameMode::Survival);
        ui.focused_button = button_count(screen) - 1;
        assert_eq!(highlighted_button(&layout, &ui), Some(ui.focused_button));
        ui.cursor = Some(layout.button(screen, 0).unwrap().center());
        assert_eq!(highlighted_button(&layout, &ui), Some(0));
        let first = layout.button(screen, 0).unwrap();
        ui.cursor = Some([first.center()[0], first.y + first.height + layout.scale]);
        assert_eq!(highlighted_button(&layout, &ui), Some(ui.focused_button));
        ui.cursor = Some([f32::NAN, 0.0]);
        assert_eq!(highlighted_button(&layout, &ui), Some(ui.focused_button));
        ui.focused_button = usize::MAX;
        assert_eq!(highlighted_button(&layout, &ui), None);
    }
    for screen in SCREENS {
        for cursor in [
            [f32::NAN, 200.0],
            [200.0, f32::INFINITY],
            [f32::NEG_INFINITY, 0.0],
            [-1.0, 0.0],
            [1280.0, 400.0],
            [640.0, 800.0],
        ] {
            assert_eq!(hit_test(1280, 800, screen, cursor), None);
        }
    }
}

#[test]
fn storage_then_hotbar_maps_to_all_thirty_six_slots_without_gap_hits() {
    for (width, height) in [(160, 96), (640, 400), (1280, 800), (1920, 1080)] {
        let layout = UiLayout::new(width, height).unwrap();
        let mut rects = Vec::new();
        for index in 0..36 {
            let rect = layout.inventory_slot(index).unwrap();
            assert_eq!(
                hit_test(width, height, Screen::Inventory, rect.center()),
                Some(UiHit::InventorySlot(index))
            );
            assert_eq!(
                hit_test(width, height, Screen::Inventory, [rect.x, rect.y]),
                Some(UiHit::InventorySlot(index))
            );
            assert_eq!(
                hit_test(
                    width,
                    height,
                    Screen::Inventory,
                    [rect.x + rect.width + layout.scale, rect.center()[1]]
                ),
                None
            );
            for previous in &rects {
                assert!(!overlaps(*previous, rect));
            }
            rects.push(rect);
        }
        for index in 9..36 {
            assert!(rects[index].y < rects[0].y);
            assert!((rects[index].x - rects[(index - 9) % 9].x).abs() < 0.01);
        }
        assert!(layout.inventory_slot(36).is_none());
        assert!(layout.inventory_slot(usize::MAX).is_none());
        assert!(layout.hotbar_slot(9).is_none());
        let gap = [
            rects[9].center()[0],
            rects[9].y + rects[9].height + layout.scale,
        ];
        assert_eq!(hit_test(width, height, Screen::Inventory, gap), None);
        let panel = layout.inventory_panel();
        assert_eq!(
            hit_test(
                width,
                height,
                Screen::Inventory,
                [panel.center()[0], panel.y + 30.0 * layout.scale]
            ),
            None
        );
    }
}

#[test]
fn hotbar_and_counts_are_readable_separated_and_selected_at_minimum_size() {
    for (width, height) in [(640, 400), (1280, 800), (1920, 1080)] {
        let layout = UiLayout::new(width, height).unwrap();
        let mut previous = None;
        for index in 0..9 {
            let rect = layout.hotbar_slot(index).unwrap();
            assert!(rect.y + rect.height < height as f32 - 36.0 * layout.scale);
            if let Some(previous) = previous {
                assert!(!overlaps(previous, rect));
            }
            previous = Some(rect);
            for count in [1, 9, 10, 64, 999, u16::MAX] {
                let (icon, label) = stack_rects(rect, count);
                assert!(!overlaps(icon, label));
                for area in [icon, label] {
                    assert!(area.x >= rect.x + 2.0 * layout.scale);
                    assert!(area.y >= rect.y + 2.0 * layout.scale);
                    assert!(
                        area.x + area.width <= rect.x + rect.width - 2.0 * layout.scale + 0.001
                    );
                    assert!(
                        area.y + area.height <= rect.y + rect.height - 2.0 * layout.scale + 0.001
                    );
                }
            }
        }
        for screen in [Screen::Playing, Screen::Inventory] {
            let vertices = build_interface(
                width,
                height,
                &info(true),
                &state(screen, GameMode::Survival),
            );
            assert_eq!(
                vertices
                    .iter()
                    .filter(|vertex| vertex.color == SELECTION)
                    .count(),
                24
            );
        }
    }
    for swimming in [false, true] {
        let lines = survival_controls(swimming);
        assert!(
            !lines
                .iter()
                .any(|text| text.contains("F FLY") || text.contains("CTRL"))
        );
        assert!(
            lines[0].contains("HOLD LEFT MINE")
                && lines[0].contains("RIGHT PLACE")
                && lines[0].contains("I INVENTORY")
        );
        assert!(
            lines[1].contains("1-9/WHEEL")
                && lines[1].contains("H LAMP")
                && lines[1].contains("ESC PAUSE")
        );
    }
}

#[test]
fn populated_icon_count_geometry_stays_inside_cells_and_has_a_fixed_budget() {
    let layout = UiLayout::new(640, 400).unwrap();
    let mut painter = Painter {
        width: 640.0,
        height: 400.0,
        vertices: Vec::new(),
    };
    for index in 0..36 {
        let rect = layout.inventory_slot(index).unwrap();
        let start = painter.vertices.len();
        let stack = ItemStack {
            item: Block::ALL[index % (Block::COUNT - 1) + 1].into(),
            count: u16::MAX,
        };
        draw_stack(&mut painter, rect, &stack);
        let vertices = &painter.vertices[start..];
        assert!(!vertices.is_empty());
        assert!(vertices.len() < 4000);
        for quad in vertices.as_chunks::<6>().0 {
            let bounds = quad_bounds(quad, 640.0, 400.0);
            assert!(bounds.x >= rect.x - 0.001 && bounds.y >= rect.y - 0.001);
            assert!(bounds.x + bounds.width <= rect.x + rect.width + 0.001);
            assert!(bounds.y + bounds.height <= rect.y + rect.height + 0.001);
        }
    }
    assert!(painter.vertices.len() < 120_000);
    assert_valid(&painter.vertices);
    for stack in [
        ItemStack {
            item: Block::Air.into(),
            count: 1,
        },
        ItemStack {
            item: Block::Stone.into(),
            count: 0,
        },
    ] {
        let before = painter.vertices.len();
        draw_stack(&mut painter, layout.inventory_slot(0).unwrap(), &stack);
        assert_eq!(painter.vertices.len(), before);
    }
}

#[test]
fn cursor_held_stacks_are_clamped_at_edges_and_skip_nonfinite_positions() {
    for (width, height) in [(160, 96), (640, 400), (1280, 800)] {
        let layout = UiLayout::new(width, height).unwrap();
        for cursor in [
            None,
            Some([-20.0, -30.0]),
            Some([0.0, 0.0]),
            Some([width as f32, height as f32]),
            Some([f32::MAX, -f32::MAX]),
        ] {
            let rect = layout.held_stack(cursor).unwrap();
            assert!(rect.x >= 0.0 && rect.y >= 0.0);
            assert!(rect.x + rect.width <= width as f32 && rect.y + rect.height <= height as f32);
            let mut painter = Painter {
                width: width as f32,
                height: height as f32,
                vertices: Vec::new(),
            };
            draw_stack(
                &mut painter,
                rect,
                &ItemStack {
                    item: Block::Wood.into(),
                    count: 64,
                },
            );
            assert_valid(&painter.vertices);
        }
        assert!(layout.held_stack(Some([f32::NAN, 0.0])).is_none());
        assert!(layout.held_stack(Some([0.0, f32::INFINITY])).is_none());
    }
}

#[test]
fn hostile_game_readouts_do_not_expand_menus_or_survival_geometry() {
    let biome = "W".repeat(100_000);
    let view = HudInfo {
        biome: &biome,
        position: [f32::NAN, f32::MAX, f32::NEG_INFINITY],
        fps: f32::NAN,
        loaded_chunks: usize::MAX,
        pending_chunks: usize::MAX,
        swimming: true,
        underground: true,
        ..info(false)
    };
    for screen in SCREENS {
        let ui = UiState {
            cursor: Some([f32::NAN, f32::INFINITY]),
            focused_button: usize::MAX,
            ..state(screen, GameMode::Survival)
        };
        let vertices = build_interface(640, 400, &view, &ui);
        assert_valid(&vertices);
        assert!(vertices.len() < 40_000);
    }
}
