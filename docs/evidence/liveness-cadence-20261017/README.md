# Attended liveness attempts, 17 October 2026

Owner reports a click then failure on each channel. These are unmodified local
attempt records from revision 3875bfd, copied after output stopped. SHA-256 values
cover each CSV and metadata file. No vendor material is included.

All three attempts stopped on the software speed interlock after 31.95–32.96 ms,
not a transport acknowledgement failure. Each channel reached an 80-count demand.
The later sample intervals are about 0.28 ms; the worker had no connected-loop
pacing despite the protocol's nominal 2.4 ms cadence. The four-tick velocity
filter therefore covered about 1 ms, not about 10 ms. Final filtered speeds exceed
50 mm/s despite start-to-end model displacement of only 0.039, 0.154 and 0.105 mm.
The short records do not establish actuator failure or delivered force. None
completed the two-second liveness programme. No pass is inferred from the click.

The regression replay can show the observed stop and the effect of sampling
these same observations at the intended cadence. It cannot predict how the
physical arm will behave during a new, longer pulse sequence.
