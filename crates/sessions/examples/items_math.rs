use smolmc_protocol_777::items::{ITEMS, Item};
fn main() {
    let mut armor = Vec::new();
    for a in [0., 7., 15., 20.] {
        for t in [0., 8., 12.] {
            for damage in [1., 4., 6., 9., 20., 40.] {
                armor.push(serde_json::json!({"armor":a,"toughness":t,"damage":damage,"result":smolmc_sessions::gameplay::after_armor(damage,a,t)}));
            }
        }
    }
    let gear:Vec<_>=ITEMS.iter().filter(|i|i.max_damage>0).map(|i|{
        let mut s=Item::new(i.id,1,i.max_damage-1).unwrap();let before=s.damage();let broken=s.wear(1);
        serde_json::json!({"id":i.id,"max":i.max_damage,"mine":i.mine_cost,"attack":i.attack_cost,"armor":i.armor,"toughness":i.toughness,"resistance":i.resistance,"slot":i.armor_slot,"before":before,"broken":broken,"empty":s==Item::Empty})
    }).collect();
    let mut knockback = Vec::new();
    for resistance in [0., 0.10000000149011612, 0.4000000059604645] {
        for ground in [false, true] {
            for power in [0.4000000059604645, 0.5] {
                let velocity = [0.2, -0.0784000015258789, -0.3];
                let result = smolmc_sessions::gameplay::directed_knockback(
                    velocity,
                    [0., 1.],
                    ground,
                    power * (1. - resistance),
                );
                knockback.push(serde_json::json!({"resistance":resistance,"ground":ground,"power":power,"velocity":velocity,"result":result}));
            }
        }
    }
    let replaceable: Vec<_> = [
        "minecraft:short_grass",
        "minecraft:tall_grass",
        "minecraft:large_fern",
        "minecraft:grass_block",
        "minecraft:stone",
    ]
    .into_iter()
    .map(|n| {
        let m = smolmc_world::Material::parse(n, None).unwrap();
        serde_json::json!({"state":m.state(),"replaceable":m.info().replaceable})
    })
    .collect();
    println!(
        "{}",
        serde_json::json!({"armor":armor,"gear":gear,"knockback":knockback,"replaceable":replaceable})
    );
}
