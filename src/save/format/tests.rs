use super::*;
use crate::game::Game;

fn fixture() -> GameSnapshot {
    let mut saved = Game::with_mode(u64::MAX, GameMode::Survival).snapshot();
    saved.player.position = Vec3::new(-16.5, -30.0, -0.5);
    saved.player.yaw = -2.0;
    saved.player.pitch = 0.7;
    saved.headlamp = false;
    saved.selected_slot = 8;
    for (index, block) in Block::ALL
        .into_iter()
        .filter(|block| block.is_solid() && *block != Block::Bedrock)
        .enumerate()
    {
        saved.slots[index] = Some(ItemStack {
            item: block.into(),
            count: (index + 1) as u16,
        });
    }
    saved.edits = vec![
        BlockEdit {
            position: [-1, -63, -17],
            block: Block::Air,
        },
        BlockEdit {
            position: [17, 79, -16],
            block: Block::Clay,
        },
    ];
    saved.pickups = vec![SavedPickup {
        position: Vec3::new(-0.5, -20.0, -17.5),
        item: Block::Crystal.into(),
        count: 64,
        velocity: Vec3::new(0.2, -10.0, -0.2),
        age: 0.15,
    }];
    saved
}

pub(super) fn reseal(bytes: &mut [u8]) {
    let length = bytes.len() - HEADER_BYTES;
    bytes[16..24].copy_from_slice(&(length as u64).to_le_bytes());
    let hash = checksum(&bytes[HEADER_BYTES..]);
    bytes[24..32].copy_from_slice(&hash.to_le_bytes());
}

#[test]
fn every_collectible_material_negative_y_pose_and_dropped_motion_round_trip_exactly() {
    let saved = fixture();
    let bytes = encode(&saved).unwrap();
    assert_eq!(decode(&bytes).unwrap(), saved);
    assert_eq!(encode(&decode(&bytes).unwrap()).unwrap(), bytes);
    for speed in [
        SpeedMultiplier::Normal,
        SpeedMultiplier::Double,
        SpeedMultiplier::Quadruple,
        SpeedMultiplier::Octuple,
        SpeedMultiplier::Sixteen,
    ] {
        let mut creative = Game::new(42).snapshot();
        creative.speed = speed;
        creative.player.flying = true;
        assert_eq!(decode(&encode(&creative).unwrap()).unwrap(), creative);
    }
}

#[test]
fn every_truncation_and_unsealed_byte_flip_is_rejected() {
    let bytes = encode(&fixture()).unwrap();
    for length in 0..bytes.len() {
        assert!(decode(&bytes[..length]).is_err(), "prefix {length}");
        if length >= HEADER_BYTES {
            let mut truncated = bytes[..length].to_vec();
            reseal(&mut truncated);
            assert!(decode(&truncated).is_err(), "resealed prefix {length}");
        }
    }
    for index in 0..bytes.len() {
        let mut damaged = bytes.clone();
        damaged[index] ^= 1;
        assert!(decode(&damaged).is_err(), "flipped byte {index}");
    }
}

#[test]
fn versions_bad_lengths_oversize_and_trailing_data_fail_explicitly() {
    let bytes = encode(&fixture()).unwrap();
    for offset in [8, 12] {
        let mut future = bytes.clone();
        future[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let error = decode(&future).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(error.to_string().contains("version"));
        assert!(error.to_string().contains("back up"));
        assert!(
            error
                .to_string()
                .contains("confirm a new development world")
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    reseal(&mut trailing);
    assert!(
        decode(&trailing)
            .unwrap_err()
            .to_string()
            .contains("unexpected")
    );
    let mut bad_length = bytes;
    bad_length[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(decode(&bad_length).is_err());
    assert!(decode(&vec![0; MAX_SAVE_BYTES + 1]).is_err());
}

#[test]
fn resealed_bad_enums_stacks_and_record_counts_never_reach_gameplay() {
    let bytes = encode(&fixture()).unwrap();
    for (offset, value) in [
        (8, 2),
        (29, 2),
        (30, 2),
        (31, 3),
        (32, 9),
        (33, 255),
        (34, 255),
        (35, 0),
    ] {
        let mut invalid = bytes.clone();
        invalid[HEADER_BYTES + offset] = value;
        reseal(&mut invalid);
        assert!(decode(&invalid).is_err(), "payload field {offset}");
    }
    let edits_count = HEADER_BYTES + 33 + INVENTORY_SLOTS * STACK_BYTES;
    for count in [MAX_SAVED_EDITS as u32 + 1, u32::MAX, 10_000] {
        let mut invalid = bytes.clone();
        invalid[edits_count..edits_count + 4].copy_from_slice(&count.to_le_bytes());
        reseal(&mut invalid);
        assert!(decode(&invalid).is_err());
    }
    let pickups_count = edits_count + 4 + fixture().edits.len() * EDIT_BYTES;
    let mut invalid = bytes.clone();
    invalid[pickups_count..pickups_count + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    reseal(&mut invalid);
    assert!(decode(&invalid).is_err());

    let mut invalid = bytes;
    let first_edit = edits_count + 4;
    let position = invalid[first_edit..first_edit + 12].to_vec();
    invalid[first_edit + EDIT_BYTES..first_edit + EDIT_BYTES + 12].copy_from_slice(&position);
    reseal(&mut invalid);
    assert!(
        decode(&invalid)
            .unwrap_err()
            .to_string()
            .contains("duplicate")
    );
}

#[test]
fn invalid_snapshots_cannot_be_encoded_and_unknown_empty_stack_shapes_are_rejected() {
    let mut saved = fixture();
    saved.pickups[0].count = 0;
    assert!(encode(&saved).is_err());
    let mut bytes = encode(&Game::new(42).snapshot()).unwrap();
    bytes[HEADER_BYTES + 35] = 1;
    reseal(&mut bytes);
    assert!(decode(&bytes).is_err());
}
