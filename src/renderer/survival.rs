//! Bounded pickup instances and independent target/first-person presentation.

mod cracks;
mod geometry;
mod pipelines;
mod resources;
mod scene;
mod staging;
mod tools;

pub(super) use resources::SurvivalResources;

fn shader_source() -> String {
    format!(
        "{}\n{}\n{}",
        super::lighting::world_shader_source(),
        cracks::shader_segments(),
        include_str!("../shaders/survival.wgsl"),
    )
}

#[cfg(test)]
mod tests;
