//! World-local bounded dropped items, spatial pickup/merge and shared scheduling.
use super::*;
use smolmc_protocol_777::{items as wire, pvp::Item};
use smolmc_world::{
    Material,
    collision::{Aabb, TerrainSnapshot},
};
use std::{
    sync::atomic::{AtomicI32, AtomicUsize},
    time::Duration,
};
static NEXT_ITEM: AtomicI32 = AtomicI32::new(1_000_000_000);
#[derive(Debug)]
pub struct Reservation {
    id: i32,
    world: Arc<AtomicUsize>,
    shared: Arc<AtomicUsize>,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.world.fetch_sub(1, Relaxed);
        self.shared.fetch_sub(1, Relaxed);
    }
}
#[derive(Debug)]
pub struct Outcome {
    pub pos: [i32; 3],
    pub loot: Item,
    pub cost: u16,
    pub consume: Option<usize>,
    pub reservation: Option<Reservation>,
}
/// Reserved directory capacity makes commit effects independent of socket IO.
pub(crate) struct Completion {
    permit: mpsc::OwnedPermit<Command>,
    instance: WorldId,
    entity: i32,
    token: u64,
    game: gameplay::Shared,
    outcome: Outcome,
}
impl Completion {
    pub(super) fn reserve(
        index: &InstanceDir,
        instance: WorldId,
        entity: i32,
        outcome: Outcome,
    ) -> Option<Self> {
        let permit = index.sender.as_ref()?.clone().try_reserve_owned().ok()?;
        let member = index.members.get(&entity)?;
        Some(Self {
            permit,
            instance,
            entity,
            token: member.token,
            game: member.game.clone()?,
            outcome,
        })
    }
    pub(crate) fn finish(self, ok: bool) {
        self.permit.send(Command::BlockComplete {
            instance: self.instance,
            entity: self.entity,
            token: self.token,
            game: self.game,
            ok,
            outcome: self.outcome,
        });
    }
}
pub(super) fn complete_inventory(shared: &gameplay::Shared, ok: bool, outcome: &Outcome) -> bool {
    let mut game = shared.lock().unwrap();
    if !game.pending_block {
        return false;
    }
    game.pending_block = false;
    game.pending_commit = false;
    if ok {
        if let Some(slot) = outcome.consume {
            game.inventory.slots[slot].take(1);
            game.inventory.changed();
        } else {
            game.inventory.wear_main(outcome.cost);
        }
    }
    if !game.item_active
        && let Some((accounts, uuid)) = &game.account
    {
        accounts.lock().unwrap().insert(
            *uuid,
            crate::inventory::AccountEntry {
                inventory: game.inventory.clone(),
                pending: false,
            },
        );
    }
    true
}
pub(super) struct DropItem {
    stack: Item,
    position: [f64; 3],
    velocity: [f64; 3],
    pickup: Instant,
    expires: Instant,
    pub(super) viewers: HashSet<i32>,
    _reservation: Reservation,
}
#[derive(Default)]
pub(super) struct Drops {
    pub(super) entities: HashMap<i32, DropItem>,
    cells: HashMap<ChunkPos, HashSet<i32>>,
    count: Arc<AtomicUsize>,
}
fn cell(p: [f64; 3]) -> ChunkPos {
    ChunkPos {
        x: (p[0] / 16.).floor() as i32,
        z: (p[2] / 16.).floor() as i32,
    }
}
fn nearby(p: [f64; 3]) -> impl Iterator<Item = ChunkPos> {
    let c = cell(p);
    (-1..=1).flat_map(move |x| {
        (-1..=1).map(move |z| ChunkPos {
            x: c.x + x,
            z: c.z + z,
        })
    })
}
pub(crate) fn loot(material: Material, item: Item) -> Item {
    static RULES: std::sync::OnceLock<HashMap<String, i32>> = std::sync::OnceLock::new();
    let info = material.info();
    if info.hardness < 0.
        || info.requires_tool && !(item.info().is_some_and(|i| i.pickaxe) && info.iron_correct)
    {
        return Item::Empty;
    }
    RULES
        .get_or_init(|| {
            serde_json::from_str(include_str!("../../data/block_loot.json"))
                .expect("checked loot facts")
        })
        .get(info.name)
        .and_then(|&id| Item::new(id, 1, 0))
        .unwrap_or(Item::Empty)
}
impl InstanceDir {
    pub(super) fn reserve_item(&self, config: gameplay::ItemConfig) -> Option<Reservation> {
        self.drops
            .count
            .try_update(Relaxed, Relaxed, |n| {
                (n < config.per_world).then_some(n + 1)
            })
            .ok()?;
        if self
            .item_count
            .try_update(Relaxed, Relaxed, |n| (n < config.shared).then_some(n + 1))
            .is_err()
        {
            self.drops.count.fetch_sub(1, Relaxed);
            return None;
        }
        let id = match NEXT_ITEM.try_update(Relaxed, Relaxed, |n| n.checked_add(1)) {
            Ok(id) => id,
            Err(_) => {
                self.drops.count.fetch_sub(1, Relaxed);
                self.item_count.fetch_sub(1, Relaxed);
                return None;
            }
        };
        Some(Reservation {
            id,
            world: Arc::clone(&self.drops.count),
            shared: Arc::clone(&self.item_count),
        })
    }
    pub(super) fn item_spawn(
        &mut self,
        stack: Item,
        position: [f64; 3],
        velocity: [f64; 3],
        delay: Duration,
        reservation: Reservation,
    ) {
        if stack == Item::Empty {
            return;
        }
        let id = reservation.id;
        let now = Instant::now();
        self.drops.entities.insert(
            id,
            DropItem {
                stack,
                position,
                velocity,
                pickup: now + delay,
                expires: now + Duration::from_secs(300),
                viewers: HashSet::new(),
                _reservation: reservation,
            },
        );
        self.drops
            .cells
            .entry(cell(position))
            .or_default()
            .insert(id);
        self.item_publish(id, true);
    }
    fn item_publish(&mut self, id: i32, moved: bool) {
        let item = &self.drops.entities[&id];
        let pos = cell(item.position);
        let mut recipients = self.subscribers.get(&pos).cloned().unwrap_or_default();
        // Remove old viewers after a cell crossing, without querying other worlds.
        recipients.extend(item.viewers.iter().copied());
        for viewer in recipients {
            let item = &self.drops.entities[&id];
            let member = &self.members[&viewer];
            let pose = member.pending.lock().unwrap().latest;
            let had = member.item_visible.contains(&id);
            let visible =
                pose.ready && pose.view.contains(pos) && (had || member.item_visible.len() < 256);
            let packets = if visible {
                if !had {
                    vec![
                        wire::spawn(id, item.position, item.velocity),
                        wire::metadata(id, item.stack),
                    ]
                } else {
                    let mut p = if moved {
                        vec![]
                    } else {
                        vec![wire::metadata(id, item.stack)]
                    };
                    if moved {
                        p.push(presence::position(
                            id,
                            EntitySnapshot {
                                position: item.position,
                                rotation: [0.; 2],
                                on_ground: item.velocity == [0.; 3],
                                crouching: false,
                                sprinting: false,
                            },
                        ));
                        p.push(pvp::velocity(id, item.velocity));
                    }
                    p
                }
            } else if had {
                vec![presence::entity_remove(id)]
            } else {
                continue;
            };
            self.item_packets.entry(viewer).or_default().extend(packets);
            let member = self.members.get_mut(&viewer).unwrap();
            if visible {
                member.item_visible.insert(id);
            } else {
                member.item_visible.remove(&id);
            }
            let item = self.drops.entities.get_mut(&id).unwrap();
            if visible {
                item.viewers.insert(viewer);
            } else {
                item.viewers.remove(&viewer);
            }
        }
    }
    fn item_remove(&mut self, id: i32) {
        if let Some(item) = self.drops.entities.remove(&id) {
            remove_index(&mut self.drops.cells, cell(item.position), id);
            for v in item.viewers {
                if let Some(m) = self.members.get_mut(&v) {
                    m.item_visible.remove(&id);
                    self.item_refill.insert(v);
                    self.item_packets
                        .entry(v)
                        .or_default()
                        .push(presence::entity_remove(id));
                }
            }
        }
    }
    pub(super) fn item_view(&mut self, viewer: i32) {
        if self.drops.entities.is_empty() {
            return;
        }
        let pose = self.members[&viewer].pending.lock().unwrap().latest;
        let mut candidates = self.members[&viewer].item_visible.clone();
        if pose.ready {
            for c in pose.view.positions() {
                if let Some(ids) = self.drops.cells.get(&c) {
                    candidates.extend(ids);
                }
            }
        }
        let mut candidates: Vec<_> = candidates.into_iter().collect();
        candidates.sort_unstable();
        for id in candidates {
            let item = &self.drops.entities[&id];
            let had = self.members[&viewer].item_visible.contains(&id);
            let visible = pose.ready
                && pose.view.contains(cell(item.position))
                && (had || self.members[&viewer].item_visible.len() < 256);
            if visible == had {
                continue;
            }
            let packets = if visible {
                vec![
                    wire::spawn(id, item.position, item.velocity),
                    wire::metadata(id, item.stack),
                ]
            } else {
                vec![presence::entity_remove(id)]
            };
            self.item_packets.entry(viewer).or_default().extend(packets);
            let member = self.members.get_mut(&viewer).unwrap();
            let item = self.drops.entities.get_mut(&id).unwrap();
            if visible {
                member.item_visible.insert(id);
                item.viewers.insert(viewer);
            } else {
                member.item_visible.remove(&id);
                item.viewers.remove(&viewer);
            }
        }
    }
    pub(super) fn flush_items(&mut self, world: WorldId) {
        for viewer in std::mem::take(&mut self.item_refill) {
            if self.members.contains_key(&viewer) {
                self.item_view(viewer);
            }
        }
        for id in std::mem::take(&mut self.item_dirty) {
            let game = self.members[&id].game.as_ref().unwrap().lock().unwrap();
            let own = game.inventory.packet();
            let equipment = game.inventory.equipment(id);
            drop(game);
            self.critical(id, gameplay::Delivery::packets(None, vec![own]));
            if self.item_equipment.remove(&id) {
                for &v in &self.members[&id].viewers {
                    self.item_packets
                        .entry(v)
                        .or_default()
                        .push(equipment.clone());
                }
            }
        }
        for (viewer, packets) in std::mem::take(&mut self.item_packets) {
            if packets.len() > 1024 || packets.iter().map(Vec::len).sum::<usize>() > 65536 {
                if let Some(m) = self.members.get(&viewer) {
                    m.closed.send_replace(true);
                    m.wake.notify_one();
                }
                continue;
            }
            self.critical(
                viewer,
                gameplay::Delivery::Items {
                    world,
                    visible: self.members[&viewer]
                        .item_visible
                        .iter()
                        .copied()
                        .collect::<Vec<_>>()
                        .into(),
                    packets: packets.into(),
                },
            );
        }
    }
    pub(super) fn inventory_sync(&self, id: i32) {
        let Some(g) = self.members[&id].game.as_ref() else {
            return;
        };
        let g = g.lock().unwrap();
        let own = vec![g.inventory.packet(), g.inventory.attributes(id)];
        let equipment = g.inventory.equipment(id);
        drop(g);
        self.critical(id, gameplay::Delivery::packets(None, own));
        self.broadcast_critical(id, vec![equipment], false);
    }
    #[cfg(test)]
    fn block_complete(&mut self, id: i32, ok: bool, outcome: Outcome) {
        let game = self.members[&id].game.clone().unwrap();
        if complete_inventory(&game, ok, &outcome) {
            self.block_committed(id, ok, outcome);
        }
    }
    pub(super) fn block_committed(&mut self, id: i32, ok: bool, outcome: Outcome) {
        if ok
            && outcome.loot != Item::Empty
            && let Some(r) = outcome.reservation
        {
            self.item_spawn(
                outcome.loot,
                outcome.pos.map(|v| v as f64 + 0.5),
                [0., 0.2, 0.],
                Duration::from_millis(500),
                r,
            );
        }
        self.inventory_sync(id);
    }
    pub(super) fn item_tick(&mut self, now: Instant) {
        let geometry = self
            .members
            .values()
            .find_map(|m| m.game.as_ref())
            .and_then(|g| {
                let mut g = g.lock().unwrap();
                g.refresh_terrain();
                g.terrain.clone()
            });
        let mut changed = HashSet::new();
        let ids: Vec<_> = self.drops.entities.keys().copied().collect();
        for id in ids {
            let Some(item) = self.drops.entities.get_mut(&id) else {
                continue;
            };
            if now >= item.expires || item.position[1] < -128. {
                self.item_remove(id);
                continue;
            }
            let old = item.position;
            {
                let was_moving = item.velocity != [0.; 3];
                step(item, geometry.as_ref());
                let new = item.position;
                if cell(old) != cell(new) {
                    remove_index(&mut self.drops.cells, cell(old), id);
                    self.drops.cells.entry(cell(new)).or_default().insert(id);
                }
                if was_moving || old != new {
                    self.item_publish(id, true);
                }
            }
            self.item_merge_inner(id, &mut changed);
            if self.drops.entities.contains_key(&id) {
                self.item_pickup(id, now);
            }
        }
        for id in changed {
            if self.drops.entities.contains_key(&id) {
                self.item_publish(id, false);
            }
        }
    }
    #[cfg(test)]
    fn item_merge(&mut self, id: i32) {
        let mut changed = HashSet::new();
        self.item_merge_inner(id, &mut changed);
        for id in changed {
            self.item_publish(id, false);
        }
    }
    fn item_merge_inner(&mut self, id: i32, changed: &mut HashSet<i32>) {
        let Some(item) = self.drops.entities.get(&id) else {
            return;
        };
        if item.stack.count() == item.stack.max_count() {
            return;
        }
        let position = item.position;
        let candidates: Vec<_> = nearby(position)
            .filter_map(|c| self.drops.cells.get(&c))
            .flatten()
            .copied()
            .filter(|&i| i > id)
            .take(128)
            .collect();
        for other in candidates {
            let a = &self.drops.entities[&id];
            let Some(b) = self.drops.entities.get(&other) else {
                continue;
            };
            if !a.stack.compatible(b.stack)
                || a.stack.count() as u16 + b.stack.count() as u16 > a.stack.max_count() as u16
                || (a.position[0] - b.position[0]).abs() > 0.75
                || (a.position[2] - b.position[2]).abs() > 0.75
                || (a.position[1] - b.position[1]).abs() > 0.25
            {
                continue;
            }
            let count = a.stack.count() + b.stack.count();
            let pickup = a.pickup.max(b.pickup);
            let expires = a.expires.max(b.expires);
            let a = self.drops.entities.get_mut(&id).unwrap();
            a.stack = a.stack.with_count(count);
            a.pickup = pickup;
            a.expires = expires;
            self.item_remove(other);
            changed.insert(id);
        }
    }
    fn item_pickup(&mut self, id: i32, now: Instant) {
        let Some(item) = self.drops.entities.get(&id) else {
            return;
        };
        if now < item.pickup {
            return;
        }
        let position = item.position;
        let candidates: Vec<_> = nearby(position)
            .filter_map(|c| self.occupants.get(&c))
            .flatten()
            .copied()
            .collect();
        for player in candidates {
            let pose = self.members[&player].pending.lock().unwrap().latest;
            if !pose.ready
                || (position[0] - pose.state.position[0]).abs() > 1.425
                || (position[2] - pose.state.position[2]).abs() > 1.425
                || position[1] + 0.25 < pose.state.position[1] - 0.5
                || position[1]
                    > pose.state.position[1] + if pose.state.crouching { 2.0 } else { 2.3 }
            {
                continue;
            }
            let Some(shared) = self.members[&player].game.clone() else {
                continue;
            };
            let mut game = shared.lock().unwrap();
            if !game.item_active
                || game.dead
                || game.pending_block
                || game.item_config.policy == gameplay::InventoryPolicy::ArenaKit
            {
                continue;
            }
            let equipment = game.inventory.equipment(player);
            let item = self.drops.entities.get_mut(&id).unwrap();
            let before = item.stack.count();
            item.stack = game.inventory.insert(item.stack);
            let got = before - item.stack.count();
            let empty = item.stack == Item::Empty;
            let equip_changed = equipment != game.inventory.equipment(player);
            drop(game);
            if got == 0 {
                continue;
            }
            let recipients = self.drops.entities[&id].viewers.clone();
            for v in recipients {
                self.item_packets
                    .entry(v)
                    .or_default()
                    .push(wire::take(id, player, got));
            }
            self.item_dirty.insert(player);
            if equip_changed {
                self.item_equipment.insert(player);
            }
            if empty {
                self.item_remove(id);
                break;
            } else {
                self.item_publish(id, false);
            }
        }
    }
}
fn step(item: &mut DropItem, terrain: Option<&TerrainSnapshot>) {
    item.velocity[1] -= 0.04;
    let b = Aabb {
        min: [
            item.position[0] - 0.125,
            item.position[1],
            item.position[2] - 0.125,
        ],
        max: [
            item.position[0] + 0.125,
            item.position[1] + 0.25,
            item.position[2] + 0.125,
        ],
    };
    let result = smolmc_world::collision::resolve_box(b, item.velocity, false, |p| {
        terrain.map_or_else(|| smolmc_world::VirtualStone.block(p[1]), |t| t.block(p))
    });
    let Some(r) = result else {
        item.velocity = [0.; 3];
        return;
    };
    for axis in 0..3 {
        item.position[axis] += r.delta[axis];
        if (r.delta[axis] - item.velocity[axis]).abs() > 1e-7 {
            item.velocity[axis] = 0.;
        }
    }
    let friction = if r.grounded {
        0.98_f32 as f64 * 0.6_f32 as f64
    } else {
        0.98_f32 as f64
    };
    item.velocity[0] *= friction;
    item.velocity[2] *= friction;
    item.velocity[1] *= 0.98_f32 as f64;
    if r.grounded && item.velocity[0].abs() + item.velocity[2].abs() < 0.003 {
        item.velocity = [0.; 3];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn block(n: u8) -> Item {
        Item::named("minecraft:cobblestone", n).unwrap()
    }
    fn fixture() -> (InstanceDir, gameplay::Shared, Inbox) {
        let mut d = InstanceDir::new(Arc::default());
        let mut g = gameplay::State::new(Instant::now(), [8., 64., 8.]);
        g.item_config.policy = gameplay::InventoryPolicy::ItemsKeep;
        g.inventory.slots.fill(Item::Empty);
        let game = Arc::new(SlotMutex::new(g));
        let mut view = ChunkView::default();
        while let Some(c) = view.next_missing() {
            view.insert(c);
        }
        let pose = Replica {
            state: EntitySnapshot {
                position: [8., 64., 8.],
                rotation: [0.; 2],
                on_ground: true,
                crouching: false,
                sprinting: false,
            },
            view,
            ready: true,
            version: 0,
            at: Instant::now(),
        };
        let inbox = d
            .join_with_game(
                1,
                1,
                Arc::new(Profile {
                    uuid: [1; 16],
                    name: "Items".into(),
                    properties: vec![],
                }),
                pending(pose),
                Some(game.clone()),
            )
            .unwrap();
        (d, game, inbox)
    }
    fn spawn(d: &mut InstanceDir, stack: Item, p: [f64; 3], delay: Duration) -> i32 {
        let r = d.reserve_item(gameplay::ItemConfig::default()).unwrap();
        d.item_spawn(stack, p, [0., 0.2, 0.], delay, r);
        *d.drops.entities.keys().max().unwrap()
    }
    #[test]
    fn grounded_loot_rules_require_harvest_tools_and_preserve_colors() {
        for (name, tool, drop) in [
            ("stone", Item::IronPickaxe, "cobblestone"),
            ("stone", Item::IronSword, "air"),
            ("dirt", Item::Empty, "dirt"),
            ("oak_planks", Item::Empty, "oak_planks"),
            ("red_wool", Item::IronSword, "red_wool"),
            ("bedrock", Item::IronPickaxe, "air"),
        ] {
            let material = Material::parse(&format!("minecraft:{name}"), None).unwrap();
            let got = loot(material, tool);
            if drop == "air" {
                assert_eq!(got, Item::Empty);
            } else {
                assert_eq!(got, Item::named(&format!("minecraft:{drop}"), 1).unwrap());
            }
        }
        let worn = Item::new(1047, 1, 249).unwrap();
        assert_eq!(loot(Material::Stone, worn), block(1));
        assert_eq!(
            gameplay::mining_progress(Material::Stone, worn, true),
            gameplay::mining_progress(Material::Stone, Item::IronPickaxe, true)
        );
    }
    #[test]
    fn delay_partial_full_duplicate_and_competing_pickups_conserve_stacks() {
        let (mut d, g, _inbox) = fixture();
        let now = Instant::now();
        let id = spawn(&mut d, block(8), [8., 64., 8.], Duration::from_secs(1));
        {
            let mut g = g.lock().unwrap();
            for i in 9..45 {
                g.inventory.slots[i] = block(64);
            }
            g.inventory.slots[36] = block(61);
        }
        d.item_pickup(id, now);
        assert_eq!(d.drops.entities[&id].stack.count(), 8);
        d.item_pickup(id, now + Duration::from_secs(2));
        assert_eq!(d.drops.entities[&id].stack.count(), 5);
        assert_eq!(g.lock().unwrap().inventory.main().count(), 64);
        d.item_pickup(id, now + Duration::from_secs(2));
        assert_eq!(d.drops.entities[&id].stack.count(), 5);
        let p = *d.members[&1].state.borrow();
        let mut other = gameplay::State::new(now, [8., 64., 8.]);
        other.item_config.policy = gameplay::InventoryPolicy::ItemsKeep;
        other.inventory.slots.fill(Item::Empty);
        let other = Arc::new(SlotMutex::new(other));
        let _other_inbox = d
            .join_with_game(
                2,
                2,
                Arc::new(Profile {
                    uuid: [2; 16],
                    name: "Other".into(),
                    properties: vec![],
                }),
                pending(p),
                Some(other.clone()),
            )
            .unwrap();
        d.item_pickup(id, now + Duration::from_secs(2));
        assert!(!d.drops.entities.contains_key(&id));
        assert_eq!(other.lock().unwrap().inventory.main().count(), 5);
        assert_eq!(d.drops.count.load(Relaxed), 0);
    }
    #[test]
    fn item_motion_merging_component_identity_lifetime_and_budget_cleanup() {
        let (mut d, _g, _inbox) = fixture();
        let id = spawn(&mut d, block(20), [12., 66., 8.], Duration::from_secs(10));
        let second = spawn(&mut d, block(30), [12., 66., 8.], Duration::from_secs(20));
        d.item_merge(id);
        assert_eq!(d.drops.entities.len(), 1);
        assert!(!d.drops.entities.contains_key(&second));
        assert_eq!(d.drops.entities[&id].stack.count(), 50);
        assert_eq!(d.drops.count.load(Relaxed), 1);
        let first = d.drops.entities[&id].position;
        d.item_tick(Instant::now());
        assert_ne!(d.drops.entities[&id].position, first);
        for _ in 0..80 {
            d.item_tick(Instant::now());
        }
        assert!((d.drops.entities[&id].position[1] - 64.).abs() < 1e-8);
        d.item_tick(Instant::now() + Duration::from_secs(301));
        assert!(d.drops.entities.is_empty());
        assert_eq!(d.item_count.load(Relaxed), 0);
        let config = gameplay::ItemConfig {
            per_world: 1,
            shared: 1,
            ..Default::default()
        };
        let r = d.reserve_item(config).unwrap();
        assert!(d.reserve_item(config).is_none());
        drop(r);
        assert!(d.reserve_item(config).is_some());
    }
    #[test]
    fn failed_and_duplicate_terrain_completions_have_no_loot_or_durability_effect() {
        let (mut d, g, _inbox) = fixture();
        g.lock().unwrap().inventory.slots[36] = Item::IronPickaxe;
        for ok in [false, true] {
            g.lock().unwrap().pending_block = true;
            let r = d.reserve_item(gameplay::ItemConfig::default()).unwrap();
            d.block_complete(
                1,
                ok,
                Outcome {
                    pos: [10, 64, 8],
                    loot: block(1),
                    cost: 1,
                    consume: None,
                    reservation: Some(r),
                },
            );
            assert_eq!(g.lock().unwrap().inventory.main().damage(), u16::from(ok));
            assert_eq!(d.drops.entities.len(), usize::from(ok));
        }
        d.block_complete(
            1,
            true,
            Outcome {
                pos: [10, 64, 8],
                loot: block(1),
                cost: 1,
                consume: None,
                reservation: None,
            },
        );
        assert_eq!(g.lock().unwrap().inventory.main().damage(), 1);
    }
    #[test]
    fn disconnect_during_commit_cannot_refresh_a_tool_or_lose_its_receipt() {
        for ok in [false, true] {
            let (_, g, _inbox) = fixture();
            g.lock().unwrap().inventory.slots[36] = Item::new(1047, 1, 17).unwrap();
            let accounts = crate::inventory::Accounts::default();
            let lease =
                crate::inventory::AccountLease::attach(accounts.clone(), [7; 16], g.clone())
                    .unwrap();
            {
                let mut game = g.lock().unwrap();
                game.pending_block = true;
                game.pending_commit = true;
            }
            drop(lease);
            let fresh = Arc::new(SlotMutex::new(gameplay::State::new(
                Instant::now(),
                [8., 64., 8.],
            )));
            assert!(
                crate::inventory::AccountLease::attach(accounts.clone(), [7; 16], fresh.clone())
                    .is_err()
            );
            let outcome = Outcome {
                pos: [10, 64, 8],
                loot: Item::Empty,
                cost: 1,
                consume: None,
                reservation: None,
            };
            assert!(complete_inventory(&g, ok, &outcome));
            assert!(!complete_inventory(&g, ok, &outcome));
            let _lease =
                crate::inventory::AccountLease::attach(accounts, [7; 16], fresh.clone()).unwrap();
            assert_eq!(
                fresh.lock().unwrap().inventory.main().damage(),
                17 + u16::from(ok)
            );
        }
    }
    #[test]
    fn concurrent_disconnect_and_commit_preserve_the_final_inventory() {
        for i in 0..128 {
            let (_, game, _inbox) = fixture();
            game.lock().unwrap().inventory.slots[36] = Item::new(1047, 1, 17).unwrap();
            let accounts = crate::inventory::Accounts::default();
            let lease =
                crate::inventory::AccountLease::attach(accounts.clone(), [7; 16], game.clone())
                    .unwrap();
            {
                let mut g = game.lock().unwrap();
                g.pending_block = true;
                g.pending_commit = true;
            }
            let barrier = std::sync::Barrier::new(2);
            let ok = i % 2 == 0;
            std::thread::scope(|scope| {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    drop(lease);
                });
                let game = &game;
                scope.spawn(move || {
                    barrier.wait();
                    assert!(complete_inventory(
                        game,
                        ok,
                        &Outcome {
                            pos: [10, 64, 8],
                            loot: Item::Empty,
                            cost: 1,
                            consume: None,
                            reservation: None,
                        }
                    ));
                });
            });
            let fresh = Arc::new(SlotMutex::new(gameplay::State::new(
                Instant::now(),
                [8., 64., 8.],
            )));
            let _lease =
                crate::inventory::AccountLease::attach(accounts, [7; 16], fresh.clone()).unwrap();
            assert_eq!(
                fresh.lock().unwrap().inventory.main().damage(),
                17 + u16::from(ok)
            );
        }
    }
    #[test]
    fn independent_directories_isolate_items_and_slow_viewers_fail_closed() {
        let (mut a, _ga, mut inbox) = fixture();
        let (mut b, gb, _ib) = fixture();
        spawn(&mut a, block(1), [8., 64., 8.], Duration::ZERO);
        b.item_tick(Instant::now());
        assert_eq!(gb.lock().unwrap().inventory.main(), Item::Empty);
        a.item_tick(Instant::now());
        assert!(a.drops.entities.is_empty());
        assert!(b.drops.entities.is_empty());
        for _ in 0..=gameplay::QUEUE {
            a.critical(1, gameplay::Delivery::packets(None, vec![vec![1]]));
        }
        assert!(*inbox.closed.borrow_and_update());
        assert_eq!(a.metrics.slow_viewers.load(Relaxed), 1);
    }
    #[test]
    fn reliable_payload_leases_bound_readers_and_release_shared_charges() {
        let local = Arc::new(AtomicUsize::new(0));
        let shared = Arc::new(AtomicUsize::new(0));
        let a = gameplay::PacketCharge::reserve(local.clone(), shared.clone(), 64 * 1024).unwrap();
        let b = gameplay::PacketCharge::reserve(local.clone(), shared.clone(), 64 * 1024).unwrap();
        assert!(gameplay::PacketCharge::reserve(local.clone(), shared.clone(), 1).is_none());
        drop(a);
        drop(b);
        assert_eq!(local.load(Relaxed), 0);
        assert_eq!(shared.load(Relaxed), 0);
        let mut leases = Vec::new();
        for _ in 0..128 {
            leases.push(
                gameplay::PacketCharge::reserve(
                    Arc::new(AtomicUsize::new(0)),
                    shared.clone(),
                    64 * 1024,
                )
                .unwrap(),
            );
        }
        assert!(
            gameplay::PacketCharge::reserve(Arc::new(AtomicUsize::new(0)), shared.clone(), 1)
                .is_none()
        );
        assert_eq!(shared.load(Relaxed), 8 * 1024 * 1024);
        drop(leases);
        assert_eq!(shared.load(Relaxed), 0);
    }
    #[test]
    fn rapid_item_pile_batches_are_bounded_and_last_leave_releases_every_reservation() {
        let (mut d, _g, mut inbox) = fixture();
        for n in 0..1000 {
            spawn(
                &mut d,
                block(64),
                [8. + (n % 16) as f64 / 32., 68., 8.],
                Duration::from_secs(10),
            );
        }
        assert_eq!(d.drops.count.load(Relaxed), 1000);
        assert_eq!(d.members[&1].item_visible.len(), 256);
        d.flush_items(WorldId(1));
        let rx = inbox.critical.as_mut().unwrap();
        assert_eq!(rx.len(), 1);
        assert!(!*inbox.closed.borrow());
        rx.try_recv().unwrap();
        d.item_tick(Instant::now());
        d.flush_items(WorldId(1));
        assert_eq!(rx.len(), 1);
        assert!(!*inbox.closed.borrow());
        let count = Arc::clone(&d.item_count);
        d.leave(1, 1);
        drop(d);
        assert_eq!(count.load(Relaxed), 0);
    }
    #[test]
    fn armored_sixteen_player_sweep_fits_existing_critical_queues() {
        let (mut d, source, first) = fixture();
        let pose = *d.members[&1].state.borrow();
        let mut inboxes = vec![first];
        for id in 2..=16 {
            let mut g = gameplay::State::new(Instant::now(), pose.state.position);
            g.item_config.policy = gameplay::InventoryPolicy::ItemsKeep;
            for (slot, part) in [
                (5, "helmet"),
                (6, "chestplate"),
                (7, "leggings"),
                (8, "boots"),
            ] {
                g.inventory.slots[slot] =
                    Item::named(&format!("minecraft:iron_{part}"), 1).unwrap();
            }
            inboxes.push(
                d.join_with_game(
                    id,
                    id as u64,
                    Arc::new(Profile {
                        uuid: [id as u8; 16],
                        name: format!("Armor{id}"),
                        properties: vec![],
                    }),
                    pending(pose),
                    Some(Arc::new(SlotMutex::new(g))),
                )
                .unwrap(),
            );
        }
        source.lock().unwrap().inventory.slots[36] = Item::IronSword;
        d.critical_batch.replace(Some(HashMap::new()));
        d.action(
            1,
            pvp::Action::Attack(2),
            ActionSource {
                state: pose.state,
                ready: true,
            },
            0,
        );
        d.flush_critical_batch();
        assert_eq!(d.metrics.combat_hits.load(Relaxed), 15);
        assert_eq!(source.lock().unwrap().inventory.main().damage(), 1);
        for inbox in inboxes {
            assert!(!*inbox.closed.borrow());
            assert!(inbox.critical.as_ref().unwrap().len() <= gameplay::QUEUE);
        }
        assert_eq!(d.metrics.slow_viewers.load(Relaxed), 0);
    }
    #[test]
    #[ignore = "explicit short item memory/processing profile"]
    fn item_resource_profile() {
        fn rss() -> u64 {
            let out = std::process::Command::new("ps")
                .args(["-o", "rss=", "-p", &std::process::id().to_string()])
                .output()
                .unwrap();
            assert!(out.status.success());
            String::from_utf8(out.stdout)
                .unwrap()
                .trim()
                .parse()
                .unwrap()
        }
        fn drain(d: &mut InstanceDir, inbox: &mut Inbox) {
            d.flush_items(WorldId(1));
            while inbox.critical.as_mut().unwrap().try_recv().is_ok() {}
            assert!(!*inbox.closed.borrow());
        }
        let (mut d, g, mut inbox) = fixture();
        let baseline = rss();
        let mut rows = vec![];
        for target in [100, 1000] {
            while d.drops.entities.len() < target {
                let n = d.drops.entities.len();
                spawn(
                    &mut d,
                    block(64),
                    [12. + (n % 16) as f64 / 32., 64., 8.],
                    Duration::from_secs(20),
                );
            }
            drain(&mut d, &mut inbox);
            for _ in 0..80 {
                d.item_tick(Instant::now());
                drain(&mut d, &mut inbox);
            }
            let memory = rss();
            let start = std::time::Instant::now();
            for _ in 0..100 {
                d.item_tick(Instant::now());
                drain(&mut d, &mut inbox);
            }
            let micros = start.elapsed().as_secs_f64() * 1e6 / 100.;
            rows.push(serde_json::json!({"items":target,"rss_kib":memory,"increment_from_baseline_kib":memory.saturating_sub(baseline),"settled_tick_mean_us":micros}));
        }
        drop(d);
        let (mut d, _g, mut inbox) = fixture();
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            spawn(&mut d, block(1), [12., 64., 8.], Duration::from_secs(20));
        }
        drain(&mut d, &mut inbox);
        d.item_tick(Instant::now());
        drain(&mut d, &mut inbox);
        let merge_us = start.elapsed().as_secs_f64() * 1e6;
        let merged = d.drops.entities.len();
        let total: u64 = d
            .drops
            .entities
            .values()
            .map(|i| i.stack.count() as u64)
            .sum();
        assert_eq!(total, 1000);
        let (mut pickup, pg, mut pi) = fixture();
        for _ in 0..1000 {
            spawn(&mut pickup, block(1), [8., 64., 8.], Duration::ZERO);
        }
        drain(&mut pickup, &mut pi);
        let start = std::time::Instant::now();
        pickup.item_tick(Instant::now());
        drain(&mut pickup, &mut pi);
        let pickup_us = start.elapsed().as_secs_f64() * 1e6;
        let count: u32 = pg
            .lock()
            .unwrap()
            .inventory
            .slots
            .iter()
            .map(|s| s.count() as u32)
            .sum();
        assert_eq!(count, 1000);
        assert!(pickup.drops.entities.is_empty());
        let _ = g;
        let report = serde_json::json!({"baseline_rss_kib":baseline,"rows":rows,"spawn_and_merge_1000_us":merge_us,"remaining_merged_entities":merged,"merge_conserved_items":total,"pickup_1000_processing_us":pickup_us,"pickup_conserved_items":count,"method":"Release in-process item owner microprobe: one active viewer, default budgets, real spatial merge/pickup and bounded queue drains; no socket encryption/client rendering. RSS is whole test-process KiB; allocation/page granularity applies. 100 settled processing passes per size, no sleep or idle CPU claim."});
        let path = std::env::var("SMOLMC_ITEMS_REPORT").unwrap();
        std::fs::write(path, format!("{report:#}\n")).unwrap();
        println!("{report}");
    }
}
