use super::*;
use crate::app::audio::catalog::test_bank;
use crate::game::SoundSurface;

fn mixer(rate: u32) -> Mixer {
    let mut mixer = Mixer::new(test_bank(), rate).unwrap();
    mixer.set_frame(MixFrame {
        environment: SoundEnvironment {
            wind: 1.0,
            ..Default::default()
        },
        ..Default::default()
    });
    mixer.control(true, false, false);
    mixer
}

fn event(action: SoundAction) -> SoundEvent {
    SoundEvent {
        action,
        position: Vec3::ZERO,
    }
}

fn render(mixer: &mut Mixer, samples: usize) -> f32 {
    (0..samples)
        .map(|_| mixer.next().into_iter().map(f32::abs).sum::<f32>())
        .sum()
}

#[test]
fn priority_stealing_is_bounded_and_does_not_replace_successes_with_steps() {
    let mut mixer = mixer(48_000);
    for _ in 0..TRANSIENT_LIMIT {
        mixer.enqueue(event(SoundAction::Break(SoundSurface::Stone)));
    }
    mixer.enqueue(event(SoundAction::Step(SoundSurface::Stone)));
    assert!(
        mixer
            .voices
            .iter()
            .flatten()
            .all(|voice| voice.replacement.is_none())
    );
    mixer.enqueue(event(SoundAction::Place(SoundSurface::Wood)));
    assert_eq!(
        mixer
            .voices
            .iter()
            .flatten()
            .filter(|voice| voice.replacement.is_some())
            .count(),
        1
    );
    render(&mut mixer, 500);
    assert_eq!(mixer.voices.iter().flatten().count(), TRANSIENT_LIMIT);
    assert!(
        mixer
            .voices
            .iter()
            .flatten()
            .any(|voice| voice.request.clip
                == action_clip(SoundAction::Place(SoundSurface::Wood), 0))
    );
    render(&mut mixer, 48_000);
    assert!(mixer.voices.iter().all(Option::is_none));
}

#[test]
fn muted_output_is_exact_zero_and_clears_transients_without_resume_replay() {
    let mut mixer = mixer(48_000);
    mixer.enqueue(event(SoundAction::Pickup));
    assert!(render(&mut mixer, 5000) > 0.0);
    mixer.control(true, true, false);
    mixer.enqueue(event(SoundAction::EnterWater));
    assert!((0..1000).all(|_| mixer.next() == [0.0; 2]));
    assert!(mixer.voices.iter().all(Option::is_none));
    mixer.control(true, false, false);
    assert!(render(&mut mixer, 1000) > 0.0);
    assert!(mixer.voices.iter().all(Option::is_none));
}

#[test]
fn pause_fades_to_exact_silence_and_scene_reset_clears_all_beds() {
    let mut mixer = mixer(44_100);
    assert!(render(&mut mixer, 44_100) > 0.0);
    mixer.control(false, false, false);
    render(&mut mixer, 4410);
    assert!((0..1000).all(|_| mixer.next() == [0.0; 2]));
    mixer.control(false, false, true);
    assert_eq!(mixer.loop_gains, [0.0; LOOP_COUNT]);
    assert_eq!(mixer.loop_cursors, [0.0; LOOP_COUNT]);
    mixer.control(true, false, false);
    assert_eq!(render(&mut mixer, 5000), 0.0);
}

#[test]
fn output_stays_finite_with_all_voices_max_gain_and_bad_readouts() {
    let mut mixer = mixer(48_000);
    mixer.set_frame(MixFrame {
        settings: AudioSettings {
            master: 1000.0,
            actions: 1000.0,
            ambience: 1000.0,
            muted: false,
        },
        environment: SoundEnvironment {
            wind: 1.0,
            surf: 1.0,
            freshwater: 1.0,
            cave: 1.0,
            underwater: false,
        },
        listener: Vec3::NAN,
        yaw: f32::NAN,
    });
    for _ in 0..TRANSIENT_LIMIT {
        mixer.enqueue(event(SoundAction::EnterWater));
    }
    let mut peak = 0.0_f32;
    for _ in 0..48_000 {
        for value in mixer.next() {
            assert!(value.is_finite() && value.abs() < 0.9);
            peak = peak.max(value.abs());
        }
    }
    assert!(peak > 0.01);
    println!(
        "Worst-case 12 transients + four beds, all settings at 1: measured output peak {peak:.5}, finite and below 0.9 limiter ceiling"
    );
    mixer.set_frame(MixFrame {
        settings: AudioSettings {
            master: f32::NAN,
            ..Default::default()
        },
        environment: SoundEnvironment {
            wind: f32::NAN,
            surf: f32::INFINITY,
            ..Default::default()
        },
        ..Default::default()
    });
    assert!((0..1000).all(|_| mixer.next() == [0.0; 2]));
    assert!(Mixer::new(test_bank(), 0).is_err());
    assert!(Mixer::new(test_bank(), u32::MAX).is_err());
}

#[test]
fn crossfades_follow_sample_time_not_render_frame_partitions() {
    let mut low = mixer(44_100);
    let mut high = mixer(48_000);
    render(&mut low, 44_100);
    for _ in 0..60 {
        render(&mut high, 800);
    }
    assert!((low.loop_gains[0] - high.loop_gains[0]).abs() < 0.002);
    assert!((low.loop_gains[0] - (1.0 - (-2.0_f32).exp())).abs() < 0.002);
    assert!((low.loop_cursors[0] - high.loop_cursors[0]).abs() < 0.0001);
    low.enqueue(event(SoundAction::Step(SoundSurface::Stone)));
    low.enqueue(event(SoundAction::Step(SoundSurface::Stone)));
    let clips: Vec<_> = low
        .voices
        .iter()
        .flatten()
        .map(|voice| voice.request.clip)
        .collect();
    assert_ne!(clips[0], clips[1]);
}

#[test]
fn distance_and_pan_use_actual_listener_space_and_invalid_events_are_ignored() {
    let mut mixer = mixer(48_000);
    for position in [Vec3::NAN, Vec3::X * 32.0, Vec3::splat(f32::MAX)] {
        mixer.enqueue(SoundEvent {
            action: SoundAction::Pickup,
            position,
        });
    }
    assert!(mixer.voices.iter().all(Option::is_none));
    mixer.enqueue(SoundEvent {
        action: SoundAction::Pickup,
        position: Vec3::X * 4.0,
    });
    let voice = mixer.voices[0].unwrap();
    assert!(voice.request.pan[1] > voice.request.pan[0]);
    assert!(voice.request.gain < action_gain(SoundAction::Pickup));
}

#[test]
fn underwater_filter_tracks_camera_state_and_recovers_after_surfacing() {
    let mut mixer = mixer(48_000);
    let mut frame = mixer.frame;
    frame.environment.underwater = true;
    mixer.set_frame(frame);
    render(&mut mixer, 48_000);
    assert!(mixer.underwater > 0.99);
    frame.environment.underwater = false;
    mixer.set_frame(frame);
    render(&mut mixer, 48_000);
    assert!(mixer.underwater < 0.001);
    mixer.control(false, false, true);
    assert_eq!(mixer.underwater, 0.0);
}
