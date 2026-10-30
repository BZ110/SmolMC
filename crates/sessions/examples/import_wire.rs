//! Private operator-owned map corpus for independent actual-codec validation.
use smolmc_world::terrain::ImportBounds;
use std::{fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    if a.len() != 3 {
        return Err("usage: import_wire WORLD OUTPUT".into());
    }
    let biomes = smolmc_sessions::Service::biome_names()?;
    let t = smolmc_world::terrain::import_world_policy(
        Path::new(&a[1]),
        ImportBounds {
            min: [-16, -64, -80],
            max: [48, 320, -16],
        },
        &biomes,
        true,
    )?;
    let encoded = smolmc_chunks::ImportedChunks::new(&t, biomes["minecraft:plains"], biomes.len())?;
    fs::create_dir_all(&a[2])?;
    for pos in t.chunks.keys() {
        let mut packet = Vec::new();
        encoded.encode(*pos, &mut packet);
        fs::write(
            Path::new(&a[2]).join(format!("{}.{}.wire", pos.x, pos.z)),
            packet,
        )?;
    }
    let light = smolmc_chunks::light_update(
        smolmc_world::ChunkPos { x: -1, z: -3 },
        [(0, &[255u8; 2048][..]), (1, &[0u8; 2048][..])],
    );
    fs::write(Path::new(&a[2]).join("light.packet"), light)?;
    fs::write(
        Path::new(&a[2]).join("mode.packet"),
        smolmc_protocol_777::play::game_mode(0),
    )?;
    fs::write(
        Path::new(&a[2]).join("supported.txt"),
        smolmc_world::STATES
            .iter()
            .map(|s| s.id.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    )?;
    fs::write(
        Path::new(&a[2]).join("conversion.json"),
        format!("{:?}", t.removed_palette_entries),
    )?;
    for action in 0..4 {
        let mut out = smolmc_protocol_777::login::Encoder::new(0x29);
        out.varint(action);
        out.raw(&smolmc_protocol_777::play::pack_block_pos([8, 63, -40]).to_be_bytes());
        out.byte(1);
        out.varint(action + 1);
        fs::write(
            Path::new(&a[2]).join(format!("dig{action}.server")),
            out.finish(),
        )?;
    }
    fs::write(
        Path::new(&a[2]).join("cracks.packet"),
        smolmc_protocol_777::pvp::block_destruction(1, [8, 63, -40], 7),
    )?;
    println!("encoded_bytes={}", encoded.bytes());
    Ok(())
}
