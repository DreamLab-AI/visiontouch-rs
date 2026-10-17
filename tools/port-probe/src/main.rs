use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().unwrap_or_else(|| "/dev/parport0".into());
    if path == "--help" || path == "-h" {
        println!(
            "Usage: port-probe [DEVICE]\nQueries advertised host capabilities without claiming or negotiating."
        );
        return ExitCode::SUCCESS;
    }
    if args.next().is_some() {
        eprintln!("Usage: port-probe [DEVICE]");
        return ExitCode::FAILURE;
    }
    match visiontouch_hal::query_capabilities(&path) {
        Ok(caps) => {
            println!(
                "{}: host modes 0x{:08x}: {}",
                std::path::Path::new(&path).display(),
                caps.0,
                caps.names().join(",")
            );
            println!(
                "This query does not test unit communication. Use device-readout for an active device test."
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}: {error}", std::path::Path::new(&path).display());
            ExitCode::FAILURE
        }
    }
}
