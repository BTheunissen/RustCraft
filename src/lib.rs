//! A voxel sandbox with testable exploration, survival building, and world generation.

mod app;
pub mod atlas;
pub mod camera;
pub mod game;
pub mod hud;
mod input;
pub mod lighting;
pub mod mesh;
mod options;
pub mod phase;
pub mod player;
mod playtest;
mod renderer;
pub mod save;
pub mod streaming;
pub mod water;
pub mod world;

pub use app::run;
pub use options::{HELP, LaunchCommand, LaunchOptions, parse_arguments};
