# Calibration worker cadence

P-0065 corrects an implementation omission against the nominal 2.4 ms cadence in
14-force-calibration.md. Pace the connected real-device worker to at least 2.4 ms
per iteration, including I/O, controller and snapshot work. Sleep only the unused
part of that interval. When work overruns, do not accumulate a schedule debt or
execute catch-up reads; begin a new measured iteration. The next read must still
report its actual monotonic interval to the existing 25 ms sample-gap interlock.
Do not replace measured time with nominal time or change any force, raw-channel,
velocity, excursion or hold threshold. The simulator retains its own clock.

The interlock velocity is now displacement over a fixed 9.6 ms window rather
than a four-sample mean, so it covers the same span at any cadence; pacing
remains required so the window holds about four samples rather than thirty. This does
not establish hard-real-time scheduling or guarantee that liveness passes.
