# Gradual channel diagnostic

P-0068 defines a separate diagnostic for the owner-requested progressive motor
test. It does not replace or pass the periodic liveness test. A connected,
registered arm is required so recorded model displacement has a current reference.
The operator supports the pen clear of obstructions, selects channel 0, 1 or 2
and positive or negative drive, then releases and freshly holds the local control.
Channel number and drive sign do not imply a Cartesian direction.

The fixed waveform ramps from zero to 40 DAC counts over two seconds, ramps
back to zero over one second, then remains zero for half a second. There is no
peak dwell and no configurable amplitude. It uses the existing raw-channel
96-count ceiling, 50 model mm/s speed stop, 15 model mm excursion stop and
four-second time bound. Hold release, sample gap, heartbeat, transport and other
existing interlocks remain in force. A stop immediately zeros output and disarms;
the ramp-down never delays a stop.

The local API may prepare `channel_ramp` with `channel` and `sign` (`pos` or
`neg`), but cannot supply the hold. Bench controls expose the same request.
Record waveform parameters, encoder samples, applied DAC counts, registration,
stop reason and operator felt/heard observations. Do not perform periodic lock-in
scoring on a ramp or use completion or observations to unlock force steps.
This measures response to a command, not delivered force, mechanical clearance,
or volume reachability. Actual slow volume motion still requires attended
registration, direction/scale evidence and an observed route.
