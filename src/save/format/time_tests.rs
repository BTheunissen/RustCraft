use super::*;
use crate::game::Game;

#[test]
fn exact_solar_phase_round_trips_but_debug_acceleration_does_not_resume() {
    let mut game = Game::new(42);
    game.time_mut().set_hour(17.123_456_789).unwrap();
    game.time_mut().set_speed(60.0).unwrap();
    game.update(Default::default(), 1.0 / 60.0);
    let saved = game.snapshot();
    let decoded = decode(&encode(&saved).unwrap()).unwrap();
    assert_eq!(decoded, saved);
    let restored = Game::from_snapshot(decoded).unwrap();
    assert_eq!(restored.time().hour(), game.time().hour());
    assert_eq!(restored.time().speed(), 1.0);
}

#[test]
fn legacy_format_three_keeps_all_progress_and_starts_in_daylight_without_resetting() {
    let mut saved = Game::with_mode(42, GameMode::Survival).snapshot();
    saved.edits.push(BlockEdit {
        position: [-17, -30, 16],
        block: Block::Lantern,
    });
    saved.slots[0] = Some(ItemStack {
        item: Tool::StonePickaxe.into(),
        count: 1,
    });
    saved.edits.push(BlockEdit {
        position: [-16, -30, 16],
        block: Block::StorageCrate,
    });
    let mut slots = [None; CRATE_SLOTS];
    slots[17] = Some(ItemStack {
        item: Block::Crystal.into(),
        count: 64,
    });
    saved.crates.push(SavedCrate {
        position: [-16, -30, 16],
        slots,
    });
    saved.pickups.push(SavedPickup {
        position: Vec3::new(-17.5, -30.0, 16.5),
        item: Tool::StoneAxe.into(),
        count: 1,
        velocity: Vec3::new(0.0, -2.0, 0.0),
        age: 0.1,
    });
    let mut legacy = encode(&saved).unwrap();
    legacy.truncate(legacy.len() - 8);
    legacy[8..12].copy_from_slice(&LEGACY_FORMAT_VERSION.to_le_bytes());
    tests::reseal(&mut legacy);
    let restored = decode(&legacy).unwrap();
    assert_eq!(restored, saved);
    let upgraded = encode(&restored).unwrap();
    assert_eq!(decode(&upgraded).unwrap(), saved);
    assert_eq!(&upgraded[8..12], &FORMAT_VERSION.to_le_bytes());
    let mut unexpected = legacy;
    unexpected.push(0);
    tests::reseal(&mut unexpected);
    assert!(decode(&unexpected).is_err());
}

#[test]
fn corrupt_or_out_of_range_solar_phases_are_rejected_before_restoring_progress() {
    for hour in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 24.0] {
        let mut saved = Game::new(42).snapshot();
        saved.time_of_day = hour;
        assert_eq!(saved.validate(), Err("invalid saved solar time"));
        assert!(encode(&saved).is_err());
        let mut bytes = encode(&Game::new(42).snapshot()).unwrap();
        let offset = bytes.len() - 8;
        bytes[offset..].copy_from_slice(&hour.to_le_bytes());
        tests::reseal(&mut bytes);
        assert!(
            decode(&bytes)
                .unwrap_err()
                .to_string()
                .contains("solar time")
        );
    }
}
