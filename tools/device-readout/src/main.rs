use haptics_hal::device::DeviceReader;
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--help" || a == "-h") {
        println!(
            "Usage: device-readout [DEVICE] [SAMPLES]\nActive device setup and raw encoder acquisition. No force-output packets.\nDefault: /dev/parport0, 10 samples at 10 Hz; maximum 600 samples."
        );
        return Ok(());
    }
    if args.len() > 2 {
        return Err("Usage: device-readout [DEVICE] [SAMPLES]".into());
    }
    let path = args.first().map(String::as_str).unwrap_or("/dev/parport0");
    let count: usize = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(10);
    if !(1..=600).contains(&count) {
        return Err("SAMPLES must be 1..=600".into());
    }
    let mut device = DeviceReader::connect(path)?;
    let start = Instant::now();
    let mut state = haptics_device::EncoderState::default();
    let mut output = io::BufWriter::new(io::stdout().lock());
    writeln!(
        output,
        "sample,elapsed_ms,e0,e1,e2,e3,e4,e5,raw_hex,button,d0,d1,d2,d3,d4,d5"
    )?;
    for n in 0..count {
        let sample = device.sample()?;
        write!(output, "{n},{:.3}", start.elapsed().as_secs_f64() * 1000.)?;
        for value in sample.encoder_counts() {
            write!(output, ",{value}")?;
        }
        write!(output, ",")?;
        for byte in sample.raw {
            write!(output, "{byte:02x}")?;
        }
        state.update(sample);
        write!(output, ",{}", u8::from(state.button_pressed))?;
        for value in state.relative_counts {
            write!(output, ",{value}")?;
        }
        writeln!(output)?;
        output.flush()?;
        if n + 1 < count {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    device.close()?;
    eprintln!("Received {count} complete device packets; port released.");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("device-readout: {e}");
            ExitCode::FAILURE
        }
    }
}
