use super::*;
use crate::options::LaunchOptions;
use crate::save::test_support::TestDirectory;
use crate::world::Block;

fn inventory() -> App {
    let mut app = App::new(LaunchOptions::default());
    app.game = crate::game::Game::with_mode(42, GameMode::Survival);
    app.screen = Screen::Inventory;
    app.focused = true;
    app
}

#[test]
fn crafting_is_inventory_only_paused_and_clicks_cannot_edit_the_world() {
    let mut app = inventory();
    app.game.inventory_mut().insert(Block::Wood, 1);
    app.input.mouse_button(MouseButton::Right, true);
    let world = app.game.snapshot().edits;
    app.open_crafting();
    assert_eq!(app.screen, Screen::Crafting);
    assert!(!app.input.take_place_press() && !app.pointer_authorized);
    let index = app.click_hit(Some(UiHit::Recipe(0))).unwrap();
    assert!(!app.apply_button(index).unwrap());
    assert_eq!(
        app.game.inventory().slots()[0].unwrap().item,
        Block::Planks.into()
    );
    assert_eq!(app.game.inventory().slots()[0].unwrap().count, 4);
    app.simulate(0.25);
    assert_eq!(app.game.snapshot().edits, world);
    app.escape();
    assert_eq!(app.screen, Screen::Inventory);
    app.toggle_inventory();
    assert_eq!(app.screen, Screen::Paused);
    assert!(!app.captured);
    app.open_crafting();
    assert_eq!(app.screen, Screen::Paused);
}

#[test]
fn held_split_cursor_survives_failed_craft_focus_loss_save_and_close() {
    let mut app = inventory();
    let directory = TestDirectory::new();
    app.persistence = crate::app::persistence::Persistence::with_slot(directory.slot());
    app.persistence.begin_world(std::time::Instant::now());
    app.game.inventory_mut().insert(Block::Wood, 3);
    app.game.inventory_mut().right_click_slot(0);
    let before = app.game.snapshot();
    app.open_crafting();
    app.apply_button(0).unwrap();
    assert_eq!(app.game.snapshot(), before);
    app.focused = false;
    app.release_pointer();
    app.autosave(std::time::Instant::now() + crate::app::persistence::AUTOSAVE_INTERVAL);
    assert_eq!(directory.slot().load().unwrap().unwrap(), before);
    app.toggle_inventory();
    assert!(app.game.inventory().cursor_stack().is_none());
    assert_eq!(app.game.inventory().slots()[0].unwrap().count, 3);
    assert!(!app.captured);
}

#[test]
fn hovered_crafting_rows_and_back_have_the_same_enter_activation_index() {
    for index in 0..crate::game::RECIPES.len() {
        assert_eq!(
            hovered_button(Screen::Crafting, UiHit::Recipe(index)),
            Some(index)
        );
    }
    assert_eq!(
        hovered_button(Screen::Crafting, UiHit::CraftBack),
        Some(crate::game::RECIPES.len())
    );
    assert_eq!(
        hovered_button(Screen::Crafting, UiHit::Recipe(usize::MAX)),
        None
    );
    assert_eq!(hovered_button(Screen::Playing, UiHit::Recipe(0)), None);
    assert_eq!(hovered_button(Screen::Storage, UiHit::CrateSlot(0)), None);
}
