//! Original codec vectors, consumable by an independently supplied reference
//! decoder. No reference source/assets/dependencies are built or distributed.
use smolmc_protocol_777::{
    play,
    pvp::{self, Item},
};
use std::io::{self, Write};
fn main() -> io::Result<()> {
    let mut slots = [Item::Empty; 46];
    slots[36] = Item::IronSword;
    slots[9] = Item::DiamondSword;
    let packets = [
        (
            "ClientboundCommandsPacket",
            play::commands(&["arena_status".into(), "isolate".into(), "together".into()]),
        ),
        (
            "ClientboundContainerSetContentPacket",
            pvp::inventory(0, &slots, Item::Empty),
        ),
        ("ClientboundSetHealthPacket", pvp::health(20.)),
        (
            "ClientboundSetEquipmentPacket",
            pvp::equipment(7, Item::IronSword, Item::DiamondSword),
        ),
        ("ClientboundSwingAnimationPacket", pvp::swing(7, 0)),
        (
            "ClientboundSetEntityMotionPacket",
            pvp::velocity(7, [0.4, 0.4, -0.4]),
        ),
        ("ClientboundSetEntityDataPacket", pvp::metadata(7, 14.)),
        ("ClientboundHurtAnimationPacket", pvp::hurt(7, 42.)),
        ("ClientboundEntityEventPacket", pvp::death(7)),
        ("ClientboundPlayerCombatKillPacket", pvp::killed(7)),
        ("ClientboundRespawnPacket", pvp::respawn(0, 2)),
        ("ClientboundDamageEventPacket", pvp::damage(7, 8, 0)),
    ];
    let additional = [
        (
            "ClientboundUpdateAttributesPacket",
            pvp::attributes(7, Item::IronSword),
        ),
        ("ClientboundAnimatePacket", pvp::critical(7)),
        (
            "ServerboundPlayerActionPacket",
            vec![0x29, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ),
    ];
    let effects = [
        (
            "ClientboundSoundPacket",
            pvp::sound("minecraft:entity.player.attack.sweep", [8., 64., 8.], 42),
        ),
        (
            "ClientboundLevelParticlesPacket",
            pvp::particles(77, [8., 64.9, 9.], [0., 0., 1.], 0., 0),
        ),
        (
            "ClientboundLevelParticlesPacket",
            pvp::particles(14, [8., 64.9, 9.], [0.1, 0., 0.1], 0.2, 3),
        ),
    ];
    let mut out = io::stdout().lock();
    for (name, packet) in packets.into_iter().chain(additional).chain(effects) {
        out.write_all(&(name.len() as u16).to_be_bytes())?;
        out.write_all(name.as_bytes())?;
        out.write_all(&(packet.len() as u32).to_be_bytes())?;
        out.write_all(&packet)?;
    }
    Ok(())
}
