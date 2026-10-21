# Registered channel-zero liveness, 2026-10-17

Preserved unchanged from owner-host session
`20261017T114416Z-c106a223-2d38-4c45-9d67-06bd38f72b8e`.
This is historical evidence, not a new powered trial. The JSON identifies the
registration, software revision, limits and original speed stop. The CSV hash
matches the original JSON manifest.

The 19 recorded samples span 18.250 to 63.279 ms after attempt start. Applied
channel-zero drive reaches 80 counts. The registered model displacement between
first and last recorded positions is about 1.2103 mm; it is not a ruler measurement.
A replay of these rounded CSV positions through the corrected 9.6 ms velocity
window (P-0069) first crosses 50 mm/s on the final sample, at 53.6670 mm/s. The
previous truncated-window implementation crosses at 60.770 ms, one sample earlier.
The controller regression requires the corrected replay to stop and return zero
channel demand. It does not infer hardware overload, delivered force or that a
new motor trial will complete. This evidence motivates the gradual diagnostic.

SHA-256:

- `0001-liveness.csv`: `33bb220247fe9c93987c46554e255fc37706cdad8d6f6eb22a0b92700c469556`
- `0001-liveness.json`: `ffb1ad7b6a7a5d56834427f029da4d32a56364d2db96910316ccb4dda1f48577`
