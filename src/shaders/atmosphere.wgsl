struct SceneFragment {
    @location(0) color: vec4<f32>,
    // Ray distance, residency-fog transmission, cave classification, air participation.
    @location(1) air: vec4<f32>,
};

const CAVE_FOG_COLOR = vec3<f32>(0.005, 0.007, 0.011);

fn air_medium() -> bool {
    return camera.settings.w < 0.5 && camera.environment.z < 0.5 && camera.phase_settings.x < 0.5;
}

fn air_ray_limit() -> f32 {
    return min(AIR_MAX_DISTANCE, camera.settings.z);
}

fn air_transmittance(distance: f32) -> f32 {
    return exp(-camera.atmosphere.x * clamp(distance, 0.0, air_ray_limit()));
}

fn air_phase(alignment: f32) -> f32 {
    let g = camera.atmosphere.z;
    let denominator = 1.0 + g * g - 2.0 * g * clamp(alignment, -1.0, 1.0);
    let hg = (1.0 - g * g) / pow(denominator, 1.5);
    return 0.25 + 0.75 * min(hg, 3.0);
}

fn residency_fog(distance: f32, cave: f32) -> f32 {
    let cave_end = min(camera.settings.z, 52.0);
    let start = mix(camera.settings.y, min(14.0, cave_end * 0.55), cave);
    let end = mix(camera.settings.z, cave_end, cave);
    return smoothstep(start, end, distance);
}

fn ambient_air(color: vec3<f32>, distance: f32, fog_color: vec3<f32>) -> vec3<f32> {
    if !air_medium() {
        return color;
    }
    let transmission = air_transmittance(distance);
    // The shadow march supplies the sun's reserved share of this same extinction.
    return color * transmission + fog_color * (1.0 - transmission) * (1.0 - camera.atmosphere.y);
}

fn scene_fragment(color: vec4<f32>, distance: f32, fog: f32, cave: f32) -> SceneFragment {
    return SceneFragment(color, vec4<f32>(distance, 1.0 - fog, cave, select(0.0, 1.0, air_medium())));
}
