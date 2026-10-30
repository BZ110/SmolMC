//! Original bounded collision/mining corpus for the operator-owned reference runtime.
use smolmc_protocol_777::pvp::Item;
use smolmc_world::{Material, collision};
fn main() {
    let mut collisions = Vec::new();
    for x in [15.25, 15.5, 15.69] {
        for y in [0., 0.125, 1., 2.] {
            for z in [-1.5, 0.5] {
                for d in [[1., -0.2, 0.], [1., 0.3, 0.4], [-0.3, -1.5, 1.]] {
                    let p = [x, y, z];
                    let r = collision::resolve(p, d, false, false, |p| {
                        if p[1] == -1 || p[0] == 16 && (-1..3).contains(&p[1]) {
                            Material::Stone
                        } else {
                            Material::Air
                        }
                    })
                    .unwrap();
                    collisions.push(serde_json::json!({"position":p,"wanted":d,"result":r.delta}));
                }
            }
        }
    }
    let mut mining = Vec::new();
    for name in [
        "stone",
        "oak_planks",
        "white_wool",
        "dirt",
        "oak_leaves",
        "obsidian",
        "bedrock",
        "short_grass",
    ] {
        let Some(m) = Material::parse(&format!("minecraft:{name}"), None) else {
            continue;
        };
        for item in [Item::Empty, Item::IronSword, Item::IronPickaxe] {
            for grounded in [true, false] {
                let progress = smolmc_sessions::gameplay::mining_progress(m, item, grounded);
                mining.push(serde_json::json!({"block":m.state(),"item":item.id(),"grounded":grounded,"progress":if progress.is_finite(){progress as f64}else{-1.}}));
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({"collisions":collisions,"mining":mining})
    );
}
