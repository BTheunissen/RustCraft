//! Mean fluid profile validation and statistics.

use super::*;

#[derive(Debug)]
pub(super) struct Profiles {
    pub(super) columns: usize,
    pub(super) voxels: usize,
    pub(super) depths: Vec<f64>,
    pub(super) levels: Vec<f64>,
    pub(super) speed: Vec<f64>,
    pub(super) salinity: Vec<f64>,
    pub(super) flow_min: [f64; 3],
    pub(super) flow_max: [f64; 3],
    pub(super) beds: [usize; Block::COUNT],
}

impl Default for Profiles {
    fn default() -> Self {
        Self {
            columns: 0,
            voxels: 0,
            depths: Vec::new(),
            levels: Vec::new(),
            speed: Vec::new(),
            salinity: Vec::new(),
            flow_min: [f64::INFINITY; 3],
            flow_max: [f64::NEG_INFINITY; 3],
            beds: [0; Block::COUNT],
        }
    }
}

impl Profiles {
    pub(super) fn record(&mut self, water: WaterSample, bed: Option<Block>) -> Result<(), String> {
        validate_sample(water)?;
        self.columns += 1;
        self.depths
            .push(f64::from(water.surface) - f64::from(water.bottom + 1));
        self.levels.push(f64::from(water.surface));
        self.salinity.push(f64::from(water.salinity));
        self.speed.push(
            water
                .flow
                .into_iter()
                .map(|component| f64::from(component).powi(2))
                .sum::<f64>()
                .sqrt(),
        );
        for (axis, value) in water.flow.into_iter().enumerate() {
            self.flow_min[axis] = self.flow_min[axis].min(f64::from(value));
            self.flow_max[axis] = self.flow_max[axis].max(f64::from(value));
        }
        if let Some(bed) = bed {
            if !bed.is_solid() {
                return Err(format!(
                    "{} water has a nonsolid {bed:?} bed",
                    water.biome.name()
                ));
            }
            self.beds[bed as usize] += 1;
        }
        Ok(())
    }

    pub(super) fn report(&self, label: &str) {
        if self.columns == 0 {
            println!("  {label}: no actual wet columns sampled");
            return;
        }
        let voxel_count = if self.voxels > 0 {
            self.voxels.to_string()
        } else {
            "not counted in this footprint survey".into()
        };
        println!(
            "  {label}: {} horizontal wet columns, resident wet voxels={voxel_count}; depth p10/p50/p90={:?}, mean-line range={:?}, salinity range={:?}, current speed range={:?} blocks/s",
            self.columns,
            quantiles(&self.depths),
            range(&self.levels),
            range(&self.salinity),
            range(&self.speed),
        );
        print!("    current xyz={:?}..{:?}", self.flow_min, self.flow_max);
        if self.beds.iter().any(|&count| count != 0) {
            print!("; supported bed counts:");
        }
        for (block, count) in Block::ALL.into_iter().zip(self.beds) {
            if count != 0 {
                print!(" {block:?}={count}");
            }
        }
        println!();
    }
}

pub(super) fn validate_sample(water: WaterSample) -> Result<(), String> {
    if !(MIN_WORLD_Y..WORLD_HEIGHT).contains(&water.bottom)
        || !water.surface.is_finite()
        || water.surface <= (water.bottom + 1) as f32
        || water.surface > WORLD_HEIGHT as f32
        || !water.flow.into_iter().all(f32::is_finite)
        || !water.salinity.is_finite()
        || !(0.0..=1.0).contains(&water.salinity)
    {
        return Err(format!("invalid water profile: {water:?}"));
    }
    Ok(())
}

pub(super) fn quantiles(values: &[f64]) -> [f64; 3] {
    if values.is_empty() {
        return [0.0; 3];
    }
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    [10, 50, 90].map(|percent| values[(values.len() - 1) * percent / 100])
}

pub(super) fn range(values: &[f64]) -> [f64; 2] {
    if values.is_empty() {
        return [0.0; 2];
    }
    values
        .iter()
        .fold([f64::INFINITY, f64::NEG_INFINITY], |bounds, &value| {
            [bounds[0].min(value), bounds[1].max(value)]
        })
}
