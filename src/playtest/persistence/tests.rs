use super::*;

#[test]
fn unchanged_frame_acceptance_rejects_nonfinite_and_visible_differences() {
    for difference in [
        FrameDifference {
            changed_fraction: 0.001,
            mean_absolute_error: 0.0,
        },
        FrameDifference {
            changed_fraction: 0.0,
            mean_absolute_error: 0.001,
        },
        FrameDifference {
            changed_fraction: f32::NAN,
            mean_absolute_error: 0.0,
        },
        FrameDifference {
            changed_fraction: 0.0,
            mean_absolute_error: f32::NAN,
        },
        FrameDifference {
            changed_fraction: f32::INFINITY,
            mean_absolute_error: 0.0,
        },
    ] {
        assert!(unchanged_difference(difference).is_err());
    }
    assert!(
        unchanged_difference(FrameDifference {
            changed_fraction: 0.0,
            mean_absolute_error: 0.0
        })
        .is_ok()
    );
}

#[test]
fn diagnostic_frame_preserves_pose_drops_and_isolates_animation_from_ui() {
    let game = Game::with_mode(42, GameMode::Survival);
    let view = frame(&game);
    assert_eq!(view.player.position, game.player().position);
    assert!(view.hud.is_empty());
    assert_eq!(view.water_time, Some(view.elapsed));
    let survival = view.survival.unwrap();
    assert_eq!(survival.pickups.len(), game.pickups().len());
    assert!(!survival.show_hand);
    assert!(survival.target.is_none());
    assert_eq!(survival.mining_progress, 0.0);
}

#[test]
fn smoke_slots_are_exclusive_temporary_and_removed_after_verification() {
    let first = ScratchDirectory::new().unwrap();
    let second = ScratchDirectory::new().unwrap();
    assert_ne!(first.0, second.0);
    let path = first.0.clone();
    let slot = SaveSlot::new(path.join("world.save"));
    let snapshot = Game::new(42).snapshot();
    slot.write(&snapshot).unwrap();
    assert_eq!(slot.load().unwrap().unwrap(), snapshot);
    drop(first);
    assert!(!path.exists());
    assert!(second.0.is_dir());
}
