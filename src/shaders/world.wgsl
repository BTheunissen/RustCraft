@group(0) @binding(1) var atlas: texture_2d_array<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;
@group(0) @binding(3) var phase_voxels: texture_3d<u32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) material: u32,
    @location(4) light: f32,
    @location(5) water: vec4<f32>,
    @location(6) water_extra: vec4<f32>,
    @location(7) local_light: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) material: u32,
    @location(4) light: f32,
    @location(5) water: vec4<f32>,
    @location(6) water_extra: vec4<f32>,
    @location(7) local_light: vec3<f32>,
};

@vertex
fn vs_world(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
    output.world_position = input.position;
    output.normal = input.normal;
    output.uv = input.uv;
    output.material = input.material;
    output.light = input.light;
    output.water = input.water;
    output.water_extra = input.water_extra;
    output.local_light = input.local_light;
    return output;
}

struct WaterWave {
    direction: vec2<f32>,
    frequency: f32,
    speed: f32,
    amplitude: f32,
    phase: f32,
};

// The CPU helpers in water.rs mirror these bounded, non-simulated waves.
const WATER_WAVES = array<WaterWave, 3>(
    WaterWave(vec2<f32>(0.8, 0.6), 0.48, 0.73, 0.096, 0.0),
    WaterWave(vec2<f32>(-0.6, 0.8), 0.81, 1.11, 0.048, 1.9),
    WaterWave(vec2<f32>(0.28, -0.96), 1.37, 1.49, 0.024, 4.1),
);

fn water_wave_sample(position: vec2<f32>, water: vec4<f32>, weight: f32) -> vec3<f32> {
    let scale = mix(0.22, 1.0, clamp(water.w, 0.0, 1.0))
              * smoothstep(0.0, 2.5, clamp(water.z, 0.0, 144.0))
              * clamp(weight, 0.0, 1.0);
    var result = vec3<f32>(0.0);
    for (var index = 0u; index < 3u; index += 1u) {
        let wave = WATER_WAVES[index];
        let phase = dot(position, wave.direction) * wave.frequency
                  - camera.water_optics.w * wave.speed + wave.phase;
        result.x += wave.amplitude * sin(phase) * scale;
        result.y += wave.amplitude * wave.frequency * wave.direction.x * cos(phase) * scale;
        result.z += wave.amplitude * wave.frequency * wave.direction.y * cos(phase) * scale;
    }
    return result;
}

@vertex
fn vs_water(input: VertexInput) -> VertexOutput {
    let wave = water_wave_sample(input.position.xz, input.water, input.water_extra.w);
    var output: VertexOutput;
    output.world_position = input.position + vec3<f32>(0.0, wave.x, 0.0);
    output.clip_position = camera.view_projection * vec4<f32>(output.world_position, 1.0);
    output.normal = input.normal;
    if input.normal.y > 0.5 {
        output.normal = normalize(vec3<f32>(-wave.y, 1.0, -wave.z));
    }
    output.uv = input.uv;
    output.material = input.material;
    output.light = input.light;
    output.water = input.water;
    output.water_extra = input.water_extra;
    output.local_light = input.local_light;
    return output;
}

fn water_fog_color() -> vec3<f32> {
    return camera.water_fog.rgb * (0.08 + 0.92 * clamp(camera.environment.x, 0.0, 1.0) * camera.sky_horizon.w);
}

fn water_distance_fog(distance: f32) -> f32 {
    // Marine clarity must still end inside the smallest streamed radius.
    let end = max(1.0, min(camera.settings.z, 48.0));
    return max(1.0 - exp(-distance * max(camera.water_fog.w, 0.001)),
               smoothstep(end * 0.55, end, distance));
}

fn water_absorption_tint() -> vec3<f32> {
    return mix(vec3<f32>(0.56, 0.87, 0.64), vec3<f32>(0.47, 0.76, 0.98),
               clamp(camera.water_optics.y, 0.0, 1.0));
}

fn material_color(uv: vec2<f32>, material: u32) -> vec3<f32> {
    let tile_uv = (clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * (camera.lamp.w - 1.0) + 0.5) / camera.lamp.w;
    let texel = clamp(vec2<i32>(floor(tile_uv * camera.lamp.w)), vec2<i32>(0), vec2<i32>(i32(camera.lamp.w) - 1));
    let crisp = textureLoad(atlas, texel, i32(material), 0).rgb;
    let filtered = textureSample(atlas, atlas_sampler, tile_uv, i32(material)).rgb;
    let texel_uv = uv * (camera.lamp.w - 1.0);
    let footprint = max(length(dpdx(texel_uv)), length(dpdy(texel_uv)));
    // Keep nearby pixel art exact; subpixel texels need area filtering instead of
    // nearest-sample crawling. Each array layer owns its entire mip pyramid.
    let filtering = clamp(camera.environment.w, 0.0, 1.0);
    return mix(crisp, filtered, smoothstep(0.5, 1.0, footprint) * filtering);
}

fn shade_world(input: VertexOutput) -> SceneFragment {
    let albedo = material_color(input.uv, input.material);
    let normal = normalize(input.normal);
    // Vertical sky transmission remains atmosphere metadata, not a sun shadow mask.
    let sun = lighting.sun_radiance.rgb * max(dot(normal, camera.sun.xyz), 0.0)
            * sunlight_visibility(input.world_position, normal, input.light);
    let indirect = indirect_light(input.world_position, normal, input.light);
    let offset = input.world_position - camera.eye.xyz;
    let distance = length(offset);
    let ray = offset / max(distance, 0.001);
    let cone = smoothstep(camera.lamp.y, camera.lamp.z, dot(ray, camera.look.xyz));
    let reach = 1.0 - smoothstep(camera.lamp.x * 0.65, camera.lamp.x, distance);
    let diffuse = 0.35 + 0.65 * max(dot(input.normal, -ray), 0.0);
    let lamp = camera.environment.y * cone * reach * 1.5 / (1.0 + distance * distance * 0.025);
    let lamp_darkness = 1.0 - input.light * camera.sky_horizon.w;
    var color = albedo * (vec3<f32>(0.006) + sun + indirect + input.local_light + vec3<f32>(lamp * diffuse * lamp_darkness));
    if input.material == 27u { color += albedo * vec3<f32>(1.4, 0.8, 0.3); }
    // Luminous minerals and lava are scenery; no survival or fluid simulation is implied.
    if input.material == 15u {
        let mineral = smoothstep(0.08, 0.40, max(albedo.r, max(albedo.g, albedo.b)));
        color += (albedo * 2.6 + vec3<f32>(0.08, 0.14, 0.23)) * mineral;
    }
    if input.material == 17u {
        let flow = sin(input.world_position.x * 1.2 + camera.settings.x * 0.5)
                 * sin(input.world_position.z * 1.4 - camera.settings.x * 0.35);
        color = albedo * (4.5 + flow * 0.40) + vec3<f32>(0.45, 0.035, 0.0);
    }

    // Roofed rock fades to cave darkness even while the camera is outside its entrance.
    // Canopy-shadowed trunks/leaves must retain the outdoor atmospheric fog.
    let geological = select(0.0, 1.0, input.material == 3u || input.material >= 10u);
    let cave = clamp(max(1.0 - camera.environment.x, (1.0 - input.light) * geological * (1.0 - input.water_extra.x)), 0.0, 1.0);
    var fog = residency_fog(distance, cave);
    var fog_color = mix(camera.sky_horizon.rgb, CAVE_FOG_COLOR, cave);
    color = ambient_air(color, distance, fog_color);
    if camera.settings.w > 0.5 {
        fog = water_distance_fog(distance);
        fog_color = water_fog_color();
        color *= water_absorption_tint();
    }
    if camera.environment.z > 0.5 {
        fog = 1.0 - exp(-distance * 0.4);
        fog_color = vec3<f32>(0.55, 0.09, 0.008);
    }
    return scene_fragment(vec4<f32>(mix(color, fog_color, fog), 1.0), distance, fog, cave);
}

@fragment
fn fs_world(input: VertexOutput) -> SceneFragment {
    return shade_world(input);
}

fn sky_radiance(ray: vec3<f32>) -> vec3<f32> {
    var color = mix(camera.sky_horizon.rgb, camera.sky_top.rgb, smoothstep(-0.05, 0.72, ray.y));
    let sun_alignment = dot(ray, camera.sun.xyz);
    let sun_visibility = smoothstep(-0.025, 0.025, camera.sun.y);
    color += lighting.sun_radiance.rgb * 0.10 * pow(max(sun_alignment, 0.0), 36.0);
    color += lighting.sun_radiance.rgb * 3.0 * smoothstep(0.9984, 0.9990, sun_alignment) * sun_visibility;
    if ray.y > 0.02 && camera.sky_top.w > 0.0 {
        let star_cell = floor(ray.xz / (ray.y + 0.3) * 180.0);
        let hash = fract(sin(dot(star_cell, vec2<f32>(127.1, 311.7))) * 43758.5453);
        let star = smoothstep(0.995, 0.9998, hash) * smoothstep(0.02, 0.15, ray.y) * camera.sky_top.w;
        color += vec3<f32>(0.40, 0.48, 0.68) * star;
        let moon_alignment = dot(ray, -camera.sun.xyz);
        color = mix(color, vec3<f32>(0.45, 0.53, 0.70), smoothstep(0.9980, 0.9991, moon_alignment) * camera.sky_top.w);
    }
    if ray.y > 0.06 {
        let cloud_point = camera.eye.xz + ray.xz * (110.0 - camera.eye.y) / ray.y;
        let cell = floor((cloud_point + vec2<f32>(camera.settings.x * 0.6, 0.0)) / 14.0);
        let cluster = sin(cell.x * 0.37) + cos(cell.y * 0.43) + sin((cell.x + cell.y) * 0.21);
        let cloud = select(0.0, 1.0, cluster > 1.55) * smoothstep(0.07, 0.24, ray.y);
        color = mix(color, camera.clouds.rgb, cloud * 0.78);
    }
    return color;
}

@fragment
fn fs_water(input: VertexOutput, @builtin(front_facing) front: bool) -> SceneFragment {
    let salinity = clamp(input.water.w, 0.0, 1.0);
    let depth = clamp(input.water.z, 0.0, 144.0);
    let flow = clamp(input.water.xy, vec2<f32>(-8.0), vec2<f32>(8.0));
    let speed = length(flow);
    let direction = select(vec2<f32>(0.8, 0.6), flow / max(speed, 0.0001), speed > 0.0001);
    let advected = input.world_position.xz - flow * camera.water_optics.w;
    let along = dot(advected, direction);
    let across = dot(advected, vec2<f32>(-direction.y, direction.x));
    let current_weight = smoothstep(0.05, 0.4, speed);
    let time = camera.water_optics.w;
    let offset = input.world_position - camera.eye.xyz;
    let distance = length(offset);
    let ray = offset / max(distance, 0.001);
    let view = -ray;
    let broad_a = dot(input.world_position.xz, vec2<f32>(0.31, 0.23)) - time * 0.13;
    let broad_b = dot(input.world_position.xz, vec2<f32>(-0.19, 0.37)) + time * 0.17;
    let warped = input.world_position.xz + vec2<f32>(sin(broad_a), cos(broad_b)) * 0.72;
    let fine_a = dot(warped, vec2<f32>(0.84, 0.54)) * 3.9 - time * 0.98;
    let fine_b = dot(warped, vec2<f32>(-0.4, 0.9165)) * 5.8 + time * 1.21;
    let directional = along * 4.1 + sin(across * 0.73) * 0.74
                    + sin(dot(advected, vec2<f32>(0.16, 0.27))) * 1.35;
    let aa_a = 1.0 - smoothstep(0.8, 2.7, fwidth(fine_a));
    let aa_b = 1.0 - smoothstep(0.8, 2.7, fwidth(fine_b));
    let aa_directional = 1.0 - smoothstep(0.8, 2.7, fwidth(directional));
    let ripple_envelope = (0.38 + 0.62 * (sin(broad_a) * sin(broad_b) * 0.5 + 0.5))
                        * (1.0 - smoothstep(26.0, 110.0, distance));
    // Ripple fronts run across the current. Directional strength tends to zero
    // before the direction becomes undefined, avoiding still-water singularities.
    let fine_gradient = (vec2<f32>(0.84, 0.54) * cos(fine_a) * 0.060 * aa_a
                       + vec2<f32>(-0.4, 0.9165) * cos(fine_b) * 0.036 * aa_b
                       + direction * cos(directional) * 0.072 * current_weight * aa_directional)
                      * ripple_envelope;
    var normal = normalize(input.normal);
    let top_surface = abs(normal.y) > 0.5;
    if top_surface {
        normal = normalize(normal + vec3<f32>(-fine_gradient.x, 0.0, -fine_gradient.y)
                         * (0.55 + 0.45 * salinity));
    }
    let falling = select(0.0, 1.0, input.water_extra.x < -0.15 && input.water_extra.y > 3.5 && input.water_extra.y < 4.5);
    let curtain = input.world_position.y - input.water_extra.x * time;
    let streak_phase = curtain * 5.7 + sin((input.world_position.x + input.world_position.z) * 3.1) * 1.4;
    let curtain_filter = 1.0 - smoothstep(0.8, 2.7, fwidth(streak_phase));
    let streak = sin(streak_phase) * curtain_filter;
    if falling > 0.5 && !top_surface {
        normal = normalize(normal + vec3<f32>(0.035 * cos(curtain * 4.3), streak * 0.085,
                                             0.035 * sin(curtain * 4.3)));
    }
    if !front {
        normal = -normal;
    }

    let facing = clamp(dot(normal, view), 0.0, 1.0);
    var fresnel = 0.02 + 0.98 * pow(1.0 - facing, 5.0);
    let sun_visibility = sunlight_visibility(input.world_position, normal, input.light);
    let daylight = input.light * 0.28 * camera.sky_horizon.w
                 + 1.15 * max(dot(normal, camera.sun.xyz), 0.0) * sun_visibility * camera.sky_horizon.w;
    let indirect = indirect_light(input.world_position, normal, input.light);
    let cone = smoothstep(camera.lamp.y, camera.lamp.z, dot(ray, camera.look.xyz));
    let reach = 1.0 - smoothstep(camera.lamp.x * 0.65, camera.lamp.x, distance);
    let lamp = camera.environment.y * cone * reach * 1.5 / (1.0 + distance * distance * 0.025);
    let lamp_diffuse = 0.35 + 0.65 * max(dot(normal, view), 0.0);
    let illumination = 0.012 + daylight * 0.72 + lamp * lamp_diffuse * (1.0 - input.light * camera.sky_horizon.w)
                     + dot(input.local_light, vec3<f32>(0.2126, 0.7152, 0.0722));
    let fresh = mix(vec3<f32>(0.055, 0.23, 0.135), vec3<f32>(0.012, 0.073, 0.043), 1.0 - exp(-depth * 0.24));
    let marine = mix(vec3<f32>(0.035, 0.25, 0.32), vec3<f32>(0.007, 0.070, 0.17), 1.0 - exp(-depth * 0.18));
    var body = mix(fresh, marine, salinity) * (vec3<f32>(illumination) + indirect * 0.65);
    var sky_visibility = smoothstep(0.06, 0.65, input.light);
    if input.water_extra.y > 4.5 {
        sky_visibility = 0.0;
    }
    // An inexpensive analytic environment reflection, not SSR or a refractive
    // framebuffer. Occluded daylight prevents a sky reflection under cave roofs.
    var reflection = mix(vec3<f32>(0.004, 0.008, 0.011),
                         sky_radiance(reflect(-view, normal)), sky_visibility);
    if camera.settings.w > 0.5 {
        let air_window = smoothstep(0.64, 0.75, facing);
        reflection = mix(water_fog_color(), sky_radiance(-view), air_window * sky_visibility);
        reflection *= exp(-camera.water_optics.x * 0.045);
        fresnel = max(fresnel, 1.0 - smoothstep(0.62, 0.69, facing));
        body = mix(body, reflection, air_window * 0.38);
    }
    var color = mix(body, reflection, fresnel);
    let glint_alignment = max(dot(reflect(-camera.sun.xyz, normal), view), 0.0);
    let glint_filter = 1.0 - smoothstep(0.025, 0.11, fwidth(glint_alignment));
    let sun_glint = pow(glint_alignment, 56.0) * glint_filter;
    color += lighting.sun_radiance.rgb * 1.20 * sun_glint * sky_visibility * sun_visibility * 0.50;
    color += vec3<f32>(0.80, 0.89, 1.0) * pow(facing, 38.0) * lamp * 0.15;

    let foam_band = smoothstep(0.40, 0.88, sin(along * 3.1 + time * 0.45 + sin(across * 1.8)) * 0.5 + 0.5);
    let shore_foam = clamp(input.water_extra.z, 0.0, 1.0) * (0.24 + 0.76 * foam_band);
    let fall_foam = falling * smoothstep(0.53, 0.93, streak * 0.5 + 0.5) * 0.38;
    let foam = clamp(shore_foam * 0.76 + fall_foam, 0.0, 0.9);
    let foam_color = mix(vec3<f32>(0.73, 0.85, 0.74), vec3<f32>(0.78, 0.87, 0.92), salinity);
    color = mix(color, foam_color * illumination, foam);
    var alpha = clamp(0.34 + (1.0 - exp(-depth * 0.19)) * 0.34 + fresnel * 0.28 + foam * 0.22, 0.30, 0.96);
    if camera.settings.w > 0.5 {
        alpha = max(alpha, 0.74 + fresnel * 0.20);
    }

    let cave = clamp(max(1.0 - camera.environment.x, 1.0 - input.light), 0.0, 1.0);
    var fog = residency_fog(distance, cave);
    var fog_color = mix(camera.sky_horizon.rgb, CAVE_FOG_COLOR, cave);
    color = ambient_air(color, distance, fog_color);
    if camera.settings.w > 0.5 {
        color *= water_absorption_tint();
        fog = water_distance_fog(distance);
        fog_color = water_fog_color();
    }
    if camera.environment.z > 0.5 {
        fog = 1.0 - exp(-distance * 0.4);
        fog_color = vec3<f32>(0.55, 0.09, 0.008);
    }
    return scene_fragment(vec4<f32>(mix(color, fog_color, fog), alpha), distance, fog, cave);
}

@fragment
fn fs_phase_back(input: VertexOutput) -> SceneFragment {
    var inward = input;
    inward.normal = -input.normal;
    inward.light = 0.0;
    let shaded = shade_world(inward);
    let minimum = vec3<f32>(0.035, 0.045, 0.055);
    return SceneFragment(vec4<f32>(max(shaded.color.rgb, minimum), 0.23), shaded.air);
}

struct SkyOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) index: u32) -> SkyOutput {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var output: SkyOutput;
    output.ndc = positions[index];
    output.clip_position = vec4<f32>(output.ndc, 0.9999, 1.0);
    return output;
}

fn phase_block(cell: vec3<i32>) -> u32 {
    let local = cell - camera.phase_origin.xyz;
    if any(local < vec3<i32>(0)) || any(local >= vec3<i32>(textureDimensions(phase_voxels))) {
        return 255u;
    }
    return textureLoad(phase_voxels, local, 0).r;
}

fn phase_solid(block: u32) -> bool {
    return block > 0u && block < u32(camera.phase_origin.w) && block != 8u && block != 16u;
}

fn phase_material(block: u32, normal: vec3<f32>) -> u32 {
    if block == 1u {
        return select(select(1u, 2u, normal.y < -0.5), 0u, normal.y > 0.5);
    }
    if block == 6u {
        return select(6u, 7u, abs(normal.y) > 0.5);
    }
    return camera.phase_materials[min(block / 4u, 7u)][block % 4u];
}

fn phase_uv(position: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    if abs(normal.y) > 0.5 {
        return fract(position.xz);
    }
    if abs(normal.x) > 0.5 {
        return vec2<f32>(fract(position.z), 1.0 - fract(position.y));
    }
    return vec2<f32>(fract(position.x), 1.0 - fract(position.y));
}

@fragment
fn fs_phase_scan(input: SkyOutput) -> SceneFragment {
    let ray = camera_ray(input.ndc);
    var cell = vec3<i32>(floor(camera.eye.xyz));
    var block = phase_block(cell);
    if !phase_solid(block) || camera.phase_settings.x < 0.5 {
        return SceneFragment(vec4<f32>(0.0), vec4<f32>(0.0));
    }
    let step = vec3<i32>(sign(ray));
    let delta = 1.0 / max(abs(ray), vec3<f32>(0.000001));
    let boundary = vec3<f32>(cell) + max(vec3<f32>(step), vec3<f32>(0.0));
    var next = (boundary - camera.eye.xyz) / select(vec3<f32>(0.000001), ray, abs(ray) > vec3<f32>(0.000001));
    next = select(vec3<f32>(1e20), max(next, vec3<f32>(0.0)), abs(ray) > vec3<f32>(0.000001));
    var accumulated = vec3<f32>(0.0);
    var opacity = 0.0;
    var layers = 0u;
    // A small front-to-back material sample reveals buried blocks without meshing every solid voxel.
    for (var iteration = 0u; iteration < 96u; iteration += 1u) {
        let distance = min(next.x, min(next.y, next.z));
        if distance > camera.phase_settings.y || layers >= u32(camera.phase_settings.w) {
            break;
        }
        let tied = next <= vec3<f32>(distance + 0.00001);
        var normal = vec3<f32>(0.0);
        if tied.x {
            normal.x = -f32(step.x);
        } else if tied.y {
            normal.y = -f32(step.y);
        } else {
            normal.z = -f32(step.z);
        }
        let position = camera.eye.xyz + ray * distance;
        if distance > 0.02 {
            let uv = phase_uv(position, normal);
            let pixel_span = max(fwidth(uv.x), fwidth(uv.y));
            let tile_uv = (clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * (camera.lamp.w - 1.0) + 0.5) / camera.lamp.w;
            let texel = clamp(vec2<i32>(floor(tile_uv * camera.lamp.w)), vec2<i32>(0), vec2<i32>(i32(camera.lamp.w) - 1));
            let albedo = textureLoad(atlas, texel, i32(phase_material(block, -normal)), 0).rgb;
            let edge_distance = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
            let edge = 1.0 - smoothstep(0.012, max(0.034, pixel_span * 1.5), edge_distance);
            let angle = 0.58 + 0.42 * abs(dot(normal, ray));
            let mineral = select(vec3<f32>(0.0), vec3<f32>(0.02, 0.06, 0.08), block == 14u);
            let tint = mix(albedo * angle + mineral + vec3<f32>(0.006, 0.008, 0.010), vec3<f32>(0.30, 0.47, 0.50), edge * 0.55);
            let alpha = camera.phase_settings.z * (0.45 + 0.55 * exp(-distance * 0.14)) + edge * 0.07;
            accumulated += (1.0 - opacity) * tint * alpha;
            opacity += (1.0 - opacity) * alpha;
            layers += 1u;
        }
        cell += select(vec3<i32>(0), step, tied);
        next += select(vec3<f32>(0.0), delta, tied);
        block = phase_block(cell);
        if !phase_solid(block) {
            break;
        }
    }
    if opacity <= 0.0001 {
        return SceneFragment(vec4<f32>(0.0), vec4<f32>(0.0));
    }
    return SceneFragment(vec4<f32>(accumulated / opacity, opacity), vec4<f32>(0.0));
}

fn background_sky_exposure(ray: vec3<f32>) -> f32 {
    var exposure = clamp(camera.environment.x, 0.0, 1.0);
    if exposure > 0.95 || ray.y <= 0.02 {
        return exposure;
    }
    let local = camera.eye.xyz - vec3<f32>(lighting.origin.xyz);
    if any(local < vec3<f32>(0.0)) || any(local >= vec3<f32>(f32(lighting.origin.w))) {
        return exposure;
    }
    let safe_ray = select(vec3<f32>(0.000001), ray, abs(ray) > vec3<f32>(0.000001));
    var boundary = select(-local / safe_ray, (vec3<f32>(f32(lighting.origin.w)) - local) / safe_ray, ray > vec3<f32>(0.0));
    boundary = select(vec3<f32>(1e20), boundary, abs(ray) > vec3<f32>(0.000001));
    let reach = max(0.0, min(boundary.x, min(boundary.y, boundary.z)) - 0.02);
    let end = camera.eye.xyz + ray * reach;
    let exit = light_voxel(vec3<i32>(floor(end)));
    // A roofed camera can still see genuine outdoor sky through an opening.
    // Full-height transmission at a visible resident exit rejects buried roofs
    // above the local volume, while exact occupancy stops intervening walls.
    if exit.x == 0u && exit.y > 0u
        && light_segment_visible_bounded(camera.eye.xyz + ray * 0.04, end, 144u) {
        exposure = max(exposure, f32(exit.y) / 255.0);
    }
    return exposure;
}

@fragment
fn fs_sky(input: SkyOutput) -> SceneFragment {
    if camera.environment.z > 0.5 {
        return SceneFragment(vec4<f32>(0.55, 0.09, 0.008, 1.0), vec4<f32>(0.0));
    }
    // The water background must match submerged geometry in every direction,
    // before the outdoor lower-atmosphere/cave-mouth adjustment can darken it.
    if camera.settings.w > 0.5 {
        return SceneFragment(vec4<f32>(water_fog_color(), 1.0), vec4<f32>(0.0));
    }
    let ray = camera_ray(input.ndc);
    let exposure = background_sky_exposure(ray);
    var color = sky_radiance(ray);
    color = mix(vec3<f32>(0.005, 0.007, 0.011), color, exposure);
    // Match the fog beyond streamed ground instead of exposing a black ring from outdoor flight.
    var near_ground = 1.0 - smoothstep(3.0, 6.0, camera.look.w);
    // A wide mouth can miss the CPU center-line cavity hint. Classify only the
    // downward background ray that actually enters roofed resident air; open
    // water, canopy foliage, and high flight must still meet outdoor sky fog.
    let local_eye = camera.eye.xyz - vec3<f32>(lighting.origin.xyz);
    if near_ground < 0.5 && ray.y < -0.02 && all(local_eye >= vec3<f32>(0.0)) && all(local_eye < vec3<f32>(f32(lighting.origin.w))) {
        let safe_ray = select(vec3<f32>(0.000001), ray, abs(ray) > vec3<f32>(0.000001));
        var boundary = select(-local_eye / safe_ray, (vec3<f32>(f32(lighting.origin.w)) - local_eye) / safe_ray, ray > vec3<f32>(0.0));
        boundary = select(vec3<f32>(1e20), boundary, abs(ray) > vec3<f32>(0.000001));
        let reach = min(48.0, min(boundary.x, min(boundary.y, boundary.z)) - 0.02);
        for (var index = 1u; index <= 16u; index += 1u) {
            let point = camera.eye.xyz + ray * (f32(index) * reach / 16.0);
            let cell = vec3<i32>(floor(point));
            let voxel = light_voxel(cell);
            if voxel.x == 255u || light_opaque(voxel.x) {
                break;
            }
            if voxel.x == 0u && voxel.y == 0u {
                var geological_sides = 0u;
                for (var side = 0u; side < 4u; side += 1u) {
                    let axis = LOBE_AXES[select(side, side + 2u, side >= 2u)];
                    var enclosed = false;
                    for (var step = 1u; step <= 8u; step += 1u) {
                        let block = light_voxel(cell + vec3<i32>(axis * f32(step))).x;
                        enclosed = enclosed || (block > 0u && block != 6u && block != 7u && block != 8u && block != 255u);
                    }
                    geological_sides += select(0u, 1u, enclosed);
                }
                if geological_sides >= 2u {
                    near_ground = 1.0;
                    break;
                }
            }
        }
    }
    let lower_atmosphere = mix(vec3<f32>(0.005, 0.007, 0.011), camera.sky_horizon.rgb, camera.environment.x * (1.0 - near_ground));
    color = mix(color, lower_atmosphere, smoothstep(0.02, 0.15, -ray.y));
    let cavity = smoothstep(0.02, 0.15, -ray.y) * near_ground;
    let cave = max(1.0 - exposure, cavity);
    // The procedural sky already includes its distant atmosphere. Reserve only the
    // sun share for the shadowed foreground instead of adding a second sky fog.
    let distance = air_ray_limit();
    if air_medium() {
        color -= mix(camera.sky_horizon.rgb, CAVE_FOG_COLOR, cave)
               * (1.0 - air_transmittance(distance)) * camera.atmosphere.y;
    }
    return scene_fragment(vec4<f32>(max(color, vec3<f32>(0.0)), 1.0), distance, 0.0, cave);
}
