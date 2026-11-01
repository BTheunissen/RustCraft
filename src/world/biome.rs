//! Surface and underground environments, independent of their generators.

/// The dominant biome of a smoothly blended terrain column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Biome {
    Plains,
    Forest,
    Desert,
    Alpine,
}

impl Biome {
    /// A short display name suitable for a HUD.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Plains => "Plains",
            Self::Forest => "Forest",
            Self::Desert => "Desert",
            Self::Alpine => "Alpine",
        }
    }
}

/// Terrain information, excluding trees, for one horizontal block coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    /// Final terrain/water-bed block index before caves; its top is `height + 1`.
    pub height: i32,
    pub biome: Biome,
}

/// A coherent, three-dimensional underground climate region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaveBiome {
    Rocky,
    Lush,
    Crystal,
}

impl CaveBiome {
    /// A short display name suitable for the exploration HUD.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rocky => "Rocky Caves",
            Self::Lush => "Lush Caves",
            Self::Crystal => "Crystal Caves",
        }
    }
}

/// The geometry family of a cave, independent of its wall-material biome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaveLandform {
    Gallery,
    Rift,
    Cathedral,
    Labyrinth,
}

impl CaveLandform {
    /// A short display name suitable for a HUD or diagnostic image.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Gallery => "Gallery",
            Self::Rift => "Rift",
            Self::Cathedral => "Cathedral",
            Self::Labyrinth => "Labyrinth",
        }
    }
}

/// A diagnostic feet position and a world-space point to look toward.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaveLandmark {
    /// The actual nearby network family represented by this view.
    pub landform: CaveLandform,
    /// Player feet, centered on dry supported ground with jump headroom.
    pub position: [f32; 3],
    /// A world-space view target, not a direction vector.
    pub target: [f32; 3],
}
