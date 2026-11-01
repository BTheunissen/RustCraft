//! Sample conversion and stale-heartbeat suppression, also used by fake output tests.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use cpal::{FromSample, Sample, SizedSample};

use super::catalog::SampleBank;
use super::mixer::Mixer;
use super::shared::{ACTIVE, MUTED, SCENE_CLEAR, STOPPED, Shared};

const HEARTBEAT_TIMEOUT: Duration = Duration::from_millis(500);

pub(super) struct Callback {
    shared: Arc<Shared>,
    mixer: Mixer,
    channels: usize,
    control: u64,
    received: Option<u64>,
    updated: Instant,
}

impl Callback {
    pub(super) fn new(
        shared: Arc<Shared>,
        bank: Arc<SampleBank>,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Self, &'static str> {
        if !(1..=2).contains(&channels) {
            return Err("audio output requires mono or stereo");
        }
        Ok(Self {
            shared,
            mixer: Mixer::new(bank, sample_rate)?,
            channels: usize::from(channels),
            control: 0,
            received: None,
            updated: Instant::now(),
        })
    }

    pub(super) fn render<T>(&mut self, output: &mut [T], now: Instant)
    where
        T: SizedSample + FromSample<f32>,
    {
        output.fill(T::EQUILIBRIUM);
        let control = self.shared.control.load(Ordering::Acquire);
        let muted = control & (MUTED | STOPPED) != 0 || self.shared.failed.load(Ordering::Acquire);
        if control != self.control {
            let reset = control & (ACTIVE | SCENE_CLEAR) != 0 || self.control & ACTIVE == 0;
            self.mixer.control(false, muted, reset);
            self.control = control;
            self.received = None;
        }
        let mut events = [None; super::queue::EVENT_LIMIT];
        if let Ok(mut mailbox) = self.shared.mailbox.try_lock() {
            if mailbox.control == control {
                self.mixer.set_frame(mailbox.frame);
                self.updated = mailbox.updated;
                self.received = Some(control);
                events = mailbox.events.take(now);
            } else {
                mailbox.events.clear();
            }
        }
        let active = !muted
            && control & ACTIVE != 0
            && self.received == Some(control)
            && now.saturating_duration_since(self.updated) <= HEARTBEAT_TIMEOUT;
        self.mixer.control(active, muted, false);
        if active {
            for event in events.into_iter().flatten() {
                self.mixer.enqueue(event.event);
            }
        }
        for frame in output.chunks_exact_mut(self.channels) {
            let stereo = self.mixer.next();
            if self.channels == 1 {
                frame[0] = Sample::from_sample((stereo[0] + stereo[1]) * 0.5);
            } else {
                frame[0] = Sample::from_sample(stereo[0]);
                frame[1] = Sample::from_sample(stereo[1]);
            }
        }
        self.shared.callbacks.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests;
