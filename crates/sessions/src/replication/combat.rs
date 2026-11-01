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

fn delivery_bytes(event: &gameplay::Delivery) -> usize {
    std::mem::size_of::<gameplay::Delivery>()
        + match event {
            gameplay::Delivery::Packets { packets, .. }
            | gameplay::Delivery::Items { packets, .. }
            | gameplay::Delivery::TerrainCommitted { packets, .. } => packets
                .iter()
                .map(|p| p.len() + std::mem::size_of::<Vec<u8>>())
                .sum(),
            gameplay::Delivery::Batch(events) => events.iter().map(delivery_bytes).sum(),
            gameplay::Delivery::Charged { payload, .. } => delivery_bytes(payload),
            _ => 0,
        }
}
impl InstanceDir {
    pub(super) fn flush_critical_batch(&self) {
        if let Some(batch) = self.critical_batch.take() {
            for (id, batch) in batch {
                self.critical(id, gameplay::Delivery::Batch(batch.events));
            }
        }
    }
    pub(super) fn critical(&self, id: i32, message: gameplay::Delivery) {
        if let Some(batch) = self.critical_batch.borrow_mut().as_mut() {
            let pending = batch.entry(id).or_default();
            let bytes = if let gameplay::Delivery::Packets { packets, .. } = &message {
                packets.iter().map(Vec::len).sum()
            } else {
                0
            };
            if pending.events.len() < 128 && pending.bytes + bytes <= 65536 {
                pending.bytes += bytes;
                pending.events.push(message);
            } else if let Some(m) = self.members.get(&id) {
                if !m.closed.send_replace(true) {
                    self.metrics.slow_viewers.fetch_add(1, Relaxed);
                }
                m.wake.notify_one();
            }
            return;
        }
        let Some(member) = self.members.get(&id) else {
            return;
        };
        let Some(tx) = &member.critical else {
            return;
        };
        let bytes = delivery_bytes(&message);
        let Some(charge) = gameplay::PacketCharge::reserve(
            Arc::clone(&member.reliable_bytes),
            Arc::clone(&self.reliable_bytes),
            bytes,
        ) else {
            if !member.closed.send_replace(true) {
                self.metrics.slow_viewers.fetch_add(1, Relaxed);
            }
            member.wake.notify_one();
            return;
        };
        if tx
            .try_send(gameplay::Delivery::Charged {
                payload: Box::new(message),
                charge,
            })
            .is_err()
        {
            if !member.closed.send_replace(true) {
                self.metrics.slow_viewers.fetch_add(1, Relaxed);
            }
            member.wake.notify_one();
        }
    }

    pub(super) fn broadcast_critical(&self, id: i32, packets: Vec<Vec<u8>>, own: bool) {
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
        if !game.item_active {
            return false;
        }
        let old_health = game.health;
        let old_inventory = game.inventory.revision;
        let old_velocity = game.velocity;
        let grounded = game.motion_state().2;
        let Some((_, fresh)) = game.hurt(damage, Instant::now()) else {
            return false;
        };
        let resistance = if game.item_config.policy != gameplay::InventoryPolicy::ArenaKit {
            game.inventory.resistance()
        } else {
            0.
        };
        let inventory_changed = old_inventory != game.inventory.revision;
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
                0.4000000059604645 * (1. - resistance),
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
                extra * (1. - resistance),
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
        let inventory_update = inventory_changed.then(|| {
            (
                game.inventory.equipment(target),
                vec![game.inventory.packet(), game.inventory.attributes(target)],
            )
        });
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
        if let Some((equipment, own)) = inventory_update {
            // A sweep can wear every nearby target. Piggyback equipment on its
            // existing hurt delivery instead of adding one fanout event per hit.
            packets.push(equipment);
            self.critical(target, gameplay::Delivery::packets(None, own));
        }
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
        delayed: bool,
        retries: u8,
    ) -> Option<Vec<Vec<u8>>> {
        use smolmc_protocol_777::play;
        use smolmc_world::Material;
        let (clicked, sequence) = match action {
            pvp::Action::Dig { pos, sequence, .. } | pvp::Action::Place { pos, sequence, .. } => {
                (pos, sequence)
            }
            _ => unreachable!(),
        };
        let mining_before = game.mining;
        let mut corrections = vec![clicked];
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
            let fresh = (sequence > game.block_sequence || delayed) && subscribed;
            game.block_sequence = game.block_sequence.max(sequence);
            let mut desired = None;
            match action {
                pvp::Action::Dig { action, .. } => {
                    target = clicked;
                    let now = Instant::now();
                    let material = geometry.block(clicked);
                    let item = game.inventory.main();
                    if action == 2 {
                        game.delayed_block.retain(
                            |d| !matches!(d.action,pvp::Action::Dig{pos,..}if pos==clicked),
                        );
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
                            && mining.item.id() == item.id()
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
                                if action == 3 {
                                    let per_tick = gameplay::mining_progress(
                                        material,
                                        item,
                                        source.state.on_ground,
                                    );
                                    if per_tick.is_finite() && per_tick > 0. {
                                        let ticks = (1. / per_tick).ceil() as u64;
                                        let remaining = ticks
                                            .saturating_sub(
                                                game.tick(now).saturating_sub(mining.start) + 1,
                                            )
                                            .clamp(1, 6000);
                                        if game.delayed_block.len()<8&&!game.delayed_block.iter().any(|d|matches!(d.action,pvp::Action::Dig{pos,..}if pos==clicked)) {
                                            game.delayed_block.push(gameplay::DelayedBlock {
                                                mining: Some(mining),
                                                at: now
                                                    + std::time::Duration::from_millis(
                                                        remaining * 50,
                                                    ),
                                                action: pvp::Action::Dig {
                                                    pos: clicked,
                                                    face: 1,
                                                    action: 3,
                                                    sequence,
                                                },
                                                retries,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                pvp::Action::Place { hand, .. } => {
                    if geometry.block(clicked).info().replaceable {
                        target = clicked;
                    }
                    let held = if hand == 0 {
                        game.inventory.main()
                    } else {
                        game.inventory.off()
                    };
                    if let Some(state) = held.block_state()
                        && let Some(material) = Material::from_state(state)
                        && material.placeable()
                        && geometry.block(target).info().replaceable
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
                let mut edits = vec![(target, material)];
                let old = geometry.block(target);
                let info = old.info();
                if info.replaceable && info.boxes.is_empty() {
                    let dy = if info.properties.contains("half=lower") {
                        1
                    } else if info.properties.contains("half=upper") {
                        -1
                    } else {
                        0
                    };
                    if dy != 0 {
                        let paired = [target[0], target[1] + dy, target[2]];
                        if geometry.base.bounds.contains(paired)
                            && geometry.block(paired).info().name == info.name
                        {
                            edits.push((paired, Material::Air));
                        }
                    }
                }
                corrections.extend(edits.iter().map(|e| e.0));
                corrections.sort_unstable();
                corrections.dedup();
                let expected = edits.iter().map(|e| geometry.block(e.0)).collect();
                let outcome = if game.item_config.policy != gameplay::InventoryPolicy::ArenaKit {
                    let loot = if material.air() {
                        super::items::loot(geometry.block(target), game.inventory.main())
                    } else {
                        pvp::Item::Empty
                    };
                    let reservation = if loot != pvp::Item::Empty {
                        match self.reserve_item(game.item_config) {
                            Some(r) => Some(r),
                            None => {
                                return Some(vec![
                                    play::block_correction(target, geometry.block(target).state()),
                                    play::block_ack(sequence),
                                ]);
                            }
                        }
                    } else {
                        None
                    };
                    let consume = if let pvp::Action::Place { hand, .. } = action {
                        Some(if hand == 0 {
                            36 + game.inventory.selected as usize
                        } else {
                            45
                        })
                    } else {
                        None
                    };
                    let cost = if consume.is_none() && geometry.block(target).info().hardness != 0.
                    {
                        game.inventory.main().info().map_or(0, |i| i.mine_cost)
                    } else {
                        0
                    };
                    Some(super::items::Outcome {
                        pos: target,
                        loot,
                        cost,
                        consume,
                        reservation,
                    })
                } else {
                    None
                };
                let outcome = Some(outcome.unwrap_or(super::items::Outcome {
                    pos: target,
                    loot: pvp::Item::Empty,
                    cost: 0,
                    consume: None,
                    reservation: None,
                }));
                let pending = true;
                let completion = if let Some(outcome) = outcome {
                    match super::items::Completion::reserve(self, owner.handle.id, id, outcome) {
                        Some(c) => Some(c),
                        None => {
                            return Some(vec![
                                play::block_correction(target, geometry.block(target).state()),
                                play::block_ack(sequence),
                            ]);
                        }
                    }
                } else {
                    None
                };
                let reply = crate::worlds::lighting::Reply::Client {
                    completion,
                    sender: sender.clone(),
                    closed: self.members[&id].closed.clone(),
                    wake: Arc::clone(&self.members[&id].wake),
                    positions: corrections.clone(),
                    sequence,
                };
                match owner
                    .worlds
                    .submit_terrain(owner.handle, edits, true, Some(expected), reply)
                {
                    Ok(()) => {
                        game.pending_block = pending;
                        game.pending_commit = true;
                        return None;
                    }
                    Err(smolmc_world::WorldError::Busy)
                        if retries < 16 && game.delayed_block.len() < 8 =>
                    {
                        game.mining = game.mining.or(mining_before);
                        game.pending_block = true;
                        game.delayed_block.push(gameplay::DelayedBlock {
                            mining: game.mining,
                            at: Instant::now() + std::time::Duration::from_millis(20),
                            action,
                            retries: retries + 1,
                        });
                        return None;
                    }
                    Err(_) => {}
                }
            }
            geometry.overlay = owner.terrain.borrow().clone();
            corrections.push(target);
            corrections.sort_unstable();
            corrections.dedup();
            let mut packets: Vec<_> = corrections
                .into_iter()
                .map(|pos| play::block_correction(pos, geometry.block(pos).state()))
                .collect();
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
    pub(super) fn sync_block_deadline(&mut self, id: i32) {
        if let Some(old) = self.block_keys.remove(&id) {
            self.block_deadlines.remove(&(old, id));
        }
        if let Some(game) = self.members.get(&id).and_then(|m| m.game.as_ref()) {
            let game = game.lock().unwrap();
            if let Some(delay) = game.delayed_block.iter().min_by_key(|d| d.at)
                && game.item_active
                && !game.dead
            {
                self.block_keys.insert(id, delay.at);
                self.block_deadlines.insert((delay.at, id));
            }
        }
    }
    pub(super) fn replay_deferred(&mut self, id: i32) {
        for _ in 0..16 {
            if self.members[&id]
                .game
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .pending_block
            {
                break;
            }
            let Some((action, source, damage_type)) =
                self.members.get_mut(&id).unwrap().deferred.pop_front()
            else {
                break;
            };
            self.process_action(id, action, source, damage_type, false);
        }
        self.sync_block_deadline(id);
    }
    pub(super) fn process_block_deadlines(&mut self, now: Instant) {
        for _ in 0..16 {
            let Some(&(at, id)) = self.block_deadlines.first() else {
                break;
            };
            if at > now {
                break;
            }
            self.block_deadlines.remove(&(at, id));
            self.block_keys.remove(&id);
            let Some(shared) = self.members.get(&id).and_then(|m| m.game.clone()) else {
                continue;
            };
            let source = self.members[&id].pending.lock().unwrap().latest;
            let mut game = shared.lock().unwrap();
            let Some(index) = game
                .delayed_block
                .iter()
                .enumerate()
                .min_by_key(|(_, d)| d.at)
                .map(|(i, _)| i)
            else {
                continue;
            };
            if game.pending_commit {
                game.delayed_block[index].at = now + std::time::Duration::from_millis(20);
                drop(game);
                self.sync_block_deadline(id);
                continue;
            }
            let delay = game.delayed_block.remove(index);
            game.pending_block = false;
            if game.dead || !game.item_active || !source.ready {
                game.mining = None;
                continue;
            }
            let active_mining = game.mining;
            if let Some(mining) = delay.mining {
                game.mining = Some(mining);
            }
            game.advance_motion(now);
            let mut pose = source.state;
            pose.on_ground = game.motion_state().2;
            let packets = self.block_action(
                id,
                delay.action,
                ActionSource {
                    state: pose,
                    ready: source.ready,
                },
                &mut game,
                true,
                delay.retries,
            );
            if let Some(active) = active_mining
                && delay.mining.is_some_and(|m| m.pos != active.pos)
            {
                game.mining = Some(active);
            }
            drop(game);
            if let Some(packets) = packets {
                self.critical(id, gameplay::Delivery::packets(None, packets));
            }
            self.replay_deferred(id);
        }
    }
    pub(super) fn action(
        &mut self,
        id: i32,
        action: pvp::Action,
        source: ActionSource,
        damage_type: i32,
    ) {
        self.process_action(id, action, source, damage_type, true);
        self.sync_block_deadline(id);
    }
    fn process_action(
        &mut self,
        id: i32,
        action: pvp::Action,
        mut source: ActionSource,
        damage_type: i32,
        admit: bool,
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
        if !game.item_active {
            return;
        }
        game.refresh_terrain();
        let admitted = if !admit {
            true
        } else if game.blocks.is_some()
            && matches!(
                action,
                pvp::Action::Dig { .. } | pvp::Action::Place { .. } | pvp::Action::Punch
            )
        {
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
        if game.pending_block {
            drop(game);
            let member = self.members.get_mut(&id).unwrap();
            if member.deferred.len() < 16 {
                member.deferred.push_back((action, source, damage_type));
            } else {
                member.closed.send_replace(true);
                member.wake.notify_one();
            }
            return;
        }
        game.advance_motion(now);
        source.state.on_ground = game.motion_state().2;
        match action {
            pvp::Action::Dig { .. } | pvp::Action::Place { .. } => {
                let packets = self.block_action(id, action, source, &mut game, false, 0);
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
                {
                    let mut g = shared.lock().unwrap();
                    if g.item_config.policy != gameplay::InventoryPolicy::ArenaKit {
                        let cost = g.inventory.main().info().map_or(0, |i| i.attack_cost);
                        g.inventory.wear_main(cost);
                        drop(g);
                        self.inventory_sync(id);
                    }
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
            | pvp::Action::Drop { .. }
            | pvp::Action::Close => {
                let old = game.inventory.main();
                let allow = game.item_config.policy != gameplay::InventoryPolicy::ArenaKit;
                let wants_drop = match action {
                    pvp::Action::Drop { .. } => game.inventory.main() != pvp::Item::Empty,
                    pvp::Action::Click { slot: -999, .. } => {
                        game.inventory.cursor != pvp::Item::Empty
                    }
                    pvp::Action::Click { slot, kind: 4, .. } => {
                        (5..46).contains(&slot)
                            && game.inventory.slots[slot as usize] != pvp::Item::Empty
                            && game.inventory.cursor == pvp::Item::Empty
                    }
                    _ => false,
                };
                let reservation = if allow && wants_drop {
                    self.reserve_item(game.item_config)
                } else {
                    None
                };
                let dropped = game
                    .inventory
                    .transact(&action, allow && reservation.is_some())
                    .unwrap_or(pvp::Item::Empty);
                let changed = old != game.inventory.main();
                if changed {
                    game.delayed_block.clear();
                    if let Some(mining) = game.mining.take() {
                        self.broadcast_critical(
                            id,
                            vec![pvp::block_destruction(id, mining.pos, -1)],
                            false,
                        );
                    }
                    game.reset_charge(now);
                }
                let attributes = Some(game.inventory.attributes(id));
                let contents = game.inventory.packet();
                let equipment = game.inventory.equipment(id);
                drop(game);
                let mut own = vec![contents];
                if let Some(attributes) = attributes {
                    own.push(attributes);
                }
                self.critical(id, gameplay::Delivery::packets(None, own));
                self.broadcast_critical(id, vec![equipment], false);
                if dropped != pvp::Item::Empty
                    && let Some(reservation) = reservation
                {
                    let p = source.state.position;
                    let axes = gameplay::yaw_axes(source.state.rotation[0]);
                    self.item_spawn(
                        dropped,
                        [p[0], p[1] + 1.32, p[2]],
                        [axes[0] * 0.3, 0.1, axes[1] * 0.3],
                        std::time::Duration::from_secs(2),
                        reservation,
                    );
                }
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
                if game.item_config.policy != gameplay::InventoryPolicy::ArenaKit {
                    let index = if hand == 0 {
                        36 + game.inventory.selected as usize
                    } else {
                        45
                    };
                    if let Some(slot) = game.inventory.slots[index].armor_slot() {
                        game.inventory.slots.swap(index, slot);
                        game.inventory.changed();
                    }
                }
                drop(game);
                self.inventory_sync(id);
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
