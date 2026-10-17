# Driver acquisition and examination — 2026-10-16

## Live outcome

Reference SDK initialization and an independent Rust readout now work on the actual
unit. The owner was present and explicitly authorized powered-motor testing.
Native ECP/EPP reads changed control 0x00 to 0x04 and failed; software EPP cycles
preserving INIT succeeded. The Rust reader received 55 complete packets across
two fresh connections and released the port. Six signed encoder words match a
successful SDK query. Calibration and force control remain unimplemented.

See [readout specification](../spec/10-device-readout.md), P-0018–P-0023 and
[run instructions](../../tools/device-readout/README.md). All earlier statements
below about a missing power answer or untested hardware are historical.

## Acquired packages

Originals, extracted packages, vendor documentation, and local experiment harnesses
are in ignored `vendor-original/`. No installer was executed and no vendor library
was installed system-wide. [driver-downloads.json](driver-downloads.json) records
source URLs, date, size, and SHA-256 for all acquired originals and compatibility
libraries. P-0007 through P-0013 record sources and findings.

- Windows PDD 5.1.9: acquired directly from the vendor public S3 bucket;
  extracted as an NSIS archive with 7zip. Includes parallel support components,
  configuration/test utilities and Premium 1.0 documentation. No Windows executable
  was run; unit compatibility remains untested.
- Linux SDK 3.4: acquired directly from the vendor bucket. ELF dependency metadata
  shows that it needs a separate I/O library plus ncurses/tinfo ABI 5.
- Linux driver 2016.1: acquired directly from the vendor bucket and extracted. It
  supplies the library named by SDK 3.4, but parallel support has not been established.
- Linux SDK 3.0 bundle mirrored by Johns Hopkins: contains PDD 4.3 packages in
  32/64-bit variants. The supplied Linux installation guide lists Premium 1.0 and
  a parallel interface. Guide statements alone do not prove actual unit compatibility.
- JUJU 64-bit patch mirrored by Johns Hopkins: uses current libraw1394 ABI 11,
  unlike the original PDD 4.3 library, which needs ABI 8. The mirror maintainer says
  the patch origin is unknown. Retain this uncertainty when reporting results.

The vendor support site still exposes a loading shell to the web reader. Listing
public keys in the vendor S3 bucket identified direct downloads; no access bypass
or authenticated download was needed.

## Offline runtime observations

The SDK 3.0 and JUJU library load using the host runtime plus ncurses ABI-5
libraries extracted locally from Ubuntu packages. The configuration utility also
needed Open Motif, libXpm and a locally extracted GLw library. Xvfb and xdotool were
used for an isolated virtual display. Rust, strace and 7zip were installed through
Omarchy package commands. Compatibility packages were extracted, not installed.

The initial offline vendor execution occurred as ordinary John in bubblewrap with a fresh `/dev`,
no host parallel port, isolated network namespace and no desktop connection.
A generated Premium A / workspace 1.0 configuration advances initialization to
port access. With no port it returns a device error. With an ordinary mock file
at the expected port path it attempts claim and mode-setting ioctls, fails with
ENOTTY, releases/closes, and returns a device error. These observations establish
a ppdev capture route for this library; they establish no peripheral protocol
values or working hardware connection. No disassembly/decompilation was performed.

The generated configuration stays ignored. Its encoder settings are unverified
utility defaults, and its legacy base address is not the host card address. It is
an offline experiment input, not a reviewed live configuration.

## Rust and host verification

`cargo build --workspace` succeeds. `scripts/with-port-access target/debug/port-probe`
opens `/dev/parport0` as John and returns host capability mask `0x4f`. A syscall
trace confirms only open, PPGETMODES, close on that node. No port claim,
negotiation or data transfer occurred. The host query is the functional verification of port-probe. The recorder now
has automated integration checks using ordinary files; see tools/port-recorder/README.md.

## Continue from here

1. Establish the actual unit power arrangement and a reliable way to prevent motor
   output before live initialization. Owner was asked; answer is pending.
2. Review the offline configuration against the unit and host. Determine whether
   the legacy library uses the configured address only for selection or performs
   any direct I/O. Do not substitute 0x378 for the actual card base.
3. Capture initialization against the real port only once the interlock is
   understood. Use strace with epoch timestamps, durations, full hex buffers and
   descriptor paths; check for unresolved ioctl pointers, truncation, and I/O
   outside ppdev before treating it as a complete timeline.
4. Specify observed initialization and encoder acquisition with provenance before
   implementing those operations in Rust. Validate replies against actual unit
   movement. Force commands require the watchdog, clamp and amplifier interlock.

`scripts/vendor-sandbox COMMAND [ARG ...]` is a reusable offline launcher. It exposes
reference files read-only at `/vendor`, writable experiment state at `/state`,
and defaults to `/state` as the working directory. It provides no live capture
mode. Supply runtime search paths explicitly when launching reference tools.
The experiment harness and generated configuration remain under `vendor-original/`.

## Recorder and additional offline observations

The Rust recorder captures ioctl scalar arguments and data buffers through a
small C/assembly preload shim. The automated integration checks compare baseline
and instrumented execution on regular files, including failure/errno behavior,
invalid pointers, oversized snapshots, threads, repeated sessions and log errors.
A real host query recorded `4f000000` as the four-byte capability result. The
mock-file reference trace records three failed ioctls; counts/order/results agree
with simultaneous strace. P-0014 and P-0015 cover the sources and observations.

In a separate, explicitly synthetic experiment, a mock accepts host claim and
mode-setting requests on a regular file. The reference then attempts EPP mode
and negotiation. With negotiation also accepted by the mock, the reference
attempts control ioctls and EPP address/data transfers, then reports a configuration
error. Several control operations fail without immediate termination. No unit
replies were supplied, no real port was exposed, and none of those transfer bytes
has been adopted as a peripheral specification. See P-0016. The earlier `0x100`
mode from the failed-claim trace is compatibility-mode cleanup, not evidence of
the normal EPP initialization path.

The recorder captures completed dynamically linked calls only. Raw syscalls,
hidden entry points, signals, interrupted runs and direct I/O require separate
coverage checks. A successful preload test does not establish a complete wire
trace. Real initialization/encoder verification remains pending the owner's
power/interlock details.

## PCIe address comparison and live-test blocker

P-0017 compares the generated configuration with only its base-address field
changed from 0x378 to the host card's 0xd010. Both runs reach the same EPP
negotiation attempt in the isolated host model. The library therefore does not
reject the higher address before this point, despite the legacy UI's range check.
This does not establish later behavior or hardware compatibility. The comparison
reproducer and both traces/configurations remain in ignored vendor-original/.
All nine packages in driver-downloads.json were rehashed successfully.

The next decisive evidence must come from the actual unit. Owner information about
current power state, available plugs/switches and motor-power isolation is still
missing after three goal turns. The goal is blocked pending that information;
no actual device initialization has been attempted. Further mock results cannot
satisfy the requested working connection. Resume by resolving that physical setup,
then capture actual initialization/encoder behavior and specify/implement Rust
operations from validated observations.
