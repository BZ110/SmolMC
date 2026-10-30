//! Optional arena combat/inventory. Mutations are serialized by the directory;
//! shared state locks are per session and never cover IO or Java callbacks.
use smolmc_protocol_777::{
    presence::EntitySnapshot,
    pvp::{self, Action, Item},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::Instant;
pub type Shared = Arc<Mutex<State>>;
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub enabled: bool,
    pub spawn: [f64; 3],
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            spawn: smolmc_world::SPAWN,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Inventory {
    pub slots: [Item; 46],
    pub cursor: Item,
    pub selected: u8,
    pub revision: i32,
}
impl Default for Inventory {
    fn default() -> Self {
        let mut slots = [Item::Empty; 46];
        slots[36] = Item::IronSword;
        slots[9] = Item::DiamondSword;
        Self {
            slots,
            cursor: Item::Empty,
            selected: 0,
            revision: 0,
        }
    }
}
impl Inventory {
    pub fn main(&self) -> Item {
        self.slots[36 + self.selected as usize]
    }
    pub fn off(&self) -> Item {
        self.slots[45]
    }
    pub fn packet(&self) -> Vec<u8> {
        pvp::inventory(self.revision, &self.slots, self.cursor)
    }
    pub fn apply(&mut self, a: &Action) -> bool {
        match *a {
            Action::Select(slot) => {
                if slot >= 9 || slot == self.selected {
                    return false;
                }
                self.selected = slot;
            }
            Action::Swap => self.slots.swap(36 + self.selected as usize, 45),
            Action::Close => {
                if self.cursor == Item::Empty {
                    return false;
                }
                if let Some(slot) = (9..45).find(|&slot| self.slots[slot] == Item::Empty) {
                    self.slots[slot] = self.cursor;
                    self.cursor = Item::Empty;
                } else {
                    return false;
                }
            }
            Action::Click {
                state,
                slot,
                button,
                kind,
            } => {
                if state != self.revision || !(9..46).contains(&slot) {
                    return false;
                }
                let slot = slot as usize;
                match kind {
                    0 if button <= 1 => std::mem::swap(&mut self.slots[slot], &mut self.cursor),
                    1 if button == 0 || button == 1 => {
                        let range = if slot >= 36 { 9..36 } else { 36..45 };
                        if let Some(target) =
                            range.into_iter().find(|&i| self.slots[i] == Item::Empty)
                        {
                            self.slots.swap(slot, target);
                        } else {
                            return false;
                        }
                    }
                    2 if button <= 8 || button == 40 => self.slots.swap(
                        slot,
                        if button == 40 {
                            45
                        } else {
                            36 + button as usize
                        },
                    ),
                    _ => return false,
                }
            }
            _ => return false,
        }
        self.revision = (self.revision + 1) & 32767;
        true
    }
}
pub(crate) struct Notices {
    pub worlds: Arc<crate::worlds::Worlds>,
    pub world: smolmc_world::WorldHandle,
}
impl Clone for Notices {
    fn clone(&self) -> Self {
        Self {
            worlds: Arc::clone(&self.worlds),
            world: self.world,
        }
    }
}
#[derive(Clone)]
pub(crate) struct BlockOwner {
    pub terrain: tokio::sync::watch::Receiver<smolmc_world::InstanceOverlay>,
    pub worlds: Arc<crate::worlds::Worlds>,
    pub handle: smolmc_world::WorldHandle,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mining {
    pub pos: [i32; 3],
    pub material: smolmc_world::Material,
    pub item: Item,
    pub start: u64,
    pub last_stage: i32,
}
pub fn mining_progress(material: smolmc_world::Material, item: Item, grounded: bool) -> f32 {
    let info = material.info();
    if info.hardness < 0. {
        return 0.;
    }
    let mut speed = if item == Item::IronPickaxe && info.pickaxe {
        6.
    } else if item.sword() {
        info.sword_speed
    } else {
        1.
    };
    if !grounded {
        speed /= 5.;
    }
    let correct = !info.requires_tool || (item == Item::IronPickaxe && info.iron_correct);
    speed / info.hardness / if correct { 30. } else { 100. }
}
pub struct State {
    pub(crate) mining: Option<Mining>,
    pub(crate) blocks: Option<BlockOwner>,
    pub(crate) block_sequence: i32,
    pub(crate) combat_enabled: bool,
    pub(crate) mode: i32,
    pub terrain: Option<smolmc_world::collision::TerrainSnapshot>,
    pub(crate) notices: Option<Notices>,
    pub(crate) trace: bool,
    pub inventory: Inventory,
    pub health: f32,
    pub dead: bool,
    origin: Instant,
    last_attack: u64,
    last_hurt: u64,
    last_damage: f32,
    pub velocity: [f64; 3],
    motion_tick: u64,
    motion_y: f64,
    motion_ground: bool,
    pub respawn_requested: bool,
    pub fall_distance: f64,
    previous_y: f64,
    pub known_movement: [f64; 3],
    moved_this_tick: bool,
    action_credit: f64,
    action_at: Instant,
    block_credit: f64,
    block_at: Instant,
    known_position: [f64; 3],
}
impl State {
    pub fn snapshot(&self) -> crate::control::CombatSnapshot {
        crate::control::CombatSnapshot {
            health: self.health,
            dead: self.dead,
            selected: self.inventory.selected,
            main: self.inventory.main().id(),
            off: self.inventory.off().id(),
        }
    }

    pub fn new(now: Instant, spawn: [f64; 3]) -> Self {
        Self {
            blocks: None,
            mining: None,
            block_sequence: -1,
            combat_enabled: true,
            mode: 2,
            terrain: None,
            notices: None,
            trace: false,
            inventory: Inventory::default(),
            health: 20.,
            dead: false,
            origin: now,
            last_attack: 0,
            last_hurt: 0,
            last_damage: 0.,
            velocity: [0.; 3],
            motion_tick: 0,
            motion_y: spawn[1],
            motion_ground: spawn[1] <= 64.01,
            respawn_requested: false,
            fall_distance: 0.,
            previous_y: spawn[1],
            known_movement: [0.; 3],
            moved_this_tick: false,
            action_credit: 8.,
            action_at: now,
            block_credit: 16.,
            block_at: now,
            known_position: spawn,
        }
    }
    pub(crate) fn refresh_terrain(&mut self) {
        if let (Some(owner), Some(terrain)) = (&self.blocks, &mut self.terrain) {
            terrain.overlay = owner.terrain.borrow().clone();
        }
    }
    pub(crate) fn occupies(&self, pos: [i32; 3]) -> bool {
        smolmc_world::collision::Aabb::player(self.known_position, false).intersects(
            smolmc_world::collision::Aabb {
                min: pos.map(|v| v as f64),
                max: pos.map(|v| v as f64 + 1.),
            },
        )
    }
    pub fn build_kit(&mut self) {
        self.inventory.slots[37] = Item::IronPickaxe;
        for (slot, name) in [
            (38, "minecraft:stone"),
            (39, "minecraft:oak_planks"),
            (40, "minecraft:cobblestone"),
            (41, "minecraft:white_wool"),
        ] {
            if let Some(m) = smolmc_world::Material::parse(name, None) {
                self.inventory.slots[slot] = Item::Block {
                    state: m.state(),
                    item: m.info().item,
                };
            }
        }
    }
    pub(crate) fn admit_build_action(&mut self, now: Instant) -> bool {
        self.block_credit = (self.block_credit
            + now.saturating_duration_since(self.block_at).as_secs_f64() * 64.)
            .min(16.);
        self.block_at = now;
        if self.block_credit < 1. {
            false
        } else {
            self.block_credit -= 1.;
            true
        }
    }
    pub fn admit_action(&mut self, now: Instant) -> bool {
        self.action_credit = (self.action_credit
            + now.saturating_duration_since(self.action_at).as_secs_f64() * 32.)
            .min(8.);
        self.action_at = now;
        if self.action_credit < 1. {
            false
        } else {
            self.action_credit -= 1.;
            true
        }
    }
    pub(crate) fn align_clock(&mut self, epoch: Instant) {
        self.origin = epoch;
        self.motion_tick = self.tick(Instant::now());
    }
    pub fn tick(&self, now: Instant) -> u64 {
        (now.saturating_duration_since(self.origin).as_nanos() / 50_000_000) as u64
    }

    pub fn attack_strength(&self, now: Instant) -> f32 {
        let speed = 4.0 + self.inventory.main().speed_modifier().map_or(0., f64::from);
        let delay = (20.0 / speed) as f32;
        let ticks = self.tick(now);
        let elapsed = if self.last_attack == 0 {
            20
        } else {
            ticks.saturating_sub(self.last_attack - 1)
        };
        ((elapsed as f32 + 0.5) / delay).clamp(0., 1.)
    }
    pub fn attack_damage(&mut self, now: Instant) -> f32 {
        let ticks = self.tick(now);
        let scale = self.attack_strength(now);
        self.last_attack = ticks + 1;
        let base = match self.inventory.main() {
            Item::IronSword => 6.,
            Item::DiamondSword => 7.,
            Item::IronPickaxe => 4.,
            _ => 1.,
        };
        base * (0.2 + scale * scale * 0.8)
    }
    pub fn reset_charge(&mut self, now: Instant) {
        self.last_attack = self.tick(now) + 1;
    }
    pub fn hurt(&mut self, damage: f32, now: Instant) -> Option<(f32, bool)> {
        if self.dead || !damage.is_finite() || damage <= 0. {
            return None;
        }
        let tick = self.tick(now);
        let fresh = self.last_damage == 0. || tick.saturating_sub(self.last_hurt) >= 10;
        let applied = if fresh {
            self.last_hurt = tick;
            damage
        } else {
            if damage <= self.last_damage {
                return None;
            }
            damage - self.last_damage
        };
        self.last_damage = damage;
        self.health = (self.health - applied).max(0.);
        self.dead = self.health == 0.;
        Some((applied, fresh))
    }
    pub fn reset(&mut self, now: Instant, spawn: [f64; 3]) {
        let notices = self.notices.clone();
        let terrain = self.terrain.clone();
        let blocks = self.blocks.clone();
        let combat_enabled = self.combat_enabled;
        let mode = self.mode;
        let trace = self.trace;
        let epoch = self.origin;
        *self = Self::new(now, spawn);
        self.notices = notices;
        self.terrain = terrain;
        self.blocks = blocks;
        self.combat_enabled = combat_enabled;
        self.mode = mode;
        if self.blocks.is_some() {
            self.build_kit();
        }
        self.trace = trace;
        self.origin = epoch;
        self.motion_tick = self.tick(now);
    }
    pub fn can_crit(&self, pose: EntitySnapshot, now: Instant) -> bool {
        self.attack_strength(now) > 0.9
            && self.fall_distance > 0.
            && !pose.on_ground
            && !pose.sprinting
            && (self.terrain.is_some() || pose.position[1] > 64.01)
    }
    pub fn finish_tick(&mut self) {
        if !self.moved_this_tick {
            self.known_movement = [0.; 3];
        }
        self.moved_this_tick = false;
    }
    pub fn classify(&self, pose: EntitySnapshot, now: Instant) -> AttackKind {
        if self.attack_strength(now) <= 0.9 {
            return AttackKind::Weak;
        }
        if pose.sprinting {
            return AttackKind::Knockback;
        }
        if self.can_crit(pose, now) {
            return AttackKind::Critical;
        }
        let limit = 0.1f32 as f64 * 2.5;
        if pose.on_ground
            && self.known_movement[0].powi(2) + self.known_movement[2].powi(2) < limit * limit
            && self.inventory.main().sword()
        {
            return AttackKind::Sweep;
        }
        AttackKind::Strong
    }
    pub fn pose(&mut self, pose: EntitySnapshot, now: Instant) {
        let position = pose.position;
        let moved_up = position[1] > self.previous_y;
        self.known_movement = std::array::from_fn(|i| position[i] - self.known_position[i]);
        self.known_position = position;
        self.moved_this_tick = true;
        let supported = self.terrain.as_ref().map_or(position[1] <= 64.01, |t| {
            t.grounded(position, pose.crouching)
        });
        if supported {
            self.fall_distance = 0.;
        } else if position[1] < self.previous_y {
            self.fall_distance += self.previous_y - position[1];
        } else if position[1] > self.previous_y {
            self.fall_distance = 0.;
        }
        self.previous_y = position[1];
        self.advance_motion(now);
        // Server movement handling invokes jumpFromGround on a rising departure.
        // Client displacement is known movement, never the server motion vector.
        if self.motion_ground && !pose.on_ground && moved_up {
            self.velocity[1] = self.velocity[1].max(0.42_f32 as f64);
            if pose.sprinting {
                let axes = yaw_axes(pose.rotation[0]);
                self.velocity[0] += axes[0] * 0.2;
                self.velocity[2] += axes[1] * 0.2;
            }
        }
        self.motion_y = position[1];
        self.motion_ground = supported;
        // Classic arena policy: settle contact before a same-phase hit. An
        // airborne falling vector must not survive a confirmed roof landing.
        // This is the ordinary post-contact vanilla gravity/drag value; the
        // knockback equation itself and airborne Y handling stay unchanged.
        let resting_y = -0.08 * 0.98_f32 as f64;
        if self.motion_ground && self.velocity[1] < resting_y {
            self.velocity[1] = resting_y;
        }
    }
    pub fn motion_state(&self) -> ([f64; 3], f64, bool) {
        (self.velocity, self.motion_y, self.motion_ground)
    }
    pub fn reposition(&mut self, now: Instant, spawn: [f64; 3]) {
        self.mining = None;
        self.velocity = [0.; 3];
        self.motion_y = spawn[1];
        self.motion_ground = self
            .terrain
            .as_ref()
            .map_or(spawn[1] <= 64.01, |t| t.grounded(spawn, false));
        self.motion_tick = self.tick(now);
        self.previous_y = spawn[1];
        self.known_position = spawn;
        self.known_movement = [0.; 3];
        self.fall_distance = 0.;
        self.moved_this_tick = false;
    }
    /// Lazy flat-stone subset of LivingEntity.travelInAir. No player/world timer.
    pub fn advance_motion(&mut self, now: Instant) {
        let tick = self.tick(now);
        let elapsed = tick.saturating_sub(self.motion_tick);
        self.motion_tick = tick;
        // At the supported height/speed bounds 256 ticks suffice to land and
        // settle below vanilla's zero thresholds; bound work after long idles.
        for _ in 0..elapsed.min(256) {
            if self.velocity[0].powi(2) + self.velocity[2].powi(2) < 9e-6 {
                self.velocity[0] = 0.;
                self.velocity[2] = 0.;
            }
            if self.velocity[1].abs() < 0.003 {
                self.velocity[1] = 0.;
            }
            let friction = if self.motion_ground {
                (0.6_f32 * 0.91_f32) as f64
            } else {
                0.91_f32 as f64
            };
            if let Some(terrain) = &self.terrain {
                if let Some(resolved) = smolmc_world::collision::resolve(
                    self.known_position,
                    self.velocity,
                    false,
                    false,
                    |p| terrain.block(p),
                ) {
                    for axis in 0..3 {
                        if (resolved.delta[axis] - self.velocity[axis]).abs() > 1e-7 {
                            self.velocity[axis] = 0.;
                        }
                    }
                    // Classify from confirmed contact, not predicted next-step collision.
                    self.motion_ground = terrain.grounded(self.known_position, false);
                }
                self.velocity[0] *= friction;
                self.velocity[2] *= friction;
                self.velocity[1] = (self.velocity[1] - 0.08) * 0.98_f32 as f64;
                continue;
            }
            let accepted_y = self.motion_y;
            let next_y = accepted_y + self.velocity[1];
            if next_y < 64. {
                self.motion_y = 64.;
                self.motion_ground = true;
                self.velocity[1] = 0.;
            } else {
                self.motion_y = next_y;
                if self.velocity[1] != 0. {
                    self.motion_ground = false;
                }
            }
            // ServerGamePacketListenerImpl.tickPlayer restores the accepted
            // position after doTick, while retaining motion/collision results.
            self.motion_y = accepted_y;
            self.velocity[0] *= friction;
            self.velocity[2] *= friction;
            self.velocity[1] = (self.velocity[1] - 0.08) * 0.98_f32 as f64;
        }
    }
}
pub fn in_reach(a: EntitySnapshot, b: EntitySnapshot) -> bool {
    let eye = [
        a.position[0],
        a.position[1] + if a.crouching { 1.27 } else { 1.62 },
        a.position[2],
    ];
    let h = if b.crouching { 1.5 } else { 1.8 };
    let min = [b.position[0] - 0.3, b.position[1], b.position[2] - 0.3];
    let max = [b.position[0] + 0.3, b.position[1] + h, b.position[2] + 0.3];
    eye.into_iter()
        .enumerate()
        .map(|(i, v)| (v - v.clamp(min[i], max[i])).powi(2))
        .sum::<f64>()
        <= 36.
}
/// 1.19.4 Mth uses float multiplication/indexing for its 65536-entry table.
/// Compute only the selected entries, avoiding a retained 256 KiB table.
pub fn yaw_axes(yaw: f32) -> [f64; 2] {
    let radians = yaw * 0.017453292_f32;
    let sin = ((radians * 10430.378_f32) as i32 & 65535) as f64;
    let cos = ((radians * 10430.378_f32 + 16384_f32) as i32 & 65535) as f64;
    let entry = |index: f64| (index * std::f64::consts::PI * 2. / 65536.).sin() as f32 as f64;
    [-entry(sin), entry(cos)]
}
/// Preserve classic 1.19.4 packet precision inside the current LP-vector wire
/// envelope. Security/framing/encryption still use protocol 777 unchanged.
pub fn classic_velocity(v: [f64; 3]) -> [f64; 3] {
    v.map(|n| ((n.clamp(-3.9, 3.9) * 8000.) as i32) as f64 / 8000.)
}
pub fn directed_knockback(v: [f64; 3], mut away: [f64; 2], ground: bool, power: f64) -> [f64; 3] {
    if away[0] * away[0] + away[1] * away[1] < 9.999999747378752E-6 {
        // Same bounded provider RNG already used by authentication, not bespoke
        // cryptography. Rare coincident positions use vanilla's triangular offsets.
        for _ in 0..16 {
            let mut bytes = [0u8; 32];
            if openssl::rand::rand_bytes(&mut bytes).is_err() {
                break;
            }
            let unit = |i: usize| {
                ((u64::from_be_bytes(bytes[i..i + 8].try_into().unwrap()) >> 11) as f64)
                    / (1u64 << 53) as f64
            };
            away = [(unit(0) - unit(8)) * 0.01, (unit(16) - unit(24)) * 0.01];
            if away[0] * away[0] + away[1] * away[1] >= 9.999999747378752E-6 {
                break;
            }
        }
        if away[0] * away[0] + away[1] * away[1] < 9.999999747378752E-6 {
            away = [1., 0.];
        }
    }
    let length = (away[0] * away[0] + away[1] * away[1]).sqrt();
    [
        v[0] / 2. + away[0] / length * power,
        if ground {
            (v[1] / 2. + power).min(0.4)
        } else {
            v[1]
        },
        v[2] / 2. + away[1] / length * power,
    ]
}
pub fn knockback(v: [f64; 3], a: EntitySnapshot, b: EntitySnapshot, power: f64) -> [f64; 3] {
    directed_knockback(
        v,
        [b.position[0] - a.position[0], b.position[2] - a.position[2]],
        b.on_ground,
        power,
    )
}
pub fn in_sweep(attacker: EntitySnapshot, primary: EntitySnapshot, other: EntitySnapshot) -> bool {
    let ph = if primary.crouching { 1.5 } else { 1.8 };
    let oh = if other.crouching { 1.5 } else { 1.8 };
    (0..3)
        .map(|i| (attacker.position[i] - other.position[i]).powi(2))
        .sum::<f64>()
        < 9.
        && (primary.position[0] - other.position[0]).abs() < 1.6
        && (primary.position[2] - other.position[2]).abs() < 1.6
        && other.position[1] < primary.position[1] + ph + 0.25
        && other.position[1] + oh > primary.position[1] - 0.25
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackKind {
    Weak,
    Strong,
    Critical,
    Sweep,
    Knockback,
}
impl AttackKind {
    pub fn sound(self) -> &'static str {
        match self {
            Self::Weak => "minecraft:entity.player.attack.weak",
            Self::Strong => "minecraft:entity.player.attack.strong",
            Self::Critical => "minecraft:entity.player.attack.crit",
            Self::Sweep => "minecraft:entity.player.attack.sweep",
            Self::Knockback => "minecraft:entity.player.attack.knockback",
        }
    }
}
#[derive(Debug)]
pub enum Delivery {
    TerrainCommitted {
        at: Instant,
        world: smolmc_world::WorldHandle,
        packets: Arc<[Vec<u8>]>,
    },
    Packets {
        entity: Option<i32>,
        packets: Arc<[Vec<u8>]>,
        at: Instant,
    },
    Died,
    SprintStopped,
    Respawn,
}
impl Delivery {
    pub fn packets(entity: Option<i32>, packets: Vec<Vec<u8>>) -> Self {
        Self::Packets {
            entity,
            packets: packets.into(),
            at: Instant::now(),
        }
    }
}
pub const QUEUE: usize = 32;
pub const ACTION_WINDOW: Duration = Duration::from_millis(50);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combat_sender_budget_has_bounded_burst_and_refills_without_a_timer() {
        let now = Instant::now();
        let mut g = State::new(now, [0., 64., 0.]);
        for _ in 0..8 {
            assert!(g.admit_action(now));
        }
        assert!(!g.admit_action(now));
        assert!(g.admit_action(now + Duration::from_millis(32)));
    }
    #[test]
    fn sword_attack_classes_and_vanilla_knockback_equations() {
        let now = Instant::now();
        let mut g = State::new(now, [0., 64., 0.]);
        let mut pose = EntitySnapshot {
            position: [0., 64., 0.],
            rotation: [0.; 2],
            on_ground: true,
            crouching: false,
            sprinting: false,
        };
        assert_eq!(g.classify(pose, now), AttackKind::Sweep);
        g.known_movement = [0.3, 0., 0.];
        assert_eq!(g.classify(pose, now), AttackKind::Strong);
        pose.sprinting = true;
        assert_eq!(g.classify(pose, now), AttackKind::Knockback);
        pose.sprinting = false;
        pose.on_ground = false;
        pose.position[1] = 64.9;
        g.fall_distance = 0.1;
        assert_eq!(g.classify(pose, now), AttackKind::Critical);
        g.reset_charge(now);
        assert_eq!(g.classify(pose, now), AttackKind::Weak);
        let base = directed_knockback([0.2, -0.1, -0.3], [0., 1.], true, 0.4);
        for (a, b) in base.into_iter().zip([0.1, 0.35, 0.25]) {
            assert!((a - b).abs() < 1e-12);
        }
        let sprint = directed_knockback(base, yaw_axes(90.), true, 0.5);
        assert!((sprint[0] + 0.45).abs() < 1e-12);
        assert_eq!(sprint[1], 0.4);
        assert!((sprint[2] - 0.125).abs() < 1e-12);
        let airborne = directed_knockback([0.2, -0.1, -0.3], [0., 1.], false, 0.4);
        assert_eq!(airborne[1], -0.1);
        let mut target = pose;
        target.position = [0., 64., 1.];
        let mut near = target;
        near.position = [1., 64., 1.];
        assert!(in_sweep(pose, target, near));
        near.position = [3., 64., 1.];
        assert!(!in_sweep(pose, target, near));
    }
    #[test]
    fn charged_falling_critical_requires_descent_and_excludes_sprinting_or_ground() {
        let now = Instant::now();
        let mut g = State::new(now, [0., 64., 0.]);
        let mut pose = EntitySnapshot {
            position: [0., 65., 0.],
            rotation: [0.; 2],
            on_ground: false,
            crouching: false,
            sprinting: false,
        };
        g.pose(pose, now + Duration::from_millis(50));
        assert!(!g.can_crit(pose, now + Duration::from_millis(50)));
        pose.position[1] = 64.9;
        g.pose(pose, now + Duration::from_millis(100));
        assert!(g.can_crit(pose, now + Duration::from_millis(100)));
        assert_eq!(g.attack_damage(now + Duration::from_millis(100)) * 1.5, 9.);
        assert!(!g.can_crit(pose, now + Duration::from_millis(150)));
        pose.sprinting = true;
        assert!(!g.can_crit(pose, now + Duration::from_secs(1)));
        pose.sprinting = false;
        pose.on_ground = true;
        assert!(!g.can_crit(pose, now + Duration::from_secs(1)));
        pose.position = [0., 64., 0.];
        g.pose(pose, now + Duration::from_secs(1));
        assert_eq!(g.fall_distance, 0.);
    }
    #[test]
    fn flat_motion_is_separate_from_displacement_and_consecutive_hits_preserve_air_y() {
        let now = Instant::now();
        let mut g = State::new(now, [0., 64., 0.]);
        g.advance_motion(now + Duration::from_millis(50));
        assert!((g.velocity[1] - -0.0784000015258789).abs() < 1e-14);
        let ground = directed_knockback(g.velocity, [0., 1.], true, 0.4000000059604645);
        assert!((ground[1] - 0.36080000519752503).abs() < 1e-14);
        let pose = EntitySnapshot {
            position: [0., 64.42, 0.],
            rotation: [0.; 2],
            on_ground: false,
            crouching: false,
            sprinting: false,
        };
        g.pose(pose, now + Duration::from_millis(50));
        assert_eq!(g.velocity[1], 0.42_f32 as f64);
        g.advance_motion(now + Duration::from_millis(100));
        assert!((g.velocity[1] - 0.33319999363422365).abs() < 1e-14);
        let air = directed_knockback(g.velocity, [0., 1.], false, 0.4000000059604645);
        assert_eq!(air[1], g.velocity[1]);
        // Receiving client positions neither overwrites motion with measured
        // delta nor gives every consecutive airborne hit another vertical jump.
        g.pose(
            EntitySnapshot {
                position: [0., 64.7532, 0.],
                ..pose
            },
            now + Duration::from_millis(100),
        );
        assert_eq!(g.velocity[1], air[1]);
        g.advance_motion(now + Duration::from_secs(300));
        assert_eq!(g.motion_y, 64.7532);
        assert!(g.velocity[1].is_finite());
    }
    #[test]
    fn server_landing_changes_knockback_even_before_the_next_client_position() {
        let now = Instant::now();
        let mut g = State::new(now, [8., 64.05, 8.]);
        g.velocity = [0., -0.2, 0.];
        g.advance_motion(now + Duration::from_millis(50));
        let (velocity, y, ground) = g.motion_state();
        assert_eq!(y, 64.05);
        assert!(ground);
        let resolved = directed_knockback(velocity, [0., 1.], ground, 0.4000000059604645);
        assert!((resolved[1] - 0.36080000519752503).abs() < 1e-14);
        assert!(directed_knockback(velocity, [0., 1.], false, 0.4000000059604645)[1] < 0.);
    }
    #[test]
    fn classic_ground_contact_prevents_marked_consecutive_hit_height_dip() {
        let now = Instant::now();
        let mut g = State::new(now, [0., 64.12, 0.]);
        g.velocity = [0., -0.30153472366278034, 0.];
        let pose = EntitySnapshot {
            position: [0., 64., 0.],
            rotation: [0.; 2],
            on_ground: true,
            crouching: false,
            sprinting: false,
        };
        g.pose(pose, now);
        let impulse = directed_knockback(g.velocity, [0., 1.], true, 0.4000000059604645);
        assert!((impulse[1] - 0.36080000519752503).abs() < 1e-14);
        assert_eq!(classic_velocity(impulse)[1], 0.36075);
        let mut air = State::new(now, [0., 65., 0.]);
        air.velocity = [0., -0.30153472366278034, 0.];
        air.pose(
            EntitySnapshot {
                position: [0., 64.12, 0.],
                on_ground: false,
                ..pose
            },
            now,
        );
        assert_eq!(
            directed_knockback(air.velocity, [0., 1.], false, 0.4)[1],
            -0.30153472366278034
        );
    }
    #[test]
    fn authoritative_inventory_pickup_shift_swap_and_stale_rejection() {
        let mut inv = Inventory::default();
        assert_eq!(inv.main(), Item::IronSword);
        assert_eq!(inv.off(), Item::Empty);
        assert!(inv.apply(&Action::Swap));
        assert_eq!(inv.main(), Item::Empty);
        assert_eq!(inv.off(), Item::IronSword);
        let before = inv.slots;
        assert!(!inv.apply(&Action::Click {
            state: 0,
            slot: 9,
            button: 0,
            kind: 0
        }));
        assert_eq!(inv.slots, before);
        assert!(inv.apply(&Action::Click {
            state: 1,
            slot: 9,
            button: 0,
            kind: 0
        }));
        assert_eq!(inv.cursor, Item::DiamondSword);
        assert!(inv.apply(&Action::Click {
            state: 2,
            slot: 36,
            button: 1,
            kind: 0
        }));
        assert_eq!(inv.main(), Item::DiamondSword);
        assert_eq!(inv.cursor, Item::Empty);
        assert!(!inv.apply(&Action::Click {
            state: 3,
            slot: 5,
            button: 0,
            kind: 0
        }));
        assert!(!inv.apply(&Action::Select(10)));
        let count = inv.slots.iter().filter(|i| **i != Item::Empty).count();
        for n in 0..100 {
            let revision = inv.revision;
            inv.apply(&Action::Click {
                state: revision,
                slot: 36,
                button: (n % 2) as u8,
                kind: 1,
            });
        }
        assert_eq!(
            inv.slots.iter().filter(|i| **i != Item::Empty).count(),
            count
        );
    }
    #[test]
    fn cooldown_quantized_ticks_and_hurt_difference_window() {
        let now = Instant::now();
        let mut g = State::new(now, [8., 65., 8.]);
        assert_eq!(g.attack_damage(now), 6.);
        assert!((g.attack_damage(now) - 1.20768).abs() < 0.0001);
        assert_eq!(g.attack_damage(now + Duration::from_millis(650)), 6.);
        assert_eq!(g.hurt(6., now), Some((6., true)));
        assert_eq!(g.hurt(6., now + Duration::from_millis(499)), None);
        assert_eq!(
            g.hurt(7., now + Duration::from_millis(499)),
            Some((1., false))
        );
        assert_eq!(g.health, 13.);
        assert_eq!(
            g.hurt(7., now + Duration::from_millis(500)),
            Some((7., true))
        );
        assert_eq!(g.hurt(f32::NAN, now), None);
        g.hurt(20., now + Duration::from_secs(1));
        assert!(g.dead);
        assert_eq!(g.health, 0.);
        assert_eq!(g.hurt(1., now + Duration::from_secs(2)), None);
        g.reset(now + Duration::from_secs(3), [16., 65., 16.]);
        assert_eq!(g.health, 20.);
        assert!(!g.dead);
        assert_eq!(g.inventory.main(), Item::IronSword);
    }
    #[test]
    fn reach_uses_eye_to_aabb_and_knockback_is_finite() {
        let a = EntitySnapshot {
            position: [0., 64., 0.],
            rotation: [0.; 2],
            on_ground: true,
            crouching: false,
            sprinting: false,
        };
        let mut b = a;
        b.position = [0., 64., 3.];
        assert!(in_reach(a, b));
        b.position[2] = 6.31;
        assert!(!in_reach(a, b));
        b.position = [0., 64., 1.];
        assert_eq!(knockback([0.; 3], a, b, 0.4), [0., 0.4, 0.4]);
        assert!(knockback([0.; 3], a, a, 0.4).iter().all(|v| v.is_finite()));
    }
}
