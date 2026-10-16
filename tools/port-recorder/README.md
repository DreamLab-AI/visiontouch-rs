# Reference-call recorder

Rust captures ppdev ioctl arguments and read/write buffers through an LD_PRELOAD
shim. This is observation tooling, not a connection driver or safety interlock.
It forwards calls; using it with live vendor software can affect the device.

Build and verify without hardware:

```sh
tools/build-recorder
python3 tools/port-recorder/tests/check.py
```

The build produces `target/debug/libport_capture.so`. The small C/assembly shim
preserves the Linux x86-64 calling convention, forwards to the next libc entry
point, and restores errno. Rust owns snapshots, timing and JSON serialization.
The test compares an instrumented run with an uninstrumented run on ordinary
files and exercises failures, invalid pointers, truncation, multiple threads,
reused output prefixes and log failures.

For the query-only Rust host tool:

```sh
cargo build -p port-probe
mkdir -p vendor-original
tools/with-port-access env \
  LD_PRELOAD="$PWD/target/debug/libport_capture.so" \
  PORT_CAPTURE_TARGET=/dev/parport0 \
  PORT_CAPTURE_PREFIX="$PWD/vendor-original/host-query" \
  target/debug/port-probe /dev/parport0
```

This command only queries host capabilities. It does not identify the unit.
The environment must be set after the group-refresh launcher because sudo
sanitizes loader variables. Never preload reference software into a root process.

`PORT_CAPTURE_TARGET` selects one canonical descriptor target; symlinks resolve
to the same target. `PORT_CAPTURE_PREFIX` selects the log path prefix. The parent
directory must already exist. Files are named
`PREFIX.SESSION_NS.PID.TID.jsonl`; each event carries the same identifiers.
Every process starts a new session, so repeated sandbox PIDs do not merge runs.
Check stderr for `capture incomplete` and reject affected captures.

See [the format specification](../../docs/spec/01-call-capture.md) for field
semantics and coverage limits. Byte strings are hex in memory order; the current
ABI is little-endian. A successful write only transfers the prefix specified by
its result. Failed requests and incomplete snapshots must remain explicit.

Cross-check against a simultaneous strace with full buffers and descriptor paths.
The recorder does not intercept direct syscalls, direct port I/O, readv/writev,
hidden libc entry points, or calls in other loader namespaces. It alters timing
and records only completed calls. Capture completeness requires evidence from
the specific run, not merely a successful recorder test.

Vendor experiments belong in `tools/vendor-sandbox` until the hardware interlock
is understood. The current ignored `vendor-original/offline-record.sh` reproduces
the mock-file experiment. Its expected outcome is failed initialization.
