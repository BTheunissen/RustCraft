use super::*;

fn swimming_site(world: &ResidentWorld, kind: WaterBiome) -> (Vec3, WaterSample) {
    let site = landmark(&world.terrain, kind);
    let mut best = None;
    for dz in -12..=12 {
        for dx in -12..=12 {
            let [x, z] = [site.water[0] + dx, site.water[2] + dz];
            let Some(sample) = world
                .terrain
                .water_column(x, z)
                .filter(|sample| sample.biome == kind && depth(*sample) >= 3.0)
            else {
                continue;
            };
            let safe = (-2..=2).all(|dz| {
                (-2..=2).all(|dx| {
                    world
                        .terrain
                        .water_column(x + dx, z + dz)
                        .is_some_and(|water| water.biome == kind && depth(water) >= 2.5)
                })
            });
            let stroke_clear = kind != WaterBiome::River
                || [-1.0, 1.0].into_iter().all(|sign| {
                    let direction = Vec3::from_array(sample.flow).with_y(0.0).normalize();
                    (0..=8).all(|step| {
                        let distance = sign * step as f32 / 4.0;
                        let point = Vec3::new(x as f32 + 0.5, sample.surface - 1.2, z as f32 + 0.5)
                            + direction * distance;
                        world
                            .terrain
                            .water_column(point.x.floor() as i32, point.z.floor() as i32)
                            .is_some_and(|water| {
                                water.biome == kind
                                    && depth(water) >= 2.5
                                    && (water.surface - sample.surface).abs() < 0.8
                            })
                    })
                });
            if safe
                && stroke_clear
                && best.is_none_or(|(_, previous)| depth(sample) > depth(previous))
            {
                best = Some((
                    Vec3::new(x as f32 + 0.5, sample.surface - 2.1, z as f32 + 0.5),
                    sample,
                ));
            }
        }
    }
    let (feet, sample) = best.unwrap_or_else(|| {
        panic!(
            "seed {}, no broad swimmable {} site",
            world.terrain.seed(),
            kind.name()
        )
    });
    world.load_neighborhood([feet.x.floor() as i32, feet.z.floor() as i32]);
    assert!(body_is_clear(feet, |point| world.block(point)));
    (feet, sample)
}

fn generated_exit_shore(world: &ResidentWorld, kind: WaterBiome) -> (Vec3, Vec3) {
    let site = landmark(&world.terrain, kind);
    let center = Vec3::new(site.water[0] as f32 + 0.5, 0.0, site.water[2] as f32 + 0.5);
    for angle in 0..32 {
        let yaw = angle as f32 * std::f32::consts::TAU / 32.0;
        let ray = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
        for distance in (0..=192).step_by(2) {
            let point = center + ray * distance as f32;
            let [x, _, z] = point.floor().as_ivec3().to_array();
            let Some(water) = world
                .terrain
                .water_column(x, z)
                .filter(|water| water.biome == kind)
            else {
                continue;
            };
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (bank_x, bank_z) = (x + dx, z + dz);
                if world.terrain.water_column(bank_x, bank_z).is_some() {
                    continue;
                }
                let bank = world.terrain.column(bank_x, bank_z);
                let top = (bank.height + 1) as f32;
                if !(water.surface - 0.1..=water.surface + 1.0).contains(&top) {
                    continue;
                }
                let feet = Vec3::new(bank_x as f32 + 0.5, top, bank_z as f32 + 0.5);
                if !body_is_clear(feet, |point| world.block(point))
                    || !world.block([bank_x, bank.height, bank_z]).is_solid()
                {
                    continue;
                }
                let inward = Vec3::new(-dx as f32, 0.0, -dz as f32);
                let open_water = (0..=4).all(|step| {
                    let point = feet + inward * (step as f32 * 0.5 + 1.0);
                    world
                        .terrain
                        .water_column(point.x.floor() as i32, point.z.floor() as i32)
                        .is_some_and(|profile| profile.biome == kind && depth(profile) >= 0.5)
                });
                if open_water {
                    world.load_neighborhood([bank_x, bank_z]);
                    return (feet, inward);
                }
            }
        }
    }
    panic!(
        "seed {}, no clear low {} shore for a generated entry/exit check",
        world.terrain.seed(),
        kind.name()
    );
}

#[test]
fn generated_freshwater_and_ocean_shores_support_walking_entry_and_swimming_exit() {
    for seed in [Terrain::default().seed(), 42] {
        for kind in [WaterBiome::Lake, WaterBiome::Ocean] {
            let world = ResidentWorld::new(Terrain::new(seed));
            let (shore, inward) = generated_exit_shore(&world, kind);
            let mut player = Player::new(shore);
            player.yaw = inward.x.atan2(-inward.z);
            let forward = MovementInput {
                forward: true,
                ..Default::default()
            };
            let mut entered = false;
            for _ in 0..480 {
                world.advance(&mut player, forward, 1);
                if player.is_swimming() && (player.position - shore).dot(inward) > 1.8 {
                    entered = true;
                    break;
                }
            }
            assert!(
                entered,
                "seed {seed}, {} walking entry failed at {:?} from {shore:?}",
                kind.name(),
                player.position
            );
            assert!(!player.flying);
            player.yaw = (-inward.x).atan2(inward.z);
            player.clear_input();
            let exit = MovementInput {
                forward: true,
                jump: true,
                ..Default::default()
            };
            let mut returned = false;
            for _ in 0..960 {
                world.advance(&mut player, exit, 1);
                if (player.position - shore).dot(inward) < 0.15
                    && player.position.y >= shore.y - 0.01
                {
                    returned = true;
                    break;
                }
            }
            assert!(
                returned,
                "seed {seed}, {} swimming exit failed at {:?}, shore {shore:?}, submersion {:.3}, grounded {}, profile {:?}, direction {inward:?}",
                kind.name(),
                player.position,
                player.submersion(),
                player.is_grounded(),
                world.terrain.water_column(
                    player.position.x.floor() as i32,
                    player.position.z.floor() as i32
                ),
            );
            world.advance(
                &mut player,
                MovementInput {
                    jump: true,
                    ..Default::default()
                },
                360,
            );
            assert!(
                player.is_grounded() && !player.is_swimming() && !player.flying,
                "seed {seed}, {} shore landing failed at {:?}",
                kind.name(),
                player.position
            );
            assert!((player.position.y - shore.y).abs() < 0.01);
        }
    }
}

#[test]
fn generated_lake_buoyancy_and_held_swim_controls_are_stable_and_flight_is_unchanged() {
    for seed in [Terrain::default().seed(), 42] {
        let world = ResidentWorld::new(Terrain::new(seed));
        let (feet, sample) = swimming_site(&world, WaterBiome::Lake);
        let mut floating = Player::new(feet);
        world.advance(&mut floating, MovementInput::default(), 720);
        assert!(
            floating.is_swimming(),
            "seed {seed}, failed lake buoyancy at {:?}",
            floating.position
        );
        assert!(!floating.is_grounded());
        assert!(
            floating.position.y > sample.surface - PLAYER_HEIGHT - 0.5
                && floating.position.y < sample.surface,
            "seed {seed}, nonphysical floating height {:?}, mean={}",
            floating.position,
            sample.surface
        );
        let resting = floating.position;
        world.advance(&mut floating, MovementInput::default(), 240);
        assert!(
            floating.position.distance(resting) < 0.2,
            "seed {seed}, buoyancy keeps oscillating/drifting"
        );

        let mut sinking = Player::new(feet);
        world.advance(
            &mut sinking,
            MovementInput {
                descend: true,
                ..Default::default()
            },
            120,
        );
        assert!(
            sinking.position.y < feet.y - 0.3,
            "seed {seed}, held descent not effective"
        );
        let submerged = sinking.position;
        world.advance(
            &mut sinking,
            MovementInput {
                jump: true,
                ..Default::default()
            },
            120,
        );
        assert!(
            sinking.position.y > submerged.y + 0.3,
            "seed {seed}, held swim ascent not effective"
        );

        let mut water_flight = Player::new(feet);
        let mut legacy_flight = Player::new(feet);
        water_flight.toggle_flight(|x, y, z| world.block([x, y, z]).is_solid());
        legacy_flight.toggle_flight(|x, y, z| world.block([x, y, z]).is_solid());
        let input = MovementInput {
            forward: true,
            jump: true,
            speed: rustcraft::player::SpeedMultiplier::Double,
            ..Default::default()
        };
        for _ in 0..120 {
            water_flight.update_with_water(
                input,
                DT,
                |x, y, z| world.block([x, y, z]).is_solid(),
                |x, y, z| world.water(x, y, z),
            );
            legacy_flight.update(input, DT, |x, y, z| world.block([x, y, z]).is_solid());
        }
        assert!(water_flight.flying && !water_flight.is_swimming());
        assert!(water_flight.position.distance(legacy_flight.position) < 0.001);
    }
}

#[test]
fn real_river_current_drifts_player_and_downstream_swimming_beats_upstream_progress() {
    for seed in [Terrain::default().seed(), 42] {
        let world = ResidentWorld::new(Terrain::new(seed));
        let (feet, sample) = swimming_site(&world, WaterBiome::River);
        let feet = feet.with_y(sample.surface - 1.2);
        let downstream = Vec3::from_array(sample.flow).with_y(0.0).normalize();
        let mut idle = Player::new(feet);
        world.advance(&mut idle, MovementInput::default(), 120);
        assert!(idle.is_swimming());
        assert!(
            (idle.position - feet).dot(downstream) > 0.1,
            "seed {seed}, real current caused no downstream drift"
        );
        let mut with_current = Player::new(feet);
        let mut against_current = Player::new(feet);
        with_current.yaw = downstream.x.atan2(-downstream.z);
        against_current.yaw = (-downstream.x).atan2(downstream.z);
        let input = MovementInput {
            forward: true,
            speed: rustcraft::player::SpeedMultiplier::Double,
            ..Default::default()
        };
        world.advance(&mut with_current, input, 60);
        world.advance(&mut against_current, input, 60);
        let downstream_distance = (with_current.position - feet).dot(downstream);
        let upstream_distance = -(against_current.position - feet).dot(downstream);
        assert!(with_current.is_swimming() && against_current.is_swimming());
        assert!(
            upstream_distance > 0.05 && downstream_distance > upstream_distance + 0.1,
            "seed {seed}, downstream={downstream_distance}, upstream={upstream_distance}, current={:?}",
            sample.flow
        );
    }
}

#[test]
fn absent_or_unloaded_water_keeps_legacy_gravity_and_dry_spawn_behavior() {
    let terrain = Terrain::default();
    let world = ResidentWorld::new(terrain);
    let spawn = Vec3::from_array(terrain.spawn_position());
    let mut walking = Player::new(spawn);
    world.advance(&mut walking, MovementInput::default(), 240);
    assert!(walking.is_grounded() && !walking.is_swimming());
    assert!(walking.position.distance(spawn) < 0.002);
    let site = landmark(&terrain, WaterBiome::Ocean);
    let profile = terrain.water_column(site.water[0], site.water[2]).unwrap();
    let feet = Vec3::new(
        site.water[0] as f32 + 0.5,
        profile.surface - 1.8,
        site.water[2] as f32 + 0.5,
    );
    let mut legacy = Player::new(feet);
    let mut absent = Player::new(feet);
    for _ in 0..120 {
        legacy.update(MovementInput::default(), DT, |_, _, _| false);
        absent.update_with_water(
            MovementInput::default(),
            DT,
            |_, _, _| false,
            |_, _, _| None,
        );
    }
    assert!(!absent.is_swimming() && !absent.is_grounded());
    assert!(absent.position.distance(legacy.position) < 0.001);
    assert!(absent.position.y < feet.y - 5.0);
}
