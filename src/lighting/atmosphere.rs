//! Clear-air parameters and reference math shared with the shadowed scattering pass.

pub(crate) const AIR_EXTINCTION: f32 = 0.0014;
pub(crate) const SUN_SCATTER_SHARE: f32 = 0.32;
pub(crate) const PHASE_ANISOTROPY: f32 = 0.55;
pub(crate) const MAX_DISTANCE: f32 = 128.0;
pub(crate) const MARCH_STEPS: u32 = 64;

/// Fixed horizon intervals leave completed foreground samples unchanged when depth moves.
#[cfg(test)]
pub(crate) fn march_interval(index: u32, distance: f32, limit: f32) -> Option<[f32; 2]> {
    let horizon = limit.clamp(0.0, MAX_DISTANCE);
    let distance = distance.clamp(0.0, horizon);
    let start = horizon * (index as f32 / MARCH_STEPS as f32).powi(2);
    (index < MARCH_STEPS && start < distance).then(|| {
        let end = (horizon * ((index + 1) as f32 / MARCH_STEPS as f32).powi(2)).min(distance);
        [start, end]
    })
}

pub(crate) fn half_resolution(width: u32, height: u32) -> [u32; 2] {
    [width.max(1).div_ceil(2), height.max(1).div_ceil(2)]
}

pub(crate) fn parameters(enabled: bool, underwater: bool, lava: bool, phasing: bool) -> [f32; 4] {
    [
        AIR_EXTINCTION,
        SUN_SCATTER_SHARE,
        PHASE_ANISOTROPY,
        (enabled && !underwater && !lava && !phasing) as u8 as f32,
    ]
}

#[cfg(test)]
pub(crate) fn transmittance(distance: f32) -> f32 {
    (-AIR_EXTINCTION * distance.clamp(0.0, MAX_DISTANCE)).exp()
}

/// Bounded Henyey–Greenstein forward scattering; its peak cannot make clear air opaque.
#[cfg(test)]
pub(crate) fn phase_weight(alignment: f32) -> f32 {
    let g = PHASE_ANISOTROPY;
    let denominator = 1.0 + g * g - 2.0 * g * alignment.clamp(-1.0, 1.0);
    0.25 + 0.75 * ((1.0 - g * g) / denominator.powf(1.5)).min(3.0)
}

/// Sun-color multiplier for a fully visible ray, before the residency fog transition.
#[cfg(test)]
pub(crate) fn unoccluded_scattering(distance: f32, alignment: f32) -> f32 {
    (1.0 - transmittance(distance)) * SUN_SCATTER_SHARE * phase_weight(alignment)
}

pub(crate) fn shader_constants() -> String {
    format!(
        "const AIR_MAX_DISTANCE: f32 = {MAX_DISTANCE:.1};\nconst AIR_MARCH_STEPS: u32 = {MARCH_STEPS}u;\n"
    )
}

#[cfg(test)]
mod tests;
