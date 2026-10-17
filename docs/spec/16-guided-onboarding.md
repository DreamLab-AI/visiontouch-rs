# Guided calibration onboarding

P-0066. The default Studio view presents one physical task at a time, with a
large scene and an always visible output state and Stop control. The existing
bench remains available for measurements, effects and detailed diagnostics.

The guide starts with input-only connection and observation, then explains the
physical reset pose with a diagram. Saving that reference and capturing a return
are separate screens. First capture without deliberately moving; then move away
and reproduce the physical pose independently of the display. Only acknowledged
worker captures count. Failed captures never advance the guide. Re-registration
invalidates guide progress and reconnecting starts over.

Raw liveness remains the existing bounded two-second test. The guide adds the
stricter prerequisite of a registered, checked reference before offering it.
Selecting a test prepares it; a fresh Space hold starts it. Releasing, Stop or
navigation cancels pending activation. Navigation never arms output. An aborted
attempt is described as stopped with its actual reason, not a broken motor.
Observations attach to that attempt, with unknown distinct from no. A stopped
attempt cannot unlock the next channel. After motor checks, the bench provides
the existing gravity, ruler, sign and measured-force protocol with its existing
worker prerequisites. The guide never claims complete force calibration.

No gauge is needed for the input-only and liveness onboarding. It becomes
relevant in the measurement phase. A screen marker is a model reference, not an
independent measurement of physical repeatability. The guide displays this
limitation during the return check. Reference illustration is our own schematic,
not a vendor image or a verified reconstruction.

This changes presentation and adds UI restrictions only. Controller interlocks,
SDK limits, channel plans, scoring and saved evidence remain authoritative.
