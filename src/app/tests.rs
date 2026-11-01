use super::pacing::{FALLBACK_REFRESH_MILLIHERTZ, REFRESH_CHECK_INTERVAL, next_frame_deadline};
use super::{App, FramePacing, SMOKE_TIMEOUT, size_changed, smoke_timed_out};
use crate::game::Game;
use crate::hud::Screen;
use crate::options::LaunchOptions;
use crate::player::MovementInput;
use crate::playtest::surface::{overview_height, water_capture_position};
use crate::world::{Block, SEA_LEVEL, Terrain};
use std::time::{Duration, Instant};
use winit::dpi::PhysicalSize;
use winit::keyboard::KeyCode;

fn exploring(options: LaunchOptions) -> App {
    let mut app = App::new(options);
    app.game = Game::new(app.options.seed);
    app.screen = Screen::Playing;
    app
}

#[test]
fn simulation_requires_both_focus_and_pointer_capture() {
    let mut app = exploring(LaunchOptions::default());
    let initial = app.game.player.position;
    app.input.key(KeyCode::KeyW, true, false, 0.0);
    app.simulate(0.1);
    assert_eq!(app.game.player.position, initial);
    app.captured = true;
    app.simulate(0.1);
    assert_eq!(app.game.player.position, initial);
    app.focused = true;
    app.input.key(KeyCode::Space, true, false, 0.0);
    app.simulate(0.1);
    assert!(app.game.player.position.y > initial.y);
    assert!((app.game.player.position - initial).with_y(0.0).length() > 0.1);
}

#[test]
fn toggling_headlamp_preserves_position_view_and_movement_mode() {
    let mut app = exploring(LaunchOptions::default());
    let pose = (
        app.game.player.position,
        app.game.player.yaw,
        app.game.player.pitch,
        app.game.player.flying,
    );
    assert!(app.game.headlamp);
    app.game.toggle_headlamp();
    assert!(!app.game.headlamp);
    assert_eq!(
        (
            app.game.player.position,
            app.game.player.yaw,
            app.game.player.pitch,
            app.game.player.flying
        ),
        pose
    );
    app.game.toggle_headlamp();
    assert!(app.game.headlamp);
}

#[test]
fn low_distance_overview_stays_within_surface_fog_instead_of_showing_only_sky() {
    assert_eq!(overview_height(40.0, 25, 2), 33.0);
    assert_eq!(overview_height(40.0, 25, 3), 33.0);
    assert_eq!(overview_height(40.0, 25, 8), 65.0);
    assert_eq!(overview_height(60.0, 45, 8), 78.0);
}

#[test]
fn frame_pacing_counts_render_work_but_failed_acquisition_gets_a_full_backoff() {
    let start = Instant::now();
    let complete = start + Duration::from_millis(4);
    let mut pacing = FramePacing::new(start);
    pacing.update(Some(165_000), start);
    let interval = pacing.interval();
    assert_eq!(
        next_frame_deadline(start, complete, true, interval),
        start + interval
    );
    assert_eq!(
        next_frame_deadline(start, complete, false, interval),
        complete + interval
    );
    assert_eq!(
        next_frame_deadline(start, start + interval * 2, true, interval),
        start + interval
    );
}

#[test]
fn display_refresh_rates_replace_the_fixed_sixty_fps_limit() {
    let now = Instant::now();
    let mut pacing = FramePacing::new(now);
    for (rate, nanoseconds) in [
        (60_000, 16_666_667),
        (59_940, 16_683_351),
        (120_000, 8_333_334),
        (144_000, 6_944_445),
        (165_000, 6_060_607),
        (240_000, 4_166_667),
    ] {
        pacing.update(Some(rate), now);
        assert_eq!(pacing.interval(), Duration::from_nanos(nanoseconds));
        assert!(pacing.interval().as_nanos() * u128::from(rate) >= 1_000_000_000_000);
    }
    for invalid in [None, Some(0)] {
        pacing.update(invalid, now);
        assert_eq!(pacing.refresh_millihertz, FALLBACK_REFRESH_MILLIHERTZ);
        assert_eq!(pacing.interval(), Duration::from_nanos(16_666_667));
    }
    pacing.update(Some(u32::MAX), now);
    assert!(!pacing.interval().is_zero());
    pacing.update(Some(1), now);
    assert_eq!(pacing.interval(), Duration::from_secs(1000));
}

#[test]
fn refresh_checks_are_cached_and_monitor_changes_update_the_cap() {
    let now = Instant::now();
    let mut pacing = FramePacing::new(now);
    assert!(pacing.refresh_due(now));
    assert!(pacing.update(Some(165_000), now));
    assert!(!pacing.refresh_due(now + REFRESH_CHECK_INTERVAL - Duration::from_nanos(1)));
    assert!(pacing.refresh_due(now + REFRESH_CHECK_INTERVAL));
    assert!(!pacing.update(Some(165_000), now + REFRESH_CHECK_INTERVAL));
    assert!(pacing.update(Some(60_000), now + REFRESH_CHECK_INTERVAL));
    assert_eq!(pacing.interval(), Duration::from_nanos(16_666_667));
}

#[test]
fn premature_smoke_exit_fails_without_replacing_an_existing_error() {
    let mut app = App::new(LaunchOptions {
        smoke_test: true,
        ..Default::default()
    });
    app.record_exit();
    assert!(app.error.as_ref().unwrap().contains("before completing"));
    app.error = Some("GPU failed".into());
    app.record_exit();
    assert_eq!(app.error.as_deref(), Some("GPU failed"));
    let mut complete = App::new(LaunchOptions {
        smoke_test: true,
        ..Default::default()
    });
    complete.finished = true;
    complete.record_exit();
    assert!(complete.error.is_none());
    let mut interactive = App::new(LaunchOptions::default());
    interactive.record_exit();
    assert!(interactive.error.is_none());
}

#[test]
fn zero_size_pauses_and_restoring_size_records_smoke_resize() {
    let mut app = exploring(LaunchOptions::default());
    app.captured = true;
    app.input.key(KeyCode::KeyW, true, false, 0.0);
    app.resize(PhysicalSize::new(0, 720));
    assert!(app.zero_sized);
    assert!(!app.captured);
    assert!(!app.input.movement(false).forward);
    app.smoke.step = Some(30);
    app.smoke.size_before = Some(PhysicalSize::new(1280, 720));
    app.resize(PhysicalSize::new(1200, 760));
    assert!(!app.zero_sized);
    assert!(app.smoke.resized);
}

#[test]
fn smoke_watchdog_expires_even_when_rendering_is_suspended() {
    let start = Instant::now();
    assert!(!smoke_timed_out(
        true,
        false,
        start,
        start + SMOKE_TIMEOUT - Duration::from_nanos(1)
    ));
    assert!(smoke_timed_out(true, false, start, start + SMOKE_TIMEOUT));
    assert!(!smoke_timed_out(true, true, start, start + SMOKE_TIMEOUT));
    assert!(!smoke_timed_out(false, false, start, start + SMOKE_TIMEOUT));
}

#[test]
fn refused_or_zero_size_is_not_resize_evidence() {
    let before = PhysicalSize::new(1280, 720);
    assert!(!size_changed(Some(before), before));
    assert!(!size_changed(Some(before), PhysicalSize::new(0, 720)));
    assert!(!size_changed(None, PhysicalSize::new(1200, 760)));
    assert!(size_changed(Some(before), PhysicalSize::new(1200, 760)));
}

#[test]
fn pointer_release_clears_both_application_and_controller_input() {
    let mut app = exploring(LaunchOptions::default());
    app.captured = true;
    app.focused = true;
    app.input.key(KeyCode::Space, true, false, 0.0);
    app.simulate(0.1);
    let terrain = app.game.terrain;
    for _ in 0..120 {
        app.game.player.update(
            MovementInput {
                jump: true,
                ..Default::default()
            },
            1.0 / 60.0,
            |x, y, z| terrain.block_at(x, y, z).is_solid(),
        );
    }
    assert!(app.game.player.is_grounded());
    app.input.key(KeyCode::KeyW, true, false, 0.0);
    app.release_pointer();
    assert!(!app.captured);
    let input = app.input.movement(false);
    assert!(!input.forward && !input.jump);
    let ground = app.game.player.position.y;
    app.captured = true;
    app.screen = Screen::Playing;
    app.input.key(KeyCode::Space, true, false, 1.0);
    app.simulate(0.1);
    assert!(app.game.player.position.y > ground);
}

#[test]
fn integration_flight_toggle_and_ground_recovery_use_the_generated_world() {
    let mut app = exploring(LaunchOptions {
        seed: 42,
        ..Default::default()
    });
    app.game.toggle_flight();
    assert!(app.game.player.flying);
    app.game.player.position.y = 2.0;
    app.game.toggle_flight();
    assert!(!app.game.player.flying);
    let eye = app.game.player.eye_position();
    assert!(
        !app.game
            .terrain
            .block_at(
                eye.x.floor() as i32,
                eye.y.floor() as i32,
                eye.z.floor() as i32
            )
            .is_solid()
    );
    let feet = app.game.player.position;
    assert!(
        feet.y
            >= app
                .game
                .terrain
                .column(feet.x.floor() as i32, feet.z.floor() as i32)
                .height as f32
                + 1.0
    );
}

#[test]
fn water_capture_targets_real_water_below_a_clear_camera() {
    let terrain = Terrain::default();
    let position = water_capture_position(terrain).unwrap();
    assert!(position.is_finite());
    assert_eq!(
        terrain.block_at(
            position.x.floor() as i32,
            SEA_LEVEL - 1,
            position.z.floor() as i32 - 16
        ),
        Block::Water
    );
    assert_eq!(
        terrain.block_at(
            position.x.floor() as i32,
            position.y.floor() as i32,
            position.z.floor() as i32
        ),
        Block::Air
    );
}

#[test]
fn control_speed_actions_are_capture_gated_and_persist_after_release_and_pause() {
    use crate::input::Action;
    use crate::player::SpeedMultiplier;
    let mut app = exploring(LaunchOptions::default());
    app.gameplay_action(&Action::CycleSpeed);
    assert_eq!(app.game.speed(), SpeedMultiplier::Normal);
    app.captured = true;
    app.focused = true;
    let action = app
        .input
        .key(KeyCode::ControlLeft, true, false, 0.0)
        .unwrap();
    app.gameplay_action(&action);
    app.input.key(KeyCode::ControlLeft, false, false, 0.1);
    assert_eq!(app.game.speed(), SpeedMultiplier::Double);
    app.game.player.flying = true;
    let start = app.game.player.position;
    app.input.key(KeyCode::KeyW, true, false, 0.2);
    app.simulate(0.1);
    assert!(((app.game.player.position - start).length() - 1.0).abs() < 0.0001);
    app.release_pointer();
    assert_eq!(app.game.speed(), SpeedMultiplier::Double);
    assert!(!app.input.movement(false).forward);
}
