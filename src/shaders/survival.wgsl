// Appended to world.wgsl so dropped blocks use exactly its atlas, lamps, and fog.
struct Survival {
    hand_projection: mat4x4<f32>,
    target_model: mat4x4<f32>,
    mining: vec4<f32>,
};

@group(1) @binding(0) var<uniform> survival: Survival;

struct CubeInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) face: u32,
};

struct InstanceInput {
    @location(4) model_x: vec4<f32>,
    @location(5) model_y: vec4<f32>,
    @location(6) model_z: vec4<f32>,
    @location(7) model_w: vec4<f32>,
    @location(8) materials: vec4<u32>,
    @location(9) light: f32,
};

fn instance_model(instance: InstanceInput) -> mat4x4<f32> {
    return mat4x4<f32>(instance.model_x, instance.model_y, instance.model_z, instance.model_w);
}

@vertex
fn vs_pickup(cube: CubeInput, instance: InstanceInput) -> VertexOutput {
    let model = instance_model(instance);
    var output: VertexOutput;
    output.world_position = (model * vec4<f32>(cube.position, 1.0)).xyz;
    output.clip_position = camera.view_projection * vec4<f32>(output.world_position, 1.0);
    output.normal = normalize((model * vec4<f32>(cube.normal, 0.0)).xyz);
    output.uv = cube.uv;
    output.material = instance.materials[cube.face];
    output.light = instance.light;
    output.water = vec4<f32>(0.0);
    output.water_extra = vec4<f32>(0.0);
    output.local_light = vec3<f32>(0.0);
    return output;
}

struct HandOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) material: u32,
    @location(3) @interpolate(flat) appearance: u32,
};

@vertex
fn vs_hand(cube: CubeInput, instance: InstanceInput) -> HandOutput {
    let model = instance_model(instance);
    var output: HandOutput;
    output.clip_position = survival.hand_projection * model * vec4<f32>(cube.position, 1.0);
    output.normal = normalize((model * vec4<f32>(cube.normal, 0.0)).xyz);
    output.uv = cube.uv;
    output.material = instance.materials[cube.face];
    output.appearance = instance.materials.w;
    return output;
}

@fragment
fn fs_hand(input: HandOutput) -> @location(0) vec4<f32> {
    var albedo = material_color(input.uv, input.material);
    let pixel = floor(input.uv * 16.0);
    let grain = fract(sin(dot(pixel, vec2<f32>(12.9898, 78.233))) * 43758.5453) - 0.5;
    if input.appearance == 1u {
        albedo = vec3<f32>(0.63, 0.36, 0.22) + grain * 0.045;
        let knuckle = select(0.0, 0.035, pixel.y < 3.0 && (u32(pixel.x) % 4u) == 0u);
        albedo -= vec3<f32>(knuckle);
    }
    if input.appearance == 2u {
        let cuff = select(0.0, 0.08, pixel.y < 2.0);
        albedo = vec3<f32>(0.055, 0.22, 0.30) + grain * 0.028 - cuff * vec3<f32>(0.3, 0.6, 0.7);
    }
    let directional = 0.58 + 0.42 * max(dot(input.normal, normalize(vec3<f32>(-0.45, 0.72, 0.60))), 0.0);
    let sky = clamp(camera.environment.x, 0.0, 1.0);
    let illumination = 0.24 + 0.60 * sky * camera.sky_horizon.w
                     - 0.16 * sky * (1.0 - camera.sky_horizon.w) + 0.18 * camera.environment.y;
    var color = albedo * illumination * directional;
    if input.appearance == 0u && input.material == 15u {
        let mineral = smoothstep(0.08, 0.40, max(albedo.r, max(albedo.g, albedo.b)));
        color += (albedo * 2.6 + vec3<f32>(0.08, 0.14, 0.23)) * mineral;
    }
    if input.appearance == 0u && input.material == 27u { color += albedo * vec3<f32>(1.4, 0.8, 0.3); }
    if camera.settings.w > 0.5 {
        color *= water_absorption_tint();
    }
    return vec4<f32>(color, 1.0);
}

struct TargetOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_target(cube: CubeInput) -> TargetOutput {
    var output: TargetOutput;
    output.clip_position = camera.view_projection * survival.target_model * vec4<f32>(cube.position, 1.0);
    output.uv = cube.uv;
    return output;
}

fn crack_distance(point: vec2<f32>, start: vec2<f32>, end: vec2<f32>) -> f32 {
    let segment = end - start;
    let along = clamp(dot(point - start, segment) / dot(segment, segment), 0.0, 1.0);
    return length(point - start - segment * along);
}

@fragment
fn fs_target(input: TargetOutput) -> @location(0) vec4<f32> {
    let uv = clamp(input.uv, vec2<f32>(0.0), vec2<f32>(1.0));
    let aa = max(0.0008, max(fwidth(uv.x), fwidth(uv.y)) * 0.65);
    let edge_distance = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    let edge = 1.0 - smoothstep(0.006, 0.006 + aa, edge_distance);
    var fracture = 0.0;
    if survival.mining.x > 0.0 {
        for (var index = 0u; index < 24u; index += 1u) {
            let segment = CRACK_SEGMENTS[index];
            let activation = smoothstep(segment.onset, segment.onset + 0.16, survival.mining.x);
            let distance = crack_distance(uv, segment.start, segment.end);
            let mask = 1.0 - smoothstep(survival.mining.y, survival.mining.y + aa, distance);
            fracture = max(fracture, mask * activation);
        }
    }
    let alpha = max(edge * 0.82, fracture * survival.mining.z);
    let color = mix(vec3<f32>(0.006, 0.008, 0.012), vec3<f32>(0.78, 0.86, 0.94), edge);
    // Only the narrow border and jagged branches blend; the atlas remains visible between them.
    return vec4<f32>(color, alpha);
}
