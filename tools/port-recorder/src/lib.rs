//! Reference-call instrumentation, not a peripheral driver. See the capture spec.
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("The recorder currently supports Linux x86-64 only.");

use serde::Serialize;
use std::cell::Cell;
use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

const LIMIT: usize = 65536;

struct Config {
    target: PathBuf,
    prefix: PathBuf,
    session_ns: u64,
}

static CONFIG: OnceLock<Option<Config>> = OnceLock::new();
thread_local! { static SEQUENCE: Cell<u64> = const { Cell::new(0) }; }

#[derive(Serialize)]
struct Snapshot {
    requested: usize,
    copied: usize,
    complete: bool,
    hex: String,
    memory_errno: Option<i32>,
}

#[derive(Serialize)]
struct Event {
    schema: u32,
    session_ns: u64,
    pid: i32,
    tid: i64,
    sequence: u64,
    start_ns: u64,
    end_ns: u64,
    fd: i32,
    target: String,
    operation: &'static str,
    count: Option<usize>,
    request: Option<u64>,
    request_name: Option<&'static str>,
    result: i64,
    errno: Option<i32>,
    before: Option<Snapshot>,
    after: Option<Snapshot>,
}

struct Pending {
    event: Event,
    pointer: usize,
    after_size: usize,
}

fn config() -> Option<&'static Config> {
    CONFIG
        .get_or_init(|| {
            let target = std::env::var_os("PORT_CAPTURE_TARGET")?;
            let prefix = std::env::var_os("PORT_CAPTURE_PREFIX")?;
            match std::fs::canonicalize(target) {
                Ok(target) => Some(Config {
                    target,
                    prefix: prefix.into(),
                    session_ns: now_ns(),
                }),
                Err(_) => {
                    report_failure();
                    None
                }
            }
        })
        .as_ref()
}

fn report_failure() {
    // A constant diagnostic avoids allocations and recursion through the shim.
    const MESSAGE: &[u8] =
        b"port-recorder: capture incomplete (configuration, memory allocation, or log failure)\n";
    unsafe {
        libc::syscall(libc::SYS_write, 2, MESSAGE.as_ptr(), MESSAGE.len());
    }
}

fn now_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // CLOCK_MONOTONIC is process-independent and suited to merging thread logs.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) } != 0 {
        report_failure();
    }
    (ts.tv_sec as u64) * 1_000_000_000 + ts.tv_nsec as u64
}

fn snapshot(pointer: usize, length: usize) -> Snapshot {
    let mut bytes = vec![0u8; length.min(LIMIT)];
    let copied = if bytes.is_empty() {
        0
    } else {
        let local = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: bytes.len(),
        };
        let remote = libc::iovec {
            iov_base: pointer as *mut c_void,
            iov_len: bytes.len(),
        };
        // The kernel validates the remote range; we never dereference it in Rust.
        unsafe { libc::process_vm_readv(libc::getpid(), &local, 1, &remote, 1, 0) }
    };
    let memory_errno =
        (copied < 0).then(|| std::io::Error::last_os_error().raw_os_error().unwrap_or(0));
    bytes.truncate(copied.max(0) as usize);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in &bytes {
        hex.push(HEX[(byte >> 4) as usize] as char);
        hex.push(HEX[(byte & 15) as usize] as char);
    }
    Snapshot {
        requested: length,
        copied: bytes.len(),
        complete: bytes.len() == length,
        hex,
        memory_errno,
    }
}

// Values/names from public Linux ppdev UAPI (P-0014). Match full requests so an
// arbitrary ioctl with the same low byte is not interpreted as a ppdev buffer.
fn request_name(request: u64) -> Option<&'static str> {
    Some(match request {
        0x40047080 => "PPSETMODE",
        0x80017081 => "PPRSTATUS",
        0x80017083 => "PPRCONTROL",
        0x40017084 => "PPWCONTROL",
        0x80017085 => "PPRDATA",
        0x40017086 => "PPWDATA",
        0x708b => "PPCLAIM",
        0x708c => "PPRELEASE",
        0x708d => "PPYIELD",
        0x4002708e => "PPFCONTROL",
        0x708f => "PPEXCL",
        0x40047090 => "PPDATADIR",
        0x40047091 => "PPNEGOT",
        0x40017092 => "PPWCTLONIRQ",
        0x80047093 => "PPCLRIRQ",
        0x40047094 => "PPSETPHASE",
        0x80107095 => "PPGETTIME",
        0x40107096 => "PPSETTIME",
        0x80047097 => "PPGETMODES",
        0x80047098 => "PPGETMODE",
        0x80047099 => "PPGETPHASE",
        0x8004709a => "PPGETFLAGS",
        0x4004709b => "PPSETFLAGS",
        _ => return None,
    })
}

fn begin(fd: i32, operation: i32, argument: u64, pointer: usize) -> Option<Pending> {
    let config = config()?;
    let target = std::fs::read_link(format!("/proc/self/fd/{fd}")).ok()?;
    if target != config.target {
        return None;
    }
    let (name, count, request) = match operation {
        0 => ("read", Some(argument as usize), None),
        1 => ("write", Some(argument as usize), None),
        2 => ("ioctl", None, Some(argument)),
        _ => return None,
    };
    let request_name = request.and_then(request_name);
    let size = if request_name.is_some() {
        ((argument >> 16) & 0x3fff) as usize
    } else {
        0
    };
    let before_size = match operation {
        1 => Some(argument as usize),
        2 if argument & (1 << 30) != 0 && size != 0 => Some(size),
        _ => None,
    };
    let before = before_size.map(|length| snapshot(pointer, length));
    let after_size = if operation == 2 && argument & (2 << 30) != 0 {
        size
    } else {
        0
    };
    Some(Pending {
        event: Event {
            schema: 1,
            session_ns: config.session_ns,
            pid: unsafe { libc::getpid() },
            tid: unsafe { libc::syscall(libc::SYS_gettid) },
            sequence: SEQUENCE.with(|n| {
                let next = n.get() + 1;
                n.set(next);
                next
            }),
            start_ns: now_ns(),
            end_ns: 0,
            fd,
            target: target.to_string_lossy().into_owned(),
            operation: name,
            count,
            request,
            request_name,
            result: 0,
            errno: None,
            before,
            after: None,
        },
        pointer,
        after_size,
    })
}

/// Called only by the ABI shim. The returned allocation must be consumed once
/// by `port_capture_end`, even when the forwarded operation fails.
#[unsafe(no_mangle)]
pub extern "C" fn port_capture_begin(
    fd: i32,
    operation: i32,
    argument: u64,
    pointer: usize,
) -> *mut c_void {
    match std::panic::catch_unwind(|| begin(fd, operation, argument, pointer)) {
        Ok(Some(pending)) => Box::into_raw(Box::new(pending)).cast(),
        Ok(None) => std::ptr::null_mut(),
        Err(_) => {
            report_failure();
            std::ptr::null_mut()
        }
    }
}

/// Finish a forwarded call and append its result to this process/thread's log.
///
/// # Safety
/// `pending` must be null or an unconsumed pointer from `port_capture_begin`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn port_capture_end(pending: *mut c_void, result: i64, error: i32) {
    if pending.is_null() {
        return;
    }
    let outcome = std::panic::catch_unwind(|| {
        // SAFETY: ownership is transferred back exactly once by the shim.
        let mut pending = unsafe { Box::from_raw(pending.cast::<Pending>()) };
        pending.event.end_ns = now_ns();
        pending.event.result = result;
        pending.event.errno = (result < 0).then_some(error);
        if pending.event.operation == "read" && result > 0 {
            pending.event.after = Some(snapshot(pending.pointer, result as usize));
        } else if result >= 0 && pending.after_size != 0 {
            pending.event.after = Some(snapshot(pending.pointer, pending.after_size));
        }
        let config = config().expect("configuration existed at entry");
        let mut filename = config.prefix.as_os_str().to_os_string();
        filename.push(format!(
            ".{}.{}.{}.jsonl",
            config.session_ns, pending.event.pid, pending.event.tid
        ));
        let mut encoded = serde_json::to_vec(&pending.event)?;
        encoded.push(b'\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(PathBuf::from(filename))?;
        file.write_all(&encoded)
    });
    if !matches!(outcome, Ok(Ok(()))) {
        report_failure();
    }
}
