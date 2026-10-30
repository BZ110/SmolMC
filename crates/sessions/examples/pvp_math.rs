//! Original corpus for the 1.19.4-based arena profile. No game code/assets.
use smolmc_sessions::gameplay;
fn main() {
    let axes: Vec<_> = [
        -180_f32, -177.1234, -135., -123.4567, -90., -45., -0.001, 0., 0.001, 12.34567, 30., 45.,
        89.999, 90., 123.4567, 179.99,
    ]
    .into_iter()
    .map(|yaw| serde_json::json!({"yaw":yaw,"axes":gameplay::yaw_axes(yaw)}))
    .collect();
    let velocities: Vec<_> = [
        [0.; 3],
        [0.4, 0.36080000519752503, -0.4],
        [-0.123456, 0.2492326441290743, 0.654321],
        [4., -4., 0.00001],
        [0.7, 0.4, -0.2],
        [-0.000125, 0.000125, 3.9],
    ]
    .into_iter()
    .map(|v| serde_json::json!({"input":v,"classic":gameplay::classic_velocity(v)}))
    .collect();
    println!(
        "{}",
        serde_json::json!({"axes":axes,"velocities":velocities})
    );
}
