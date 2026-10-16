# Protocol specification

Implemented specifications:

- [Host capabilities](00-host-capabilities.md): query-only ppdev access.
- [Call capture](01-call-capture.md): recorder format and limitations.
- [Device readout](10-device-readout.md): observed bootstrap, software EPP
  compatibility fix, and raw encoder packet decoding on the actual unit.

- [Relative pointer](11-relative-pointer.md): software mapping of encoder motion
  into the desktop scene, including owner-confirmed axis orientation/reference.

- [Device SDK and force feedback](12-force-feedback.md): observed output lifecycle,
  reference kinematics, bounded force requests and simulation coupling.

Future phases, each citing `PROVENANCE.md` row IDs:

- `20-mode-negotiation.md`
- `30-amplifier-enable.md`
- `40-servo-cycle.md`
- `50-homing-calibration.md`
- `60-shutdown.md`

Written in our own words: prose, tables, timing diagrams, pseudocode. No vendor text.
