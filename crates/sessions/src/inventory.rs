//! Authoritative player menu subset. Client-predicted stacks never enter this owner.
use smolmc_protocol_777::pvp::{self, Action, Item};
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
    pub fn equipment(&self, id: i32) -> Vec<u8> {
        smolmc_protocol_777::items::equipment(
            id,
            [
                self.main(),
                self.off(),
                self.slots[8],
                self.slots[7],
                self.slots[6],
                self.slots[5],
            ],
        )
    }
    pub fn attributes(&self, id: i32) -> Vec<u8> {
        let (armor, toughness) = self.armor();
        smolmc_protocol_777::items::attributes(id, self.main(), armor, toughness, self.resistance())
    }
    pub fn changed(&mut self) {
        self.revision = (self.revision + 1) & 32767;
    }
    fn accepts(slot: usize, item: Item) -> bool {
        (9..46).contains(&slot)
            || (5..9).contains(&slot) && (item == Item::Empty || item.armor_slot() == Some(slot))
    }
    pub fn insert_range(
        &mut self,
        mut item: Item,
        range: impl IntoIterator<Item = usize> + Clone,
    ) -> Item {
        for i in range.clone() {
            let dst = &mut self.slots[i];
            if dst.compatible(item) {
                let n = (dst.max_count() - dst.count()).min(item.count());
                *dst = dst.with_count(dst.count() + n);
                item = item.with_count(item.count() - n);
            }
        }
        for i in range {
            if item == Item::Empty {
                break;
            }
            if self.slots[i] == Item::Empty {
                self.slots[i] = item;
                item = Item::Empty;
            }
        }
        item
    }
    pub fn insert(&mut self, item: Item) -> Item {
        let left = self.insert_range(item, (36..45).chain(9..36));
        if left.count() != item.count() {
            self.changed();
        }
        left
    }
    pub fn apply(&mut self, a: &Action) -> bool {
        self.transact(a, false).is_some()
    }
    /// Some(empty) is an accepted operation, Some(stack) an atomic drop intent.
    /// Caller reserves world entity capacity BEFORE permitting drop intents.
    pub fn transact(&mut self, a: &Action, drop_allowed: bool) -> Option<Item> {
        let mut dropped = Item::Empty;
        match *a {
            Action::Select(slot) if slot < 9 && slot != self.selected => self.selected = slot,
            Action::Swap => self.slots.swap(36 + self.selected as usize, 45),
            Action::Close => {
                if self.cursor == Item::Empty {
                    return None;
                }
                self.cursor = self.insert_range(self.cursor, (36..45).chain(9..36));
            }
            Action::Drop { all } if drop_allowed => {
                let slot = &mut self.slots[36 + self.selected as usize];
                dropped = slot.take(if all { slot.count() } else { 1 });
            }
            Action::Click {
                state,
                slot,
                button,
                kind,
            } => {
                // Vanilla uses a mismatched revision to request a full resync;
                // it still executes the click against authoritative slots.
                if state < 0 {
                    return None;
                }
                if slot == -999 && kind == 0 && button <= 1 && drop_allowed {
                    dropped = self
                        .cursor
                        .take(if button == 0 { self.cursor.count() } else { 1 });
                } else {
                    if !(5..46).contains(&slot) {
                        return None;
                    }
                    let i = slot as usize;
                    match kind {
                        0 if button <= 1 => {
                            let dst = self.slots[i];
                            let cursor = self.cursor;
                            if dst == Item::Empty && cursor == Item::Empty {
                                return None;
                            }
                            if cursor == Item::Empty {
                                self.cursor = self.slots[i].take(if button == 0 {
                                    dst.count()
                                } else {
                                    dst.count().div_ceil(2)
                                });
                            } else if Self::accepts(i, cursor) {
                                if dst == Item::Empty || dst.compatible(cursor) {
                                    let cap = if i < 9 { 1 } else { cursor.max_count() };
                                    let n = (cap - dst.count()).min(if button == 0 {
                                        cursor.count()
                                    } else {
                                        1
                                    });
                                    if n > 0 {
                                        self.slots[i] = cursor.with_count(dst.count() + n);
                                        self.cursor = cursor.with_count(cursor.count() - n);
                                    }
                                } else {
                                    std::mem::swap(&mut self.slots[i], &mut self.cursor);
                                }
                            } else {
                                return None;
                            }
                        }
                        1 if button <= 1 => {
                            let item = self.slots[i];
                            if i >= 9
                                && let Some(target) = item.armor_slot()
                                && self.slots[target] == Item::Empty
                            {
                                self.slots.swap(i, target);
                            } else {
                                self.slots[i] = Item::Empty;
                                let left = if i < 9 || i == 45 {
                                    self.insert_range(item, 9..45)
                                } else if i >= 36 {
                                    self.insert_range(item, 9..36)
                                } else {
                                    self.insert_range(item, 36..45)
                                };
                                self.slots[i] = left;
                            }
                        }
                        2 if button <= 8 || button == 40 => {
                            let target = if button == 40 {
                                45
                            } else {
                                36 + button as usize
                            };
                            if !Self::accepts(i, self.slots[target])
                                || !Self::accepts(target, self.slots[i])
                            {
                                return None;
                            }
                            self.slots.swap(i, target);
                        }
                        4 if button <= 1 && drop_allowed && self.cursor == Item::Empty => {
                            dropped = self.slots[i].take(if button == 0 {
                                1
                            } else {
                                self.slots[i].count()
                            });
                        }
                        6 if button == 0 && self.cursor != Item::Empty => {
                            for i in 9..46 {
                                if self.slots[i].compatible(self.cursor) {
                                    let n = (self.cursor.max_count() - self.cursor.count())
                                        .min(self.slots[i].count());
                                    self.slots[i].take(n);
                                    self.cursor = self.cursor.with_count(self.cursor.count() + n);
                                }
                            }
                        }
                        _ => return None,
                    }
                }
            }
            _ => return None,
        }
        self.changed();
        Some(dropped)
    }
    pub fn wear_main(&mut self, amount: u16) -> bool {
        let slot = &mut self.slots[36 + self.selected as usize];
        let old = *slot;
        let broken = slot.wear(amount);
        if old != *slot {
            self.changed();
        }
        broken
    }
    pub fn resistance(&self) -> f64 {
        self.slots[5..9]
            .iter()
            .filter_map(|i| i.info())
            .map(|i| i.resistance)
            .sum::<f64>()
            .clamp(0., 1.)
    }
    pub fn armor(&self) -> (f32, f32) {
        self.slots[5..9]
            .iter()
            .filter_map(|s| s.info())
            .fold((0., 0.), |(a, t), i| (a + i.armor, t + i.toughness))
    }
    pub fn hurt_armor(&mut self, damage: f32) {
        let cost = (damage / 4.).max(1.) as u16;
        let mut changed = false;
        for slot in &mut self.slots[5..9] {
            let old = *slot;
            slot.wear(cost);
            changed |= old != *slot;
        }
        if changed {
            self.changed();
        }
    }
}
pub fn after_armor(damage: f32, armor: f32, toughness: f32) -> f32 {
    damage * (1. - (armor - damage / (2. + toughness / 4.)).clamp(armor * 0.2, 20.) / 25.)
}

/// In-process reconnect continuity. Admission reserves a bounded identity entry;
/// no eviction can turn an old damaged kit into a fresh one. No disk persistence.
pub(crate) type Accounts =
    std::sync::Arc<std::sync::Mutex<std::collections::HashMap<[u8; 16], AccountEntry>>>;
pub(crate) struct AccountEntry {
    pub inventory: Inventory,
    pub pending: bool,
}
pub(crate) struct AccountLease {
    accounts: Accounts,
    uuid: [u8; 16],
    game: crate::gameplay::Shared,
}
impl AccountLease {
    pub(crate) fn attach(
        accounts: Accounts,
        uuid: [u8; 16],
        game: crate::gameplay::Shared,
    ) -> std::io::Result<Self> {
        {
            let mut g = game.lock().unwrap();
            let mut all = accounts.lock().unwrap();
            if let Some(entry) = all.get(&uuid) {
                if entry.pending {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        "inventory transaction completing",
                    ));
                }
                g.inventory = entry.inventory.clone();
            } else if all.len() < 1024 {
                all.insert(
                    uuid,
                    AccountEntry {
                        inventory: g.inventory.clone(),
                        pending: false,
                    },
                );
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "item identity budget full",
                ));
            }
            g.account = Some((accounts.clone(), uuid));
        }
        Ok(Self {
            accounts,
            uuid,
            game,
        })
    }
}
impl Drop for AccountLease {
    fn drop(&mut self) {
        // Match attach/completion's game -> accounts order and keep the snapshot
        // protected until it is stored. A receipt must not be overwritten by an
        // older disconnect snapshot after it finalized the cached inventory.
        let mut g = self.game.lock().unwrap();
        g.item_active = false;
        if !g.pending_commit {
            g.pending_block = false;
            g.delayed_block.clear();
        }
        let entry = AccountEntry {
            inventory: g.inventory.clone(),
            pending: g.pending_block,
        };
        self.accounts.lock().unwrap().insert(self.uuid, entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn block(n: u8) -> Item {
        Item::named("minecraft:cobblestone", n).unwrap()
    }
    fn click(inv: &mut Inventory, slot: i16, button: u8, kind: u8) -> Option<Item> {
        inv.transact(
            &Action::Click {
                state: inv.revision,
                slot,
                button,
                kind,
            },
            true,
        )
    }
    #[test]
    fn pipelined_armor_clicks_share_a_revision_without_losing_the_cursor() {
        let mut inv = Inventory::default();
        let armor = Item::named("minecraft:iron_chestplate", 1).unwrap();
        inv.slots[6] = armor;
        let revision = inv.revision;
        for slot in [6, 26] {
            assert!(
                inv.transact(
                    &Action::Click {
                        state: revision,
                        slot,
                        button: 0,
                        kind: 0
                    },
                    false
                )
                .is_some()
            );
        }
        assert_eq!(inv.slots[6], Item::Empty);
        assert_eq!(inv.slots[26], armor);
        assert_eq!(inv.cursor, Item::Empty);
        assert_eq!(inv.slots.iter().filter(|&&i| i == armor).count(), 1);
    }
    #[test]
    fn counts_split_merge_capacity_and_partial_pickup_conserve_items() {
        let mut inv = Inventory::default();
        inv.slots.fill(Item::Empty);
        inv.slots[9] = block(63);
        assert!(click(&mut inv, 9, 1, 0).is_some());
        assert_eq!(inv.cursor.count(), 32);
        assert_eq!(inv.slots[9].count(), 31);
        click(&mut inv, 10, 1, 0);
        assert_eq!(inv.slots[10].count(), 1);
        assert_eq!(inv.cursor.count(), 31);
        click(&mut inv, 9, 0, 0);
        assert_eq!(inv.slots[9].count(), 62);
        assert_eq!(inv.cursor, Item::Empty);
        for slot in 9..45 {
            inv.slots[slot] = block(64);
        }
        inv.slots[36] = block(61);
        assert_eq!(inv.insert(block(8)).count(), 5);
        assert_eq!(inv.slots[36].count(), 64);
        assert_eq!(inv.insert(block(8)).count(), 8);
        let dropped = inv.transact(&Action::Drop { all: false }, true).unwrap();
        assert_eq!(dropped.count(), 1);
        assert_eq!(inv.main().count(), 63);
        assert_eq!(
            inv.transact(&Action::Drop { all: true }, true)
                .unwrap()
                .count(),
            63
        );
        assert_eq!(inv.main(), Item::Empty);
    }
    #[test]
    fn armor_clicks_shift_hotbar_and_offhand_preserve_damaged_stacks() {
        let mut inv = Inventory::default();
        let helm = Item::named("minecraft:diamond_helmet", 1).unwrap();
        inv.slots[10] = helm;
        click(&mut inv, 10, 0, 1);
        assert_eq!(inv.slots[5], helm);
        assert_eq!(inv.armor(), (3., 2.));
        assert_eq!(click(&mut inv, 5, 0, 2), None);
        assert_eq!(inv.slots[5], helm);
        click(&mut inv, 5, 0, 0);
        assert_eq!(inv.cursor, helm);
        click(&mut inv, 11, 0, 0);
        assert_eq!(inv.slots[5], Item::Empty);
        inv.slots[36] = Item::new(1045, 1, 249).unwrap();
        assert!(inv.apply(&Action::Swap));
        assert_eq!(inv.off().damage(), 249);
        click(&mut inv, 45, 0, 1);
        assert_eq!(inv.off(), Item::Empty);
        assert!(inv.slots.iter().any(|i| i.damage() == 249));
    }
    #[test]
    fn invalid_and_unsupported_clicks_cannot_forge_repair_or_drop_items() {
        let mut inv = Inventory::default();
        inv.wear_main(17);
        let before = inv.slots;
        assert!(!inv.apply(&Action::Click {
            state: -1,
            slot: 36,
            button: 0,
            kind: 0
        }));
        for (slot, button, kind) in [
            (0, 0, 0),
            (4, 0, 1),
            (46, 0, 0),
            (36, 9, 2),
            (36, 0, 5),
            (-999, 0, 0),
        ] {
            assert!(!inv.apply(&Action::Click {
                state: inv.revision,
                slot,
                button,
                kind
            }));
            assert_eq!(inv.slots, before);
        }
        assert!(!inv.apply(&Action::Drop { all: true }));
        assert_eq!(inv.main().damage(), 17);
    }
    #[test]
    fn damage_bounds_tool_weapon_and_armor_breakage() {
        assert!(Item::new(1045, 2, 0).is_none());
        assert!(Item::new(1045, 1, 250).is_none());
        assert!(Item::new(i32::MAX, 1, 0).is_none());
        let mut inv = Inventory::default();
        inv.slots[36] = Item::new(1047, 1, 248).unwrap();
        assert!(!inv.wear_main(1));
        assert_eq!(inv.main().damage(), 249);
        assert!(inv.wear_main(1));
        assert_eq!(inv.main(), Item::Empty);
        let mut sword = Item::new(1050, 1, 1559).unwrap();
        assert!(!sword.wear(1));
        assert!(sword.wear(u16::MAX));
        assert_eq!(sword, Item::Empty);
        let helm = Item::named("minecraft:leather_helmet", 1).unwrap();
        inv.slots[5] = Item::new(helm.id(), 1, 54).unwrap();
        inv.hurt_armor(1.);
        assert_eq!(inv.slots[5], Item::Empty);
        assert_eq!(after_armor(6., 0., 0.), 6.);
        assert!(after_armor(6., 20., 8.) < after_armor(6., 20., 0.));
    }
    #[test]
    fn death_respawn_and_reconnect_policies_do_not_refresh_equipment() {
        let now = tokio::time::Instant::now();
        for policy in [
            crate::gameplay::InventoryPolicy::ArenaKit,
            crate::gameplay::InventoryPolicy::ItemsKeep,
            crate::gameplay::InventoryPolicy::ItemsClear,
        ] {
            let mut g = crate::gameplay::State::new(now, [8., 64., 8.]);
            g.item_config.policy = policy;
            g.inventory.wear_main(17);
            g.hurt(40., now);
            g.reset(now, [8., 64., 8.]);
            match policy {
                crate::gameplay::InventoryPolicy::ArenaKit => {
                    assert_eq!(g.inventory.main(), Item::IronSword)
                }
                crate::gameplay::InventoryPolicy::ItemsKeep => {
                    assert_eq!(g.inventory.main().damage(), 17)
                }
                crate::gameplay::InventoryPolicy::ItemsClear => {
                    assert!(g.inventory.slots.iter().all(|i| *i == Item::Empty))
                }
            }
        }
        let accounts = Accounts::default();
        let g = std::sync::Arc::new(std::sync::Mutex::new(crate::gameplay::State::new(
            now,
            [8., 64., 8.],
        )));
        let lease = AccountLease::attach(accounts.clone(), [7; 16], g.clone()).unwrap();
        g.lock().unwrap().inventory.wear_main(91);
        drop(lease);
        assert!(!g.lock().unwrap().item_active);
        let fresh = std::sync::Arc::new(std::sync::Mutex::new(crate::gameplay::State::new(
            now,
            [8., 64., 8.],
        )));
        let _lease = AccountLease::attach(accounts, [7; 16], fresh.clone()).unwrap();
        assert_eq!(fresh.lock().unwrap().inventory.main().damage(), 91);
    }
}
