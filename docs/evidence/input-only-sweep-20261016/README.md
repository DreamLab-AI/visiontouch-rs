# Owner-directed input-only workspace sweep

The owner requested continuous input capture and then explicitly said stop.
The reader collected 31,271 samples in 74.109 seconds, closed the port and flushed
successfully. It never armed a force session. The unit was subsequently powered
off by the owner. `input.csv.gz` preserves the original CSV losslessly; its
uncompressed digest is recorded in `analysis.json`.

The owner's statement after stopping was that the pen was left on the desk at
bottom dead centre "ish". The final two-second median is a provisional reference
only. Do not label it an exact mechanical zero or infer that it is the full lower
workspace limit. The final-tail movement is under 0.03 model mm on each axis.

All six raw and accumulated encoder channels, reference-model Cartesian position,
button, packet bytes, interval and read timing are retained. Model dimensions
are not independent physical measurements. No interval in this short run exceeds
25 ms; that is not hard-real-time qualification. Analysis figures can be
reproduced directly from the archived CSV. The compact Three.js example retains
one point per 2 mm cell and every tenth pose sample, plus the last pose.
