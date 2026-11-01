//! The actual device-free mixer; native output only converts its stereo samples.

use std::sync::Arc;

use glam::Vec3;

use super::catalog::{ACTION_CLIPS, LOOP_COUNT, SOURCE_RATE, SampleBank, action_clip};
use super::settings::{AudioSettings, gain};
use crate::game::{SoundAction, SoundEnvironment, SoundEvent};

pub(super) const TRANSIENT_LIMIT: usize = 12;
const REPLACEMENT_RELEASE: f32 = 0.008;

#[derive(Clone, Copy)]
pub(super) struct MixFrame {
    pub(super) settings: AudioSettings,
    pub(super) environment: SoundEnvironment,
    pub(super) listener: Vec3,
    pub(super) yaw: f32,
}

impl Default for MixFrame {
    fn default() -> Self {
        Self {
            settings: AudioSettings::default(),
            environment: SoundEnvironment::default(),
            listener: Vec3::ZERO,
            yaw: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
struct Request {
    clip: usize,
    gain: f32,
    pan: [f32; 2],
    priority: u8,
    sequence: u64,
}

#[derive(Clone, Copy)]
struct Voice {
    request: Request,
    cursor: f64,
    release: f32,
    replacement: Option<Request>,
}

impl Voice {
    fn new(request: Request) -> Self {
        Self {
            request,
            cursor: 0.0,
            release: 0.0,
            replacement: None,
        }
    }

    fn priority(self) -> u8 {
        self.replacement.unwrap_or(self.request).priority
    }
}

pub(super) struct Mixer {
    bank: Arc<SampleBank>,
    frame: MixFrame,
    voices: [Option<Voice>; TRANSIENT_LIMIT],
    loop_cursors: [f64; LOOP_COUNT],
    loop_gains: [f32; LOOP_COUNT],
    variants: [usize; 5],
    sequence: u64,
    step: f64,
    dt: f32,
    crossfade: f32,
    underwater_fade: f32,
    world_gain: f32,
    underwater: f32,
    filtered: [f32; 2],
    active: bool,
    muted: bool,
}

impl Mixer {
    pub(super) fn new(bank: Arc<SampleBank>, sample_rate: u32) -> Result<Self, &'static str> {
        if !(8_000..=192_000).contains(&sample_rate) {
            return Err("unsupported audio sample rate");
        }
        let dt = 1.0 / sample_rate as f32;
        Ok(Self {
            bank,
            frame: MixFrame::default(),
            voices: [None; TRANSIENT_LIMIT],
            loop_cursors: [0.0; LOOP_COUNT],
            loop_gains: [0.0; LOOP_COUNT],
            variants: [0; 5],
            sequence: 0,
            step: f64::from(SOURCE_RATE) / f64::from(sample_rate),
            dt,
            crossfade: -(-dt / 0.5).exp_m1(),
            underwater_fade: -(-dt / 0.12).exp_m1(),
            world_gain: 0.0,
            underwater: 0.0,
            filtered: [0.0; 2],
            active: false,
            muted: false,
        })
    }

    pub(super) fn set_frame(&mut self, mut frame: MixFrame) {
        frame.settings = frame.settings.normalized();
        if !frame.listener.is_finite() {
            frame.listener = Vec3::ZERO;
        }
        if !frame.yaw.is_finite() {
            frame.yaw = 0.0;
        }
        self.frame = frame;
    }

    pub(super) fn control(&mut self, active: bool, muted: bool, reset: bool) {
        if reset {
            self.voices.fill(None);
            self.loop_cursors.fill(0.0);
            self.loop_gains.fill(0.0);
            self.world_gain = 0.0;
            self.underwater = 0.0;
            self.filtered.fill(0.0);
            self.frame.environment = SoundEnvironment::default();
        }
        if !active || muted {
            self.voices.fill(None);
        }
        self.active = active;
        self.muted = muted;
    }

    pub(super) fn enqueue(&mut self, event: SoundEvent) {
        if !self.active || self.muted || !event.position.is_finite() {
            return;
        }
        let offset = event.position - self.frame.listener;
        let distance = offset.length();
        if !distance.is_finite() || distance >= 32.0 {
            return;
        }
        let right = Vec3::new(self.frame.yaw.cos(), 0.0, self.frame.yaw.sin());
        let pan = (offset.normalize_or_zero().dot(right) * 0.7).clamp(-1.0, 1.0);
        let variation = match event.action {
            SoundAction::Step(surface) => Some(surface.index()),
            SoundAction::Swim => Some(4),
            _ => None,
        };
        let variant = variation.map_or(0, |index| {
            let variant = self.variants[index];
            self.variants[index] = (variant + 1) % 3;
            variant
        });
        self.sequence = self.sequence.wrapping_add(1);
        let request = Request {
            clip: action_clip(event.action, variant),
            gain: action_gain(event.action) / (1.0 + distance * 0.12),
            pan: [((1.0 - pan) * 0.5).sqrt(), ((1.0 + pan) * 0.5).sqrt()],
            priority: event.action.priority(),
            sequence: self.sequence,
        };
        if let Some(slot) = self.voices.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(Voice::new(request));
            return;
        }
        let victim = self
            .voices
            .iter_mut()
            .flatten()
            .filter(|voice| voice.priority() <= request.priority)
            .min_by_key(|voice| (voice.priority(), voice.request.sequence));
        if let Some(voice) = victim {
            voice.replacement = Some(request);
            if voice.release <= 0.0 {
                voice.release = REPLACEMENT_RELEASE;
            }
        }
    }

    pub(super) fn next(&mut self) -> [f32; 2] {
        let targets = self.frame.environment.gains().map(gain);
        let envelope_step = self.dt / 0.08;
        self.world_gain = if self.active {
            (self.world_gain + envelope_step).min(1.0)
        } else {
            (self.world_gain - envelope_step).max(0.0)
        };
        let mut ambience = 0.0;
        for (index, target) in targets.into_iter().enumerate() {
            self.loop_gains[index] += (target - self.loop_gains[index]) * self.crossfade;
            let clip = self.bank.clip(ACTION_CLIPS + index);
            ambience += sample(clip, self.loop_cursors[index], true) * self.loop_gains[index];
            self.loop_cursors[index] =
                (self.loop_cursors[index] + self.step).rem_euclid(clip.len() as f64);
        }
        let mut result = [ambience * self.frame.settings.ambience * 0.7; 2];
        for slot in &mut self.voices {
            let Some(voice) = slot else { continue };
            let clip = self.bank.clip(voice.request.clip);
            let release = if voice.release > 0.0 {
                voice.release / REPLACEMENT_RELEASE
            } else {
                1.0
            };
            let value = sample(clip, voice.cursor, false)
                * voice.request.gain
                * release
                * self.frame.settings.actions;
            for (output, pan) in result.iter_mut().zip(voice.request.pan) {
                *output += value * pan;
            }
            voice.cursor += self.step;
            if voice.release > 0.0 {
                voice.release = (voice.release - self.dt).max(0.0);
            }
            if voice.cursor >= clip.len() as f64
                || (voice.replacement.is_some() && voice.release == 0.0)
            {
                *slot = voice.replacement.map(Voice::new);
            }
        }
        let underwater_target = f32::from(self.frame.environment.underwater);
        self.underwater += (underwater_target - self.underwater) * self.underwater_fade;
        let cutoff = 18_000.0 + (850.0 - 18_000.0) * self.underwater;
        let coefficient = 1.0 - (-std::f32::consts::TAU * cutoff * self.dt).exp();
        let master = self.frame.settings.master * self.world_gain * (1.0 - self.underwater * 0.30);
        for (output, filtered) in result.iter_mut().zip(&mut self.filtered) {
            *filtered += (*output - *filtered) * coefficient;
            *output = (*filtered * master).tanh() * 0.9;
        }
        if self.muted || self.frame.settings.muted || self.world_gain == 0.0 {
            [0.0; 2]
        } else {
            result
        }
    }
}

fn action_gain(action: SoundAction) -> f32 {
    match action {
        SoundAction::Step(_) => 0.55,
        SoundAction::Mine(_) => 0.35,
        SoundAction::Break(_) => 0.70,
        SoundAction::Place(_) => 0.60,
        SoundAction::Pickup => 0.50,
        SoundAction::EnterWater => 0.65,
        SoundAction::Swim => 0.35,
    }
}

fn sample(clip: &[f32], cursor: f64, looping: bool) -> f32 {
    let first = cursor as usize;
    if first >= clip.len() {
        return 0.0;
    }
    let next = if first + 1 < clip.len() {
        first + 1
    } else if looping {
        0
    } else {
        first
    };
    clip[first] + (clip[next] - clip[first]) * cursor.fract() as f32
}

#[cfg(test)]
mod tests;
