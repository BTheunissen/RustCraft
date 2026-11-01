use super::*;
use crate::game::{Game, StorageSlot};
use crate::options::LaunchOptions;
use crate::world::Block;

#[test]
fn actual_right_click_open_pauses_and_ui_transfer_focus_close_is_lossless() {
    let mut app = App::new(LaunchOptions::default());
    app.game = crate::game::test_support::arena();
    app.game.world.edit_block([8, 2, 6], Block::Stone);
    app.game.inventory_mut().insert(Block::StorageCrate, 1);
    assert!(app.game.place_selected());
    app.game.inventory_mut().insert(Block::Wood, 7);
    app.screen = Screen::Playing;
    app.captured = true;
    app.focused = true;
    app.input.mouse_button(MouseButton::Right, true);
    app.simulate(0.01);
    assert_eq!(app.screen, Screen::Storage);
    assert!(!app.captured && !app.input.take_place_press());
    app.right_click_hit(Some(UiHit::InventorySlot(0)));
    assert_eq!(app.game.storage_view().unwrap().cursor.unwrap().count, 4);
    app.click_hit(Some(UiHit::CrateSlot(0)));
    assert_eq!(app.game.storage_view().unwrap().slots[0].unwrap().count, 4);
    app.game.storage_click(StorageSlot::Crate(0), true);
    app.focused = false;
    app.release_pointer();
    assert!(app.game.storage_view().unwrap().cursor.is_none());
    app.escape();
    assert_eq!(app.screen, Screen::Paused);
    assert!(app.game.storage_view().is_none());
    assert_eq!(app.game.snapshot().crates[0].slots[0].unwrap().count, 4);
    assert!(!app.captured);
    assert!(Game::from_snapshot(app.game.snapshot()).is_ok());
}
