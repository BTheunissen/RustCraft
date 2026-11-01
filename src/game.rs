//! Renderer-independent creative exploration and survival-building sessions.

use glam::Vec3;

mod environment;
pub use environment::Environment;
mod feedback;
pub use feedback::{MAX_SOUND_EVENTS, SoundAction, SoundEnvironment, SoundEvent, SoundSurface};
mod bounds;
mod coastal_route;
pub use coastal_route::{CoastalRoute, CoastalRoutes, survey_coastal_routes};
mod crafting;
mod interaction;
mod inventory;
mod item;
pub use crafting::{CraftError, RECIPES, Recipe, RecipeId};
mod mode;
mod pickup;
mod pickup_physics;
mod raycast;
mod recovery;
mod snapshot;
mod storage;
mod time;
pub use storage::{CRATE_SLOTS, MAX_CRATES, SavedCrate, StorageSlot, StorageView};
pub(crate) use time::parse_hour;
pub use time::{DAY_NIGHT_SECONDS, MAX_TIME_SPEED, TimeCommandResult, WorldTime};

pub use inventory::{HOTBAR_SLOTS, INVENTORY_SLOTS, Inventory, MAX_STACK, block_name};
pub use item::{Item, ItemStack, Tool};
pub use mode::GameMode;
pub use pickup::Pickup;
pub use raycast::{BlockTarget, INTERACTION_REACH, raycast};
pub use snapshot::{GameSnapshot, SavedPickup, SavedPlayer};
pub(crate) use snapshot::{MAX_SAVED_EDITS, MAX_SAVED_PICKUPS};

use crate::player::{MovementInput, Player, SpeedMultiplier};
use crate::world::{MIN_WORLD_Y, ResidentWorld, SampledWorld, Terrain, WorldView};

/// Gameplay owns terrain, CPU residency, and player state; native/GPU adapters borrow them.
pub struct Game {
    pub(crate) terrain: Terrain,
    pub(crate) world: ResidentWorld,
    pub(crate) player: Player,
    pub(crate) headlamp: bool,
    mode: GameMode,
    original_spawn: Vec3,
    speed: SpeedMultiplier,
    time: WorldTime,
    exposure: crate::lighting::ExposureCache,
    exposure_revision: u64,
    outdoor_roof: Option<([i32; 3], bool)>,
    coastal_exposure: Option<environment::overhang::CachedExposure>,
    inventory: Inventory,
    target: Option<BlockTarget>,
    mining: Option<interaction::Mining>,
    pickups: Vec<Pickup>,
    feedback: feedback::Feedback,
    ambience: feedback::Ambience,
    storage: storage::Storage,
    interaction_notice: Option<&'static str>,
}

impl Game {
    /// Starts on deterministic, dry surface ground.
    pub fn new(seed: u64) -> Self {
        Self::with_mode(seed, GameMode::Creative)
    }

    /// Starts an empty survival inventory or the unchanged creative exploration session.
    pub fn with_mode(seed: u64, mode: GameMode) -> Self {
        let terrain = Terrain::new(seed);
        let original_spawn = Vec3::from_array(terrain.spawn_position());
        let mut player = Player::new(original_spawn);
        player.yaw = -0.55;
        player.pitch = -0.10;
        Self {
            terrain,
            world: ResidentWorld::default(),
            player,
            headlamp: true,
            mode,
            original_spawn,
            speed: SpeedMultiplier::Normal,
            time: WorldTime::default(),
            exposure: crate::lighting::ExposureCache::default(),
            exposure_revision: 0,
            outdoor_roof: None,
            coastal_exposure: None,
            inventory: Inventory::default(),
            target: None,
            mining: None,
            pickups: Vec::new(),
            feedback: feedback::Feedback::default(),
            ambience: feedback::Ambience::default(),
            storage: storage::Storage::default(),
            interaction_notice: None,
        }
    }

    /// Immutable session rules; survival cannot switch into creative flight or speed.
    pub const fn mode(&self) -> GameMode {
        self.mode
    }

    /// Deterministic terrain sampler for world generation workers.
    pub const fn terrain(&self) -> Terrain {
        self.terrain
    }

    /// Read-only exploration pose and movement state.
    pub const fn player(&self) -> &Player {
        &self.player
    }

    /// Resident CPU voxels shared with native and headless presentation.
    pub const fn world(&self) -> &ResidentWorld {
        &self.world
    }

    /// Chunk insertion/eviction boundary for the streaming adapter.
    pub fn world_mut(&mut self) -> &mut ResidentWorld {
        &mut self.world
    }

    /// Apply a view delta without coupling gameplay to physical input events.
    pub fn look(&mut self, delta_x: f64, delta_y: f64) {
        self.player.look(delta_x, delta_y);
    }

    /// Whether personal illumination is enabled.
    pub const fn headlamp(&self) -> bool {
        self.headlamp
    }

    /// Independent solar clock; debug acceleration does not affect movement or animation.
    pub const fn time(&self) -> WorldTime {
        self.time
    }

    /// Validated time-only debug controls for both exploration modes.
    pub fn time_mut(&mut self) -> &mut WorldTime {
        &mut self.time
    }

    /// Advances gameplay from logical controls, with persistent speed applied here.
    pub fn update(&mut self, mut input: MovementInput, dt: f32) {
        self.time.advance(dt);
        input.speed = self.speed;
        let world = SampledWorld::new(&self.world, &self.terrain);
        let report = self.player.update_observed(
            input,
            dt,
            |x, y, z| {
                world
                    .block_at(x, y, z)
                    .is_some_and(|block| block.is_solid())
            },
            |x, y, z| world.water_at(x, y, z),
        );
        self.feedback.movement(&report, &world, input);
        if below_world(self.player.position.y) {
            self.respawn();
        }
    }

    /// Changes movement mode with solid-body clearance on flight exit.
    pub fn toggle_flight(&mut self) {
        if self.mode == GameMode::Survival {
            return;
        }
        let world = SampledWorld::new(&self.world, &self.terrain);
        self.player.toggle_flight(|x, y, z| {
            world
                .block_at(x, y, z)
                .is_some_and(|block| block.is_solid())
        });
        self.feedback.clear();
        self.ambience = feedback::Ambience::default();
    }

    /// Changes personal illumination without modifying the world.
    pub fn toggle_headlamp(&mut self) {
        self.headlamp = !self.headlamp;
    }

    /// Changes speed once per application action; key release does not reset it.
    pub fn cycle_speed(&mut self) {
        if self.mode == GameMode::Creative {
            self.speed = self.speed.next();
        }
    }

    /// Selected exploration speed, preserved across pause and respawn.
    pub const fn speed(&self) -> SpeedMultiplier {
        self.speed
    }

    /// Respawn changes movement/position, not exploration settings.
    pub fn respawn(&mut self) {
        self.close_storage();
        let world = SampledWorld::new(&self.world, &self.terrain);
        if let Some(position) = recovery::spawn_position(&world, self.original_spawn) {
            self.player.respawn(position);
        }
        self.cancel_mining();
        self.target = None;
        self.feedback.clear();
        self.ambience = feedback::Ambience::default();
    }

    /// Pause/focus loss releases jump latches without resetting speed or momentum.
    pub fn clear_input(&mut self) {
        self.close_storage_cursor();
        self.player.clear_input();
        self.cancel_mining();
        self.feedback.clear();
        self.ambience = feedback::Ambience::default();
    }

    /// Drains bounded transient feedback once; gameplay never requires a consumer.
    pub fn take_sound_events(&mut self) -> [Option<SoundEvent>; MAX_SOUND_EVENTS] {
        self.feedback.take()
    }

    /// Fixed profiles from actual camera exposure and at most 289 resident columns at 4 Hz.
    pub fn sound_environment(&mut self, environment: Environment, dt: f32) -> SoundEnvironment {
        self.ambience
            .sample(&self.world, self.player.eye_position(), environment, dt)
    }
}

fn below_world(y: f32) -> bool {
    y < (MIN_WORLD_Y - 20) as f32
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expedition_tests;
#[cfg(test)]
mod survival_tests;

#[cfg(test)]
pub(crate) mod test_support;
