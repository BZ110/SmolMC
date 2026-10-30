//! Real Service lifecycle/resource driver. One fixed stdin thread, live async runtime.
use serde_json::{Value, json};
use smolmc_sessions::Service;
use smolmc_world::{MAX_EDIT_BATCH, Material, WorldHandle, WorldState};
use std::{
    io::{self, BufRead},
    time::Instant,
};
fn latency(samples: &mut [u128]) -> Value {
    samples.sort_unstable();
    if samples.is_empty() {
        return Value::Null;
    }
    let at = |p: usize| samples[((samples.len() - 1) * p) / 100];
    json!({"samples":samples.len(),"p50_ns":at(50),"p95_ns":at(95),"p99_ns":at(99),"max_ns":samples.last(),"sum_ns":samples.iter().sum::<u128>()})
}
fn fill(
    service: &Service,
    world: WorldHandle,
    edits: &[([i32; 3], Material)],
    times: &mut Vec<u128>,
) {
    for batch in edits.chunks(MAX_EDIT_BATCH) {
        let t = Instant::now();
        service.edit_world_batch(world, batch).unwrap();
        times.push(t.elapsed().as_nanos());
    }
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let service = Service::new().unwrap();
    service
        .destroy_world(service.default_world().unwrap())
        .unwrap();
    let count: usize = std::env::var("SMOLMC_PROFILE_EDITS")
        .unwrap_or_else(|_| "0".into())
        .parse()
        .unwrap();
    let spread = std::env::var("SMOLMC_PROFILE_SPREAD").is_ok();
    assert!(count <= 65_536);
    let edits: Vec<_> = (0..count as i32)
        .map(|i| {
            let p = if spread {
                [
                    (i % 16) * 16 + (i / 16) % 16,
                    -64 + i / 4096,
                    (i / 256) % 16,
                ]
            } else {
                [
                    i % 16,
                    -64 + (i / 256) % 127,
                    (i / 16) % 16 + (i / (127 * 256)) * 16,
                ]
            };
            (p, Material::Air)
        })
        .collect();
    let mut worlds: Vec<WorldHandle> = Vec::new();
    let mut source = None;
    let (input, mut commands) = tokio::sync::mpsc::channel(1);
    let reader = std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let line = line.unwrap();
            let quit = line.trim() == "quit";
            if input.blocking_send(line).is_err() || quit {
                break;
            }
        }
    });
    println!(
        "{}",
        json!({"ready":true,"pid":std::process::id(),"edits_per_independent_world":count,"spread":spread})
    );
    while let Some(line) = commands.recv().await {
        let parts: Vec<_> = line.split_whitespace().collect();
        let start = Instant::now();
        let (mut create, mut edit, mut reset, mut destroy) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        match parts[0] {
            "hold" | "clones" => {
                let target: usize = parts[1].parse().unwrap();
                if parts[0] == "clones" && source.is_none() {
                    let s = service.create_world().unwrap();
                    fill(&service, s, &edits, &mut edit);
                    source = Some(s);
                }
                while worlds.len() < target {
                    let t = Instant::now();
                    let w = match source {
                        Some(s) => service.clone_world(s).unwrap(),
                        None => service.create_world().unwrap(),
                    };
                    create.push(t.elapsed().as_nanos());
                    if source.is_none() {
                        fill(&service, w, &edits, &mut edit);
                    }
                    service.park_world(w).unwrap();
                    worlds.push(w);
                }
                assert_eq!(worlds.len(), target);
            }
            "cow" => {
                let original = source.unwrap();
                for &w in &worlds {
                    let t = Instant::now();
                    service.edit_world(w, edits[0].0, Material::Stone).unwrap();
                    edit.push(t.elapsed().as_nanos());
                }
                assert_eq!(service.world_resources(original).unwrap().edits, count);
                for &w in &worlds {
                    assert_eq!(service.world_resources(w).unwrap().edits, count - 1);
                }
            }
            "reset" => {
                for w in &mut worlds {
                    let old = *w;
                    let t = Instant::now();
                    *w = service.reset_world(old).unwrap();
                    reset.push(t.elapsed().as_nanos());
                    assert!(service.edit_world(old, [0, 0, 0], Material::Air).is_err());
                }
            }
            "clear" => {
                for w in worlds.drain(..).chain(source.take()) {
                    let t = Instant::now();
                    service.destroy_world(w).unwrap();
                    destroy.push(t.elapsed().as_nanos());
                }
                assert_eq!(service.world_overlay_memory().charged_bytes, 0);
            }
            "churn" => {
                assert!(worlds.is_empty() && source.is_none());
                let rounds: usize = parts[1].parse().unwrap();
                let width: usize = parts[2].parse().unwrap();
                for _ in 0..rounds {
                    for _ in 0..width {
                        let t = Instant::now();
                        let w = service.create_world().unwrap();
                        create.push(t.elapsed().as_nanos());
                        fill(&service, w, &edits, &mut edit);
                        worlds.push(w);
                    }
                    for old in worlds.drain(..) {
                        let t = Instant::now();
                        let new = service.reset_world(old).unwrap();
                        reset.push(t.elapsed().as_nanos());
                        let t = Instant::now();
                        service.destroy_world(new).unwrap();
                        destroy.push(t.elapsed().as_nanos());
                    }
                    assert_eq!(service.world_overlay_memory().charged_bytes, 0);
                }
            }
            "quit" => break,
            _ => panic!("unknown command"),
        }
        let mut logical = 0;
        let mut parked = 0;
        for &w in worlds.iter().chain(source.iter()) {
            let r = service.world_resources(w).unwrap();
            assert_eq!(r.owned_tasks + r.owned_timers, 0);
            logical += r.overlay_charged_bytes;
            parked += usize::from(r.state == WorldState::Parked);
        }
        let m = service.world_overlay_memory();
        println!(
            "{}",
            json!({"command":line,"instances":worlds.len()+usize::from(source.is_some()),"parked":parked,"logical_overlay_bytes_summed":logical,"retained_overlay_charged_bytes":m.charged_bytes,"peak_overlay_charged_bytes":m.peak_charged_bytes,"overlay_budget":m.limit,"world_owned_tasks":0,"world_owned_timers":0,"operation_ns":start.elapsed().as_nanos(),"create":latency(&mut create),"edit_batch":latency(&mut edit),"reset":latency(&mut reset),"destroy":latency(&mut destroy)})
        );
    }
    assert!(worlds.is_empty() && source.is_none());
    assert_eq!(service.world_overlay_memory().charged_bytes, 0);
    service.shutdown().await.unwrap();
    reader.join().unwrap();
}
