//! Private native audio composition with explicit silent output for tests and smoke.

mod callback;
mod catalog;
mod device;
mod listening;
mod mixer;
pub(in crate::app) mod preferences;
mod queue;
mod settings;
mod shared;
mod smoke;
mod synthesis;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::JoinHandle;
use std::time::Instant;

use glam::Vec3;

use crate::game::{SoundEnvironment, SoundEvent};
use device::DeviceStatus;
use mixer::MixFrame;
use queue::{EventQueue, PendingEvent};
pub(super) use settings::AudioSettings;
use shared::{ACTIVE, FLAG_BITS, MUTED, SCENE_CLEAR, STOPPED, Shared};

struct NativeOutput {
    stop: SyncSender<()>,
    status: Receiver<DeviceStatus>,
    worker: JoinHandle<()>,
}

pub(super) struct Audio {
    shared: Arc<Shared>,
    native: Option<NativeOutput>,
    enabled: bool,
    started: bool,
    active: bool,
    scene_clear: bool,
    epoch: u64,
    frame: MixFrame,
    staged: EventQueue,
    warned: bool,
    pub(in crate::app) preferences: preferences::Preferences,
}

impl Audio {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            shared: Arc::new(Shared::new()),
            native: None,
            enabled,
            started: false,
            active: false,
            scene_clear: false,
            epoch: 0,
            frame: MixFrame::default(),
            staged: EventQueue::default(),
            warned: false,
            preferences: preferences::Preferences::disabled(),
        }
    }

    pub(super) fn start(&mut self) {
        if !self.enabled || self.started || self.shared.stopped() {
            return;
        }
        self.started = true;
        let (stop, receiver) = mpsc::sync_channel(1);
        let (sender, status) = mpsc::sync_channel(2);
        let shared = Arc::clone(&self.shared);
        match std::thread::Builder::new()
            .name("rustcraft-audio".into())
            .spawn(move || device::run(shared, receiver, sender))
        {
            Ok(worker) => {
                self.native = Some(NativeOutput {
                    stop,
                    status,
                    worker,
                })
            }
            Err(error) => self.unavailable(&error.to_string()),
        }
    }

    pub(super) fn settings(&self) -> AudioSettings {
        self.frame.settings
    }

    pub(super) fn load_preferences(&mut self, enabled: bool) {
        if !enabled {
            return;
        }
        self.preferences = preferences::Preferences::local();
        self.frame.settings = self.preferences.load();
        if self.frame.settings.muted {
            self.advance_epoch();
        }
    }

    pub(super) fn save_preferences(&mut self) {
        self.preferences.save(self.settings());
    }

    pub(super) fn controls(&self) -> crate::hud::AudioControls {
        let settings = self.settings();
        crate::hud::AudioControls {
            muted: settings.muted,
            master: (settings.master * 100.0).round() as u8,
            actions: (settings.actions * 100.0).round() as u8,
            ambience: (settings.ambience * 100.0).round() as u8,
            unavailable: !self.enabled || self.shared.failed.load(Ordering::Acquire),
            preferences_unavailable: self.preferences.unavailable(),
        }
    }

    pub(super) fn set_settings(&mut self, settings: AudioSettings) {
        if self.shared.stopped() {
            return;
        }
        let settings = settings.normalized();
        let changed_mute = settings.muted != self.frame.settings.muted;
        self.frame.settings = settings;
        if changed_mute {
            self.advance_epoch();
        }
        self.publish(Instant::now());
    }

    pub(super) fn update(
        &mut self,
        environment: SoundEnvironment,
        listener: Vec3,
        yaw: f32,
        events: impl IntoIterator<Item = SoundEvent>,
        active: bool,
        now: Instant,
    ) {
        if self.shared.stopped() {
            return;
        }
        if active != self.active {
            self.active = active;
            if active {
                self.scene_clear = false;
            }
            self.advance_epoch();
        }
        self.frame.environment = environment;
        self.frame.listener = listener;
        self.frame.yaw = yaw;
        if self.active && !self.frame.settings.muted {
            for event in events {
                self.staged.push(PendingEvent {
                    event,
                    created: now,
                });
            }
        }
        self.publish(now);
        self.report();
    }

    pub(super) fn suspend(&mut self) {
        if self.shared.stopped() {
            return;
        }
        self.active = false;
        self.advance_epoch();
        self.publish(Instant::now());
    }

    pub(super) fn clear_scene(&mut self) {
        if self.shared.stopped() {
            return;
        }
        self.active = false;
        self.scene_clear = true;
        self.frame.environment = SoundEnvironment::default();
        self.advance_epoch();
        self.publish(Instant::now());
    }

    pub(super) fn shutdown(&mut self) {
        self.active = false;
        self.staged.clear();
        self.shared.control.fetch_or(STOPPED, Ordering::Release);
        if let Some(native) = self.native.take() {
            let _ = native.stop.try_send(());
            if native.worker.is_finished() {
                let _ = native.worker.join();
            }
        }
    }

    fn advance_epoch(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.staged.clear();
        self.shared.control.store(self.control(), Ordering::Release);
    }

    fn control(&self) -> u64 {
        (self.epoch << FLAG_BITS)
            | if self.active { ACTIVE } else { 0 }
            | if self.frame.settings.muted { MUTED } else { 0 }
            | if self.scene_clear { SCENE_CLEAR } else { 0 }
    }

    fn publish(&mut self, now: Instant) {
        let control = self.control();
        if let Ok(mut mailbox) = self.shared.mailbox.try_lock() {
            if mailbox.control != control {
                mailbox.events.clear();
            }
            mailbox.control = control;
            mailbox.frame = self.frame;
            mailbox.updated = now;
            for event in self.staged.take(now).into_iter().flatten() {
                mailbox.events.push(event);
            }
        }
    }

    fn report(&mut self) {
        let mut failure = None;
        if let Some(native) = &self.native {
            for status in native.status.try_iter().take(2) {
                match status {
                    DeviceStatus::Ready {
                        name,
                        rate,
                        channels,
                        bytes,
                    } => println!(
                        "Audio ready: {name}, {rate} Hz, {channels} channels, {bytes} sample bytes"
                    ),
                    DeviceStatus::Unavailable(error) => failure = Some(error),
                }
            }
            if !self.warned && failure.is_none() && native.worker.is_finished() {
                failure =
                    Some("audio worker stopped unexpectedly; restart RustCraft to retry".into());
            }
        }
        if let Some(error) = failure {
            self.unavailable(&error);
        }
    }

    fn unavailable(&mut self, error: &str) {
        self.shared.failed.store(true, Ordering::Release);
        if !self.warned {
            eprintln!("Audio unavailable; continuing silently: {error}");
            self.warned = true;
        }
        self.suspend();
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(super) fn listening_check() -> Result<(), String> {
    listening::run()
}

pub(super) fn verify_silent_output() -> Result<(), String> {
    smoke::verify()
}

#[cfg(test)]
mod tests;
