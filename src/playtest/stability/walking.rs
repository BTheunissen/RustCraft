use crate::player::{MovementInput, Player};
use crate::world::{ResidentWorld, WorldView};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};
use std::io;

pub(super) fn route(world: &ResidentWorld, initial: &Player) -> io::Result<Vec<Player>> {
    let headings = [
        initial.yaw,
        0.0,
        -FRAC_PI_2,
        FRAC_PI_2,
        PI,
        -FRAC_PI_4,
        FRAC_PI_4,
        PI - FRAC_PI_4,
        FRAC_PI_4 - PI,
    ];
    for jumping in [false, true] {
        for yaw in headings {
            let mut player = initial.clone();
            player.yaw = yaw;
            player.pitch = -0.18;
            let mut poses = Vec::new();
            for step in 0_u32..240 {
                let before = player.position;
                player.update_with_water(
                    MovementInput {
                        forward: true,
                        jump: jumping && step.is_multiple_of(36),
                        ..Default::default()
                    },
                    1.0 / 60.0,
                    |x, y, z| world.block_at(x, y, z).is_none_or(|block| block.is_solid()),
                    |x, y, z| world.water_at(x, y, z),
                );
                if (player.position - initial.position).with_y(0.0).length() > 16.0 {
                    return Ok(poses);
                }
                if step > 36 && (player.position - before).with_y(0.0).length() < 0.001 {
                    break;
                }
                if step.is_multiple_of(2) {
                    poses.push(player.clone());
                }
            }
        }
    }
    Err(io::Error::other(
        "normal spawn has no bounded forward walking route",
    ))
}

pub(super) fn difference(before: &[u8], after: &[u8], width: u32, height: u32) -> f32 {
    let mut total = 0.0_f64;
    let mut count = 0;
    for y in height / 4..height * 3 / 4 {
        for x in width / 4..width * 3 / 4 {
            let offset = ((y * width + x) * 4) as usize;
            for channel in 0..3 {
                total +=
                    f64::from(before[offset + channel].abs_diff(after[offset + channel])) / 255.0;
                count += 1;
            }
        }
    }
    (total / count.max(1) as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_difference_ignores_alpha_and_outer_margins() {
        let before = vec![0; 8 * 8 * 4];
        let mut after = before.clone();
        after[..8 * 4].fill(255);
        after[(3 * 8 + 3) * 4 + 3] = 255;
        assert_eq!(difference(&before, &after, 8, 8), 0.0);
        after[(3 * 8 + 3) * 4] = 255;
        assert!((difference(&before, &after, 8, 8) - 1.0 / 48.0).abs() < 0.000001);
    }

    #[test]
    fn default_and_seed42_routes_walk_real_ground_crossing_several_anchor_intervals() {
        use crate::world::{Chunk, ChunkCoord, Terrain};
        for terrain in [Terrain::default(), Terrain::new(42)] {
            let initial = Player::new(glam::Vec3::from_array(terrain.spawn_position()));
            let center = ChunkCoord::from_world(initial.position.x, initial.position.z);
            let mut world = ResidentWorld::default();
            for z in -2..=2 {
                for x in -2..=2 {
                    world.insert(Chunk::generate(
                        &terrain,
                        ChunkCoord::new(center.x + x, center.z + z),
                    ));
                }
            }
            let poses = route(&world, &initial).unwrap();
            assert!(poses.len() >= 80);
            assert!(poses.iter().all(|player| !player.flying));
            let last = poses.last().unwrap();
            assert!((last.position - initial.position).with_y(0.0).length() > 15.0);
        }
    }
}
