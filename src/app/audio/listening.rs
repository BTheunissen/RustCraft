//! Explicit audible hardware route, completely separate from tests, UI, and world saves.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use glam::Vec3;

use super::Audio;
use crate::game::{SoundAction, SoundEnvironment, SoundEvent, SoundSurface};

pub(super) fn run() -> Result<(), String> {
    println!(
        "AUDIBLE audio check: conservative volume, 16 seconds, no window, capture, preferences, or world save.\nListen for stone/earth/sand/wood, gathering, wind/surf/freshwater/cave, underwater, pause, and mute."
    );
    let mut audio = Audio::new(true);
    audio.start();
    let initialization_deadline = Instant::now() + Duration::from_secs(3);
    while !audio.shared.ready.load(Ordering::Acquire) {
        audio.update(
            SoundEnvironment::default(),
            Vec3::ZERO,
            0.0,
            [],
            false,
            Instant::now(),
        );
        if audio.shared.failed.load(Ordering::Acquire) || Instant::now() >= initialization_deadline
        {
            return Err(
                "audio check could not initialize an output device within three seconds".into(),
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let started = Instant::now();
    let mut previous = usize::MAX;
    let mut action_time = started;
    while started.elapsed() < Duration::from_secs(16) {
        let now = Instant::now();
        let phase = started.elapsed().as_secs() as usize;
        if phase != previous {
            let label = match phase {
                0..=3 => ["stone steps", "earth steps", "sand steps", "wood steps"][phase],
                4 => "mine / break / place / pickup",
                5 => "wind",
                6..=7 => "dry coastal surf",
                8 => "freshwater",
                9 => "sheltered cave",
                10 => "water entry / underwater",
                11 => "swimming / underwater",
                12 => "pause: should become silent",
                13 => "resume cave",
                14 => "mute: exact silence",
                _ => "new scene: fresh wind",
            };
            println!("Audio check {phase:02}s: {label}");
            if phase == 12 {
                audio.suspend();
            }
            if phase == 14 {
                let mut settings = audio.settings();
                settings.muted = true;
                audio.set_settings(settings);
            }
            if phase == 15 {
                audio.clear_scene();
                let mut settings = audio.settings();
                settings.muted = false;
                audio.set_settings(settings);
            }
            previous = phase;
            action_time = now;
        }
        let environment = match phase {
            5 | 15 => SoundEnvironment {
                wind: 0.7,
                ..Default::default()
            },
            6..=7 => SoundEnvironment {
                surf: 0.9,
                wind: 0.3,
                ..Default::default()
            },
            8 => SoundEnvironment {
                freshwater: 0.9,
                wind: 0.2,
                ..Default::default()
            },
            9 | 13 => SoundEnvironment {
                cave: 0.8,
                ..Default::default()
            },
            10..=11 => SoundEnvironment {
                surf: 0.8,
                underwater: true,
                ..Default::default()
            },
            _ => SoundEnvironment::default(),
        };
        let mut event = None;
        if now >= action_time && (phase < 5 || (10..=11).contains(&phase)) {
            let action = match phase {
                0..=3 => SoundAction::Step(
                    [
                        SoundSurface::Stone,
                        SoundSurface::Earth,
                        SoundSurface::Sand,
                        SoundSurface::Wood,
                    ][phase],
                ),
                10 => SoundAction::EnterWater,
                11 => SoundAction::Swim,
                _ => match started.elapsed().as_millis() % 1000 {
                    0..=249 => SoundAction::Mine(SoundSurface::Stone),
                    250..=499 => SoundAction::Break(SoundSurface::Stone),
                    500..=749 => SoundAction::Place(SoundSurface::Wood),
                    _ => SoundAction::Pickup,
                },
            };
            event = Some(SoundEvent {
                action,
                position: Vec3::ZERO,
            });
            action_time = now
                + Duration::from_millis(match phase {
                    4 => 250,
                    10 => 2000,
                    _ => 400,
                });
        }
        audio.update(environment, Vec3::ZERO, 0.0, event, phase != 12, now);
        if audio.shared.failed.load(Ordering::Acquire) {
            return Err("audio check could not keep a native output stream; gameplay remains usable silently".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let callbacks = audio.shared.callbacks.load(Ordering::Relaxed);
    audio.shutdown();
    let shutdown_deadline = Instant::now() + Duration::from_millis(250);
    while audio.shared.ready.load(Ordering::Acquire) && Instant::now() < shutdown_deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    if audio.shared.ready.load(Ordering::Acquire) {
        return Err("audio output was suppressed, but native stream shutdown was not confirmed within 250ms".into());
    }
    if callbacks == 0 {
        return Err("audio check received no device callbacks".into());
    }
    println!(
        "Audio hardware check ended: {callbacks} callbacks; native stream stopped/dropped. Human listening/comfort/repetition NOT automatically verified."
    );
    Ok(())
}
