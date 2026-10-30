//! Authoritative arena actions on the existing instance directory owner. No IO,
//! new task, idle-world iteration or blocking Java callback occurs here.
use super::*;
#[derive(Clone, Copy)]
struct DamageContext {
    source: EntitySnapshot,
    target: EntitySnapshot,
    damage_type: i32,
    kind: gameplay::AttackKind,
    secondary: bool,
}

impl InstanceDir {
    fn critical(&self, id: i32, message: gameplay::Delivery) {
        if let Some(member) = self.members.get(&id)
            && let Some(tx) = &member.critical
            && tx.try_send(message).is_err()
        {
            member.closed.send_replace(true);
            member.wake.notify_one();
            self.metrics.slow_viewers.fetch_add(1, Relaxed);
        }
    }
    fn broadcast_critical(&self, id: i32, packets: Vec<Vec<u8>>, own: bool) {
        let packets: Arc<[Vec<u8>]> = packets.into();
        let at = Instant::now();
        for &viewer in &self.members[&id].viewers {
            self.critical(
                viewer,
                gameplay::Delivery::Packets {
                    entity: Some(id),
                    packets: Arc::clone(&packets),
                    at,
                },
            );
        }
        if own {
            self.critical(
                id,
                gameplay::Delivery::Packets {
                    entity: None,
                    packets,
                    at,
                },
            );
        }
    }
    fn effects(&self, id: i32, position: [f64; 3], range: f64, packets: Vec<Vec<u8>>) {
        let packets: Arc<[Vec<u8>]> = packets.into();
        let at = Instant::now();
        for recipient in self.members[&id]
            .viewers
            .iter()
            .copied()
            .chain(std::iter::once(id))
        {
            let pose = self.members[&recipient].pending.lock().unwrap().latest;
            if pose.ready
                && (0..3)
                    .map(|i| (pose.state.position[i] - position[i]).powi(2))
                    .sum::<f64>()
                    < range * range
            {
                self.critical(
                    recipient,
                    gameplay::Delivery::Packets {
                        entity: None,
                        packets: Arc::clone(&packets),
                        at,
                    },
                );
            }
        }
    }
    fn sound(&self, id: i32, name: &str, position: [f64; 3]) {
        let mut bytes = [0u8; 8];
        let seed = if openssl::rand::rand_bytes(&mut bytes).is_ok() {
            i64::from_be_bytes(bytes)
        } else {
            self.metrics.combat_attacks.load(Relaxed) as i64
        };
        self.effects(id, position, 16., vec![pvp::sound(name, position, seed)]);
    }
    fn damage_target(
        &mut self,
        source: i32,
        target: i32,
        damage: f32,
        context: DamageContext,
    ) -> bool {
        let Some(shared) = self.members.get(&target).and_then(|m| m.game.clone()) else {
            return false;
        };
        let mut game = shared.lock().unwrap();
        game.refresh_terrain();
        game.advance_motion(Instant::now());
        let old_health = game.health;
        let old_velocity = game.velocity;
        let grounded = game.motion_state().2;
        let Some((_, fresh)) = game.hurt(damage, Instant::now()) else {
            return false;
        };
        let health = game.health;
        let dead = game.dead;
        let actual = old_health - health;
        self.metrics.combat_hits.fetch_add(1, Relaxed);
        if dead {
            self.metrics.combat_deaths.fetch_add(1, Relaxed);
        }
        if let Some(notices) = &game.notices {
            let player = crate::control::PlayerSnapshot::new(
                target,
                &self.members[&target].profile,
                notices.world,
            );
            let notice = crate::control::CombatNotice {
                state: game.snapshot(),
                source,
                damage: actual,
            };
            notices
                .worlds
                .combat_event(11, notices.world, player.clone(), notice.clone());
            if dead {
                notices
                    .worlds
                    .combat_event(12, notices.world, player, notice);
            }
        }
        let mut velocity = old_velocity;
        if fresh {
            velocity = gameplay::knockback(
                velocity,
                context.source,
                EntitySnapshot {
                    on_ground: grounded,
                    ..context.target
                },
                0.4000000059604645,
            );
        }
        let extra = if context.secondary {
            0.4000000059604645
        } else if context.kind == gameplay::AttackKind::Knockback {
            0.5
        } else {
            0.
        };
        if extra > 0. {
            velocity = gameplay::directed_knockback(
                velocity,
                gameplay::yaw_axes(context.source.rotation[0]),
                grounded,
                extra,
            );
        }
        let trace = game.trace;
        let trace_tick = game.tick(Instant::now());
        let trace_y = game.motion_state().1;
        // Vanilla's primary-player causeExtraKnockback sends combined velocity
        // and restores the captured old motion; a sweep secondary retains it.
        if context.secondary {
            game.velocity = velocity;
        }
        drop(game);
        if trace {
            eprintln!(
                "pvp_hit tick={} source={} target={} kind={:?} health={} fresh={} client_ground={} server_ground={} client_y={} motion_y={} old={:?} impulse={:?}",
                trace_tick,
                source,
                target,
                context.kind,
                health,
                fresh,
                context.target.on_ground,
                grounded,
                context.target.position[1],
                trace_y,
                old_velocity,
                velocity
            );
        }
        let mut packets = vec![pvp::metadata(target, health)];
        if fresh {
            let yaw = ((context.source.position[2] - context.target.position[2])
                .atan2(context.source.position[0] - context.target.position[0])
                * 57.2957763671875
                - context.target.rotation[0] as f64) as f32;
            packets.extend([
                pvp::damage(target, source, context.damage_type),
                pvp::hurt(target, yaw),
            ]);
        }
        if fresh || extra > 0. {
            self.critical(
                target,
                gameplay::Delivery::packets(
                    None,
                    vec![pvp::velocity(target, gameplay::classic_velocity(velocity))],
                ),
            );
        }
        if dead {
            packets.push(pvp::death(target));
        }
        self.broadcast_critical(target, packets, true);
        self.critical(
            target,
            gameplay::Delivery::packets(None, vec![pvp::health(health)]),
        );
        // Vanilla handles hurt/death audio from DamageEvent/EntityEvent; an
        // explicit sound here would double it on every stock client.
        if dead {
            self.critical(
                target,
                gameplay::Delivery::packets(None, vec![pvp::killed(target)]),
            );
            self.critical(target, gameplay::Delivery::Died);
        }
        if actual > 2. {
            let mut pos = context.target.position;
            pos[1] += if context.target.crouching { 0.75 } else { 0.9 };
            self.effects(
                target,
                pos,
                32.,
                vec![pvp::particles(
                    14,
                    pos,
                    [0.1, 0., 0.1],
                    0.2,
                    (actual * 0.5) as i32,
                )],
            );
        }
        true
    }
    fn sweep(
        &mut self,
        id: i32,
        primary: i32,
        source: EntitySnapshot,
        target: EntitySnapshot,
        scale: f32,
        damage_type: i32,
    ) {
        // Index padding covers the existing bounded latest-position coalescing
        // window. Exact fresh AABBs/range below still govern damage, never cells.
        let x0 = ((target.position[0] - 1.6) / 16.).floor() as i32 - 2;
        let x1 = ((target.position[0] + 1.6) / 16.).floor() as i32 + 2;
        let z0 = ((target.position[2] - 1.6) / 16.).floor() as i32 - 2;
        let z1 = ((target.position[2] + 1.6) / 16.).floor() as i32 + 2;
        let mut candidates = HashSet::new();
        for x in x0..=x1 {
            for z in z0..=z1 {
                if let Some(occupants) = self.occupants.get(&ChunkPos { x, z }) {
                    candidates.extend(occupants.iter().copied());
                }
            }
        }
        for candidate in candidates {
            if candidate == id
                || candidate == primary
                || !self.members[&id].visible.contains(&candidate)
            {
                continue;
            }
            let other = self.members[&candidate].pending.lock().unwrap().latest;
            if !other.ready || !gameplay::in_sweep(source, target, other.state) {
                continue;
            }
            // No enchantments/armor/teams exist in this arena profile: vanilla's
            // unenchanted sweeping ratio is zero, hence damage = 1 * charge.
            self.damage_target(
                id,
                candidate,
                scale,
                DamageContext {
                    source,
                    target: other.state,
                    damage_type,
                    kind: gameplay::AttackKind::Sweep,
                    secondary: true,
                },
            );
        }
        self.sound(id, gameplay::AttackKind::Sweep.sound(), source.position);
        let axes = gameplay::yaw_axes(source.rotation[0]);
        let pos = [
            source.position[0] + axes[0],
            source.position[1] + if source.crouching { 0.75 } else { 0.9 },
            source.position[2] + axes[1],
        ];
        self.effects(
            id,
            pos,
            32.,
            vec![pvp::particles(
                77,
                pos,
                [axes[0] as f32, 0., axes[1] as f32],
                0.,
                0,
            )],
        );
    }
    fn block_occupied(&self, pos: [i32; 3]) -> bool {
        let block = smolmc_world::collision::Aabb {
            min: pos.map(|v| v as f64),
            max: pos.map(|v| v as f64 + 1.),
        };
        let center = ChunkPos {
            x: pos[0].div_euclid(16),
            z: pos[2].div_euclid(16),
        };
        for z in center.z - 1..=center.z + 1 {
            for x in center.x - 1..=center.x + 1 {
                if let Some(ids) = self.occupants.get(&ChunkPos { x, z }) {
                    for id in ids {
                        let pose = self.members[id].pending.lock().unwrap().latest;
                        if pose.ready
                            && smolmc_world::collision::Aabb::player(
                                pose.state.position,
                                pose.state.crouching,
                            )
                            .intersects(block)
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
    fn block_action(
        &self,
        id: i32,
        action: pvp::Action,
        source: ActionSource,
        game: &mut gameplay::State,
    ) -> Option<Vec<Vec<u8>>> {
        use smolmc_protocol_777::play;
        use smolmc_world::Material;
        let (clicked, sequence) = match action {
            pvp::Action::Dig { pos, sequence, .. } | pvp::Action::Place { pos, sequence, .. } => {
                (pos, sequence)
            }
            _ => unreachable!(),
        };
        let mut target = clicked;
        if let pvp::Action::Place { face, .. } = action {
            let offset = [
                [0, -1, 0],
                [0, 1, 0],
                [0, 0, -1],
                [0, 0, 1],
                [-1, 0, 0],
                [1, 0, 0],
            ][face as usize];
            target = std::array::from_fn(|i| clicked[i].saturating_add(offset[i]));
        }
        if let Some(owner) = game.blocks.clone()
            && let Some(mut geometry) = game.terrain.clone()
        {
            geometry.overlay = owner.terrain.borrow().clone();
            let eye = [
                source.state.position[0],
                source.state.position[1] + if source.state.crouching { 1.27 } else { 1.62 },
                source.state.position[2],
            ];
            let distance = (0..3)
                .map(|i| {
                    let lo = clicked[i] as f64;
                    let hi = lo + 1.;
                    (eye[i] - eye[i].clamp(lo, hi)).powi(2)
                })
                .sum::<f64>();
            let subscribed = self.members[&id]
                .pending
                .lock()
                .unwrap()
                .latest
                .view
                .contains(ChunkPos {
                    x: clicked[0].div_euclid(16),
                    z: clicked[2].div_euclid(16),
                });
            let fresh = sequence > game.block_sequence && subscribed;
            game.block_sequence = game.block_sequence.max(sequence);
            let mut desired = None;
            match action {
                pvp::Action::Dig { action, .. } => {
                    target = clicked;
                    let now = Instant::now();
                    let material = geometry.block(clicked);
                    let item = game.inventory.main();
                    if action == 2 {
                        if let Some(mining) = game.mining.take() {
                            self.broadcast_critical(
                                id,
                                vec![pvp::block_destruction(id, mining.pos, -1)],
                                false,
                            );
                        }
                    } else if game.mode == 0
                        && action != 1
                        && fresh
                        && distance < 30.25
                        && !material.air()
                    {
                        if action == 0 {
                            game.mining = Some(gameplay::Mining {
                                pos: clicked,
                                material,
                                item,
                                start: game.tick(now),
                                last_stage: -1,
                            });
                        }
                        if let Some(mining) = game.mining
                            && mining.pos == clicked
                            && mining.material == material
                            && mining.item == item
                        {
                            let progress =
                                gameplay::mining_progress(material, item, source.state.on_ground)
                                    * (game.tick(now).saturating_sub(mining.start) + 1) as f32;
                            if progress >= if action == 0 { 1. } else { 0.7 } {
                                desired = Some(Material::Air);
                                game.mining = None;
                                self.broadcast_critical(
                                    id,
                                    vec![pvp::block_destruction(id, clicked, -1)],
                                    false,
                                );
                            } else {
                                let stage = (progress * 10.).clamp(0., 9.) as i32;
                                self.broadcast_critical(
                                    id,
                                    vec![pvp::block_destruction(id, clicked, stage)],
                                    false,
                                );
                                if let Some(m) = &mut game.mining {
                                    m.last_stage = stage;
                                }
                            }
                        }
                    }
                }
                pvp::Action::Place { hand, .. } => {
                    let held = if hand == 0 {
                        game.inventory.main()
                    } else {
                        game.inventory.off()
                    };
                    if let pvp::Item::Block { state, .. } = held
                        && let Some(material) = Material::from_state(state)
                        && material.placeable()
                        && geometry.block(target).air()
                        && !geometry.block(clicked).air()
                        && !self.block_occupied(target)
                    {
                        desired = Some(material)
                    }
                }
                _ => {}
            }
            if fresh
                && distance < 30.25
                && geometry.base.bounds.contains(target)
                && let Some(material) = desired
                && let Some(sender) = &self.members[&id].critical
            {
                let reply = crate::worlds::lighting::Reply::Client {
                    sender: sender.clone(),
                    closed: self.members[&id].closed.clone(),
                    wake: Arc::clone(&self.members[&id].wake),
                    positions: if target == clicked {
                        vec![clicked]
                    } else {
                        vec![clicked, target]
                    },
                    sequence,
                };
                if owner
                    .worlds
                    .submit_terrain(
                        owner.handle,
                        vec![(target, material)],
                        true,
                        Some(geometry.block(target)),
                        reply,
                    )
                    .is_ok()
                {
                    return None;
                }
            }
            geometry.overlay = owner.terrain.borrow().clone();
            let mut packets = vec![play::block_correction(
                clicked,
                geometry.block(clicked).state(),
            )];
            if target != clicked {
                packets.push(play::block_correction(
                    target,
                    geometry.block(target).state(),
                ));
            }
            packets.push(play::block_ack(sequence));
            game.terrain = Some(geometry);
            return Some(packets);
        }
        Some(vec![
            play::block_correction(
                clicked,
                smolmc_world::VirtualStone.block(clicked[1]).state(),
            ),
            play::block_correction(target, smolmc_world::VirtualStone.block(target[1]).state()),
            play::block_ack(sequence),
        ])
    }
    pub(super) fn action(
        &mut self,
        id: i32,
        action: pvp::Action,
        mut source: ActionSource,
        damage_type: i32,
    ) {
        let now = Instant::now();
        let Some(shared) = self.members[&id].game.clone() else {
            return;
        };
        // The directory serializes mutations; a source snapshot comes directly
        // from the session, not from the coalesced movement publication queue.
        if !source.ready && !matches!(action, pvp::Action::Respawn) {
            return;
        }
        let mut game = shared.lock().unwrap();
        game.refresh_terrain();
        let admitted = if game.blocks.is_some()
            && matches!(
                action,
                pvp::Action::Dig { .. } | pvp::Action::Place { .. } | pvp::Action::Punch
            ) {
            game.admit_build_action(now)
        } else {
            game.admit_action(now)
        };
        if !admitted {
            self.members[&id].closed.send_replace(true);
            self.members[&id].wake.notify_one();
            return;
        }
        if matches!(action, pvp::Action::Respawn) {
            if game.dead && !game.respawn_requested {
                game.respawn_requested = true;
                drop(game);
                self.critical(id, gameplay::Delivery::Respawn);
            }
            return;
        }
        if game.dead {
            return;
        }
        game.advance_motion(now);
        source.state.on_ground = game.motion_state().2;
        match action {
            pvp::Action::Dig { .. } | pvp::Action::Place { .. } => {
                let packets = self.block_action(id, action, source, &mut game);
                drop(game);
                if let Some(packets) = packets {
                    self.critical(id, gameplay::Delivery::packets(None, packets));
                }
            }
            pvp::Action::Punch => {
                if let Some(mining) = game.mining {
                    let progress = gameplay::mining_progress(
                        mining.material,
                        mining.item,
                        source.state.on_ground,
                    ) * (game.tick(now).saturating_sub(mining.start) + 1) as f32;
                    let stage = (progress * 10.).clamp(0., 9.) as i32;
                    if stage != mining.last_stage {
                        self.broadcast_critical(
                            id,
                            vec![pvp::block_destruction(id, mining.pos, stage)],
                            false,
                        );
                        game.mining.as_mut().unwrap().last_stage = stage;
                    }
                }
                game.reset_charge(now);
                drop(game);
                self.broadcast_critical(id, vec![pvp::swing(id, 0)], false);
            }
            pvp::Action::Attack(target) => {
                if !game.combat_enabled {
                    return;
                }
                self.metrics.combat_attacks.fetch_add(1, Relaxed);
                if id == target || !self.members[&id].visible.contains(&target) {
                    return;
                }
                let Some(victim) = self.members.get(&target) else {
                    return;
                };
                let target_pose = victim.pending.lock().unwrap().latest;
                if !target_pose.ready || !gameplay::in_reach(source.state, target_pose.state) {
                    return;
                }
                if victim
                    .game
                    .as_ref()
                    .is_none_or(|game| game.lock().unwrap().dead)
                {
                    return;
                }
                let kind = game.classify(source.state, now);
                let scale = game.attack_strength(now);
                let damage = game.attack_damage(now)
                    * if kind == gameplay::AttackKind::Critical {
                        1.5
                    } else {
                        1.
                    };
                drop(game);
                self.broadcast_critical(id, vec![pvp::swing(id, 0)], false);
                if kind == gameplay::AttackKind::Knockback {
                    self.sound(id, kind.sound(), source.state.position);
                }
                let hit = self.damage_target(
                    id,
                    target,
                    damage,
                    DamageContext {
                        source: source.state,
                        target: target_pose.state,
                        damage_type,
                        kind,
                        secondary: false,
                    },
                );
                if !hit {
                    self.sound(
                        id,
                        "minecraft:entity.player.attack.nodamage",
                        source.state.position,
                    );
                    return;
                }
                if kind == gameplay::AttackKind::Knockback {
                    let mut game = shared.lock().unwrap();
                    game.refresh_terrain();
                    game.advance_motion(now);
                    game.velocity[0] *= 0.6;
                    game.velocity[2] *= 0.6;
                    drop(game);
                    self.critical(id, gameplay::Delivery::SprintStopped);
                    self.sound(
                        id,
                        gameplay::AttackKind::Strong.sound(),
                        source.state.position,
                    );
                } else if kind == gameplay::AttackKind::Sweep {
                    self.sweep(
                        id,
                        target,
                        source.state,
                        target_pose.state,
                        scale,
                        damage_type,
                    );
                } else {
                    self.sound(id, kind.sound(), source.state.position);
                    if kind == gameplay::AttackKind::Critical {
                        self.broadcast_critical(target, vec![pvp::critical(target)], true);
                    }
                }
            }
            pvp::Action::Select(_)
            | pvp::Action::Swap
            | pvp::Action::Click { .. }
            | pvp::Action::Drop
            | pvp::Action::Close => {
                let old = game.inventory.main();
                game.inventory.apply(&action);
                let changed = old != game.inventory.main();
                if changed {
                    if let Some(mining) = game.mining.take() {
                        self.broadcast_critical(
                            id,
                            vec![pvp::block_destruction(id, mining.pos, -1)],
                            false,
                        );
                    }
                    game.reset_charge(now);
                }
                let attributes = changed.then(|| pvp::attributes(id, game.inventory.main()));
                let contents = game.inventory.packet();
                let equipment = pvp::equipment(id, game.inventory.main(), game.inventory.off());
                drop(game);
                let mut own = vec![contents];
                if let Some(attributes) = attributes {
                    own.push(attributes);
                }
                self.critical(id, gameplay::Delivery::packets(None, own));
                self.broadcast_critical(id, vec![equipment], false);
            }
            pvp::Action::Interact {
                entity: target,
                hand,
                ..
            } => {
                if self.members[&id].visible.contains(&target)
                    && let Some(target) = self.members.get(&target)
                {
                    let target = target.pending.lock().unwrap().latest;
                    if gameplay::in_reach(source.state, target.state) {
                        let _ = hand; /* Supported swords have no player right-click effect. */
                    }
                }
            }
            pvp::Action::Use { hand, sequence } => {
                let _ = hand;
                drop(game);
                self.critical(
                    id,
                    gameplay::Delivery::packets(
                        None,
                        vec![smolmc_protocol_777::play::block_ack(sequence)],
                    ),
                );
            }
            pvp::Action::Release => {}
            pvp::Action::Respawn => unreachable!(),
        }
    }
}
