//! Host capabilities and bounded device readout through ppdev.
#![warn(missing_docs)]

pub mod device;

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("The ioctl ABI has only been validated on Linux x86-64.");

use std::ffi::{c_int, c_ulong};
use std::fs::OpenOptions;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;

// Public Linux UAPI, P-0010; docs/spec/00-host-capabilities.md.
const GET_MODES: c_ulong = (2 << 30) | (4 << 16) | (0x70 << 8) | 0x97;

unsafe extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Raw Linux ppdev host-mode bitmask. It describes host capabilities, not device readiness.
pub struct Capabilities(pub u32);

impl Capabilities {
    /// Whether the host advertises EPP; successful peripheral negotiation is not implied.
    pub fn supports_epp(self) -> bool {
        self.0 & 4 != 0
    }

    /// Names of recognized set capability bits; unknown bits remain available in the raw mask.
    pub fn names(self) -> Vec<&'static str> {
        [
            (1, "PCSPP"),
            (2, "TRISTATE"),
            (4, "EPP"),
            (8, "ECP"),
            (16, "COMPAT"),
            (32, "DMA"),
            (64, "SAFEININT"),
        ]
        .into_iter()
        .filter_map(|(bit, name)| (self.0 & bit != 0).then_some(name))
        .collect()
    }
}

/// Query host metadata without claiming the port or changing its signals.
/// Success does not establish that a peripheral is connected.
pub fn query_capabilities(path: impl AsRef<Path>) -> io::Result<Capabilities> {
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    if !file.metadata()?.file_type().is_char_device() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected a character device",
        ));
    }
    let mut modes: u32 = 0;
    // SAFETY: the descriptor lives through the call and the kernel writes one
    // correctly sized, aligned unsigned int to a valid mutable pointer.
    let result = unsafe { ioctl(file.as_raw_fd(), GET_MODES, &mut modes as *mut u32) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Capabilities(modes))
}
