//! Short operator-owned imported-template/COW/collision measurements; no synthetic capacity claim.
use smolmc_sessions::{ImportBounds, Service, WorldProfile};
use smolmc_world::{
    ChunkPos, Material,
    collision::{TerrainSnapshot, resolve},
};
use std::{
    io::{self, Write},
    path::Path,
    sync::Arc,
    time::Instant,
};
fn point(value: serde_json::Value) {
    println!("{value}");
    io::stdout().flush().unwrap();
    let mut line = String::new();
    io::stdin().read_line(&mut line).unwrap();
}
fn distribution(mut v: Vec<f64>) -> serde_json::Value {
    v.sort_by(f64::total_cmp);
    serde_json::json!({"samples":v.len(),"p50_us":v[v.len()/2],"p99_us":v[((v.len()-1)*99)/100],"max_us":v[v.len()-1]})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    let root = Path::new(a.get(1).ok_or("world folder required")?);
    let start = Instant::now();
    let base = smolmc_world::terrain::import_world_policy(
        root,
        ImportBounds {
            min: [-16, -64, -80],
            max: [48, 320, -16],
        },
        &Service::biome_names()?,
        true,
    )?;
    let import_us = start.elapsed().as_micros();
    let spawn = base.safe_spawn([16, -48]).ok_or("no spawn")?;
    let start = Instant::now();
    let encoded = smolmc_chunks::ImportedChunks::new(&base, 0, Service::biome_names()?.len())?;
    let encode_us = start.elapsed().as_micros();
    let encoded_bytes = encoded.bytes();
    let mut scratch = Vec::new();
    let start = Instant::now();
    for _ in 0..100 {
        for &pos in base.chunks.keys() {
            encoded.encode(pos, &mut scratch);
            std::hint::black_box(&scratch);
        }
    }
    let stream_copy_us = start.elapsed().as_secs_f64() * 1e6 / 1600.;
    drop(encoded);
    let service =
        Service::with_terrain_instances(1, WorldProfile::builder(), Arc::clone(&base), spawn)?;
    let original = service
        .default_world()
        .map_err(|e| io::Error::other(format!("{e:?}")))?;
    let mut worlds = vec![original];
    let mut create = Vec::new();
    point(
        serde_json::json!({"phase":"one_instance","import_us":import_us,"serialization_us":encode_us,"shared_decoded_charge_bytes":base.bytes,"shared_encoded_bytes":encoded_bytes,"input_bytes":base.input_bytes,"chunks":base.chunks.len(),"copy_to_stream_us_per_chunk":stream_copy_us,"removed_palette_entries":base.removed_palette_entries}),
    );
    for count in [128, 256] {
        while worlds.len() < count {
            let start = Instant::now();
            worlds.push(
                service
                    .clone_world(original)
                    .map_err(|e| io::Error::other(format!("{e:?}")))?,
            );
            create.push(start.elapsed().as_secs_f64() * 1e6);
        }
        point(
            serde_json::json!({"phase":format!("instances_{count}"),"creation":distribution(create.clone()),"overlay_charged_bytes":service.world_overlay_memory().charged_bytes}),
        );
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(4)
        .thread_keep_alive(std::time::Duration::from_secs(2))
        .build()?;
    let mut edits = Vec::new();
    runtime.block_on(async {
        for world in &worlds[..8] {
            let batch = (0..16).map(|i| ([i, 200, -40], Material::Stone)).collect();
            let start = Instant::now();
            service.edit_terrain(*world, batch).await.unwrap();
            edits.push(start.elapsed().as_secs_f64() * 1e6);
        }
    });
    point(
        serde_json::json!({"phase":"sparse_edits","worlds_edited":8,"block_edits":128,"edit_commit":distribution(edits),"overlay_charged_bytes":service.world_overlay_memory().charged_bytes}),
    );
    let terrain = TerrainSnapshot {
        base: Arc::clone(&base),
        overlay: Default::default(),
    };
    let start = Instant::now();
    for i in 0..10000 {
        let p = [spawn[0] + (i % 3) as f64 * 0.02, spawn[1], spawn[2]];
        std::hint::black_box(resolve(p, [0.1, -0.08, 0.1], false, true, |p| {
            terrain.block(p)
        }));
    }
    let collision_us = start.elapsed().as_secs_f64() * 1e6 / 10000.;
    let mut reset = Vec::new();
    for w in &mut worlds {
        let start = Instant::now();
        *w = service
            .reset_world(*w)
            .map_err(|e| io::Error::other(format!("{e:?}")))?;
        reset.push(start.elapsed().as_secs_f64() * 1e6);
    }
    for w in worlds {
        service
            .destroy_world(w)
            .map_err(|e| io::Error::other(format!("{e:?}")))?;
    }
    runtime.block_on(service.shutdown())?;
    point(
        serde_json::json!({"phase":"cleanup","reset":distribution(reset),"overlay_charged_bytes":service.world_overlay_memory().charged_bytes,"collision_us_per_operation":collision_us,"collision_samples":10000}),
    );
    std::hint::black_box(ChunkPos::ORIGIN);
    Ok(())
}
