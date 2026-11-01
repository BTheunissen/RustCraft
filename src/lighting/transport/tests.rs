use crate::lighting::materials::emission;
use crate::lighting::volume::LightVolume;
use crate::world::{Block, MIN_WORLD_Y};
use glam::Vec3;

#[test]
fn accelerated_probe_budgets_wrap_on_complete_batches_and_remain_bounded() {
    use crate::lighting::volume::{PROBE_COUNT, PROBES_PER_FRAME};
    let mut schedule = super::ProbeSchedule::default();
    assert_eq!(schedule.next_with_budget(false, false, 1024), [0, 1024]);
    assert_eq!(schedule.next_with_budget(false, false, 2048), [1024, 2048]);
    assert_eq!(schedule.next_with_budget(false, false, 2048), [3072, 2048]);
    assert_eq!(
        schedule.next_with_budget(false, false, 0),
        [1024, PROBES_PER_FRAME]
    );
    assert_eq!(
        schedule.next_with_budget(false, false, u32::MAX),
        [1536, PROBE_COUNT]
    );
    assert_eq!(
        schedule.next_with_budget(true, false, 2048),
        [0, PROBE_COUNT]
    );
    assert_eq!(
        schedule.next_with_budget(false, true, 2048),
        [0, PROBE_COUNT]
    );
    assert_eq!(schedule.next_with_budget(false, false, 513), [0, 1024]);
}

#[test]
fn bounded_schedule_visits_every_probe_once_and_resets_after_occlusion_edits() {
    use crate::lighting::volume::{PROBE_COUNT, PROBES_PER_FRAME};
    let mut schedule = super::ProbeSchedule::default();
    let mut visited = vec![false; PROBE_COUNT as usize];
    for _ in 0..PROBE_COUNT / PROBES_PER_FRAME {
        let [start, count] = schedule.next(false, false);
        assert_eq!(count, PROBES_PER_FRAME);
        for probe in start..start + count {
            assert!(!visited[probe as usize]);
            visited[probe as usize] = true;
        }
    }
    assert!(visited.into_iter().all(|visited| visited));
    assert_eq!(schedule.next(false, false), [0, PROBES_PER_FRAME]);
    assert_eq!(schedule.next(true, false), [0, PROBE_COUNT]);
    assert_eq!(schedule.next(false, true), [0, PROBE_COUNT]);
    assert_eq!(schedule.next(false, false), [0, PROBES_PER_FRAME]);
    assert_eq!(schedule.next(true, true), [0, PROBE_COUNT]);
    assert_eq!(schedule.next(false, false), [0, PROBES_PER_FRAME]);
}

#[test]
fn toroidal_world_grid_slots_are_unique_and_survive_negative_multi_axis_scrolls() {
    use crate::lighting::volume::{PROBE_COUNT, PROBE_GRID};
    for origin in [[0, 0, 0], [-10, -16, -8], [103, 4, -61]] {
        let mut visited = vec![false; PROBE_COUNT as usize];
        for z in 0..PROBE_GRID as i32 {
            for y in 0..PROBE_GRID as i32 {
                for x in 0..PROBE_GRID as i32 {
                    let cell = [origin[0] + x, origin[1] + y, origin[2] + z];
                    let index = super::probe_index(cell) as usize;
                    assert!(!visited[index]);
                    visited[index] = true;
                    for shift in [[16, 0, 0], [0, -16, 0], [-32, 16, 48]] {
                        assert_eq!(
                            super::probe_index(cell),
                            super::probe_index(
                                std::array::from_fn(|axis| cell[axis] + shift[axis])
                            )
                        );
                    }
                }
            }
        }
        assert!(visited.into_iter().all(|slot| slot));
    }
}

#[test]
fn each_interleaved_budget_updates_every_neighborhood_without_spatial_slabs() {
    use crate::lighting::volume::{PROBE_COUNT, PROBE_GRID, PROBES_PER_FRAME};
    let mut visited = vec![false; PROBE_COUNT as usize];
    for batch in 0..8 {
        let mut neighborhoods = vec![false; PROBES_PER_FRAME as usize];
        for index in batch * PROBES_PER_FRAME..(batch + 1) * PROBES_PER_FRAME {
            let cell = super::update_cell(index);
            assert!(cell.into_iter().all(|axis| axis < PROBE_GRID));
            let address = super::probe_index(cell.map(|axis| axis as i32)) as usize;
            assert!(!visited[address]);
            visited[address] = true;
            let [x, y, z] = cell.map(|axis| axis / 2);
            let address = ((z * 8 + y) * 8 + x) as usize;
            assert!(!neighborhoods[address]);
            neighborhoods[address] = true;
        }
        assert!(neighborhoods.into_iter().all(|slot| slot));
    }
    assert!(visited.into_iter().all(|slot| slot));
}

fn trace(volume: &LightVolume, origin: Vec3, direction: Vec3, limit: f32) -> Option<Block> {
    super::trace_voxel_ray(origin, direction, limit, |cell| {
        volume.sample(cell).map(|sample| sample.0)
    })
}

fn fixture(wall: bool) -> LightVolume {
    let mut volume = LightVolume::default();
    volume.rebuild([-32, -32, -32], |x, z| {
        std::array::from_fn(|index| {
            let y = MIN_WORLD_Y + index as i32;
            Some(if x == -2 && z == -1 && y == -1 {
                Block::Lava
            } else if wall && x == 0 {
                Block::Stone
            } else if x == 4 && z == -1 && y == -1 {
                Block::Crystal
            } else {
                Block::Air
            })
        })
    });
    volume
}

#[test]
fn material_rays_find_emission_and_stop_at_intervening_rock() {
    let volume = fixture(false);
    let eye = Vec3::new(1.5, -0.5, -0.5);
    assert_eq!(trace(&volume, eye, Vec3::NEG_X, 10.0), Some(Block::Lava));
    assert_eq!(trace(&volume, eye, Vec3::X, 10.0), Some(Block::Crystal));
    assert!(emission(trace(&volume, eye, Vec3::X, 10.0).unwrap()).z > 1.0);
    assert_eq!(
        trace(&fixture(true), eye, Vec3::NEG_X, 10.0),
        Some(Block::Stone)
    );
    assert_eq!(trace(&volume, eye, Vec3::X, 1.0), None);
}

#[test]
fn exact_corner_rays_cannot_leak_through_touching_wall_cells() {
    let mut volume = fixture(false);
    volume.rebuild([-32, -32, -32], |x, _| {
        std::array::from_fn(|_| Some(if x == 0 { Block::Stone } else { Block::Air }))
    });
    assert_eq!(
        trace(
            &volume,
            Vec3::new(-0.5, -0.5, -0.5),
            Vec3::new(1.0, 1.0, 1.0),
            10.0
        ),
        Some(Block::Stone)
    );
    assert_eq!(
        trace(&volume, Vec3::new(1.0, -0.5, -0.5), Vec3::NEG_X, 10.0),
        Some(Block::Stone)
    );
    for touched in [
        [0, -1, -1],
        [-1, 0, -1],
        [-1, -1, 0],
        [0, 0, -1],
        [0, -1, 0],
        [-1, 0, 0],
        [0, 0, 0],
    ] {
        assert_eq!(
            super::trace_voxel_ray(Vec3::splat(-0.5), Vec3::ONE, 4.0, |cell| {
                Some(if cell == touched {
                    Block::Stone
                } else {
                    Block::Air
                })
            }),
            Some(Block::Stone)
        );
    }
}

#[test]
fn unknown_space_and_invalid_rays_do_not_become_emissive_sources() {
    let volume = fixture(false);
    for (origin, ray, limit) in [
        (Vec3::NAN, Vec3::X, 10.0),
        (Vec3::ZERO, Vec3::ZERO, 10.0),
        (Vec3::ZERO, Vec3::X, f32::NAN),
        (Vec3::ZERO, Vec3::X, -1.0),
        (Vec3::splat(100.0), Vec3::X, 10.0),
    ] {
        assert_eq!(trace(&volume, origin, ray, limit), None);
    }
    assert_eq!(size_of::<super::IrradianceProbe>(), 112);
    assert_eq!(std::mem::offset_of!(super::IrradianceProbe, lobes), 16);
    assert_eq!(
        super::trace_voxel_ray(Vec3::splat(-0.5), Vec3::ONE, 4.0, |cell| {
            if cell == [-1, 0, -1] {
                None
            } else {
                Some(if cell == [1, 1, 1] {
                    Block::Crystal
                } else {
                    Block::Air
                })
            }
        }),
        None
    );
}
