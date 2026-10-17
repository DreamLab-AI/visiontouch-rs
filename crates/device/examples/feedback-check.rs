//! Short, bounded check on the owner's unit. Default commands zero only.
//! Hold the arm at its reset pose at start: that pose is registered as the origin.
use std::time::{Duration, Instant};
use visiontouch_device::{Demand, Device};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    visiontouch_device::shutdown::install()?;
    let force: f64 = std::env::args()
        .nth(1)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(0.);
    if !force.is_finite() || force.abs() > 0.1 {
        return Err("check force must be finite and within ±0.1 N".into());
    }
    let mut device = Device::connect("/dev/parport0")?;
    device.sample()?;
    device.register_reset()?;
    device.enable_feedback()?;
    let start = Instant::now();
    let mut samples = 0;
    let mut peak = 0_f64;
    while start.elapsed() < Duration::from_millis(400) && !visiontouch_device::shutdown::requested()
    {
        let now = Instant::now();
        let y = if start.elapsed() < Duration::from_millis(200) {
            force
        } else {
            0.
        };
        device.request(Demand::Force([0., y, 0.]), now)?;
        device.sample()?;
        peak = peak.max(device.guard.telemetry().applied_force[1].abs());
        samples += 1;
        std::thread::sleep(Duration::from_millis(2));
    }
    device.close()?;
    println!(
        "{samples} acknowledged samples; requested force peak {peak:.3} N; zero/shutdown completed"
    );
    Ok(())
}
