# visiontouch-device

Rust SDK for the owner's identified parallel-port unit. `visiontouch-hal` handles the
port and packet lifecycle; this crate handles device state, a reference kinematic
model, reset-pose registration, and a guarded force output.

**Commands are model values. Delivered force is unmeasured.** Every "newton" here is
a model command derived from a reference kinematic model and fixed gains. No delivered
force has been established; only an attended session with a gauge or mass can do
that (`docs/spec/14-force-calibration.md`). Wrist and pen actuation is unsupported:
there is no torque term anywhere in the API.

## What it provides

- Six raw and unwrapped encoder channels, including pen/wrist inputs, and the button.
- **Registration**: `Device::register_reset()` makes the current pose the model origin.
  Registered counts, position and Jacobian are the only frame the guard uses.
  **Registration precedes any Cartesian demand**: `Demand::Force` is refused while
  unregistered. A reconnect is a new `Device`, so registration never survives it.
  The raw `Demand::Channels` liveness test does not need registration.
- A guard with a software deadman, force-vector cap, slew limit and uniform DAC scaling.

## Envelope

| | Default (`ForceLimits::default()`) | Ceiling |
|---|---|---|
| Force vector | 0.5 N (`DEFAULT_FORCE_N`) | 2 N (`MAX_FORCE_N`) |
| Per-channel DAC offset | 96 counts (`DEFAULT_DAC_OFFSET`) | 300 counts (`MAX_DAC_OFFSET`) |
| Slew | 100 N/s | 100 N/s (`MAX_SLEW_N_S`) |

The default is the envelope armed live in P-0030. Applications can only lower the
ceilings; raising one needs an ADR (AGENTS.md rule 5). The 300-count ceiling is the
HAL transport clamp (`visiontouch_hal::device::MAX_MOTOR_OFFSET`), 14.7% of the 2047-count
half-span. Content without a fixed dwell stays at or below `CONTINUOUS_FORCE_N`
(1 N); the raw channel test is capped at 96 counts. A request older than 25 ms, or a
servo gap over 25 ms, yields zero immediately, and a zero demand is never slewed.

When a channel would exceed its DAC limit the *whole* force vector is scaled, so the
output direction is preserved and a wall gains no tangential force. There is no
latched release trip: limits cap, they never disarm. `Telemetry` (from
`ForceGuard::telemetry()`) reports `force_capped`, `dac_capped`, `slewing` and
`expired`, plus what was applied and sent.

## Use

Call `Device::sample()` on a dedicated worker continuously: it reads encoders,
checks acknowledgement, evaluates the guard and sends the output. Every `sample`
error is a transport fault; close and reconnect. `Device::request(Demand, Instant)`
returns `RequestError::Rejected` (non-finite, unregistered, not armed; output was shut
down, the connection is usable) or `RequestError::ShutdownFailed` (reconnect). A
servo loop that recomputes its demand from the sample it just read stamps it `now`;
never refresh the stamp of a request that was not recomputed.

```rust,no_run
use visiontouch_device::{Demand, Device};
use std::time::Instant;
let mut device = Device::connect("/dev/parport0")?;
device.sample()?;
device.register_reset().map_err(std::io::Error::other)?; // arm at its reset pose
device.enable_feedback()?; // zero-only handshake, then arm
device.request(Demand::Force([0.; 3]), Instant::now()).ok();
device.sample()?;
device.close()?;
# Ok::<(), std::io::Error>(())
```

`EncoderState` is usable without hardware. The optional `serde` feature derives
`Serialize`/`Deserialize` for `Registration`. `shutdown::install()` installs
SIGINT/SIGTERM flags; the application must check `shutdown::requested()` in its
worker and join it before exiting. Software cannot guarantee shutdown if the process
is killed or the kernel stops scheduling it; no hardware watchdog is claimed.

## Checks

```sh
cargo test -p visiontouch-device -p visiontouch-hal
cargo build -p visiontouch-device --examples
# Stop/disconnect Mass Lab before opening the same port; hold the arm at its reset pose.
scripts/with-port-access target/debug/examples/feedback-check
```

`feedback-check` registers the start pose, then commands zero for 400 ms, or an
optional signed Y force limited to ±0.1 N for the first 200 ms, and closes. It
establishes transport operation, not delivered force. `baseline` records input only.

See [force specification](../../docs/spec/12-force-feedback.md) for provenance, model
equations and lifecycle. The kinematic model matches black-box reference queries, not
a measured physical calibration of this unit.
