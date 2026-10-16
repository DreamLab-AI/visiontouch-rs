//! Short, bounded check on the owner's unit. Default commands zero only.
use haptics_device::{Device, Wrench};
use std::time::{Duration, Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    haptics_device::shutdown::install()?;
    let force: f64 = std::env::args()
        .nth(1)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(0.);
    if !force.is_finite() || force.abs() > 0.1 {
        return Err("check force must be finite and within ±0.1 N".into());
    }
    let mut device = Device::connect("/dev/parport0")?;
    device.enable_feedback()?;
    let start = Instant::now();
    let mut samples = 0;
    let mut peak = 0_f64;
    while start.elapsed() < Duration::from_millis(400) && !haptics_device::shutdown::requested() {
        let now = Instant::now();
        let y = if start.elapsed() < Duration::from_millis(200) {
            force
        } else {
            0.
        };
        device.request(
            Wrench {
                force: [0., y, 0.],
                torque: [0.; 3],
            },
            now,
        )?;
        device.sample()?;
        peak = peak.max(device.guard.applied_force[1].abs());
        samples += 1;
        std::thread::sleep(Duration::from_millis(2));
    }
    device.close()?;
    println!(
        "{samples} acknowledged samples; requested force peak {peak:.3} N; zero/shutdown completed"
    );
    Ok(())
}
