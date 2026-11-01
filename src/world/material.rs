//! Stable voxel materials and their physical properties.

/// A voxel material. Lava is opaque but, like water, noncollidable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Block {
    Air = 0,
    Grass = 1,
    Dirt = 2,
    Stone = 3,
    Sand = 4,
    Snow = 5,
    Wood = 6,
    Leaves = 7,
    Water = 8,
    Bedrock = 9,
    DeepStone = 10,
    CoalOre = 11,
    IronOre = 12,
    GoldOre = 13,
    Crystal = 14,
    Moss = 15,
    Lava = 16,
    Limestone = 17,
    Slate = 18,
    Basalt = 19,
    Calcite = 20,
    Gravel = 21,
    Clay = 22,
    Planks = 23,
    DressedLimestone = 24,
    StorageCrate = 25,
    Lantern = 26,
}

impl Block {
    /// Number of materials, including air; IDs remain contiguous and append-only.
    pub const COUNT: usize = 27;

    /// All materials in stable numeric-ID order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::Air,
        Self::Grass,
        Self::Dirt,
        Self::Stone,
        Self::Sand,
        Self::Snow,
        Self::Wood,
        Self::Leaves,
        Self::Water,
        Self::Bedrock,
        Self::DeepStone,
        Self::CoalOre,
        Self::IronOre,
        Self::GoldOre,
        Self::Crystal,
        Self::Moss,
        Self::Lava,
        Self::Limestone,
        Self::Slate,
        Self::Basalt,
        Self::Calcite,
        Self::Gravel,
        Self::Clay,
        Self::Planks,
        Self::DressedLimestone,
        Self::StorageCrate,
        Self::Lantern,
    ];

    /// Whether a player should collide with this block.
    pub const fn is_solid(self) -> bool {
        !matches!(self, Self::Air | Self::Water | Self::Lava)
    }

    /// Whether this block occludes the face of an adjacent opaque block.
    pub const fn is_opaque(self) -> bool {
        !matches!(self, Self::Air | Self::Water)
    }

    /// Whether the block belongs to a scenery liquid rather than solid ground.
    pub const fn is_liquid(self) -> bool {
        matches!(self, Self::Water | Self::Lava)
    }
}
