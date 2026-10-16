# haptics-device

Reusable Rust SDK for the owner's identified parallel-port unit. `haptics-hal`
handles the port and packet lifecycle; this crate handles device state, a
reference kinematic model, force transforms and bounded requests. Mass Lab owns
world physics and submits the equal/opposite spring-coupling reaction.

- Six raw and accumulated encoder channels, including pen/wrist inputs; button.
- Reference Cartesian position in metres and an analytic force Jacobian.
- `Wrench { force, torque }` interface; nonzero torque is rejected until the
  possible wrist actuators and their output protocol are verified.
- Explicit zero-handshake arming, 0.5 N vector cap, 2 N/s slew cap, 96-count
  actuator-offset cap, and immediate zero for stale (>25 ms) or zero requests.
- Non-finite requests disarm. Read/acknowledgement failures end the force session.
- Close/drop attempts reference zero/shutdown packets and releases the port.

Call `Device::sample()` on a dedicated worker continuously; it reads encoders,
checks acknowledgement, evaluates the force guard and sends the next output.
Submit timestamped requests from the application using `Device::request()`.
`enable_feedback()` performs the zero-only handshake before arming. `connect()`
alone does not enable motor output. `EncoderState` is usable without hardware.
The optional `shutdown::install()` installs SIGINT/SIGTERM flags; the application
must check `shutdown::requested()` in its worker and join it before exiting.

The model matches black-box reference queries, not a measured physical force
calibration. A force request in newtons is a model-derived command, not a load-cell
measurement. Hardware watchdog behavior, hard real-time timing, wrist actuation,
and physical calibration remain unverified. Software cannot guarantee shutdown
if the process is forcibly killed or the kernel stops scheduling it.

```sh
cargo test -p haptics-device -p haptics-hal
cargo build -p haptics-device --example feedback-check
# Stop/disconnect Mass Lab before opening the same port.
tools/with-port-access target/debug/examples/feedback-check
```

The check defaults to zero output for 400 ms and closes. An optional signed Y
force argument is limited to ±0.1 N for the first 200 ms, then zero for 200 ms.
Live zero and -0.1 N runs each received 168 acknowledged samples and completed
shutdown. They establish transport operation, not measured force magnitude.
The owner subsequently reported that motors do not stop pen movement at block
surfaces (P-0033). Effective physical contact resistance remains unresolved;
calibration, command mapping, app scaling/limits and actuator response need
investigation before claiming validated physical feedback.

See [force specification](../../docs/spec/12-force-feedback.md) for provenance,
model equations, lifecycle and application coupling.

`EncoderState::continuous_counts()` maintains absolute unwrapped history;
`position_m()` and the device force transform share that history. `recenter()`
only resets relative counters. `ForceLimits` can lower the SDK ceilings and
optionally latch a sustained-command-limit release; `UNIT_DERATING` scales force,
slew and DAC ceilings once. Neither facility measures actual motor load.
After a failed sample, `take_force_fault()` distinguishes a successfully stopped
command-limit trip from an I/O failure. Failed shutdown always requires closing
and reconnecting; it cannot be reported as a recoverable software trip.
