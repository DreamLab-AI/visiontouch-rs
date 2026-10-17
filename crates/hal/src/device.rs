//! Observed readout subset: docs/spec/10-device-readout.md, P-0018–P-0028.
//! This is not a force controller or a hardware safety interlock.

use super::ioctl;
use std::ffi::c_ulong;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};
use std::path::Path;
use std::time::{Duration, Instant};

/// Transport backstop for every motor channel, in DAC counts from neutral.
///
/// `write_motor_offsets` clamps each channel to `±MAX_MOTOR_OFFSET` regardless of
/// what the caller asks for. 300 counts is 14.7% of the 2047-count half-span of the
/// 12-bit output word. It is an independent bound on the amplifier command, not a
/// force rating: the SDK guard applies tighter application limits above it.
pub const MAX_MOTOR_OFFSET: i16 = 300;

fn clamp_offsets(offsets: [i16; 3]) -> [i16; 3] {
    offsets.map(|x| x.clamp(-MAX_MOTOR_OFFSET, MAX_MOTOR_OFFSET))
}

const CLAIM: c_ulong = 0x708b;
const RELEASE: c_ulong = 0x708c;
const EXCLUSIVE: c_ulong = 0x708f;
const READ_STATUS: c_ulong = 0x80017081;
const WRITE_CONTROL: c_ulong = 0x40017084;
const READ_DATA: c_ulong = 0x80017085;
const WRITE_DATA: c_ulong = 0x40017086;
const DATA_DIRECTION: c_ulong = 0x40047090;

trait Bus {
    fn control(&mut self, value: u8) -> io::Result<()>;
    fn direction(&mut self, input: bool) -> io::Result<()>;
    fn status(&mut self) -> io::Result<u8>;
    fn put(&mut self, value: u8) -> io::Result<()>;
    fn get(&mut self) -> io::Result<u8>;
}

struct Port(File);

impl Port {
    fn no_arg(&self, request: c_ulong) -> io::Result<()> {
        // SAFETY: valid descriptor; these requests take no argument.
        checked(unsafe { ioctl(self.0.as_raw_fd(), request) })
    }

    fn byte(&self, request: c_ulong, mut value: u8) -> io::Result<u8> {
        // SAFETY: all callers use a one-byte ppdev UAPI request with a live pointer.
        checked(unsafe { ioctl(self.0.as_raw_fd(), request, &mut value as *mut u8) })?;
        Ok(value)
    }
}

/// A failed shutdown after a failed motor write outranks the write error: the caller
/// must learn that the neutral packets may not have been delivered.
fn write_failure(write: io::Error, shutdown: io::Result<()>) -> io::Error {
    match shutdown {
        Ok(()) => write,
        Err(shutdown) => io::Error::new(
            shutdown.kind(),
            format!("output shutdown failed: {shutdown}; after: {write}"),
        ),
    }
}

fn checked(result: i32) -> io::Result<()> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

impl Bus for Port {
    fn control(&mut self, value: u8) -> io::Result<()> {
        self.byte(WRITE_CONTROL, value).map(|_| ())
    }
    fn direction(&mut self, input: bool) -> io::Result<()> {
        let value = i32::from(input);
        // SAFETY: DATA_DIRECTION reads a correctly aligned four-byte integer.
        checked(unsafe { ioctl(self.0.as_raw_fd(), DATA_DIRECTION, &value as *const i32) })
    }
    fn status(&mut self) -> io::Result<u8> {
        self.byte(READ_STATUS, 0)
    }
    fn put(&mut self, value: u8) -> io::Result<()> {
        self.byte(WRITE_DATA, value).map(|_| ())
    }
    fn get(&mut self) -> io::Result<u8> {
        self.byte(READ_DATA, 0)
    }
}

fn wait(bus: &mut impl Bus, expected: u8, deadline: Instant) -> io::Result<()> {
    for _ in 0..1000 {
        if bus.status()? & 0x80 == expected {
            return Ok(());
        }
        if Instant::now() >= deadline {
            break;
        }
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!("EPP handshake: status bit 7 did not become {expected:#04x}"),
    ))
}

fn transfer(bus: &mut impl Bus, address: bool, input: bool, data: &mut [u8]) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_millis(5);
    // INIT stays low as requested by the observed device initialization.
    let idle = u8::from(!input);
    let active = idle | if address { 8 } else { 2 };
    let result = (|| {
        bus.direction(input)?;
        bus.control(idle)?;
        for byte in data {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "EPP transfer deadline",
                ));
            }
            if !input {
                bus.put(*byte)?;
            }
            bus.control(active)?;
            wait(bus, 0, deadline)?;
            if input {
                *byte = bus.get()?;
            }
            bus.control(idle)?;
            wait(bus, 0x80, deadline)?;
        }
        Ok(())
    })();
    // Try both cleanup steps even if the transfer or first cleanup step fails.
    let controls = bus.control(0);
    let direction = bus.direction(false);
    result.and(controls).and(direction)
}

/// One complete packet. Counts are raw wrapping values, not calibrated positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    /// Complete 16-byte packet in wire order, including counters and status bytes.
    pub raw: [u8; 16],
}

impl Sample {
    /// Active-low stylus button observed on the owner's unit (P-0026).
    pub fn stylus_button_pressed(self) -> bool {
        self.raw[13] & 0x02 == 0
    }

    /// Axis order observed in the reference SDK, without wrap accumulation.
    pub fn encoder_counts(self) -> [i16; 6] {
        [2, 0, 1, 3, 4, 5].map(|i| i16::from_le_bytes([self.raw[i * 2], self.raw[i * 2 + 1]]))
    }
}

/// Owns the port claim. Connect performs active setup writes; motor-output
/// commands require the explicit observed force-session handshake. Use only with
/// the owner's identified device.
pub struct DeviceReader {
    port: Port,
    claimed: bool,
    healthy: bool,
    force_sequence: Option<u8>,
    last_sequence: u8,
}

impl DeviceReader {
    /// Exclusively claim a ppdev character device and perform observed active setup.
    /// Does not start a force session. On partial failure, cleanup is attempted.
    /// Unsupported paths, claim failures and handshake timeouts return I/O errors.
    pub fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(0x800) // Linux O_NONBLOCK; transfers use bounded register ioctls.
            .open(path)?;
        if !file.metadata()?.file_type().is_char_device() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected a ppdev character device",
            ));
        }
        let port = Port(file);
        port.no_arg(EXCLUSIVE)?;
        port.no_arg(CLAIM)?;
        let mut reader = Self {
            port,
            claimed: true,
            healthy: true,
            force_sequence: None,
            last_sequence: 0,
        };
        reader.port.control(0)?;
        reader.select(0x17)?;
        transfer(&mut reader.port, false, false, &mut [0x01])?;
        reader.select(0x0e)?;
        let mut reply = [0];
        transfer(&mut reader.port, false, true, &mut reply)?;
        if reply != [1] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unexpected setup reply {reply:02x?}"),
            ));
        }
        // Reference selects zero twice before its first packet (sample selects again).
        reader.select(0)?;
        Ok(reader)
    }

    fn select(&mut self, address: u8) -> io::Result<()> {
        transfer(&mut self.port, true, false, &mut [address])
    }

    /// Read one complete packet and validate any pending force acknowledgement.
    /// A failed transaction marks the connection unhealthy; subsequent sampling
    /// requires a new connection. The caller must attempt session shutdown or
    /// close on error; this low-level read does not itself send shutdown packets.
    pub fn sample(&mut self) -> io::Result<Sample> {
        if !self.healthy {
            return Err(io::Error::other("connection failed; close and reconnect"));
        }
        let result = (|| {
            self.select(0)?;
            let mut raw = [0; 16];
            transfer(&mut self.port, false, true, &mut raw)?;
            if let Some(expected) = self.force_sequence {
                if raw[14] != expected || raw[13] & 0x40 == 0 {
                    return Err(io::Error::other(format!(
                        "motor acknowledgement/ready status lost: expected {expected:02x}, got {:02x}, status {:02x}, raw {raw:02x?}",
                        raw[14], raw[13]
                    )));
                }
            }
            self.last_sequence = raw[14];
            Ok(Sample { raw })
        })();
        if result.is_err() {
            self.healthy = false;
        }
        result
    }

    /// Begin the observed zero-only motor handshake. Nonzero offsets are rejected
    /// until it completes. No physical force calibration or hardware watchdog implied.
    pub fn begin_force_session(&mut self) -> io::Result<()> {
        if !self.healthy || self.force_sequence.is_some() {
            return Err(io::Error::other(
                "force session requires a healthy inactive connection",
            ));
        }
        let result = (|| {
            self.sample()?;
            self.sample()?;
            self.select(0x17)?;
            transfer(&mut self.port, false, false, &mut [0x02])?;
            self.sample()?;
            self.send_motor_packet([0; 3], 0xc0, self.last_sequence)?;
            std::thread::sleep(Duration::from_millis(5));
            self.sample()
                .map_err(|e| io::Error::other(format!("initial zero: {e}")))?;
            for _ in 0..4 {
                self.send_motor_packet([0; 3], 0xce, self.last_sequence.wrapping_add(1))?;
                std::thread::sleep(Duration::from_millis(2));
                self.sample()
                    .map_err(|e| io::Error::other(format!("zero preamble: {e}")))?;
            }
            // Leave a fresh zero output pending for the first servo sample.
            self.send_motor_packet([0; 3], 0xca, self.last_sequence.wrapping_add(1))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = self.end_force_session();
            self.healthy = false;
        }
        result
    }

    fn send_motor_packet(
        &mut self,
        offsets: [i16; 3],
        control: u8,
        sequence: u8,
    ) -> io::Result<()> {
        let mut packet = [0u8; 8];
        for (i, offset) in offsets.into_iter().enumerate() {
            packet[i * 2..i * 2 + 2]
                .copy_from_slice(&(2047u16.wrapping_add_signed(offset)).to_le_bytes());
        }
        packet[6] = control;
        packet[7] = sequence;
        // Mark pending before I/O so even a partial write gets a shutdown attempt.
        self.force_sequence = Some(sequence);
        transfer(&mut self.port, false, false, &mut packet)
    }

    /// Bounded raw actuator output for the identified unit; prefer the SDK's
    /// force guard. The transport independently caps every channel at
    /// [`MAX_MOTOR_OFFSET`] counts; this is a backstop, not the application limit.
    pub fn write_motor_offsets(&mut self, offsets: [i16; 3]) -> io::Result<()> {
        if !self.healthy || self.force_sequence.is_none() {
            return Err(io::Error::other("motor output is not armed"));
        }
        let offsets = clamp_offsets(offsets);
        match self.send_motor_packet(offsets, 0xca, self.last_sequence.wrapping_add(1)) {
            Ok(()) => Ok(()),
            Err(write) => {
                self.healthy = false;
                Err(write_failure(write, self.end_force_session()))
            }
        }
    }

    /// Attempt both neutral/shutdown packets and clear local session state.
    /// A failed cleanup marks the connection unhealthy. Success is transport-level
    /// evidence, not a measurement of zero motor force. No session is a no-op.
    pub fn end_force_session(&mut self) -> io::Result<()> {
        let Some(sequence) = self.force_sequence else {
            return Ok(());
        };
        // Best effort on every path, including partial reads/writes. These are
        // the reference shutdown packets, not an assertion of amplifier isolation.
        let first = self.send_motor_packet([0; 3], 0xc5, sequence.wrapping_add(1));
        let read = (|| {
            self.select(0)?;
            let mut raw = [0; 16];
            transfer(&mut self.port, false, true, &mut raw)?;
            self.last_sequence = raw[14];
            Ok(())
        })();
        let next = if read.is_ok() {
            self.last_sequence.wrapping_add(1)
        } else {
            sequence.wrapping_add(2)
        };
        let second = self.send_motor_packet([0; 3], 0xc5, next);
        self.force_sequence = None;
        let result = first.and(read).and(second);
        if result.is_err() {
            self.healthy = false;
        }
        result
    }

    fn release(&mut self) -> io::Result<()> {
        if !self.claimed {
            return Ok(());
        }
        let force = self.end_force_session();
        let controls = self.port.control(0);
        let direction = self.port.direction(false);
        let release = self.port.no_arg(RELEASE);
        if release.is_ok() {
            self.claimed = false;
        }
        force.and(controls).and(direction).and(release)
    }

    /// Consume the reader and attempt shutdown, signal cleanup and port release.
    /// All cleanup stages are attempted even after an earlier error; Drop retries
    /// release if the explicit release failed.
    pub fn close(mut self) -> io::Result<()> {
        self.release()
    }
}

impl Drop for DeviceReader {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn motor_offsets_clamp_at_max_motor_offset() {
        assert_eq!(clamp_offsets([301, -301, i16::MIN]), [300, -300, -300],);
        assert_eq!(clamp_offsets([300, -299, 0]), [300, -299, 0]);
        assert_eq!(clamp_offsets([i16::MAX, 1, -1]), [300, 1, -1]);
    }

    #[derive(Default)]
    struct Fake {
        statuses: VecDeque<u8>,
        controls: Vec<u8>,
        directions: Vec<bool>,
        written: Vec<u8>,
        input: VecDeque<u8>,
        fail_data: bool,
    }
    impl Bus for Fake {
        fn control(&mut self, v: u8) -> io::Result<()> {
            self.controls.push(v);
            Ok(())
        }
        fn direction(&mut self, v: bool) -> io::Result<()> {
            self.directions.push(v);
            Ok(())
        }
        fn status(&mut self) -> io::Result<u8> {
            Ok(self.statuses.pop_front().unwrap_or(0x80))
        }
        fn put(&mut self, v: u8) -> io::Result<()> {
            self.written.push(v);
            Ok(())
        }
        fn get(&mut self) -> io::Result<u8> {
            if self.fail_data {
                return Err(io::Error::other("injected data error"));
            }
            Ok(self.input.pop_front().unwrap())
        }
    }
    #[test]
    fn a_failed_shutdown_outranks_the_write_error_that_caused_it() {
        let e = write_failure(io::Error::other("write"), Ok(()));
        assert_eq!(e.to_string(), "write");
        let e = write_failure(
            io::Error::other("write"),
            Err(io::Error::new(io::ErrorKind::TimedOut, "neutral")),
        );
        assert_eq!(e.kind(), io::ErrorKind::TimedOut);
        let text = e.to_string();
        assert!(
            text.starts_with("output shutdown failed: neutral") && text.ends_with("after: write"),
            "{text}"
        );
    }
    #[test]
    fn observed_button_press_and_release() {
        let mut sample = Sample { raw: [0; 16] };
        for (byte, pressed) in [(0x1f, false), (0x1d, true), (0x1f, false)] {
            sample.raw[13] = byte;
            assert_eq!(sample.stylus_button_pressed(), pressed);
        }
    }
    #[test]
    fn observed_encoder_order_and_sign() {
        let sample = Sample {
            raw: [
                0xe4, 0xfb, 0x4c, 0x04, 0x9a, 0x18, 0x8f, 0x03, 0x85, 0xff, 0x22, 0xff, 0xe2, 0x1f,
                0x02, 0x0c,
            ],
        };
        assert_eq!(
            sample.encoder_counts(),
            [6298, -1052, 1100, 911, -123, -222]
        );
    }
    #[test]
    fn address_write_preserves_init_and_cleans_up() {
        let mut bus = Fake {
            statuses: [0x80, 0, 0, 0x80].into(),
            ..Fake::default()
        };
        transfer(&mut bus, true, false, &mut [0x17]).unwrap();
        assert_eq!(bus.written, [0x17]);
        assert_eq!(bus.controls, [1, 9, 1, 0]);
        assert_eq!(bus.directions, [false, false]);
    }
    #[test]
    fn complete_read_handshake() {
        let mut bus = Fake {
            statuses: [0, 0x80, 0, 0x80].into(),
            input: [0x80, 0xff].into(),
            ..Fake::default()
        };
        let mut data = [0; 2];
        transfer(&mut bus, false, true, &mut data).unwrap();
        assert_eq!(data, [0x80, 0xff]);
        assert_eq!(bus.controls, [0, 2, 0, 2, 0, 0]);
        assert_eq!(bus.directions, [true, false]);
    }
    #[test]
    fn stalled_handshake_is_bounded_and_cleans_up() {
        let mut bus = Fake::default();
        assert_eq!(
            transfer(&mut bus, false, true, &mut [0])
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(bus.controls.last(), Some(&0));
        assert_eq!(bus.directions.last(), Some(&false));
    }
    #[test]
    fn data_failure_deasserts_strobe_and_restores_direction() {
        let mut bus = Fake {
            statuses: [0].into(),
            fail_data: true,
            ..Fake::default()
        };
        assert!(transfer(&mut bus, false, true, &mut [0]).is_err());
        assert_eq!(bus.controls, [0, 2, 0]);
        assert_eq!(bus.directions, [true, false]);
    }
}
