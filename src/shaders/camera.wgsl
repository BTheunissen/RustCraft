struct Camera {
    view_projection: mat4x4<f32>,
    inverse_ray_projection: mat4x4<f32>,
    eye: vec4<f32>,
    sky_top: vec4<f32>,
    sky_horizon: vec4<f32>,
    sun: vec4<f32>,
    settings: vec4<f32>,
    look: vec4<f32>,
    environment: vec4<f32>, // Sky exposure, headlamp, lava, minification filtering.
    lamp: vec4<f32>,
    phase_origin: vec4<i32>,
    phase_settings: vec4<f32>,
    phase_materials: array<vec4<u32>, 8>,
    water_fog: vec4<f32>,
    water_optics: vec4<f32>,
    atmosphere: vec4<f32>,
    clouds: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;

fn camera_ray(ndc: vec2<f32>) -> vec3<f32> {
    // Direction reconstruction has no eye translation or near/far divide to
    // amplify matrix rounding into camera-dependent voxel/shadow sample jitter.
    return normalize((camera.inverse_ray_projection * vec4<f32>(ndc, 1.0, 1.0)).xyz);
}
