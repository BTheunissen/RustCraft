struct Lighting {
    matrices: array<mat4x4<f32>, 2>,
    eye: vec4<f32>,
    shadow_settings: vec4<f32>,
    origin: vec4<i32>,
    previous_origin: vec4<i32>,
    probe_settings: vec4<f32>,
    controls: vec4<u32>,
    features: vec4<f32>,
    sun_direction: vec4<f32>,
    sun_radiance: vec4<f32>,
    sky_irradiance: vec4<f32>,
    albedo: array<vec4<f32>, 32>,
    emission: array<vec4<f32>, 32>,
};

struct IrradianceProbe {
    position: vec4<f32>,
    lobes: array<vec4<f32>, 6>,
};

@group(0) @binding(4) var<uniform> lighting: Lighting;
@group(0) @binding(5) var sunlight_depth: texture_depth_2d_array;
@group(0) @binding(6) var sunlight_sampler: sampler_comparison;
@group(0) @binding(7) var lighting_voxels: texture_3d<u32>;
@group(0) @binding(8) var<storage, read> irradiance: array<IrradianceProbe>;

const LOBE_AXES = array<vec3<f32>, 6>(
    vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0),
    vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0),
    vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, -1.0),
);

fn light_voxel(cell: vec3<i32>) -> vec2<u32> {
    let local = cell - lighting.origin.xyz;
    if any(local < vec3<i32>(0)) || any(local >= vec3<i32>(lighting.origin.w)) {
        return vec2<u32>(255u, 0u);
    }
    return textureLoad(lighting_voxels, local, 0).rg;
}

fn light_opaque(block: u32) -> bool {
    return block != 0u && block != 8u;
}

fn light_probe_index(world_cell: vec3<i32>) -> u32 {
    let grid = i32(lighting.controls.z);
    let cell = (world_cell % vec3<i32>(grid) + vec3<i32>(grid)) % vec3<i32>(grid);
    return u32((cell.z * grid + cell.y) * grid + cell.x);
}

fn cascade_shadow(position: vec3<f32>, normal: vec3<f32>, layer: u32, fallback: f32) -> f32 {
    let projected = lighting.matrices[layer] * vec4<f32>(position + normal * 0.025, 1.0);
    let ndc = projected.xyz / projected.w;
    let uv = ndc.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    if any(uv < vec2<f32>(0.002)) || any(uv > vec2<f32>(0.998)) || ndc.z < 0.0 || ndc.z > 1.0 {
        return fallback;
    }
    var visibility = 0.0;
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * lighting.shadow_settings.w;
            visibility += textureSampleCompareLevel(sunlight_depth, sunlight_sampler, uv + offset, i32(layer), ndc.z - 0.00006);
        }
    }
    return visibility / 9.0;
}

fn sunlight_cascade_visibility(position: vec3<f32>, normal: vec3<f32>, fallback: f32) -> f32 {
    if lighting.features.x < 0.5 {
        return 1.0;
    }
    let distance = length(position - lighting.eye.xyz);
    let far_shadow = cascade_shadow(position, normal, 1u, fallback);
    var shadow = far_shadow;
    let near_end = lighting.shadow_settings.x * 0.78;
    if distance < near_end {
        let near_shadow = cascade_shadow(position, normal, 0u, far_shadow);
        shadow = mix(near_shadow, far_shadow, smoothstep(near_end * 0.80, near_end, distance));
    }
    shadow = mix(shadow, fallback, smoothstep(lighting.shadow_settings.y, lighting.shadow_settings.z, distance));
    return shadow;
}

fn sunlight_visibility(position: vec3<f32>, normal: vec3<f32>, fallback: f32) -> f32 {
    let sun = lighting.sun_direction.xyz;
    if max(lighting.sun_radiance.r, max(lighting.sun_radiance.g, lighting.sun_radiance.b)) <= 0.0 || dot(normal, sun) <= 0.0 {
        return 0.0;
    }
    if lighting.features.x < 0.5 {
        return 1.0;
    }
    let shadow = sunlight_cascade_visibility(position, normal, fallback);
    if shadow > 0.001 && fallback < 0.95 {
        // Exposed-face meshes omit buried roofs whose top lies beyond residency.
        // The local voxel check catches those roofs without inventing unloaded casters.
        let start = position + normal * 0.06;
        let local = start - vec3<f32>(lighting.origin.xyz);
        if all(local >= vec3<f32>(0.0)) && all(local < vec3<f32>(f32(lighting.origin.w))) {
            let safe_sun = select(vec3<f32>(0.000001), sun, abs(sun) > vec3<f32>(0.000001));
            var edge = select(-local / safe_sun, (vec3<f32>(f32(lighting.origin.w)) - local) / safe_sun, sun > vec3<f32>(0.0));
            edge = select(vec3<f32>(1e20), edge, abs(sun) > vec3<f32>(0.000001));
            let reach = max(0.0, min(16.0, min(edge.x, min(edge.y, edge.z)) - 0.01));
            if !light_segment_visible(start, start + sun * reach) {
                return 0.0;
            }
        } else if fallback <= 0.001 {
            return 0.0;
        }
    }
    return shadow;
}

// Probe-to-receiver visibility is tested through actual one-block occupancy;
// interpolation never assumes that two neighboring probes share the same room.
fn light_segment_visible(start: vec3<f32>, end: vec3<f32>) -> bool {
    return light_segment_visible_bounded(start, end, 32u);
}

fn light_segment_visible_bounded(start: vec3<f32>, end: vec3<f32>, crossings: u32) -> bool {
    let offset = end - start;
    let distance = length(offset);
    if distance < 0.001 {
        return true;
    }
    let ray = offset / distance;
    var cell = vec3<i32>(floor(start));
    let step = vec3<i32>(sign(ray));
    let delta = 1.0 / max(abs(ray), vec3<f32>(0.000001));
    let boundary = vec3<f32>(cell) + max(vec3<f32>(step), vec3<f32>(0.0));
    var next = (boundary - start) / select(vec3<f32>(0.000001), ray, abs(ray) > vec3<f32>(0.000001));
    next = select(vec3<f32>(1e20), max(next, vec3<f32>(0.0)), abs(ray) > vec3<f32>(0.000001));
    if light_opaque(light_voxel(cell).x) {
        return false;
    }
    for (var iteration = 0u; iteration < crossings; iteration += 1u) {
        let crossing = min(next.x, min(next.y, next.z));
        if crossing >= distance - 0.002 {
            return true;
        }
        let tied = next <= vec3<f32>(crossing + 0.00001);
        if countOneBits(u32(tied.x) | (u32(tied.y) << 1u) | (u32(tied.z) << 2u)) == 1u {
            cell += select(vec3<i32>(0), step, tied);
            next += select(vec3<f32>(0.0), delta, tied);
            if light_opaque(light_voxel(cell).x) { return false; }
            continue;
        }
        for (var mask = 1u; mask < 8u; mask += 1u) {
            var entered = cell;
            var valid = true;
            for (var axis = 0u; axis < 3u; axis += 1u) {
                if (mask & (1u << axis)) != 0u {
                    if !tied[axis] {
                        valid = false;
                        break;
                    }
                    entered[axis] += step[axis];
                }
            }
            if valid && light_opaque(light_voxel(entered).x) {
                return false;
            }
        }
        cell += select(vec3<i32>(0), step, tied);
        next += select(vec3<f32>(0.0), delta, tied);
    }
    return false;
}

fn indirect_light(position: vec3<f32>, normal: vec3<f32>, skylight: f32) -> vec3<f32> {
    let sky_fill = lighting.sky_irradiance.rgb * skylight * (0.28 + 0.72 * max(normal.y * 0.5 + 0.5, 0.0));
    if lighting.features.y < 0.5 {
        return sky_fill;
    }
    let receiver = position + normal * 0.12;
    let local = receiver - vec3<f32>(lighting.origin.xyz);
    let edge = min(min(min(local.x, local.y), local.z), min(min(f32(lighting.origin.w) - local.x, f32(lighting.origin.w) - local.y), f32(lighting.origin.w) - local.z));
    if edge <= 2.0 {
        return sky_fill;
    }
    let coordinate = receiver / lighting.probe_settings.x - vec3<f32>(0.5);
    let base = vec3<i32>(floor(coordinate));
    let fraction = fract(coordinate);
    let grid = i32(lighting.controls.z);
    var total = vec3<f32>(0.0);
    var weight_sum = 0.0;
    for (var z = 0; z < 2; z += 1) {
        for (var y = 0; y < 2; y += 1) {
            for (var x = 0; x < 2; x += 1) {
                let cell = base + vec3<i32>(x, y, z);
                let local_cell = cell - lighting.origin.xyz / i32(lighting.probe_settings.x);
                if any(local_cell < vec3<i32>(0)) || any(local_cell >= vec3<i32>(grid)) {
                    continue;
                }
                let index = light_probe_index(cell);
                let probe = irradiance[index];
                if probe.position.w < 0.5 || !light_segment_visible(receiver, probe.position.xyz) {
                    continue;
                }
                let corner_weight = select(vec3<f32>(1.0) - fraction, fraction, vec3<bool>(x == 1, y == 1, z == 1));
                let weight = corner_weight.x * corner_weight.y * corner_weight.z;
                var value = vec3<f32>(0.0);
                for (var axis = 0u; axis < 6u; axis += 1u) {
                    value += probe.lobes[axis].rgb * max(dot(normal, LOBE_AXES[axis]), 0.0);
                }
                total += value * weight;
                weight_sum += weight;
            }
        }
    }
    if weight_sum <= 0.0001 {
        return sky_fill;
    }
    return mix(sky_fill, total / weight_sum, smoothstep(2.0, 8.0, edge));
}
