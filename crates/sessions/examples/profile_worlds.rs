//! Reproducible trusted-control lifecycle benchmark, driven by measure_m2a_worlds.py.
//! No clients, simulation tasks or network listeners; includes real Service baseline.
use smolmc_sessions::Service;
use smolmc_world::{Material, WorldHandle};
use std::{
    io::{self, BufRead},
    time::Instant,
};
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let service = Service::new().unwrap();
    service
        .destroy_world(service.default_world().unwrap())
        .unwrap();
    let mut worlds: Vec<WorldHandle> = Vec::new();
    // Keep the async executor live during idle observations. Only this fixed
    // benchmark controller thread blocks on stdin; no thread belongs to a world.
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
    println!("{{\"ready\":true,\"pid\":{}}}", std::process::id());
    while let Some(line) = commands.recv().await {
        let parts: Vec<_> = line.split_whitespace().collect();
        let start = Instant::now();
        match parts[0] {
            "hold" => {
                let count: usize = parts[1].parse().unwrap();
                assert!(count <= smolmc_world::instance_limit());
                while worlds.len() < count {
                    worlds.push(service.create_world().unwrap());
                }
                while worlds.len() > count {
                    service.destroy_world(worlds.pop().unwrap()).unwrap();
                }
                println!(
                    "{{\"phase\":\"hold\",\"instances\":{},\"operation_ns\":{}}}",
                    worlds.len(),
                    start.elapsed().as_nanos()
                );
            }
            "churn" => {
                assert!(worlds.is_empty());
                let rounds: usize = parts[1].parse().unwrap();
                let width: usize = parts[2].parse().unwrap();
                for _ in 0..rounds {
                    for _ in 0..width {
                        let a = service.create_world().unwrap();
                        service.edit_world(a, [8, 62, 8], Material::Air).unwrap();
                        worlds.push(a);
                    }
                    for a in worlds.drain(..) {
                        let b = service.reset_world(a).unwrap();
                        assert!(service.edit_world(a, [8, 62, 8], Material::Air).is_err());
                        service.destroy_world(b).unwrap();
                    }
                }
                println!(
                    "{{\"phase\":\"churn\",\"instances\":0,\"created_edited_reset_destroyed\":{},\"operation_ns\":{}}}",
                    rounds * width,
                    start.elapsed().as_nanos()
                );
            }
            "quit" => break,
            _ => panic!("unknown profile command"),
        }
    }
    assert!(worlds.is_empty());
    service.shutdown().await.unwrap();
    reader.join().unwrap();
}
