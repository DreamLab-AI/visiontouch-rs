# Device SDK and force feedback

Evidence: P-0028 (black-box observation), P-0029 (application design), P-0061 (envelope), P-0062 (registration), P-0064 (Mass Lab coupling). Verification procedure: [14-force-calibration.md](14-force-calibration.md).

## Layer boundaries

`hal` owns bounded software EPP transactions and the observed motor-session
lifecycle. `visiontouch-device` is the reusable Rust SDK: six-channel counter
accumulation, reference-model kinematics, force transforms, clamps, slew limits,
and stale-command rejection, registration, and the force envelope below. The app owns bodies, gravity, contacts and grabs.
It sends the spring-coupling reaction, including the effects of inertia and friction,
not merely a mass-dependent constant force. Angular inertia contributes through
contact/grip lever arms. Independent stylus torque actuation is not established;
all six input counters must be retained even though only three motor outputs are
observed. Counts for the last three axes remain counts until calibrated.

## Output observation

Real zero-force reference scheduler trace: initial readout setup, two samples,
address 0x17 write 0x02, sample. Write eight bytes: three little-endian 2047 words,
control 0xc0, input byte 14 echoed. Next sample acknowledges that last byte and
has bit 6 of byte 13 set. Four zero packets use control 0xce and increment byte 14
by one each time, with a sample between packets. Running packets use 0xca and the
same increment/acknowledgement sequence. Reference shutdown sends two zero packets
with control 0xc5, incrementing/reading between them. These are observed lifecycle
patterns, not a complete interpretation of their control bits.

Reject failed/partial transfers, sequence mismatches and missing fresh-output status. On
fault or close, attempt zero/shutdown before releasing the claim. An output
session requires the zero handshake before any nonzero request. No automatic
rearming after a fault. A whole transfer and each handshake are bounded.

## Reference force model

Black-box encoder perturbations yield q0 = e0 * 0.00018075138,
q1 = -e2 * 0.00021136618, q2 = -e1 * 0.00021136618 radians.
Fitting independently queried positions yields link length L = 0.1397 metres:
r = L(cos(q1)+sin(q2)); x = -r sin(q0),
y = L(sin(q1)-cos(q2)+1), z = r cos(q0)-L.
This reproduces the reference configuration; physical home/calibration accuracy
is not established. The Jacobian is the derivative of this expression.
Force queries and isolated torque queries give output word changes from 2047:
- channel 0: approximately -825 * dot(J_q0, force_newtons)
- channel 1: approximately -960 * dot(J_q2, force_newtons)
- channel 2: approximately -960 * dot(J_q1, force_newtons)
Round to nearest integer. Quantization is roughly one count. Compare independent
force fixtures before enabling this path. Never copy runtime configuration data.

## Force envelope

Design: P-0061, ADR-0002. All values are commands, not measured forces (P-0060).

| Quantity | Value |
|---|---|
| SDK model-force ceiling | 2.0 N |
| Default (`ForceLimits::default`) | 0.5 N, 96 counts, 100 N/s |
| Continuous-content cap | 1.0 N (effects, demos, Mass Lab) |
| DAC ceiling | 300 counts, equal to the HAL clamp |
| Raw channel test cap | 96 counts |
| Slew | 100 N/s (0.24 N per 2.4 ms tick) |
| Request expiry | 25 ms |

The default is the envelope armed live in P-0030. Limits can only be lowered below the
ceilings, and only while disarmed. 134 counts per model newton on channels 1 and 2
(P-0028 gains, equal Jacobian column norms) gives 268 counts at 2 N; 300 adds 10 percent.
The DAC limit is the only bound that does not depend on the uncertain gains.

Requests older than 25 ms, non-finite requests and zero demands produce immediate zero;
no stale force is held and zero is never slewed. Arm and disarm are explicit; there is no
automatic rearm. Input failure ends the session. A UI pause or disconnect sends zero.
Demand above a cap is clamped and reported in telemetry (`force_capped`, `dac_capped`,
`slewing`, `expired`). There is no sustained-command release trip, so every error from
`Device::sample` is a transport fault and the connection is closed. Invalid demand is
rejected synchronously by `Device::request`; only a failed shutdown after a rejection
obliges the caller to reconnect, and it takes precedence over the rejection.

**Uniform DAC scaling.** If the largest channel demand exceeds the DAC limit, the whole
applied force vector is scaled by one factor. Per-channel clamping would rotate the force
and leak tangential force into a wall. The HAL clamp remains an independent backstop.

**Raw channel demand.** `Demand::Channels` drives the three channels directly for the
actuator liveness check (14-force-calibration.md section 6.2). It shares arming, expiry
and both clamps, skips slew and the force model, and is capped at 96 counts. It does not
need registration.

**Registration is a precondition for Cartesian demand.** `Demand::Force` yields zero
while unregistered, and `Device::request` rejects it. The Jacobian is evaluated at
registered counts (continuous counters minus the reset capture, P-0062), never at
power-on-relative angles. Registration is cleared on reconnect.

A software watchdog cannot cover an OS stall or process kill; hardware timeout behaviour
remains to be established.

## Mass Lab servo-rate coupling

Design: P-0064. Mass Lab hands its worker a `Coupling` descriptor in registered model
metres: proxy position, proxy velocity, stiffness, damping and the instant it was issued.
On every servo tick the worker reads the sample, computes F = K (proxy - p) - B (v - v_proxy)
from that sample, and requests it, so output no longer depends on the UI frame rate (F3).

Each per-tick request is stamped with the evaluation time. This does not refresh a stale
request: the request is recomputed from the sample just read and is new each tick. The
SDK rule "never refresh a stale request's time" still holds for any caller that resends
an old value. The descriptor itself has its own age limit of 100 ms (six frames at 60 fps).
An older descriptor produces zero and the reason "force muted: scene stalled", so a UI
stall cannot tether the pen to a frozen proxy. Other mute reasons are paused simulation,
time scale not 1.00x, unregistered, and feedback off.

Mass Lab uses `ForceLimits::default` (0.5 N, 96 counts), a fixed stiffness of 150 model
N/m and no measured K, and may not raise limits. A 2 kg block then produces about 0.45 N
through proxy lag. The old 0.02 scene-force scale is removed.

## Simulation (application side)

The minimum-mass block is free in translation and rotation (lowest ID breaks
ties). Other blocks have X/Z translation and all rotation locked. Re-evaluate
on add/remove/mass changes. A vertical-only grab constrains Y only; the free
block uses a spherical grip. Release retains linear and angular velocities.
No artificial linear/angular damping; contacts dissipate energy according to
friction and restitution. Show linear momentum m*v and angular momentum I_world*w.

The pen proxy is a dynamic 0.2 kg virtual body with zero gravity and locked
rotation. A spring/damper couples it to the measured target (600 N/m, 60 N·s/m,
250 N scene-force cap, target speed capped at 8 m/s). Stabilize the discrete
coupling with denominator 1 + damping*dt/proxy_mass + stiffness*dt²/proxy_mass. Apply the coupling force
to the proxy and report its equal/opposite reaction to the device SDK. This
includes contact, grab loads, linear inertia and off-centre rotational effects
without relying on solver warm-start impulses as force measurements. A stationary
supported load produces its weight through spring deflection. The rendering
shows the proxy position; the input trail shows the measured target. The worker evaluates the tether at servo rate (section above). No direct wrist torque is sent.

## Native lifecycle validation

The first native check rejected a duplicate sample after the zero preamble:
sequence matched but byte-13 bit 6 had cleared. Leave a fresh 0xca zero packet
pending after the preamble, so each subsequent servo read has a preceding write.
Native startup also allows 5 ms after the first zero and 2 ms between preamble
packets. With that sequence, zero and -0.1 N bounded runs each completed 168
acknowledged samples and normal zero/shutdown (P-0030). Sequence wraps were also
observed in the app's continuing capture. The bit's wider semantics remain open.

SIGINT/SIGTERM only set an atomic flag. The worker notices it and performs I/O
cleanup outside signal context. The GUI handles window-close requests and joins
the worker before exiting. Software shutdown cannot cover SIGKILL or kernel stalls.

## History and the unresolved contact report

On 2026-10-16 the owner reported "the motors don't stop me moving when i get to the
surface of the block" (P-0033). The audit of revision bef1ccd (P-0060) explains this
without any hardware fault and the explanation is untested. The 2026-10-16 envelope
(0.5 N, 2 N/s slew, 0.02 Mass Lab scale, 250 ms sustained-command release) put commanded
force below uncompensated arm weight and static friction at the tip for the
calibration moves (P-0060), and a pen held in a hand masks small forces in Mass Lab: a saturating spring tripped the release
after about 0.6 s. Amplifier enable is also unproven (force-audit F6), and every force
run evaluated the Jacobian at unregistered angles. Passing numerical tests and
acknowledged packets establish neither delivered force nor a rigid contact. Only the
attended procedure in 14-force-calibration.md can test the explanation.
