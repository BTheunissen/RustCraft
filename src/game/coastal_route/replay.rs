//! Replay the accepted voxel route through the real swept-collision controller.

use crate::player::{MovementInput, PLAYER_HEIGHT, Player};
use glam::Vec3;

use super::{CoastalRoute, survey::Survey};

const DT: f32 = 1.0 / 120.0;

pub(super) fn verify(
    survey: &Survey,
    route: &CoastalRoute,
    reverse: bool,
) -> Result<usize, String> {
    let points: Vec<_> = if reverse {
        route.feet.iter().rev().copied().collect()
    } else {
        route.feet.clone()
    };
    let Some(&first) = points.first() else {
        return Err("empty coastal replay".into());
    };
    let mut player = Player::new(Vec3::from_array(first));
    let mut updates = 0;
    for feet in &points[1..] {
        let target = Vec3::from_array(*feet);
        let mut arrived = false;
        for _ in 0..240 {
            let offset = (target - player.position).with_y(0.0);
            if offset.length() < 0.065
                && (target.y - player.position.y).abs() < 0.02
                && player.is_grounded()
            {
                arrived = true;
                break;
            }
            let forward = offset.length() > 0.045;
            if forward {
                player.yaw = offset.x.atan2(-offset.z);
            }
            let jump = player.is_grounded() && target.y > player.position.y + 0.5;
            player.update_with_water(
                MovementInput {
                    forward,
                    jump,
                    ..Default::default()
                },
                DT,
                |x, y, z| survey.block([x, y, z]).is_solid(),
                |x, y, z| survey.water([x, y, z]),
            );
            updates += 1;
            if player.flying
                || player.is_swimming()
                || player.submersion() > 0.0
                || !player.position.is_finite()
                || !survey.clear(player.position, PLAYER_HEIGHT)
            {
                return Err(format!(
                    "invalid walking replay at {:?}, target {target:?}",
                    player.position
                ));
            }
        }
        if !arrived {
            return Err(format!(
                "walking replay blocked at {:?}, target {target:?}, reverse={reverse}",
                player.position
            ));
        }
        player.update_with_water(
            MovementInput::default(),
            DT,
            |x, y, z| survey.block([x, y, z]).is_solid(),
            |x, y, z| survey.water([x, y, z]),
        );
        updates += 1;
    }
    Ok(updates)
}
