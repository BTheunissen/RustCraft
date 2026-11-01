use super::*;
use crate::options::LaunchOptions;

#[test]
fn split_inventory_ui_is_paused_and_never_becomes_world_placement_or_capture() {
    let mut app = App::new(LaunchOptions::default());
    app.game = crate::game::Game::with_mode(42, GameMode::Survival);
    app.screen = Screen::Inventory;
    app.game
        .inventory_mut()
        .insert(crate::world::Block::Wood, 7);
    app.pointer_authorized = true;
    app.right_click_hit(Some(UiHit::InventorySlot(0)));
    assert_eq!(app.game.inventory().cursor_stack().unwrap().count, 4);
    assert_eq!(app.game.inventory().slots()[0].unwrap().count, 3);
    assert!(!app.pointer_authorized && !app.input.take_place_press());
    app.right_click_hit(Some(UiHit::InventorySlot(35)));
    assert_eq!(app.game.inventory().slots()[35].unwrap().count, 1);
    app.escape();
    assert!(app.game.inventory().cursor_stack().is_none());
    assert_eq!(app.game.inventory().slots()[0].unwrap().count, 6);
    assert_eq!(app.screen, Screen::Paused);
    assert!(!app.captured);
    let before = app.game.snapshot();
    app.right_click_hit(Some(UiHit::InventorySlot(0)));
    app.right_click_hit(Some(UiHit::Button(0)));
    assert_eq!(app.game.snapshot(), before);
}
