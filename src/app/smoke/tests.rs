use super::*;

#[test]
fn smoke_motion_requires_real_jump_landing_walk_flight_and_presented_resize() {
    let origin = Vec3::new(0.0, 25.0, 0.0);
    let mut player = Player::new(origin + Vec3::Y * 5.0);
    player.flying = true;
    let valid = || SmokeState {
        step: Some(240),
        origin,
        peak: 30.0,
        jump_start: Some(25.0),
        jump_peak: 26.0,
        landed: true,
        walk_position: origin + Vec3::X,
        resized: true,
        size_before: Some(PhysicalSize::new(1280, 720)),
        resize_rendered: true,
    };
    assert!(valid().verify_motion(&player).is_ok());
    for check in 0..7 {
        let mut state = valid();
        match check {
            0 => state.resized = false,
            1 => state.resize_rendered = false,
            2 => state.peak = origin.y,
            3 => state.landed = false,
            4 => state.jump_start = None,
            5 => state.jump_peak = origin.y,
            _ => state.walk_position = origin,
        }
        assert!(
            state.verify_motion(&player).is_err(),
            "missing check {check} passed"
        );
    }
    player.flying = false;
    assert!(valid().verify_motion(&player).is_err());
}
