# Device SDK and force feedback

Evidence: P-0028 (black-box observation), P-0029 (application design).

## Layer boundaries

`hal` owns bounded software EPP transactions and the observed motor-session
lifecycle. `haptics-device` is the reusable Rust SDK: six-channel counter
accumulation, reference-model kinematics, force transforms, clamps, slew limits,
and stale-command rejection. The app owns bodies, gravity, contacts and grabs.
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

## Limits and stale data

Initial force-vector limit 0.5 N, slew limit 2 N/s, per-channel offset limit 96
counts. Requests older than 25 ms or non-finite requests produce immediate zero;
no stale force is held. Explicitly arm/disarm; input failure ends the session.
A UI pause/disconnect sends zero. These limits deliberately scale/cap simulated
weights; a simulated 10 kg block must not command its full weight at the pen.
The force loop must run independently of rendering. An OS stall or process kill
cannot be covered by a software watchdog; hardware timeout behavior remains to
be established.

## Simulation

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
shows the proxy position; the input trail shows the measured target. Scene force
is scaled by 0.02 before the device-level clamp. No direct wrist torque is sent.

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

## Unresolved physical contact resistance

On 2026-10-16 the owner reported: “the motors don't stop me moving when i get to
the surface of the block” (P-0033). The current app therefore has not demonstrated
the expected physical surface resistance. Passing numerical force tests and
acknowledged packets establish neither delivered force nor a rigid contact.
The 2% scene-force scale and 0.5 N cap may contribute; the cause is unconfirmed.
Next investigation should correlate contact reaction, guarded force, motor
commands and measured/observed physical response, checking calibration and axis
mapping. Preserve current limits until evidence supports any change.
