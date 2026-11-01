use super::inventory_tests::{name_geometry, view};
use super::{Screen, UiState, build_interface};
use crate::game::{GameMode, Inventory, Tool};

#[test]
fn tools_show_original_distinct_icons_and_names_without_mutating_inventory() {
    let mut rendered = Vec::new();
    for tool in Tool::ALL {
        let mut inventory = Inventory::default();
        inventory.insert(tool, 1);
        let before = inventory.clone();
        let vertices = build_interface(
            640,
            400,
            &view(),
            &UiState {
                screen: Screen::Playing,
                mode: GameMode::Survival,
                inventory: Some(&inventory),
                storage: None,
                interaction_notice: None,
                cursor: None,
                focused_button: 0,
                can_continue: false,
                save_notice: None,
            },
        );
        assert_eq!(inventory, before);
        let name = name_geometry(640, 400, tool_name(tool));
        assert!(
            vertices
                .windows(name.len())
                .any(|window| bytemuck::cast_slice::<_, u8>(window)
                    == bytemuck::cast_slice::<_, u8>(&name))
        );
        rendered.push(bytemuck::cast_slice::<_, u8>(&vertices).to_vec());
    }
    assert_ne!(rendered[0], rendered[1]);
}

fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::StonePickaxe => "STONE PICKAXE",
        Tool::StoneAxe => "STONE AXE",
    }
}
