# Device readout

Independent Rust connection to the owner's parallel 1.0A on Linux x86-64.
Uses ppdev register ioctls as an ordinary user, without vendor libraries.

```sh
cargo build -p device-readout
tools/with-port-access timeout -s KILL 10s target/debug/device-readout /dev/parport0 50
```

Arguments are device path and sample count (default `/dev/parport0`, 10; maximum
600). Each sample is a full 16-byte packet. Output is CSV containing elapsed
milliseconds, six raw signed encoder counters, the original hex packet, a button
flag and six accumulated relative counters (including wrist/pen channels).
Sampling is about 10 Hz and is not a real-time control loop.

Connection performs active setup writes observed in a successful reference
capture. Keep the workspace clear and power cutoff accessible. This readout tool does not enable output or send force packets. The separate
`haptics-device` SDK implements bounded force requests. The button and all six
counter rollovers are decoded; physical calibration and wrist axis meanings
remain unverified.

The Oxford card advertises EPP, but the Linux ECP/EPP callbacks overwrite a control
line used by this unit. Software EPP handshakes preserve INIT low and fix the
connection on this host. Each handshake wait has a 5 ms deadline and 1000-poll
limit. Failed or partial samples are rejected and stop acquisition. Normal/error
unwinding clears strobes, returns data direction to output, and releases the claim.
The external timeout also bounds process lifetime, including a potentially blocked
kernel claim; forced termination relies on kernel descriptor cleanup.

Verified: five packets under strace, then a fresh connection receiving 50 packets
over 4.916 seconds; no errors and no remaining port owner. SDK comparison validates
the packet's signed word order. This is a working readout connection, not force
feedback support. Capture evidence and source attribution are in
[the protocol specification](../../docs/spec/10-device-readout.md).

Library use:

```rust,no_run
use haptics_hal::device::DeviceReader;

# fn main() -> std::io::Result<()> {
let mut device = DeviceReader::connect("/dev/parport0")?;
let sample = device.sample()?;
println!("{:?}", sample.encoder_counts());
device.close()?;
# Ok(())
# }
```
