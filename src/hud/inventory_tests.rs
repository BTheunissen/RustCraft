use crate::game::{GameMode, Inventory, block_name};
use crate::world::Block;

use super::interface_layout::UiLayout;
use super::inventory::{HOVER, SELECTION, draw_stack};
use super::{HudInfo, HudVertex, Painter, Screen, TEXT, UiState, build_interface};

pub(super) fn view() -> HudInfo<'static> {
    HudInfo {
        captured: true,
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

fn ui(screen: Screen, inventory: &Inventory, cursor: Option<[f32; 2]>) -> UiState<'_> {
    UiState {
        screen,
        mode: GameMode::Survival,
        inventory: Some(inventory),
        storage: None,
        interaction_notice: None,
        cursor,
        focused_button: 0,
        can_continue: false,
        save_notice: None,
    }
}

fn geometry_contains(actual: &[HudVertex], expected: &[HudVertex]) -> bool {
    !expected.is_empty()
        && actual.windows(expected.len()).any(|vertices| {
            vertices
                .iter()
                .zip(expected)
                .all(|(a, b)| a.position == b.position && a.color == b.color)
        })
}

pub(super) fn name_geometry(width: u32, height: u32, text: &str) -> Vec<HudVertex> {
    let layout = UiLayout::new(width, height).unwrap();
    let mut painter = Painter {
        width: width as f32,
        height: height as f32,
        vertices: Vec::new(),
    };
    painter.centered_text(
        layout.width * 0.5,
        layout.height - 102.0 * layout.scale,
        text,
        layout.scale,
        TEXT,
    );
    painter.vertices
}

#[test]
fn selected_slot_border_and_block_name_follow_inventory_without_mutating_it() {
    let mut inventory = Inventory::default();
    for block in [
        Block::Stone,
        Block::Wood,
        Block::Sand,
        Block::Clay,
        Block::Gravel,
        Block::Crystal,
        Block::Moss,
        Block::CoalOre,
        Block::Limestone,
    ] {
        assert_eq!(inventory.insert(block, 64), 0);
    }
    for (width, height) in [(640, 400), (1280, 800)] {
        let layout = UiLayout::new(width, height).unwrap();
        for selected in 0..9 {
            inventory.select(selected);
            let snapshot = inventory.clone();
            for screen in [Screen::Playing, Screen::Inventory] {
                let vertices =
                    build_interface(width, height, &view(), &ui(screen, &inventory, None));
                let rect = if screen == Screen::Playing {
                    layout.hotbar_slot(selected)
                } else {
                    layout.inventory_slot(selected)
                }
                .unwrap();
                let border: Vec<_> = vertices
                    .iter()
                    .filter(|vertex| vertex.color == SELECTION)
                    .collect();
                assert_eq!(border.len(), 24);
                let x = (border[0].position[0] + 1.0) * width as f32 * 0.5;
                let y = (1.0 - border[0].position[1]) * height as f32 * 0.5;
                assert!((x - rect.x).abs() < 0.001 && (y - rect.y).abs() < 0.001);
                if screen == Screen::Playing {
                    let block = inventory.selected_stack().unwrap().item.block().unwrap();
                    assert!(geometry_contains(
                        &vertices,
                        &name_geometry(width, height, block_name(block))
                    ));
                }
            }
            assert_eq!(inventory, snapshot);
        }
    }
}

#[test]
fn empty_inventory_and_missing_inventory_paint_the_same_empty_hand() {
    let inventory = Inventory::default();
    for screen in [Screen::Playing, Screen::Inventory] {
        let with_inventory = build_interface(640, 400, &view(), &ui(screen, &inventory, None));
        let missing = build_interface(
            640,
            400,
            &view(),
            &UiState {
                inventory: None,
                ..ui(screen, &inventory, None)
            },
        );
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&with_inventory),
            bytemuck::cast_slice::<_, u8>(&missing)
        );
        if screen == Screen::Playing {
            assert!(geometry_contains(
                &with_inventory,
                &name_geometry(640, 400, "EMPTY HAND")
            ));
        }
    }
}

#[test]
fn storage_does_not_leak_into_the_playing_hotbar() {
    let mut inventory = Inventory::default();
    assert_eq!(inventory.insert(Block::Stone, 9 * 64), 0);
    let hotbar_before = build_interface(640, 400, &view(), &ui(Screen::Playing, &inventory, None));
    let storage_before =
        build_interface(640, 400, &view(), &ui(Screen::Inventory, &inventory, None));
    assert_eq!(inventory.insert(Block::Crystal, 64), 0);
    assert_eq!(inventory.slots()[9].unwrap().item, Block::Crystal.into());
    let hotbar_after = build_interface(640, 400, &view(), &ui(Screen::Playing, &inventory, None));
    let storage_after =
        build_interface(640, 400, &view(), &ui(Screen::Inventory, &inventory, None));
    assert_eq!(
        bytemuck::cast_slice::<_, u8>(&hotbar_before),
        bytemuck::cast_slice::<_, u8>(&hotbar_after)
    );
    assert!(storage_after.len() > storage_before.len());
    let layout = UiLayout::new(640, 400).unwrap();
    let mut painter = Painter {
        width: 640.0,
        height: 400.0,
        vertices: Vec::new(),
    };
    draw_stack(
        &mut painter,
        layout.inventory_slot(9).unwrap(),
        &inventory.slots()[9].unwrap(),
    );
    assert!(geometry_contains(&storage_after, &painter.vertices));
}

#[test]
fn inventory_hover_uses_hit_slot_and_retains_the_hotbar_selection() {
    let mut inventory = Inventory::default();
    inventory.select(4);
    let layout = UiLayout::new(1280, 800).unwrap();
    for index in [0, 4, 8, 9, 35] {
        let rect = layout.inventory_slot(index).unwrap();
        let vertices = build_interface(
            1280,
            800,
            &view(),
            &ui(Screen::Inventory, &inventory, Some(rect.center())),
        );
        assert_eq!(
            vertices
                .iter()
                .filter(|vertex| vertex.color == SELECTION)
                .count(),
            24
        );
        let hover: Vec<_> = vertices
            .iter()
            .filter(|vertex| vertex.color == HOVER)
            .collect();
        if index == 4 {
            assert!(hover.is_empty());
        } else {
            assert_eq!(hover.len(), 24);
            let x = (hover[0].position[0] + 1.0) * 1280.0 * 0.5;
            let y = (1.0 - hover[0].position[1]) * 800.0 * 0.5;
            assert!((x - rect.x).abs() < 0.001 && (y - rect.y).abs() < 0.001);
        }
    }
    assert_eq!(inventory.selected(), 4);
}

#[test]
fn fully_populated_inventory_with_a_held_stack_stays_bounded_and_read_only() {
    let mut inventory = Inventory::default();
    let collectible: Vec<_> = Block::ALL
        .into_iter()
        .filter(|block| block.is_solid() && *block != Block::Bedrock)
        .collect();
    for index in 0..36 {
        assert_eq!(
            inventory.insert(collectible[index % collectible.len()], 64),
            0
        );
    }
    inventory.select(8);
    inventory.click_slot(0);
    let snapshot = inventory.clone();
    assert!(inventory.cursor_stack().is_some());
    assert!(inventory.slots()[0].is_none());
    for (width, height) in [(160, 96), (640, 400), (1280, 800), (3840, 2160)] {
        let layout = UiLayout::new(width, height).unwrap();
        for cursor in [
            None,
            Some([0.0, 0.0]),
            Some([width as f32, height as f32]),
            Some([f32::NAN, 0.0]),
        ] {
            let vertices = build_interface(
                width,
                height,
                &view(),
                &ui(Screen::Inventory, &inventory, cursor),
            );
            assert!(vertices.len() < 120_000);
            assert!(vertices.len().is_multiple_of(6));
            assert!(vertices.iter().all(|vertex| {
                vertex
                    .position
                    .into_iter()
                    .all(|value| value.is_finite() && (-1.0..=1.0).contains(&value))
            }));
            let held_panels = vertices
                .iter()
                .filter(|vertex| vertex.color == [0.014, 0.023, 0.029, 0.84])
                .count();
            if let Some(rect) = layout.held_stack(cursor) {
                assert_eq!(held_panels, 6);
                let mut painter = Painter {
                    width: width as f32,
                    height: height as f32,
                    vertices: Vec::new(),
                };
                draw_stack(&mut painter, rect, &inventory.cursor_stack().unwrap());
                assert!(geometry_contains(&vertices, &painter.vertices));
            } else {
                assert_eq!(held_panels, 0);
            }
        }
    }
    assert_eq!(inventory, snapshot);
}
