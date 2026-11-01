//! Bounded default-component items with authoritative count and damage patches.
use crate::login::Encoder;
#[derive(Debug)]
pub struct ItemInfo {
    pub id: i32,
    pub name: &'static str,
    pub max_count: u8,
    pub max_damage: u16,
    pub armor_slot: u8,
    pub armor: f32,
    pub toughness: f32,
    pub resistance: f64,
    pub attack: f32,
    pub speed: f32,
    pub mine_cost: u16,
    pub attack_cost: u16,
    pub state: i32,
    pub sword: bool,
    pub pickaxe: bool,
}
include!("item_catalog.rs");
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Item {
    id: i32,
    count: u8,
    damage: u16,
}
#[allow(non_upper_case_globals)]
impl Item {
    pub const Empty: Self = Self {
        id: 0,
        count: 0,
        damage: 0,
    };
    pub const IronSword: Self = Self {
        id: 1045,
        count: 1,
        damage: 0,
    };
    pub const DiamondSword: Self = Self {
        id: 1050,
        count: 1,
        damage: 0,
    };
    pub const IronPickaxe: Self = Self {
        id: 1047,
        count: 1,
        damage: 0,
    };
    pub fn new(id: i32, count: u8, damage: u16) -> Option<Self> {
        if count == 0 {
            return (id == 0 && damage == 0).then_some(Self::Empty);
        }
        let info = Self::info_id(id)?;
        (count <= info.max_count && (damage == 0 || damage < info.max_damage)).then_some(Self {
            id,
            count,
            damage,
        })
    }
    pub fn info_id(id: i32) -> Option<&'static ItemInfo> {
        ITEMS
            .binary_search_by_key(&id, |i| i.id)
            .ok()
            .map(|i| &ITEMS[i])
    }
    pub fn named(name: &str, count: u8) -> Option<Self> {
        let info = ITEMS.iter().find(|i| i.name == name)?;
        Self::new(info.id, count, 0)
    }
    pub fn info(self) -> Option<&'static ItemInfo> {
        Self::info_id(self.id)
    }
    pub fn id(self) -> i32 {
        self.id
    }
    pub fn count(self) -> u8 {
        self.count
    }
    pub fn damage(self) -> u16 {
        self.damage
    }
    pub fn max_count(self) -> u8 {
        self.info().map_or(0, |i| i.max_count)
    }
    pub fn compatible(self, other: Self) -> bool {
        self.id != 0 && self.id == other.id && self.damage == other.damage
    }
    pub fn with_count(self, count: u8) -> Self {
        if count == 0 {
            Self::Empty
        } else {
            Self::new(self.id, count, self.damage).expect("bounded server count")
        }
    }
    pub fn take(&mut self, count: u8) -> Self {
        let n = count.min(self.count);
        let out = self.with_count(n);
        *self = self.with_count(self.count - n);
        out
    }
    pub fn wear(&mut self, amount: u16) -> bool {
        let max = self.info().map_or(0, |i| i.max_damage);
        if max == 0 || amount == 0 {
            return false;
        }
        if self.damage.saturating_add(amount) >= max {
            *self = Self::Empty;
            true
        } else {
            self.damage += amount;
            false
        }
    }
    pub fn sword(self) -> bool {
        self.info().is_some_and(|i| i.sword)
    }
    pub fn speed_modifier(self) -> Option<f32> {
        self.info().and_then(|i| (i.speed != 0.).then_some(i.speed))
    }
    pub fn block_state(self) -> Option<i32> {
        self.info().and_then(|i| (i.state >= 0).then_some(i.state))
    }
    pub fn armor_slot(self) -> Option<usize> {
        self.info()
            .and_then(|i| (i.armor_slot != 0).then_some(i.armor_slot as usize))
    }
}
pub fn stack(out: &mut Encoder, item: Item) {
    out.varint(item.count as i32);
    if item.count == 0 {
        return;
    }
    out.varint(item.id);
    out.varint(i32::from(item.damage != 0));
    out.varint(0);
    if item.damage != 0 {
        out.varint(DAMAGE_COMPONENT);
        out.varint(item.damage as i32);
    }
}
pub fn equipment(id: i32, items: [Item; 6]) -> Vec<u8> {
    let mut out = Encoder::new(0x68);
    out.varint(id);
    for (slot, item) in items.into_iter().enumerate() {
        out.byte(slot as u8 | if slot != 5 { 128 } else { 0 });
        stack(&mut out, item);
    }
    out.finish()
}
/// Base armor and resistance attributes are authoritative.
pub fn attributes(id: i32, main: Item, armor: f32, toughness: f32, resistance: f64) -> Vec<u8> {
    let mut out = Encoder::new(0x86);
    out.varint(id);
    out.varint(4);
    out.varint(5);
    out.raw(&4f64.to_be_bytes());
    if let Some(speed) = main.speed_modifier() {
        out.varint(1);
        out.string("minecraft:base_attack_speed");
        out.raw(&(speed as f64).to_be_bytes());
        out.varint(0);
    } else {
        out.varint(0);
    }
    for (key, value) in [
        (1, armor as f64),
        (2, toughness as f64),
        (KNOCKBACK_ATTRIBUTE, resistance),
    ] {
        out.varint(key);
        out.raw(&value.to_be_bytes());
        out.varint(0);
    }
    out.finish()
}
pub fn spawn(id: i32, position: [f64; 3], velocity: [f64; 3]) -> Vec<u8> {
    let mut out = Encoder::new(1);
    out.varint(id);
    out.raw(&[0x53, 0x4d, 0x4f, 0x4c, 0x4d, 0x43, 0x40, 0, 0x80, 0, 0, 0]);
    out.raw(&id.to_be_bytes());
    out.varint(ITEM_ENTITY);
    for v in position {
        out.raw(&v.to_be_bytes());
    }
    crate::pvp::write_lp(&mut out, velocity);
    out.raw(&[0, 0, 0]);
    out.varint(0);
    out.finish()
}
pub fn metadata(id: i32, item: Item) -> Vec<u8> {
    let mut out = Encoder::new(0x65);
    out.varint(id);
    out.byte(8);
    out.varint(ITEM_SERIALIZER);
    stack(&mut out, item);
    out.byte(255);
    out.finish()
}
pub fn take(id: i32, player: i32, count: u8) -> Vec<u8> {
    let mut out = Encoder::new(0x7f);
    out.varint(id);
    out.varint(player);
    out.varint(count as i32);
    out.finish()
}
