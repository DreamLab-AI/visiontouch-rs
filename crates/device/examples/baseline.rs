//! Input-only baseline: no force session is armed and no motor packet is sent.
//! Run through scripts/with-port-access; redirect CSV to a session-specific path.
use std::io::{self, Write};
use std::time::{Duration, Instant};
use visiontouch_device::Device;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    visiontouch_device::shutdown::install()?;
    let argument = std::env::args().nth(1);
    let seconds = if argument.as_deref() == Some("--until-stop") {
        None
    } else {
        let value: f64 = argument.map(|s| s.parse()).transpose()?.unwrap_or(10.);
        if !value.is_finite() || !(0.1..=60.).contains(&value) {
            return Err("duration must be 0.1..=60 seconds, or --until-stop".into());
        }
        Some(value)
    };
    let mut device = Device::connect("/dev/parport0")?;
    device.sample()?;
    let first = device.state.continuous_counts();
    eprintln!(
        "Recording input only; PID {}; SIGTERM/SIGINT stops and flushes. No force session armed.",
        std::process::id()
    );
    let start = Instant::now();
    let mut previous = start;
    let mut samples = 0_u64;
    let mut output = io::BufWriter::new(io::stdout().lock());
    writeln!(
        output,
        "sample,elapsed_ms,interval_us,io_us,e0,e1,e2,e3,e4,e5,d0,d1,d2,d3,d4,d5,unreg_x_m,unreg_y_m,unreg_z_m,button,raw_hex"
    )?;
    while seconds.is_none_or(|limit| start.elapsed().as_secs_f64() < limit)
        && !visiontouch_device::shutdown::requested()
    {
        let before = Instant::now();
        let packet = device.sample()?;
        let now = Instant::now();
        write!(
            output,
            "{samples},{:.6},{:.3},{:.3}",
            now.duration_since(start).as_secs_f64() * 1000.,
            now.duration_since(previous).as_secs_f64() * 1e6,
            now.duration_since(before).as_secs_f64() * 1e6
        )?;
        for value in device.state.counts {
            write!(output, ",{value}")?;
        }
        for (value, start) in device.state.continuous_counts().into_iter().zip(first) {
            write!(output, ",{}", value - start)?;
        }
        // Power-on-relative, unregistered: for display and diagnosis only.
        let position = device.state.unregistered_position_m();
        for value in position {
            write!(output, ",{value:.10}")?;
        }
        write!(output, ",{},", u8::from(device.state.button_pressed))?;
        for byte in packet.raw {
            write!(output, "{byte:02x}")?;
        }
        writeln!(output)?;
        samples += 1;
        if samples % 40 == 0 {
            output.flush()?;
        }
        previous = now;
        std::thread::sleep(Duration::from_millis(2));
    }
    device.close()?;
    output.flush()?;
    eprintln!(
        "{samples} input-only samples in {:.3}s; no force session armed; port closed.",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
