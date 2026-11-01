//! Bounded native smoke progression, isolated from interactive gameplay.

use super::App;
use crate::player::{MovementInput, Player};
use glam::Vec3;
use std::time::Instant;
use winit::dpi::{LogicalSize, PhysicalSize};

pub(super) const SMOKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

pub(super) struct SmokeState {
    pub(super) step: Option<u32>,
    pub(super) origin: Vec3,
    pub(super) peak: f32,
    pub(super) jump_start: Option<f32>,
    pub(super) jump_peak: f32,
    pub(super) landed: bool,
    pub(super) walk_position: Vec3,
    pub(super) resized: bool,
    pub(super) size_before: Option<PhysicalSize<u32>>,
    pub(super) resize_rendered: bool,
}

impl SmokeState {
    pub(super) fn new(origin: Vec3) -> Self {
        Self {
            step: None,
            origin,
            peak: origin.y,
            jump_start: None,
            jump_peak: origin.y,
            landed: false,
            walk_position: origin,
            resized: false,
            size_before: None,
            resize_rendered: false,
        }
    }

    pub(super) fn verify_motion(&self, player: &Player) -> Result<(), String> {
        if !player.flying
            || !self.resized
            || !self.resize_rendered
            || self.peak < self.origin.y + 2.0
            || !self.landed
            || self
                .jump_start
                .is_none_or(|start| self.jump_peak < start + 0.7)
            || (self.walk_position - self.origin).with_y(0.0).length() < 0.2
        {
            return Err(format!(
                "smoke motion failed: flight={}, resized={}, jump={:.3}, landed={}, walk={:.3}, elevation={:.3}",
                player.flying,
                self.resized,
                self.jump_peak - self.jump_start.unwrap_or(self.jump_peak),
                self.landed,
                (self.walk_position - self.origin).with_y(0.0).length(),
                self.peak - self.origin.y
            ));
        }
        Ok(())
    }
}

impl App {
    pub(super) fn simulate_smoke(&mut self) {
        let Some(step) = self.smoke.step else {
            if self
                .streamer
                .as_ref()
                .is_some_and(|streamer| streamer.pending_count() == 0)
            {
                println!(
                    "Smoke: initial terrain loaded; exercising walk, jump, mouse look, and flight"
                );
                self.smoke.step = Some(0);
                self.smoke.origin = self.game.player.position;
                self.smoke.peak = self.game.player.position.y;
            }
            return;
        };
        let mut input = MovementInput::default();
        if step < 90 {
            input.forward = (55..90).contains(&step);
            input.jump = step == 10;
        } else if step < 210 {
            if step == 90 {
                self.game.toggle_flight();
                self.game.cycle_speed();
            }
            input.forward = true;
            input.jump = true;
            self.game.player.look(-1.0, 0.2);
        }
        if step < 90 {
            self.game.player.look(0.5, -0.02);
        }
        if step == 10 {
            self.smoke.jump_start = Some(self.game.player.position.y);
        }
        if step == 90 {
            self.smoke.walk_position = self.game.player.position;
        }
        if step == 30
            && let Some(window) = &self.window
        {
            self.smoke.size_before = Some(window.inner_size());
            if let Some(size) = window.request_inner_size(LogicalSize::new(1200.0, 760.0)) {
                self.resize(size);
            }
        }
        self.game.update(input, 1.0 / 60.0);
        self.smoke.peak = self.smoke.peak.max(self.game.player.position.y);
        if (10..55).contains(&step) {
            self.smoke.jump_peak = self.smoke.jump_peak.max(self.game.player.position.y);
            if step > 20 && self.game.player.is_grounded() {
                self.smoke.landed = true;
            }
        }
        if step < 240 {
            self.smoke.step = Some(step + 1);
        }
    }
}

pub(super) fn smoke_timed_out(smoke: bool, finished: bool, started: Instant, now: Instant) -> bool {
    smoke && !finished && now.duration_since(started) >= SMOKE_TIMEOUT
}

pub(super) fn size_changed(before: Option<PhysicalSize<u32>>, after: PhysicalSize<u32>) -> bool {
    after.width > 0 && after.height > 0 && before.is_some_and(|before| before != after)
}

#[cfg(test)]
mod tests;
