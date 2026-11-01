struct LightHit {
    position: vec3<f32>,
    normal: vec3<f32>,
    block: u32,
    sky: f32,
    distance: f32,
    transmission: f32,
};

fn trace_light(start: vec3<f32>, direction: vec3<f32>) -> LightHit {
    var cell = vec3<i32>(floor(start));
    let ray = normalize(direction);
    let step = vec3<i32>(sign(ray));
    let delta = 1.0 / max(abs(ray), vec3<f32>(0.000001));
    let boundary = vec3<f32>(cell) + max(vec3<f32>(step), vec3<f32>(0.0));
    var next = (boundary - start) / select(vec3<f32>(0.000001), ray, abs(ray) > vec3<f32>(0.000001));
    next = select(vec3<f32>(1e20), max(next, vec3<f32>(0.0)), abs(ray) > vec3<f32>(0.000001));
    var sky = f32(light_voxel(cell).y) / 255.0;
    var distance = 0.0;
    var normal = vec3<f32>(0.0);
    var transmission = 1.0;
    for (var iteration = 0u; iteration < 112u; iteration += 1u) {
        let sample = light_voxel(cell);
        if sample.x == 255u {
            let local = cell - lighting.origin.xyz;
            if (any(local < vec3<i32>(0)) || any(local >= vec3<i32>(lighting.origin.w))) && ray.y > 0.0 {
                // Full-height CPU column transmission includes roofs above this local volume.
                return LightHit(start + ray * distance, normal, 0u, sky, distance, transmission);
            }
            return LightHit(start + ray * distance, normal, 255u, 0.0, distance, transmission);
        }
        if light_opaque(sample.x) {
            return LightHit(start + ray * distance, normal, sample.x, sky, distance, transmission);
        }
        sky = f32(sample.y) / 255.0;
        if sample.x == 8u {
            transmission *= 0.96;
        }
        distance = min(next.x, min(next.y, next.z));
        if distance > lighting.probe_settings.z {
            return LightHit(start + ray * lighting.probe_settings.z, normal, 0u, sky, lighting.probe_settings.z, transmission);
        }
        let tied = next <= vec3<f32>(distance + 0.00001);
        if countOneBits(u32(tied.x) | (u32(tied.y) << 1u) | (u32(tied.z) << 2u)) == 1u {
            cell += select(vec3<i32>(0), step, tied);
            next += select(vec3<f32>(0.0), delta, tied);
            normal = vec3<f32>(0.0);
            if tied.x { normal.x = -f32(step.x); }
            else if tied.y { normal.y = -f32(step.y); }
            else { normal.z = -f32(step.z); }
            continue;
        }
        for (var mask = 1u; mask < 8u; mask += 1u) {
            var entered = cell;
            var valid = true;
            var crossed_normal = vec3<f32>(0.0);
            for (var axis = 0u; axis < 3u; axis += 1u) {
                if (mask & (1u << axis)) != 0u {
                    if !tied[axis] {
                        valid = false;
                        break;
                    }
                    entered[axis] += step[axis];
                    crossed_normal = vec3<f32>(0.0);
                    crossed_normal[axis] = -f32(step[axis]);
                }
            }
            if valid {
                let block = light_voxel(entered).x;
                if block == 255u {
                    return LightHit(start + ray * distance, crossed_normal, 255u, 0.0, distance, transmission);
                }
                if light_opaque(block) {
                    return LightHit(start + ray * distance, crossed_normal, block, sky, distance, transmission);
                }
            }
        }
        cell += select(vec3<i32>(0), step, tied);
        next += select(vec3<f32>(0.0), delta, tied);
        normal = vec3<f32>(0.0);
        if tied.x { normal.x = -f32(step.x); }
        else if tied.y { normal.y = -f32(step.y); }
        else { normal.z = -f32(step.z); }
    }
    return LightHit(start, normal, 255u, 0.0, 0.0, 0.0);
}

fn ray_radiance(start: vec3<f32>, direction: vec3<f32>) -> vec3<f32> {
    let hit = trace_light(start, direction);
    if hit.block == 255u {
        return vec3<f32>(0.0);
    }
    if hit.block == 0u {
        return lighting.sky_irradiance.rgb * hit.sky * hit.transmission * (0.40 + 0.60 * max(direction.y, 0.0));
    }
    let attenuation = 1.0 / (1.0 + hit.distance * hit.distance * 0.035);
    let emissive = lighting.emission[min(hit.block, 31u)].rgb * lighting.features.z * attenuation;
    let sun = lighting.sun_radiance.rgb * max(dot(hit.normal, lighting.sun_direction.xyz), 0.0)
            * sunlight_visibility(hit.position, hit.normal, hit.sky);
    let diffuse = lighting.sky_irradiance.rgb * hit.sky * 0.55 + sun;
    let bounce = lighting.albedo[min(hit.block, 31u)].rgb * diffuse * 0.55 * lighting.features.w;
    return (emissive + bounce) * hit.transmission;
}

fn probe_position(nominal: vec3<f32>) -> vec4<f32> {
    if !light_opaque(light_voxel(vec3<i32>(floor(nominal))).x) {
        return vec4<f32>(nominal, 1.0);
    }
    // Relocation keeps a coarse grid useful along cave walls without storing light in rock.
    for (var distance = 1u; distance <= 2u; distance += 1u) {
        for (var axis = 0u; axis < 6u; axis += 1u) {
            let candidate = nominal + LOBE_AXES[axis] * f32(distance);
            if !light_opaque(light_voxel(vec3<i32>(floor(candidate))).x) {
                return vec4<f32>(candidate, 1.0);
            }
        }
    }
    return vec4<f32>(nominal, 0.0);
}

@compute @workgroup_size(64)
fn update_probes(@builtin(global_invocation_id) invocation: vec3<u32>) {
    if invocation.x >= lighting.controls.y {
        return;
    }
    let window_index = (lighting.controls.x + invocation.x) % lighting.controls.w;
    let grid = lighting.controls.z;
    let coarse = grid / 2u;
    let batch = coarse * coarse * coarse;
    let phase = window_index / batch;
    let block = window_index % batch;
    // One probe from every 2x2x2 neighborhood each frame, not two moving Z slabs.
    let cell = vec3<u32>(block % coarse, (block / coarse) % coarse, block / (coarse * coarse)) * 2u
             + vec3<u32>(phase & 1u, (phase >> 1u) & 1u, (phase >> 2u) & 1u);
    let world_cell = lighting.origin.xyz / i32(lighting.probe_settings.x) + vec3<i32>(cell);
    let nominal = (vec3<f32>(world_cell) + vec3<f32>(0.5)) * lighting.probe_settings.x;
    if lighting.previous_origin.w > 0 {
        let previous = nominal - vec3<f32>(lighting.previous_origin.xyz);
        let retained = all(previous >= vec3<f32>(0.0)) && all(previous < vec3<f32>(f32(lighting.origin.w)));
        // Retain overlapping world probes. Populate the entire entering shell before
        // drawing, plus the ordinary budget; no camera shift can erase indoor light.
        if retained && invocation.x >= u32(lighting.probe_settings.w) {
            return;
        }
    }
    let index = light_probe_index(world_cell);
    let position = probe_position(nominal);
    var probe: IrradianceProbe;
    probe.position = position;
    for (var axis = 0u; axis < 6u; axis += 1u) {
        var incoming = vec3<f32>(0.0);
        if position.w > 0.5 {
            let direction = LOBE_AXES[axis];
            let tangent = LOBE_AXES[(axis + 2u) % 6u];
            let bitangent = cross(direction, tangent);
            incoming = ray_radiance(position.xyz, direction) * 0.28;
            incoming += ray_radiance(position.xyz, normalize(direction + tangent * 0.65)) * 0.18;
            incoming += ray_radiance(position.xyz, normalize(direction - tangent * 0.65)) * 0.18;
            incoming += ray_radiance(position.xyz, normalize(direction + bitangent * 0.65)) * 0.18;
            incoming += ray_radiance(position.xyz, normalize(direction - bitangent * 0.65)) * 0.18;
        }
        probe.lobes[axis] = vec4<f32>(incoming, 0.0);
    }
    irradiance[index] = probe;
}
