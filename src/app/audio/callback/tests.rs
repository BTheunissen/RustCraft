use super::*;
use crate::app::audio::catalog::test_bank;
use crate::app::audio::mixer::MixFrame;
use crate::app::audio::queue::PendingEvent;
use crate::app::audio::shared::FLAG_BITS;
use crate::game::{SoundAction, SoundEnvironment, SoundEvent};
use glam::Vec3;

fn frame(shared: &Shared, control: u64, now: Instant) {
    shared.control.store(control, Ordering::Release);
    let mut mailbox = shared.mailbox.lock().unwrap();
    mailbox.control = control;
    mailbox.updated = now;
    mailbox.frame = MixFrame {
        environment: SoundEnvironment {
            wind: 1.0,
            ..Default::default()
        },
        ..Default::default()
    };
    mailbox.events.push(PendingEvent {
        event: SoundEvent {
            action: SoundAction::Pickup,
            position: Vec3::ZERO,
        },
        created: now,
    });
}

#[test]
fn fake_output_uses_the_real_callback_and_stale_heartbeat_fades_to_silence() {
    let now = Instant::now();
    let shared = Arc::new(Shared::new());
    frame(&shared, (1 << FLAG_BITS) | ACTIVE, now);
    let mut callback = Callback::new(Arc::clone(&shared), test_bank(), 48_000, 2).unwrap();
    let mut output = vec![0.0; 96_000];
    callback.render(&mut output, now);
    assert!(output.iter().any(|value| *value != 0.0));
    callback.render(
        &mut output,
        now + HEARTBEAT_TIMEOUT + Duration::from_millis(1),
    );
    assert!(output[9600..].iter().all(|value| *value == 0.0));
    assert_eq!(shared.callbacks.load(Ordering::Relaxed), 2);
}

#[test]
fn mute_and_stop_bypass_a_contended_full_mailbox_without_waiting() {
    let now = Instant::now();
    let shared = Arc::new(Shared::new());
    frame(&shared, (1 << FLAG_BITS) | ACTIVE, now);
    let mut callback = Callback::new(Arc::clone(&shared), test_bank(), 48_000, 2).unwrap();
    let mut output = vec![0.0_f32; 10_001];
    callback.render(&mut output, now);
    let mailbox = shared.mailbox.lock().unwrap();
    shared
        .control
        .store((2 << FLAG_BITS) | ACTIVE | MUTED, Ordering::Release);
    callback.render(&mut output, now);
    assert!(output.iter().all(|value| *value == 0.0));
    shared
        .control
        .store((3 << FLAG_BITS) | ACTIVE | STOPPED, Ordering::Release);
    callback.render(&mut output, now);
    assert!(output.iter().all(|value| *value == 0.0));
    drop(mailbox);
}

#[test]
fn integer_mono_silence_uses_sample_equilibrium_and_bad_formats_fail() {
    let now = Instant::now();
    let shared = Arc::new(Shared::new());
    let mut callback = Callback::new(Arc::clone(&shared), test_bank(), 44_100, 1).unwrap();
    let mut signed = vec![100_i16; 1000];
    callback.render(&mut signed, now);
    assert!(signed.iter().all(|sample| *sample == 0));
    let mut unsigned = vec![0_u16; 1000];
    callback.render(&mut unsigned, now);
    assert!(unsigned.iter().all(|sample| *sample == u16::EQUILIBRIUM));
    assert!(Callback::new(Arc::clone(&shared), test_bank(), 48_000, 0).is_err());
    assert!(Callback::new(shared, test_bank(), 48_000, 6).is_err());
}

#[test]
fn changed_epoch_never_consumes_old_scene_events_or_environment() {
    let now = Instant::now();
    let shared = Arc::new(Shared::new());
    frame(&shared, (1 << FLAG_BITS) | ACTIVE, now);
    shared
        .control
        .store((2 << FLAG_BITS) | ACTIVE, Ordering::Release);
    let mut callback = Callback::new(shared, test_bank(), 48_000, 2).unwrap();
    let mut output = vec![1.0_f32; 10_000];
    callback.render(&mut output, now);
    assert!(output.iter().all(|value| *value == 0.0));
}

#[test]
fn pause_without_further_redraws_fades_even_with_a_locked_mailbox() {
    let now = Instant::now();
    let shared = Arc::new(Shared::new());
    frame(&shared, (1 << FLAG_BITS) | ACTIVE, now);
    let mut callback = Callback::new(Arc::clone(&shared), test_bank(), 48_000, 2).unwrap();
    let mut output = vec![0.0_f32; 24_000];
    callback.render(&mut output, now);
    let mailbox = shared.mailbox.lock().unwrap();
    shared.control.store(2 << FLAG_BITS, Ordering::Release);
    callback.render(&mut output, now);
    assert!(output[9600..].iter().all(|sample| *sample == 0.0));
    drop(mailbox);
}

#[test]
fn scene_clear_is_immediate_even_when_no_later_frame_or_mailbox_arrives() {
    let now = Instant::now();
    let shared = Arc::new(Shared::new());
    frame(&shared, (1 << FLAG_BITS) | ACTIVE, now);
    let mut callback = Callback::new(Arc::clone(&shared), test_bank(), 48_000, 2).unwrap();
    let mut output = vec![0.0_f32; 96_000];
    callback.render(&mut output, now);
    let mailbox = shared.mailbox.lock().unwrap();
    shared
        .control
        .store((2 << FLAG_BITS) | SCENE_CLEAR, Ordering::Release);
    callback.render(&mut output, now);
    assert!(output.iter().all(|value| *value == 0.0));
    drop(mailbox);
}

#[test]
fn signed_and_unsigned_conversion_produces_non_silent_pcm_inside_hardware_range() {
    let now = Instant::now();
    for channels in [1, 2] {
        let shared = Arc::new(Shared::new());
        frame(&shared, (1 << FLAG_BITS) | ACTIVE, now);
        let mut callback =
            Callback::new(Arc::clone(&shared), test_bank(), 48_000, channels).unwrap();
        let mut signed = vec![0_i16; 12_000];
        callback.render(&mut signed, now);
        assert!(signed.iter().any(|sample| *sample != 0));
        assert!(signed.iter().all(|sample| sample.abs() < 30_000));
        frame(&shared, (1 << FLAG_BITS) | ACTIVE, now);
        let mut unsigned = vec![u16::EQUILIBRIUM; 12_000];
        callback.render(&mut unsigned, now);
        assert!(unsigned.iter().any(|sample| *sample != u16::EQUILIBRIUM));
        shared.failed.store(true, Ordering::Release);
        callback.render(&mut unsigned, now);
        assert!(unsigned.iter().all(|sample| *sample == u16::EQUILIBRIUM));
    }
}
