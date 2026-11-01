//! Catchment terrain, occupancy, sediment, and current projection.

use super::{
    BANK_APRON, Block, Column, HydrologyColumn, MIN_TERRAIN_HEIGHT, RiverProjection, SEA_LEVEL,
    WORLD_HEIGHT, WaterBiome, WaterColumn, mix, normalized, smoothstep, value_noise,
};

use super::Catchment;

impl Catchment {
    pub(super) fn coastal_clearance(&self, point: [f64; 2]) -> f64 {
        self.lakes
            .iter()
            .map(|lake| (lake, BANK_APRON))
            .chain(self.nearby_lakes.iter().map(|lake| (lake, 32.0)))
            .map(|(lake, apron)| {
                let distance = (lake.radius_at(point) - 1.0) * lake.radius[0].min(lake.radius[1]);
                smoothstep((distance - apron) / 20.0)
            })
            .fold(1.0_f64, f64::min)
    }

    pub(super) fn sample(
        &self,
        seed: u64,
        world: [f64; 2],
        base: Column,
    ) -> Option<HydrologyColumn> {
        let point = self.local(world);
        let marine_radius = self.ocean.radius_at(point);
        let mut bed = f64::from(base.height);
        let mut water = None;
        let mut sediment = None;
        let mut modified = false;
        if marine_radius < 1.3 {
            modified = true;
            let roughness = value_noise(seed ^ 0x5EA_F100, world[0] / 31.0, world[1] / 31.0);
            let deep = -11.0 + 13.0 * smoothstep(marine_radius / 0.55);
            let shelf = 2.0 + 16.0 * smoothstep((marine_radius - 0.55) / 0.45);
            bed = if marine_radius <= 0.55 {
                deep + roughness * 1.25
            } else if marine_radius < 1.0 {
                shelf + roughness * (1.0 - marine_radius) * 2.0
            } else {
                mix(18.0, bed, smoothstep((marine_radius - 1.0) / 0.3))
            };
            let island_radius = self.island.radius_at(point);
            if island_radius < 1.7 {
                let island_bed = 32.0 - 47.0 * smoothstep(island_radius / 1.7);
                bed = bed.max(island_bed);
            }
            let bed_patch = value_noise(seed ^ 0x5ED1_4E17, world[0] / 43.0, world[1] / 43.0);
            sediment = Some(if marine_radius < 0.62 && bed_patch > 0.32 {
                Block::Stone
            } else if bed_patch < -0.34 || (island_radius < 1.05 && bed_patch > 0.0) {
                Block::Gravel
            } else {
                Block::Sand
            });
            if marine_radius <= 1.0 && bed.round() as i32 + 1 < SEA_LEVEL {
                water = Some(WaterColumn {
                    bottom: 0,
                    surface: SEA_LEVEL as f32,
                    flow: self.flow([0.6, 0.8], 0.1, 0.0),
                    biome: WaterBiome::Ocean,
                    salinity: 1.0,
                });
            }
        }
        let mut in_lake = false;
        for lake in self.lakes {
            let radius = lake.radius_at(point);
            if (radius - 1.0) * lake.radius[0].min(lake.radius[1]) >= BANK_APRON {
                continue;
            }
            if in_lake && radius > 1.0 {
                continue;
            }
            modified = true;
            bed = lake.bed(radius, bed);
            sediment = Some(if radius < 0.72 {
                Block::Clay
            } else {
                Block::Gravel
            });
            if radius <= 1.0 {
                water = (bed.round() + 1.0 < lake.surface).then_some(WaterColumn {
                    bottom: 0,
                    surface: lake.surface as f32,
                    flow: [0.0; 3],
                    biome: WaterBiome::Lake,
                    salinity: 0.0,
                });
                in_lake = water.is_some();
            }
        }
        let mut river: Option<RiverProjection> = None;
        let mut valley_offset = 0.0;
        let mut valley_weight = 0.0;
        let mut bank_support = f64::NEG_INFINITY;
        let mut shoreline_stage = f64::NEG_INFINITY;
        for segment in &self.rivers {
            let Some(projection) = segment.project(point) else {
                continue;
            };
            if river.is_none_or(|nearest| {
                (projection.distance / projection.width) < (nearest.distance / nearest.width)
            }) {
                river = Some(projection);
            }
            let distance = (projection.distance - projection.width).max(0.0);
            shoreline_stage = shoreline_stage.max(projection.surface - 0.5 - distance * 1.25);
            let bank = projection.surface - 1.0 + 2.0 * smoothstep(distance / 4.0);
            let blend = smoothstep((distance - 4.0) / (BANK_APRON - 4.0));
            let weight = 1.0 - blend;
            valley_offset += (bank - bed) * weight;
            valley_weight += weight;
            // A continuous lower bound retains a containing rim next to each
            // branch while allowing two dry valley aprons to merge smoothly.
            bank_support = bank_support.max(bank - (distance - 4.0).max(0.0) * 1.5);
        }
        if let Some(river) = river {
            modified = true;
            let across = river.distance / river.width;
            if across <= 1.0 {
                let channel_bed = river.surface - 1.0 - river.depth * (1.0 - across * across);
                let same_lake = in_lake
                    && water.is_some_and(|water| {
                        (f64::from(water.surface) - river.surface).abs() < 0.2
                    });
                bed = if same_lake || river.surface <= f64::from(SEA_LEVEL) + 0.01 {
                    bed.min(channel_bed)
                } else {
                    channel_bed
                };
                if !same_lake {
                    let mixing = smoothstep((river.progress - 0.67) / 0.33);
                    let speed = if river.progress >= 0.78 {
                        mix(1.2, 0.12, smoothstep((river.progress - 0.78) / 0.22))
                    } else {
                        (1.15 + river.grade * 7.0 + if river.tributary { 0.0 } else { 0.25 })
                            .min(2.5)
                    };
                    let biome = if river.waterfall {
                        WaterBiome::Waterfall
                    } else if river.progress >= 0.98 {
                        WaterBiome::Ocean
                    } else if river.progress > 0.67 {
                        WaterBiome::Estuary
                    } else {
                        WaterBiome::River
                    };
                    water = Some(WaterColumn {
                        bottom: 0,
                        surface: river.surface as f32,
                        flow: self.flow(
                            normalized([
                                river.direction[0]
                                    + (river.center[0] - point[0]) / river.width * 0.75,
                                river.direction[1]
                                    + (river.center[1] - point[1]) / river.width * 0.75,
                            ]),
                            speed,
                            if river.waterfall {
                                -4.0
                            } else {
                                -river.grade * speed
                            },
                        ),
                        biome,
                        salinity: mixing as f32,
                    });
                    sediment = Some(if river.waterfall || river.grade > 0.09 {
                        Block::Gravel
                    } else if river.progress > 0.67 {
                        Block::Sand
                    } else {
                        Block::Clay
                    });
                }
            } else if !in_lake {
                // A sea-level mouth cuts into the shelf instead of constructing
                // a dry levee through the surrounding marine water.
                if river.surface > f64::from(SEA_LEVEL) + 0.01 || water.is_none() {
                    let valley = bed + valley_offset / valley_weight.max(1.0);
                    // Dry aprons blend every branch, rather than snapping to the
                    // nearest segment's stage where two differently elevated banks meet.
                    bed = valley.max(bank_support).max(shoreline_stage);
                    water = None;
                    sediment = Some(Block::Gravel);
                }
            }
        }
        if !modified {
            return self.nearby_lake(point, base);
        }
        let height = (bed.round() as i32).clamp(MIN_TERRAIN_HEIGHT, WORLD_HEIGHT - 10);
        let water = water.and_then(|mut water| {
            water.bottom = height;
            (water.depth() > 0.0).then_some(water)
        });
        // Sediment belongs to submerged beds and a narrow shoreline, not the
        // entire blended valley. Outer banks retain their terrestrial topsoil.
        if water.is_none() {
            let shoreline = self
                .lakes
                .iter()
                .any(|lake| lake.radius_at(point) < 1.15 && height <= lake.surface as i32 + 2)
                || (marine_radius < 1.05 && height <= SEA_LEVEL + 2)
                || river.is_some_and(|river| {
                    river.distance < river.width + 3.0 && height <= river.surface.ceil() as i32 + 2
                });
            if !shoreline {
                sediment = None;
            }
        }
        let profile = HydrologyColumn {
            column: Column { height, ..base },
            water,
            sediment,
            seal_from: Some(base.height.min(height) - 3),
            coastal: None,
            rock_lip: self.rivers.iter().any(|segment| {
                segment.waterfall
                    && segment
                        .project(point)
                        .is_some_and(|projection| projection.distance < projection.width + 14.0)
            }),
        };
        if profile.water.is_none()
            && let Some(mut lake) = self.nearby_lake(point, profile.column)
        {
            lake.seal_from = Some(base.height.min(lake.column.height) - 3);
            lake.rock_lip = profile.rock_lip;
            return Some(lake);
        }
        Some(profile)
    }

    fn nearby_lake(&self, point: [f64; 2], base: Column) -> Option<HydrologyColumn> {
        const APRON: f64 = 32.0;
        for lake in self.nearby_lakes {
            let radius = lake.radius_at(point);
            if (radius - 1.0) * lake.radius[0].min(lake.radius[1]) >= APRON {
                continue;
            }
            let height = (lake
                .bed_with_apron(radius, f64::from(base.height), APRON)
                .round() as i32)
                .clamp(MIN_TERRAIN_HEIGHT, WORLD_HEIGHT - 10);
            let water =
                (radius <= 1.0 && f64::from(height + 1) < lake.surface).then_some(WaterColumn {
                    bottom: height,
                    surface: lake.surface as f32,
                    flow: [0.0; 3],
                    biome: WaterBiome::Lake,
                    salinity: 0.0,
                });
            return Some(HydrologyColumn {
                column: Column { height, ..base },
                water,
                sediment: (water.is_some() || radius < 1.15).then_some(if radius < 0.72 {
                    Block::Clay
                } else {
                    Block::Gravel
                }),
                seal_from: Some(base.height.min(height) - 3),
                coastal: None,
                rock_lip: false,
            });
        }
        None
    }
}
