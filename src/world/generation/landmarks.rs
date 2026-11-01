//! Read-only diagnostic landmarks in the actual generated world.

use super::{Terrain, hydrology, underground};
use crate::world::{CaveBiome, CaveLandform, CaveLandmark, WaterBiome, WaterLandmark};

impl Terrain {
    /// Finds an above-water view of a real water environment within 640 blocks
    /// of the initial continent. This is a diagnostic, not a player teleport.
    pub fn water_landmark(&self, biome: WaterBiome) -> Option<WaterLandmark> {
        hydrology::water_landmark(self, biome)
    }

    /// Samples a coherent underground climate independently of cave occupancy.
    pub fn cave_biome(&self, x: i32, y: i32, z: i32) -> CaveBiome {
        underground::cave_biome(self.seed, x, y, z)
    }

    /// Finds roomy, dry cave ground near spawn for diagnostic cameras only.
    /// This does not provide a player teleport or alter surface spawning.
    pub fn cave_position(&self, biome: CaveBiome) -> Option<[f32; 3]> {
        underground::cave_position(self, biome)
    }

    /// Finds a dry, jump-clear camera pose viewing an actual nearby cave landform.
    /// This is a diagnostic only; the normal player still spawns on the surface.
    pub fn cave_landmark(&self, landform: CaveLandform) -> Option<CaveLandmark> {
        underground::cave_landmark(self, landform)
    }

    /// Returns the nearest cave family's label for the exploration HUD.
    /// The label describes the local network, not whether this block is carved.
    pub fn cave_landform(&self, x: i32, y: i32, z: i32) -> CaveLandform {
        underground::cave_landform(self.seed, x, y, z)
    }

    /// Finds safe surface ground immediately outside a connected cave mouth.
    /// The returned feet position is intended for diagnostics, not spawning.
    pub fn entrance_position(&self) -> Option<[f32; 3]> {
        underground::entrance_position(self)
    }
}
