//! Bounded water-surface animation and stylized optics, without fluid simulation.
//!
//! These helpers use the same world-space waves as `shaders/world.wgsl`. They do
//! not change voxel occupancy, the mean water line, or the hydrology current.

use crate::world::{WaterBiome, WaterSample};

/// Maximum displacement of a fully weighted, deep marine surface in blocks.
pub const MAX_WAVE_DISPLACEMENT: f32 = 0.168;

#[derive(Clone, Copy)]
struct Wave {
    direction: [f32; 2],
    frequency: f32,
    speed: f32,
    amplitude: f32,
    phase: f32,
}

// Keep the three low-frequency terms in sync with WATER_WAVES in world.wgsl.
const WAVES: [Wave; 3] = [
    Wave {
        direction: [0.8, 0.6],
        frequency: 0.48,
        speed: 0.73,
        amplitude: 0.096,
        phase: 0.0,
    },
    Wave {
        direction: [-0.6, 0.8],
        frequency: 0.81,
        speed: 1.11,
        amplitude: 0.048,
        phase: 1.9,
    },
    Wave {
        direction: [0.28, -0.96],
        frequency: 1.37,
        speed: 1.49,
        amplitude: 0.024,
        phase: 4.1,
    },
];

/// Underwater absorption/fog parameters, with depth measured below the mean line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterOptics {
    /// Linear RGB fog color. Fresh and marine hues are an art direction, not a
    /// claim that salt alone determines the color of real water.
    pub color: [f32; 3],
    /// Exponential attenuation per world-space block.
    pub density: f32,
    /// Nonnegative camera depth below the mean water surface.
    pub depth: f32,
    /// Sanitized freshwater-to-marine blend in `0..=1`.
    pub salinity: f32,
}

/// Computes camera optics, retaining a freshwater fallback for manual pool poses.
/// The caller's underwater state, not the presence of metadata, enables the fog.
pub fn underwater_optics(sample: Option<WaterSample>, eye_y: f32) -> WaterOptics {
    let salinity = sample.map_or(0.0, |water| finite_clamp(water.salinity, 0.0, 1.0));
    let depth = sample.map_or(0.0, |water| {
        if water.surface.is_finite() && eye_y.is_finite() {
            finite_clamp(water.surface - eye_y, 0.0, 144.0)
        } else {
            0.0
        }
    });
    let fresh = [0.04, 0.16, 0.11];
    let marine = [0.02, 0.12, 0.21];
    let dimming = 1.0 / (1.0 + depth * 0.018);
    WaterOptics {
        color: std::array::from_fn(|axis| mix(fresh[axis], marine[axis], salinity) * dimming),
        density: mix(0.065, 0.04, salinity) * (1.0 + smoothstep(0.0, 12.0, depth) * 0.2),
        depth,
        salinity,
    }
}

/// Returns the geometric-wave weight for open water away from a shoreline.
/// Falling curtains stay fixed; sheltered cave pools have much smaller waves.
pub const fn biome_wave_weight(biome: WaterBiome) -> f32 {
    match biome {
        WaterBiome::Waterfall => 0.0,
        WaterBiome::CavePool => 0.2,
        _ => 1.0,
    }
}

/// Evaluates three waves at world `[x, z]`, matching the water vertex shader.
/// Shared corners must supply shared depth, salinity, and weight to stay sealed.
/// Freshwater displacement is below 0.037 blocks; marine displacement is below
/// [`MAX_WAVE_DISPLACEMENT`]. Invalid inputs produce a stationary surface.
pub fn wave_displacement(
    position: [f32; 2],
    time: f32,
    salinity: f32,
    depth: f32,
    weight: f32,
) -> f32 {
    wave_sample(position, time, salinity, depth, weight).0
}

/// Analytic upward normal of the three geometric waves, before fine ripples.
pub fn wave_normal(
    position: [f32; 2],
    time: f32,
    salinity: f32,
    depth: f32,
    weight: f32,
) -> [f32; 3] {
    let (_, [dx, dz]) = wave_sample(position, time, salinity, depth, weight);
    let inverse_length = (dx * dx + 1.0 + dz * dz).sqrt().recip();
    [-dx * inverse_length, inverse_length, -dz * inverse_length]
}

/// Samples a visual height without moving the hydrology's mean surface.
/// `weight` is the meshed corner weight when available; for an approximate open
/// water camera sample use [`biome_wave_weight`]. Shoreline vertices use zero.
pub fn surface_height(sample: WaterSample, position: [f32; 2], time: f32, weight: f32) -> f32 {
    let surface = if sample.surface.is_finite() {
        sample.surface
    } else {
        0.0
    };
    let bed_top = (i64::from(sample.bottom) + 1) as f32;
    surface + wave_displacement(position, time, sample.salinity, surface - bed_top, weight)
}

/// Current-aligned, advected world coordinates for directional ripple fronts.
/// There is no per-block UV reset. At zero flow a fixed unit direction avoids a
/// division by zero; the shader smoothly fades directional ripples at low speed.
pub fn flow_coordinates(position: [f32; 2], flow: [f32; 2], time: f32) -> [f32; 2] {
    if !position.into_iter().all(f32::is_finite) || !time.is_finite() {
        return [0.0; 2];
    }
    let flow = flow.map(|component| finite_clamp(component, -8.0, 8.0));
    let speed = (flow[0] * flow[0] + flow[1] * flow[1]).sqrt();
    let direction = if speed > 0.0001 {
        flow.map(|component| component / speed)
    } else {
        [0.8, 0.6]
    };
    let advected = std::array::from_fn::<_, 2, _>(|axis| position[axis] - flow[axis] * time);
    let result = [
        advected[0] * direction[0] + advected[1] * direction[1],
        -advected[0] * direction[1] + advected[1] * direction[0],
    ];
    if result.into_iter().all(f32::is_finite) {
        result
    } else {
        [0.0; 2]
    }
}

fn wave_sample(
    position: [f32; 2],
    time: f32,
    salinity: f32,
    depth: f32,
    weight: f32,
) -> (f32, [f32; 2]) {
    if !position.into_iter().all(f32::is_finite) || !time.is_finite() {
        return (0.0, [0.0; 2]);
    }
    let scale = mix(0.22, 1.0, finite_clamp(salinity, 0.0, 1.0))
        * smoothstep(0.0, 2.5, finite_clamp(depth, 0.0, 144.0))
        * finite_clamp(weight, 0.0, 1.0);
    let mut height = 0.0;
    let mut gradient = [0.0; 2];
    for wave in WAVES {
        let phase = (position[0] * wave.direction[0] + position[1] * wave.direction[1])
            * wave.frequency
            - time * wave.speed
            + wave.phase;
        if !phase.is_finite() {
            return (0.0, [0.0; 2]);
        }
        height += wave.amplitude * phase.sin() * scale;
        for (axis, derivative) in gradient.iter_mut().enumerate() {
            *derivative +=
                wave.amplitude * wave.frequency * wave.direction[axis] * phase.cos() * scale;
        }
    }
    (height, gradient)
}

fn finite_clamp(value: f32, low: f32, high: f32) -> f32 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        0.0_f32.clamp(low, high)
    }
}

fn smoothstep(low: f32, high: f32, value: f32) -> f32 {
    let fraction = ((value - low) / (high - low)).clamp(0.0, 1.0);
    fraction * fraction * (3.0 - 2.0 * fraction)
}

fn mix(low: f32, high: f32, fraction: f32) -> f32 {
    low + (high - low) * fraction
}

#[cfg(test)]
mod tests;
