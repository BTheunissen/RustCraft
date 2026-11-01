//! Supported, sealed underground liquid basins.

use super::{Block, Bounds, Mask, Node};

#[derive(Clone, Copy)]
pub(super) struct Pool {
    pub(super) x: f64,
    pub(super) z: f64,
    pub(super) radius: f64,
    pub(super) level: i32,
    pub(super) liquid: Block,
}

impl Pool {
    pub(super) fn from_node(node: Node, deep: bool) -> Option<Self> {
        if !deep && !node.hash.is_multiple_of(4) {
            return None;
        }
        let site = node.point(-10.0, node.radius[2] * 0.7 + 7.0, node.floor);
        Some(Self {
            x: site[0],
            z: site[2],
            radius: 3.5 + ((node.hash >> 52) % 3) as f64 * 0.5,
            level: node.floor as i32 + 1,
            liquid: if deep && (node.hash >> 60) & 3 != 0 {
                Block::Lava
            } else {
                Block::Water
            },
        })
    }

    pub(super) fn bounds(self) -> Bounds {
        Bounds {
            min_x: self.x - self.radius - 1.5,
            max_x: self.x + self.radius + 1.5,
            min_z: self.z - self.radius - 1.5,
            max_z: self.z + self.radius + 1.5,
            min_y: f64::from(self.level - 3),
            max_y: f64::from(self.level),
        }
    }

    pub(super) fn column(self, x: f64, z: f64) -> Option<bool> {
        let distance = (x - self.x).powi(2) + (z - self.z).powi(2);
        if distance <= self.radius * self.radius {
            Some(true)
        } else if distance <= (self.radius + 1.5).powi(2) {
            Some(false)
        } else {
            None
        }
    }

    pub(super) fn apply_mask(self, mask: &mut Mask, inside: bool) {
        // The two-block basin has its own foundation and rim. A crossing tunnel
        // cannot leave floating liquid or punch a leak through its low banks.
        mask.range(self.level - 3, self.level - 1, false);
        if inside {
            mask.range(self.level - 2, self.level - 1, true);
        }
    }
}
