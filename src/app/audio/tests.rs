use super::*;
use crate::game::SoundAction;

#[test]
fn silent_output_never_starts_a_worker_and_lifecycle_clears_stale_requests() {
    let mut audio = Audio::new(false);
    audio.start();
    assert!(!audio.started && audio.native.is_none());
    let now = Instant::now();
    let event = SoundEvent {
        action: SoundAction::Pickup,
        position: Vec3::ZERO,
    };
    audio.update(
        SoundEnvironment::default(),
        Vec3::ZERO,
        0.0,
        [event],
        true,
        now,
    );
    audio.suspend();
    assert!(
        audio
            .shared
            .mailbox
            .lock()
            .unwrap()
            .events
            .take(now)
            .iter()
            .all(Option::is_none)
    );
    assert_eq!(audio.shared.control.load(Ordering::Acquire) & ACTIVE, 0);
    audio.clear_scene();
    assert_eq!(audio.frame.environment, SoundEnvironment::default());
    audio.shutdown();
    assert!(audio.shared.stopped());
    audio.suspend();
    audio.set_settings(AudioSettings::default());
    audio.update(
        SoundEnvironment::default(),
        Vec3::ZERO,
        0.0,
        [event],
        true,
        now,
    );
    assert!(audio.shared.stopped());
}

#[test]
fn unavailable_output_is_reported_once_and_does_not_fail_gameplay() {
    let mut audio = Audio::new(false);
    audio.unavailable("simulated missing output device");
    assert!(audio.warned && audio.shared.failed.load(Ordering::Acquire));
    audio.unavailable("same simulated error");
    let mut game = crate::game::Game::new(42);
    game.update(crate::player::MovementInput::default(), 0.05);
    assert!(game.snapshot().validate().is_ok());
    let now = Instant::now();
    audio.update(SoundEnvironment::default(), Vec3::ZERO, 0.0, [], true, now);
    assert!(audio.shared.failed.load(Ordering::Acquire));
}

#[test]
fn control_publication_cannot_block_on_the_sample_callback() {
    let mut audio = Audio::new(false);
    let shared = Arc::clone(&audio.shared);
    let mailbox = shared.mailbox.lock().unwrap();
    audio.update(
        SoundEnvironment::default(),
        Vec3::ZERO,
        0.0,
        [],
        true,
        Instant::now(),
    );
    audio.set_settings(AudioSettings {
        muted: true,
        ..Default::default()
    });
    assert!(audio.settings().muted);
    assert_ne!(shared.control.load(Ordering::Acquire) & MUTED, 0);
    audio.shutdown();
    assert!(shared.stopped());
    drop(mailbox);
}

#[test]
fn smoke_launch_never_enables_native_audio_even_without_no_audio_flag() {
    let mut app = crate::app::App::new(crate::options::LaunchOptions {
        smoke_test: true,
        ..Default::default()
    });
    assert!(!app.audio.enabled);
    app.audio.start();
    assert!(app.audio.native.is_none());
    app.captured = true;
    app.focused = true;
    app.screen = crate::hud::Screen::Playing;
    app.audio.update(
        SoundEnvironment {
            wind: 1.0,
            ..Default::default()
        },
        Vec3::ZERO,
        0.0,
        [],
        true,
        Instant::now(),
    );
    app.release_pointer();
    assert_eq!(app.audio.shared.control.load(Ordering::Acquire) & ACTIVE, 0);
}

#[test]
fn real_game_action_reaches_fake_pcm_without_altering_saved_gameplay() {
    let mut game = crate::game::test_support::arena();
    game.world.edit_block([8, 2, 6], crate::world::Block::Dirt);
    for _ in 0..3 {
        game.interact(true, false, 0.2);
    }
    let snapshot = game.snapshot();
    let events = game.take_sound_events();
    let mut audio = Audio::new(false);
    let now = Instant::now();
    audio.update(
        SoundEnvironment::default(),
        game.player.eye_position(),
        game.player.yaw,
        events.into_iter().flatten(),
        true,
        now,
    );
    let mut callback =
        callback::Callback::new(Arc::clone(&audio.shared), catalog::test_bank(), 48_000, 2)
            .unwrap();
    let mut pcm = vec![0.0_f32; 24_000];
    callback.render(&mut pcm, now);
    assert!(pcm.iter().any(|sample| sample.abs() > 0.001));
    assert_eq!(game.snapshot(), snapshot);
    audio.suspend();
    callback.render(&mut pcm, now);
    assert!(pcm[9600..].iter().all(|sample| *sample == 0.0));
    assert!(game.take_sound_events().iter().all(Option::is_none));
}

#[test]
fn terminated_native_worker_becomes_a_once_reported_silent_failure() {
    let mut audio = Audio::new(false);
    let (stop, _receiver) = std::sync::mpsc::sync_channel(1);
    let (_sender, status) = std::sync::mpsc::sync_channel(2);
    let worker = std::thread::spawn(|| {});
    while !worker.is_finished() {
        std::thread::yield_now();
    }
    audio.native = Some(NativeOutput {
        stop,
        status,
        worker,
    });
    audio.report();
    assert!(audio.warned && audio.shared.failed.load(Ordering::Acquire));
    audio.report();
    assert_eq!(audio.shared.control.load(Ordering::Acquire) & ACTIVE, 0);
}
