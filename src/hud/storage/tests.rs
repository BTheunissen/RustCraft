use super::*;
use crate::game::{CRATE_SLOTS, GameMode, Inventory, StorageView};
use crate::hud::inventory_tests::view;
use crate::hud::{build_interface, hit_test};

#[test]
fn both_inventory_grids_share_distinct_resized_hit_and_draw_regions() {
    let slots = [None; CRATE_SLOTS];
    let inventory = Inventory::default();
    for (width, height) in [(640, 400), (1920, 1080), (160, 96)] {
        let layout = UiLayout::new(width, height).unwrap();
        for index in 0..CRATE_SLOTS {
            assert_eq!(
                hit_test(
                    width,
                    height,
                    Screen::Storage,
                    crate_slot(&layout, index).unwrap().center()
                ),
                Some(UiHit::CrateSlot(index))
            );
        }
        for index in 0..36 {
            assert_eq!(
                hit_test(
                    width,
                    height,
                    Screen::Storage,
                    player_slot(&layout, index).unwrap().center()
                ),
                Some(UiHit::InventorySlot(index))
            );
        }
        assert!(crate_slot(&layout, CRATE_SLOTS).is_none() && player_slot(&layout, 36).is_none());
        let ui = UiState {
            screen: Screen::Storage,
            mode: GameMode::Survival,
            inventory: Some(&inventory),
            storage: Some(StorageView {
                position: [-1, -30, -17],
                slots: &slots,
                cursor: None,
            }),
            interaction_notice: None,
            cursor: None,
            focused_button: 0,
            can_continue: false,
            save_notice: None,
        };
        let vertices = build_interface(width, height, &view(), &ui);
        assert!(!vertices.is_empty() && vertices.len() < 160_000);
        assert!(vertices.iter().all(|vertex| {
            vertex
                .position
                .into_iter()
                .all(|value| value.is_finite() && (-1.0..=1.0).contains(&value))
        }));
    }
}
