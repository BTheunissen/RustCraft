use super::*;

#[test]
fn a_short_open_arch_gets_lateral_sky_but_a_long_closed_cave_does_not() {
    let cell = [-17, 23, -1];
    let block = |[x, y, _]: [i32; 3]| {
        if y == 30 && (x + 17).abs() <= 4 {
            Block::Limestone
        } else {
            Block::Air
        }
    };
    let vertical = |[x, _, _]: [i32; 3]| if (x + 17).abs() <= 4 { 0.0 } else { 1.0 };
    assert_eq!(lateral_exposure(cell, 8, block, vertical), 1.0);
    assert_eq!(lateral_exposure(cell, 4, block, vertical), 0.0);
    assert_eq!(lateral_exposure(cell, 8, |_| Block::Air, |_| 0.0), 0.0);
    assert_eq!(lateral_exposure(cell, 8, |_| Block::Stone, |_| 0.0), 0.0);
}

#[test]
fn lateral_sky_respects_solid_walls_water_attenuation_and_bounded_extreme_coordinates() {
    let cell = [0, 20, 0];
    let vertical = |[x, _, z]: [i32; 3]| if x == 2 && z == 0 { 1.0 } else { 0.0 };
    assert!((lateral_exposure(cell, 8, |_| Block::Water, vertical) - 0.81).abs() < 0.000001);
    assert_eq!(
        lateral_exposure(
            cell,
            8,
            |[x, _, _]| if x == 1 { Block::Stone } else { Block::Air },
            vertical
        ),
        0.0
    );
    let mut queries = 0;
    assert_eq!(
        lateral_exposure(
            [i32::MAX, -1, i32::MIN],
            i32::MAX,
            |_| {
                queries += 1;
                Block::Air
            },
            |_| 0.0
        ),
        0.0
    );
    assert!(queries <= 32);
    assert_eq!(
        lateral_exposure(
            cell,
            -1,
            |_| panic!("negative reach must not sample"),
            |_| 0.7
        ),
        0.7
    );
    assert_eq!(lateral_exposure(cell, 0, |_| Block::Air, |_| f32::NAN), 0.0);
}

#[test]
fn exposed_coastal_water_does_not_create_a_black_lower_sky_ring() {
    let eye = Vec3::new(-0.5, 20.62, -0.5);
    let direction = Vec3::new(1.0, -0.2, 0.0);
    let ocean = |_, y, _| Some(if y < 18 { Block::Water } else { Block::Air });
    assert_eq!(camera_cavity_clearance(eye, direction, ocean), 1000.0);
    assert_eq!(
        camera_cavity_clearance(Vec3::NAN, direction, |_, _, _| panic!("invalid eye")),
        1000.0
    );
    assert_eq!(camera_cavity_clearance(eye, Vec3::ZERO, ocean), 1000.0);
    assert_eq!(
        camera_cavity_clearance(eye, direction, |_, _, _| None),
        1000.0
    );
}

#[test]
fn real_enclosed_air_cavity_retains_dark_downward_background() {
    let eye = Vec3::new(0.5, 26.12, 0.5);
    let block = |_, y, z: i32| {
        Some(if y == 23 || y == 29 || z.abs() >= 1 {
            Block::Stone
        } else {
            Block::Air
        })
    };
    let clearance = camera_cavity_clearance(eye, Vec3::new(1.0, -0.3, 0.0), block);
    assert!((clearance + 3.88).abs() < 0.0001);
}

#[test]
fn a_broad_roof_without_enclosed_side_walls_does_not_darken_the_coastal_horizon() {
    let eye = Vec3::new(0.5, 26.12, 0.5);
    let block = |_, y, _| {
        Some(if y == 23 || y == 29 {
            Block::Limestone
        } else {
            Block::Air
        })
    };
    assert_eq!(
        camera_cavity_clearance(eye, Vec3::new(1.0, -0.3, 0.0), block),
        1000.0
    );
}
