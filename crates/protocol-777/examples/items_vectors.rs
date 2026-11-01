use smolmc_protocol_777::{
    items::{self, Item},
    presence::{self, EntitySnapshot},
    pvp,
};
use std::io::{self, Write};
fn main() {
    let mut slots = [Item::Empty; 46];
    slots[36] = Item::new(1047, 1, 249).unwrap();
    slots[5] = Item::named("minecraft:diamond_helmet", 1).unwrap();
    slots[9] = Item::named("minecraft:cobblestone", 64).unwrap();
    let position = [1.5, 65., -2.5];
    let velocity = [0., 0.2, 0.];
    let packets = [
        (
            "ClientboundUpdateAttributesPacket",
            items::attributes(5, slots[36], 20., 8., 0.4),
        ),
        (
            "ClientboundContainerSetContentPacket",
            pvp::inventory(7, &slots, Item::new(1045, 1, 17).unwrap()),
        ),
        (
            "ClientboundAddEntityPacket",
            items::spawn(1_000_000_001, position, velocity),
        ),
        (
            "ClientboundSetEntityDataPacket",
            items::metadata(1_000_000_001, slots[9]),
        ),
        (
            "ClientboundSetEntityDataPacket",
            items::metadata(1_000_000_001, slots[36]),
        ),
        (
            "ClientboundSetEquipmentPacket",
            items::equipment(
                5,
                [
                    slots[36],
                    Item::Empty,
                    Item::Empty,
                    Item::Empty,
                    Item::Empty,
                    slots[5],
                ],
            ),
        ),
        (
            "ClientboundTakeItemEntityPacket",
            items::take(1_000_000_001, 5, 32),
        ),
        (
            "ClientboundEntityPositionSyncPacket",
            presence::position(
                1_000_000_001,
                EntitySnapshot {
                    position,
                    rotation: [0.; 2],
                    on_ground: false,
                    crouching: false,
                    sprinting: false,
                },
            ),
        ),
        (
            "ClientboundRemoveEntitiesPacket",
            presence::entity_remove(1_000_000_001),
        ),
    ];
    let mut out = io::stdout().lock();
    for (name, body) in packets {
        out.write_all(&(name.len() as u16).to_be_bytes()).unwrap();
        out.write_all(name.as_bytes()).unwrap();
        out.write_all(&(body.len() as u32).to_be_bytes()).unwrap();
        out.write_all(&body).unwrap();
    }
}
