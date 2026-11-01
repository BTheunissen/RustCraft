//! Mean fluid occupancy and current metadata shared by simulation and presentation.

/// A water environment, separate from the four terrestrial biomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum WaterBiome {
    Lake = 0,
    River = 1,
    Estuary = 2,
    Ocean = 3,
    Waterfall = 4,
    CavePool = 5,
}

impl WaterBiome {
    /// Water environments in stable numeric-ID order.
    pub const ALL: [Self; 6] = [
        Self::Lake,
        Self::River,
        Self::Estuary,
        Self::Ocean,
        Self::Waterfall,
        Self::CavePool,
    ];

    /// A short exploration-HUD label.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Lake => "Freshwater Lake",
            Self::River => "Freshwater River",
            Self::Estuary => "Brackish Estuary",
            Self::Ocean => "Ocean",
            Self::Waterfall => "Waterfall",
            Self::CavePool => "Cave Pool",
        }
    }
}

/// A mean water surface and its final, pre-cave solid bed in world Y.
///
/// Water occupies `y > bottom && (y as f32) < surface`. The last occupied voxel
/// can be fractional: render its top at `surface`, not at the voxel's full top.
/// Currents are velocities in blocks/second, shared by graphics and swimming.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterColumn {
    pub bottom: i32,
    pub surface: f32,
    pub flow: [f32; 3],
    pub biome: WaterBiome,
    /// Relative marine mixing in `0..=1`, not salt concentration or water color.
    pub salinity: f32,
}

impl WaterColumn {
    /// Depth above the top of the solid bed, in blocks.
    pub fn depth(self) -> f32 {
        self.surface - (self.bottom + 1) as f32
    }

    /// Whether this mean profile occupies a world-Y voxel.
    pub fn contains_y(self, y: i32) -> bool {
        y > self.bottom && (y as f32) < self.surface
    }
}

/// A sample at an actual water voxel; it carries the complete local profile.
pub type WaterSample = WaterColumn;

/// An above-water diagnostic view and the actual wet voxel it represents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterLandmark {
    pub biome: WaterBiome,
    /// Camera/player feet in world coordinates; diagnostics do not change spawn.
    pub position: [f32; 3],
    /// A world-space look point, not a direction vector.
    pub target: [f32; 3],
    pub water: [i32; 3],
}
