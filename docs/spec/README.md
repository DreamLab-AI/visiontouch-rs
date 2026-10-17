# Protocol specification

Implemented specifications:

- [Host capabilities](00-host-capabilities.md): query-only ppdev access.
- [Call capture](01-call-capture.md): recorder format and limitations.
- [Device readout](10-device-readout.md): observed bootstrap, software EPP
  compatibility fix, and raw encoder packet decoding on the actual unit.

- [Relative pointer](11-relative-pointer.md): software mapping of encoder motion
  into the desktop scene, including owner-confirmed axis orientation/reference.

- [Device SDK and force feedback](12-force-feedback.md): observed output lifecycle,
  reference kinematics, force envelope, uniform DAC scaling, channel demand,
  registration precondition and Mass Lab servo-rate coupling.

- [Registration, coverage display and effects](13-calibration.md): reset-pose
  registration, display-only coverage cloud and canonical effects; lists removed
  workflows.

- [Force calibration and verification](14-force-calibration.md): SDK envelope,
  registration checks, staged verification steps, unlock ladder, evidence schema 2
  and the owner runbook. Not yet run on the unit.

Future phases, each citing `PROVENANCE.md` row IDs:

- `20-mode-negotiation.md`
- `30-amplifier-enable.md`
- `40-servo-cycle.md`
- `60-shutdown.md`

Written in our own words: prose, tables, timing diagrams, pseudocode. No vendor text.
