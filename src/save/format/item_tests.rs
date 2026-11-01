use super::*;
use crate::game::Game;

#[test]
fn tagged_tools_blocks_and_pickups_round_trip_without_reinterpreting_ids() {
    let mut game = Game::with_mode(42, GameMode::Survival);
    game.inventory_mut().insert(Tool::StonePickaxe, 1);
    game.inventory_mut().insert(Tool::StoneAxe, 1);
    game.inventory_mut().insert(Block::Grass, 3);
    game.inventory_mut().right_click_slot(2);
    let mut snapshot = game.snapshot();
    snapshot.pickups.push(SavedPickup {
        position: Vec3::new(-1.5, -20.0, -17.5),
        item: Tool::StoneAxe.into(),
        count: 1,
        velocity: Vec3::ZERO,
        age: 0.4,
    });
    let bytes = encode(&snapshot).unwrap();
    assert_eq!(decode(&bytes).unwrap(), snapshot);
    assert_eq!(&bytes[8..12], &4_u32.to_le_bytes());
    assert_eq!(&bytes[12..16], &2_u32.to_le_bytes());
    assert_eq!(&bytes[HEADER_BYTES + 33..HEADER_BYTES + 37], &[2, 0, 1, 0]);
    assert_eq!(&bytes[HEADER_BYTES + 41..HEADER_BYTES + 45], &[1, 1, 3, 0]);
}

#[test]
fn item_tags_unknown_tool_ids_counts_and_old_format_fail_closed() {
    let mut snapshot = Game::new(42).snapshot();
    snapshot.slots[0] = Some(ItemStack {
        item: Tool::StonePickaxe.into(),
        count: 1,
    });
    let bytes = encode(&snapshot).unwrap();
    for record in [
        [2, 0, 2, 0],
        [2, 2, 1, 0],
        [3, 0, 1, 0],
        [0, 1, 0, 0],
        [0, 0, 1, 0],
        [1, 0, 1, 0],
    ] {
        let mut bad = bytes.clone();
        bad[HEADER_BYTES + 33..HEADER_BYTES + 37].copy_from_slice(&record);
        let hash = checksum(&bad[HEADER_BYTES..]);
        bad[24..32].copy_from_slice(&hash.to_le_bytes());
        assert!(decode(&bad).is_err(), "record {record:?}");
    }
    let mut old = bytes;
    old[8..12].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(decode(&old).unwrap_err().kind(), io::ErrorKind::Unsupported);
    snapshot.slots[0].as_mut().unwrap().count = 2;
    assert!(encode(&snapshot).is_err());
}
