//! Small, validated command-line options for the game and bounded GPU smoke tests.

use std::path::PathBuf;

use crate::world::Terrain;

/// Launch settings for the native application.
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchOptions {
    /// Deterministic terrain seed.
    pub seed: u64,
    /// Radius of resident chunk columns.
    pub render_distance: i32,
    /// Exercise movement, streaming, resizing, and presentation, then exit.
    pub smoke_test: bool,
    /// Renders smoke frames to GPU textures without requiring an unlocked display.
    pub offscreen: bool,
    /// Optional GPU frame-readback destination at the end of a smoke test.
    pub screenshot: Option<PathBuf>,
    /// Disables native sound devices, independent of world progress/preferences.
    pub audio_disabled: bool,
    /// Explicit bounded audible check, without a window or player save.
    pub audio_check: bool,
    /// Session-only solar debug multiplier; zero freezes time, 1 is the normal cycle.
    pub time_speed: f64,
    /// Starting solar hour for new worlds and previews; Continue retains its saved phase.
    pub time_of_day: f64,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            seed: Terrain::default().seed(),
            render_distance: 8,
            smoke_test: false,
            offscreen: false,
            screenshot: None,
            audio_disabled: false,
            audio_check: false,
            time_speed: 1.0,
            time_of_day: crate::lighting::DEFAULT_TIME_HOUR,
        }
    }
}

impl LaunchOptions {
    pub(crate) fn validate_time(&self) -> Result<(), &'static str> {
        let mut time = crate::game::WorldTime::default();
        time.set_hour(self.time_of_day)?;
        time.set_speed(self.time_speed)
    }
}

/// Parsed launch intent; help does not create a window or initialize a GPU.
#[derive(Debug, PartialEq)]
pub enum LaunchCommand {
    /// Start an interactive game or smoke test.
    Run(LaunchOptions),
    /// Print usage information.
    Help,
}

/// Parses arguments after the executable name, rejecting typos and invalid values.
pub fn parse_arguments(args: impl IntoIterator<Item = String>) -> Result<LaunchCommand, String> {
    let mut args = args.into_iter();
    let mut options = LaunchOptions::default();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(LaunchCommand::Help),
            "--seed" => {
                options.seed = args
                    .next()
                    .ok_or("--seed requires an unsigned integer")?
                    .parse()
                    .map_err(|_| "--seed requires an unsigned integer")?;
            }
            "--render-distance" => {
                options.render_distance = args
                    .next()
                    .ok_or("--render-distance requires a number from 2 to 12")?
                    .parse()
                    .map_err(|_| "--render-distance requires a number from 2 to 12")?;
                if !(2..=12).contains(&options.render_distance) {
                    return Err("--render-distance must be between 2 and 12 chunks".into());
                }
            }
            "--smoke-test" => options.smoke_test = true,
            "--no-audio" => options.audio_disabled = true,
            "--audio-check" => options.audio_check = true,
            "--time-speed" => {
                options.time_speed = args
                    .next()
                    .ok_or("--time-speed requires a number from 0 to 120")?
                    .parse()
                    .map_err(|_| "--time-speed requires a number from 0 to 120")?;
                let mut time = crate::game::WorldTime::default();
                time.set_speed(options.time_speed)?;
            }
            "--time-of-day" => {
                options.time_of_day = crate::game::parse_hour(
                    &args
                        .next()
                        .ok_or("--time-of-day requires a phase or HH:MM")?,
                )?;
            }
            "--offscreen-smoke-test" => {
                options.smoke_test = true;
                options.offscreen = true;
            }
            "--screenshot" => {
                let path = args.next().ok_or("--screenshot requires a PNG file path")?;
                if path.is_empty() || path.starts_with("--") {
                    return Err("--screenshot requires a PNG file path".into());
                }
                options.screenshot = Some(PathBuf::from(path));
            }
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    if options.screenshot.is_some() && !options.smoke_test {
        return Err("--screenshot is only available with --smoke-test".into());
    }
    if options.audio_check && (options.smoke_test || options.audio_disabled) {
        return Err(
            "--audio-check is audible and cannot be combined with --no-audio or graphics smoke"
                .into(),
        );
    }
    Ok(LaunchCommand::Run(options))
}

/// User-facing commands and keyboard controls.
pub const HELP: &str = "RustCraft — Creative and Survival

Usage: rustcraft [--seed NUMBER] [--render-distance 2..12] [--no-audio]
                [--time-speed 0..120] [--time-of-day PHASE|HH:MM]
       rustcraft --smoke-test [--screenshot PATH.png]
       rustcraft --offscreen-smoke-test [--screenshot PATH.png]
       rustcraft --audio-check

--audio-check is an explicitly AUDIBLE 16-second check; no window, preferences, or world save.
Ordinary graphics smoke is silent and never opens an audio device. --no-audio disables playback.
Pause > Audio: mute and master/action/ambience gains; preferences are separate from world progress.

Continue resumes your one local world. New Survival/Creative replaces it only after confirmation.
Click Continue, a mode, or Resume to capture the mouse; Esc pauses and releases it.
WASD        Move       Mouse       Look
Space       Jump       Ctrl        Creative speed 1x/2x/4x/8x/16x
F           Creative fly/walk     Space/Shift Ascend/descend while flying
In water    Swim       Space/Shift Surface/dive
Double Space          Creative flight only
H           Toggle headlamp while exploring
Esc         Pause and release mouse
R           Return to spawn
F11         Toggle fullscreen
F6          Debug day-night speed: 1x/10x/60x/120x/frozen
Slash / `   Open time-only debug console (world pauses while typing)
Q           Quit while paused

One day and night lasts 20 minutes at 1x; 60x showcases a complete cycle in 20 seconds.
/time speed 60 accelerates only solar time; /time speed 1 restores normal, 0 freezes.
/time set dawn|day|noon|sunset|night|HH:MM selects a lighting phase. Enter runs, Esc cancels.
The console starts with /time already entered. H reveals headlamp lighting on night surfaces.
Solar time freezes on pause/focus loss and resumes via Continue; debug speed is not saved.
--time-of-day sets new-world/preview time only; it never overwrites Continue's saved phase.

Survival: hold left-click to mine; walk near dropped blocks to collect them.
Right-click places one held block in empty air; bedrock and liquids cannot be mined.
Right-click a crate opens its 18 slots; Shift+right-click places against it. Empty it before mining.
I           Inventory (pauses)    1-9 / Wheel Select hotbar slot
C           Crafting from inventory (materials and permanent stone tools)
Left-click inventory slots to move/merge/swap; right-click lifts half or places one. I/Esc closes it.
Inventory: 9 hotbar + 27 storage slots, stacks of 64; survival starts empty.
Survival keeps walking, jumping, swimming, the headlamp, and return-to-spawn.
Stone pickaxe/axe improve suitable gathering 3x; hand mining remains possible, no durability.
No survival flight, speed cycling, health, hunger, or combat.
Progress saves every 60 seconds and on Main Menu, Quit, or window close.
Seed, mode, edits, inventory, player pose, and dropped blocks resume via Continue.
Saves use a flushed temporary file and atomic replacement. Smoke tests use no player save.

Explore surface biomes and caves down to Y=-64; lava remains non-damaging scenery.
Swim through freshwater lakes, downhill rivers, waterfalls, estuaries, and oceans.
Currents carry swimmers; creative flight ignores them. No oxygen or drowning damage.
Flying inside solid ground automatically reveals a translucent geological cutaway.
Creative remains exploration-only: no inventory or block interaction.";

#[cfg(test)]
mod tests;
