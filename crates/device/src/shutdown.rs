//! Opt-in Linux process signal handling for applications running a force worker.
use std::sync::atomic::{AtomicBool, Ordering};
static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn on_signal(_: i32) {
    STOP.store(true, Ordering::Relaxed);
}
unsafe extern "C" {
    fn signal(number: i32, handler: usize) -> usize;
}
/// Whether an installed SIGINT/SIGTERM handler has requested worker shutdown.
/// This process-wide flag latches; polling alone performs no device I/O.
pub fn requested() -> bool {
    STOP.load(Ordering::Relaxed)
}
/// Call once before starting worker threads. The handler only sets an atomic;
/// the worker performs bounded zero/shutdown I/O in normal thread context.
pub fn install() -> std::io::Result<()> {
    for number in [2, 15] {
        // SAFETY: Linux SIGINT/SIGTERM, static signal-safe function; no captures.
        if unsafe { signal(number, on_signal as *const () as usize) } == usize::MAX {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}
