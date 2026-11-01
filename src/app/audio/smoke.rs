//! Real mixer/callback regression with discarded PCM, never an audio device or user file.

use std::sync::Arc;
use std::time::Instant;

use glam::Vec3;

use super::Audio;
use super::callback::Callback;
use super::catalog::SampleBank;
use crate::game::{SoundAction, SoundEnvironment, SoundEvent, SoundSurface};

pub(super) fn verify() -> Result<(), String> {
    let mut audio = Audio::new(false);
    let bank = Arc::new(SampleBank::generate());
    let bytes = bank.bytes();
    let mut callback = Callback::new(Arc::clone(&audio.shared), bank, 48_000, 2)?;
    let mut output = vec![0.0_f32; 24_000];
    let now = Instant::now();
    audio.update(
        SoundEnvironment {
            surf: 0.75,
            wind: 0.3,
            ..Default::default()
        },
        Vec3::ZERO,
        0.0,
        [SoundEvent {
            action: SoundAction::Break(SoundSurface::Stone),
            position: Vec3::X,
        }],
        true,
        now,
    );
    callback.render(&mut output, now);
    if !output.iter().any(|sample| sample.abs() > 0.001)
        || !output
            .iter()
            .all(|sample| sample.is_finite() && sample.abs() < 0.9)
    {
        return Err("silent audio smoke failed to mix a real bounded PCM buffer".into());
    }
    audio.suspend();
    callback.render(&mut output, now);
    require_silent(&output[9600..], "pause fade")?;
    audio.clear_scene();
    audio.update(SoundEnvironment::default(), Vec3::ZERO, 0.0, [], true, now);
    callback.render(&mut output, now);
    require_silent(&output, "scene clear")?;
    let mut settings = audio.settings();
    settings.muted = true;
    audio.set_settings(settings);
    audio.update(
        SoundEnvironment {
            surf: 1.0,
            ..Default::default()
        },
        Vec3::ZERO,
        0.0,
        [],
        true,
        now,
    );
    callback.render(&mut output, now);
    require_silent(&output, "mute")?;
    audio.shutdown();
    callback.render(&mut output, now);
    require_silent(&output, "shutdown")?;
    println!(
        "Audio fake-output verified: {bytes} preloaded bytes; action/ambience PCM, pause fade, scene clear, exact mute/shutdown. No audio device or preferences opened."
    );
    Ok(())
}

fn require_silent(output: &[f32], label: &str) -> Result<(), String> {
    if output.iter().any(|sample| *sample != 0.0) {
        Err(format!("silent audio smoke retained output after {label}"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discarded_pcm_checks_real_lifecycle_and_rejects_nonzero_or_nan_silence() {
        assert!(require_silent(&[0.0, -0.0], "fixture").is_ok());
        assert!(require_silent(&[0.001], "fixture").is_err());
        assert!(require_silent(&[f32::NAN], "fixture").is_err());
        verify().unwrap();
    }
}
