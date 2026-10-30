//! Operator-side bounded import diagnostic; never modifies the source world.
use smolmc_world::terrain::{ImportBounds, import_world};
use std::{collections::HashMap, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    if a.len() != 8 {
        return Err("usage: import WORLD minx miny minz maxx maxy maxz".into());
    }
    let n = a[2..]
        .iter()
        .map(|s| s.parse::<i32>())
        .collect::<Result<Vec<_>, _>>()?;
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("../../sessions/data/registry_names.json"))?;
    let names = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["registry"] == "minecraft:worldgen/biome")
        .unwrap()["entries"]
        .as_array()
        .unwrap();
    let biomes: HashMap<String, i32> = names
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str().unwrap().to_owned(), i as i32))
        .collect();
    let start = std::time::Instant::now();
    let t = import_world(
        Path::new(&a[1]),
        ImportBounds {
            min: [n[0], n[1], n[2]],
            max: [n[3], n[4], n[5]],
        },
        &biomes,
    )?;
    println!(
        "chunks={} retained_bytes={} input_bytes={} clipped_cells={} elapsed_us={} spawn={:?}",
        t.chunks.len(),
        t.bytes,
        t.input_bytes,
        t.clipped_blocks,
        start.elapsed().as_micros(),
        t.safe_spawn([16, -48])
    );
    Ok(())
}
