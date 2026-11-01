@group(0) @binding(0) var<uniform> settings: vec4<f32>;
@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var bloom: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;

struct PostVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_post(@builtin(vertex_index) index: u32) -> PostVertex {
    let positions = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var output: PostVertex;
    output.position = vec4<f32>(positions[index], 0.0, 1.0);
    output.uv = positions[index] * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    return output;
}

@fragment
fn fs_bright(input: PostVertex) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(scene));
    var color = vec3<f32>(0.0);
    for (var y = -1; y <= 1; y += 2) {
        for (var x = -1; x <= 1; x += 2) {
            color += textureSampleLevel(scene, linear_sampler, input.uv + vec2<f32>(f32(x), f32(y)) * texel, 0.0).rgb * 0.25;
        }
    }
    let brightness = max(color.r, max(color.g, color.b));
    let contribution = max(0.0, brightness - 1.1) / max(brightness, 0.001);
    return vec4<f32>(color * contribution, 1.0);
}

@fragment
fn fs_blur(input: PostVertex) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(scene));
    var color = vec3<f32>(0.0);
    var weight_sum = 0.0;
    for (var y = -2; y <= 2; y += 1) {
        for (var x = -2; x <= 2; x += 1) {
            let weight = exp(-f32(x * x + y * y) * 0.35);
            color += textureSampleLevel(scene, linear_sampler, input.uv + vec2<f32>(f32(x), f32(y)) * texel, 0.0).rgb * weight;
            weight_sum += weight;
        }
    }
    return vec4<f32>(color / weight_sum, 1.0);
}

fn tone_map(color: vec3<f32>) -> vec3<f32> {
    let x = max(color, vec3<f32>(0.0)) * settings.x;
    return clamp((x * (x * 2.51 + vec3<f32>(0.03))) / (x * (x * 2.43 + vec3<f32>(0.59)) + vec3<f32>(0.14)), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn encode_srgb(color: vec3<f32>) -> vec3<f32> {
    return select(color * 12.92, 1.055 * pow(color, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055), color > vec3<f32>(0.0031308));
}

@fragment
fn fs_composite(input: PostVertex) -> @location(0) vec4<f32> {
    let color = textureSampleLevel(scene, linear_sampler, input.uv, 0.0).rgb
              + textureSampleLevel(bloom, linear_sampler, input.uv, 0.0).rgb * settings.y;
    var mapped = tone_map(color);
    if settings.z > 0.5 {
        mapped = encode_srgb(mapped);
    }
    return vec4<f32>(mapped, 1.0);
}
