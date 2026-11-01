//! Deterministic filtered noise and damped impulses; these are original project assets.

use std::f32::consts::TAU;

use super::catalog::{ACTION_CLIPS, SOURCE_RATE, SURFACES};
use crate::game::SoundSurface;

pub(super) fn clip(index: usize) -> Box<[f32]> {
    if index >= ACTION_CLIPS {
        return ambience(index - ACTION_CLIPS);
    }
    let seconds = match index {
        0..12 => 0.30,
        12..16 => 0.14,
        16..20 => 0.42,
        20..24 => 0.20,
        24 => 0.16,
        25 => 0.65,
        _ => 0.40,
    };
    let length = (seconds * SOURCE_RATE as f32) as usize;
    let mut noise = Noise::new(index as u32 + 1);
    let mut low = 0.0;
    let surface = match index {
        0..12 => SURFACES[index / 3],
        12..24 => SURFACES[(index - 12) % 4],
        _ => SoundSurface::Sand,
    };
    let mut samples = Vec::with_capacity(length);
    for frame in 0..length {
        let time = frame as f32 / SOURCE_RATE as f32;
        let progress = frame as f32 / (length - 1) as f32;
        let attack = (time / 0.003).min(1.0);
        let release = ((1.0 - progress) / 0.08).min(1.0);
        let white = noise.next();
        let coefficient = match surface {
            SoundSurface::Stone => 0.18,
            SoundSurface::Earth => 0.06,
            SoundSurface::Sand => 0.30,
            SoundSurface::Wood => 0.045,
        };
        low += (white - low) * coefficient;
        let decay = (-progress * if index == 25 { 3.5 } else { 7.0 }).exp();
        let value = if index == 24 {
            (TAU * 690.0 * time).sin() * 0.15
                + (TAU * 1035.0 * time).sin() * (time / 0.045).min(1.0) * 0.08
        } else if index >= 25 {
            low * 1.6 + (white - low) * 0.10
        } else {
            let frequency = match surface {
                SoundSurface::Stone => 115.0,
                SoundSurface::Earth => 72.0,
                SoundSurface::Sand => 90.0,
                SoundSurface::Wood => 185.0,
            } * (1.0 + (index % 3) as f32 * 0.055);
            let body = (TAU * frequency * time).sin() * (-time * 50.0).exp();
            low * 1.4 + body * 0.40
        };
        samples.push(value * attack * release * decay);
    }
    normalize(&mut samples, if index == 24 { 0.18 } else { 0.30 });
    samples.into_boxed_slice()
}

fn ambience(index: usize) -> Box<[f32]> {
    let seconds = [24, 31, 29, 37][index];
    let length = seconds * SOURCE_RATE as usize;
    let overlap = SOURCE_RATE as usize;
    let mut samples = Vec::with_capacity(length + overlap);
    let mut noise = Noise::new(index as u32 + 101);
    let mut low = 0.0;
    let mut slow = 0.0;
    for frame in 0..length + overlap {
        let time = frame as f32 / SOURCE_RATE as f32;
        let phase = TAU * frame as f32 / length as f32;
        let white = noise.next();
        low += (white - low) * [0.008, 0.035, 0.16, 0.003][index];
        slow += (low - slow) * 0.001;
        let swell = 0.5
            + 0.24 * (phase * [3.0, 4.0, 7.0, 2.0][index]).sin()
            + 0.13 * (phase * 5.0 + 1.7).sin();
        let value = match index {
            0 => low * 2.8 + slow * 0.5,
            1 => low * (1.0 + swell) * 2.0 + white * swell.powi(3) * 0.08,
            2 => low * 1.4 + (white - low) * 0.12 * swell,
            _ => slow * 3.0 + (TAU * 43.0 * time).sin() * 0.012 * swell,
        };
        samples.push(value * swell);
    }
    // Overlap the ending continuation into the beginning; adjacent seam samples
    // then come from contiguous noise, rather than unrelated random endpoints.
    for frame in 0..overlap {
        let progress = frame as f32 / overlap as f32;
        let weight = progress * progress * (3.0 - 2.0 * progress);
        samples[frame] = samples[length + frame] * (1.0 - weight) + samples[frame] * weight;
    }
    samples.truncate(length);
    let mean = samples.iter().map(|&value| f64::from(value)).sum::<f64>() / length as f64;
    for value in &mut samples {
        *value -= mean as f32;
    }
    normalize(&mut samples, [0.16, 0.22, 0.18, 0.10][index]);
    samples.into_boxed_slice()
}

fn normalize(samples: &mut [f32], peak: f32) {
    let maximum = samples
        .iter()
        .fold(0.0_f32, |maximum, value| maximum.max(value.abs()));
    if maximum > 0.0 {
        for value in samples {
            *value *= peak / maximum;
        }
    }
}

struct Noise(u32);

impl Noise {
    fn new(seed: u32) -> Self {
        Self(seed.wrapping_mul(0x9e37_79b9).max(1))
    }

    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        ((self.0 >> 8) as f32 / 8_388_608.0) - 1.0
    }
}
