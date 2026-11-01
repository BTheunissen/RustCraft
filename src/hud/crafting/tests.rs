use super::*;
use crate::game::{GameMode, Inventory};
use crate::hud::inventory_tests::view;
use crate::hud::{Screen, UiHit, build_interface, hit_test};

#[test]
fn recipes_and_back_use_the_same_nonoverlapping_hit_and_draw_rectangles() {
    for (width, height) in [(640, 400), (1920, 1080), (160, 96)] {
        let layout = UiLayout::new(width, height).unwrap();
        let mut inventory = Inventory::default();
        inventory.insert(crate::world::Block::Wood, 1);
        let toggle = toggle(&layout);
        assert_eq!(
            hit_test(width, height, Screen::Inventory, toggle.center()),
            Some(UiHit::Crafting)
        );
        for index in 0..RECIPES.len() {
            let rect = recipe(&layout, index).unwrap();
            assert_eq!(
                hit_test(width, height, Screen::Crafting, rect.center()),
                Some(UiHit::Recipe(index))
            );
            let back = back(&layout);
            assert!(rect.y + rect.height <= back.y);
        }
        assert_eq!(
            hit_test(width, height, Screen::Crafting, back(&layout).center()),
            Some(UiHit::CraftBack)
        );
        for gain in [None, Some(&inventory)] {
            let ui = UiState {
                screen: Screen::Crafting,
                mode: GameMode::Survival,
                inventory: gain,
                storage: None,
                interaction_notice: None,
                cursor: None,
                focused_button: 0,
                can_continue: false,
                save_notice: None,
            };
            let vertices = build_interface(width, height, &view(), &ui);
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
