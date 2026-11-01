//! Bounded diagnostic readouts of real coastal geometry, not gameplay destinations.

/// A main detached rock body; coordinates describe generated voxels, not atlas IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoastalStack {
    /// World X/Z cell in the seeded main body's footprint.
    pub center: [i32; 2],
    /// Highest solid voxel in the center column, in world Y.
    pub top: i32,
}

/// An actual opening, grounded supports, and connected roof in the coastal ridge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoastalArch {
    /// Center of clear air above mean marine water, not a player destination.
    pub opening: [f32; 3],
    /// Center-column inclusive roof interval, in world Y.
    pub roof: [i32; 2],
    /// World X/Y/Z cells in the two rock supports.
    pub supports: [[i32; 3]; 2],
    /// Supported dry shore camera feet; the separate route survey proves approach.
    pub view: [f32; 3],
}

/// The early marine catchment's coastal region, selected without changing spawn.
#[derive(Clone, Debug, PartialEq)]
pub struct CoastalLandmark {
    /// Inclusive horizontal world-coordinate bounds, including terrain blends.
    pub bounds: [[i32; 2]; 2],
    /// Supported feet on the mainland cliff, with standing headroom.
    pub viewpoint: [f32; 3],
    /// Supported dry feet in the accessible cove.
    pub cove: [f32; 3],
    pub stacks: Vec<CoastalStack>,
    pub arch: CoastalArch,
}
