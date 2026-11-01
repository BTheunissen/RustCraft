@group(0) @binding(1) var air_guide: texture_2d<f32>;
@group(1) @binding(0) var scattered_air: texture_2d<f32>;

struct AirVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_air(@builtin(vertex_index) index: u32) -> AirVertex {
    let positions = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return AirVertex(vec4<f32>(positions[index], 0.0, 1.0), positions[index] * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5));
}

fn air_ray(uv: vec2<f32>) -> vec3<f32> {
    return camera_ray(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0));
}

fn air_sun_visibility(position: vec3<f32>, cave: f32) -> f32 {
    if lighting.sun_direction.y <= 0.0 { return 0.0; }
    let local = position - vec3<f32>(lighting.origin.xyz);
    let inside = all(local >= vec3<f32>(0.0)) && all(local < vec3<f32>(f32(lighting.origin.w)));
    var fallback = clamp(camera.environment.x, 0.0, 1.0);
    if inside {
        let voxel = light_voxel(vec3<i32>(floor(position)));
        // This pass scatters in known air, never in water, solid voxels, or unloaded data.
        if voxel.x != 0u {
            return 0.0;
        }
        fallback = f32(voxel.y) / 255.0;
    } else if cave > 0.95 || camera.environment.x < 0.05 {
        return 0.0;
    }
    let shadow = sunlight_cascade_visibility(position, vec3<f32>(0.0), fallback);
    if shadow <= 0.001 {
        return 0.0;
    }
    if inside && fallback < 0.95 {
        let sun = lighting.sun_direction.xyz;
        let safe_sun = select(vec3<f32>(0.000001), sun, abs(sun) > vec3<f32>(0.000001));
        var edge = select(-local / safe_sun, (vec3<f32>(f32(lighting.origin.w)) - local) / safe_sun, sun > vec3<f32>(0.0));
        edge = select(vec3<f32>(1e20), edge, abs(sun) > vec3<f32>(0.000001));
        let to_boundary = min(edge.x, min(edge.y, edge.z)) - 0.01;
        // Fully roofed columns need to reach an actual exit, not arbitrarily stop
        // before a sunlit aperture and mistake the remaining roof for enclosure.
        let reach = max(0.0, select(min(16.0, to_boundary), to_boundary, fallback < 0.001));
        let end = position + sun * reach;
        if !light_segment_visible_bounded(position, end, 144u) {
            return 0.0;
        }
        // A buried roof can be above both the local Y range and the exposed mesh.
        // Full-height transmission at the ray end keeps that unknown route dark.
        if fallback < 0.001 && light_voxel(vec3<i32>(floor(end))).y == 0u {
            return 0.0;
        }
    }
    return shadow;
}

fn march_air(uv: vec2<f32>, guide: vec4<f32>) -> vec4<f32> {
    if guide.w < 0.5 || camera.atmosphere.w < 0.5 || lighting.sun_direction.y <= 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, guide.x);
    }
    let ray = air_ray(uv);
    // Stay in front of the receiver even when half-float distance rounded upward.
    let distance = min(max(0.0, guide.x - max(0.025, guide.x * 0.001)), air_ray_limit());
    let source = lighting.sun_radiance.rgb * camera.atmosphere.y * air_phase(dot(ray, lighting.sun_direction.xyz));
    var scattering = 0.0;
    for (var index = 0u; index < AIR_MARCH_STEPS; index += 1u) {
        // The lattice depends on the active horizon, not a changing receiver depth.
        // Otherwise a tiny depth/2x2 selection change moves every visibility sample
        // along the ray and makes entire beams pulse rather than just their tail.
        let a = f32(index) / f32(AIR_MARCH_STEPS);
        let b = f32(index + 1u) / f32(AIR_MARCH_STEPS);
        let start = air_ray_limit() * a * a;
        if start >= distance {
            break;
        }
        let end = min(distance, air_ray_limit() * b * b);
        let midpoint = (start + end) * 0.5;
        let visibility = air_sun_visibility(camera.eye.xyz + ray * midpoint, guide.z);
        let extinction = air_transmittance(start) - air_transmittance(end);
        scattering += extinction * visibility * (1.0 - residency_fog(midpoint, guide.z));
    }
    return vec4<f32>(source * scattering, guide.x);
}

@fragment
fn fs_march_air(input: AirVertex) -> @location(0) vec4<f32> {
    let dimensions = vec2<i32>(textureDimensions(air_guide));
    let base = vec2<i32>(input.position.xy) * 2;
    var guide = textureLoad(air_guide, min(base, dimensions - vec2<i32>(1)), 0);
    // Minimum depth prevents a background ray from starting behind a block edge.
    for (var y = 0; y < 2; y += 1) {
        for (var x = 0; x < 2; x += 1) {
            let pixel = min(base + vec2<i32>(x, y), dimensions - vec2<i32>(1));
            let candidate = textureLoad(air_guide, pixel, 0);
            if candidate.x < guide.x {
                guide = candidate;
            }
        }
    }
    // Only depth selection may change within the footprint; its ray stays centered.
    let center = min(vec2<f32>(base) + vec2<f32>(1.0), vec2<f32>(dimensions) - vec2<f32>(0.5));
    let uv = center / vec2<f32>(dimensions);
    return march_air(uv, guide);
}

@fragment
fn fs_composite_air(input: AirVertex) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(input.position.xy);
    let guide = textureLoad(air_guide, pixel, 0);
    if guide.w < 0.5 {
        return vec4<f32>(0.0);
    }
    if camera.atmosphere.w < 0.5 {
        // Diagnostic disable restores unshadowed ambient fog, not an extra fog layer.
        let ambient = mix(camera.sky_horizon.rgb, CAVE_FOG_COLOR, guide.z);
        return vec4<f32>(ambient * (1.0 - air_transmittance(guide.x)) * camera.atmosphere.y * guide.y, 0.0);
    }
    if camera.atmosphere.w > 1.5 {
        let uv = input.position.xy / vec2<f32>(textureDimensions(air_guide));
        return vec4<f32>(march_air(uv, guide).rgb, 0.0);
    }
    let dimensions = vec2<i32>(textureDimensions(scattered_air));
    // Half-resolution samples represent 2x2 footprint centers, not their top-left pixel.
    let coordinate = input.position.xy * 0.5 - vec2<f32>(0.5);
    let base = vec2<i32>(floor(coordinate));
    var scattering = vec3<f32>(0.0);
    var weight_sum = 0.0;
    let tolerance = max(0.10, guide.x * 0.02);
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let sample_pixel = clamp(base + vec2<i32>(x, y), vec2<i32>(0), dimensions - vec2<i32>(1));
            let sample = textureLoad(scattered_air, sample_pixel, 0);
            let error = abs(sample.w - guide.x) / tolerance;
            if error > 2.0 {
                continue;
            }
            let offset = vec2<f32>(sample_pixel) - coordinate;
            let weight = exp(-dot(offset, offset) * 0.85 - error * error * 2.0);
            scattering += sample.rgb * weight;
            weight_sum += weight;
        }
    }
    if weight_sum > 0.00001 {
        scattering /= weight_sum;
    } else {
        // A one-pixel silhouette may have no compatible half-resolution sample.
        // Re-march only that edge instead of importing background light or a halo.
        let uv = (vec2<f32>(pixel) + vec2<f32>(0.5)) / vec2<f32>(textureDimensions(air_guide));
        scattering = march_air(uv, guide).rgb;
    }
    // Foreground air remains visible even when the receiver has faded completely
    // into cave/residency fog; that transmission is integrated along the march.
    return vec4<f32>(scattering, 0.0);
}
