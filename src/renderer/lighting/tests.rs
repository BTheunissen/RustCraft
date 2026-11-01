use super::*;

#[test]
fn lighting_layout_matches_wgsl_and_does_not_repurpose_camera_skylight() {
    assert_eq!(size_of::<LightingUniform>(), 1312);
    assert_eq!(std::mem::offset_of!(LightingUniform, eye), 128);
    assert_eq!(std::mem::offset_of!(LightingUniform, origin), 160);
    assert_eq!(std::mem::offset_of!(LightingUniform, previous_origin), 176);
    assert_eq!(std::mem::offset_of!(LightingUniform, controls), 208);
    assert_eq!(std::mem::offset_of!(LightingUniform, sun_direction), 240);
    assert_eq!(std::mem::offset_of!(LightingUniform, sun_radiance), 256);
    assert_eq!(std::mem::offset_of!(LightingUniform, sky_irradiance), 272);
    assert_eq!(std::mem::offset_of!(LightingUniform, albedo), 288);
    assert_eq!(std::mem::offset_of!(LightingUniform, emission), 800);
    assert_eq!(size_of::<super::super::frame::CameraUniform>(), 480);
    for compute in [false, true] {
        let entries = lighting_entries(compute);
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].binding, 4);
        assert_eq!(entries[4].binding, 8);
        assert_eq!(
            entries[4].ty,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage {
                    read_only: !compute
                },
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(4096 * 112)
            }
        );
    }
}
