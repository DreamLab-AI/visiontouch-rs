# Bounded device readout on the owner's unit

Evidence: P-0018–P-0022 in the provenance ledger. This is a partial,
observed readout sequence for the identified 1.0A. The later output lifecycle is
documented separately in [12-force-feedback.md](12-force-feedback.md).

## Host transport

The real vendor path claims ppdev, sets EPP mode (0x40), sets the same mode via
negotiation, and clears the four control bits. The negotiation returns success
without a peripheral handshake when the mode is already equal; it is not device
identification. Native ECP/EPP operations on this card overwrite the control
register with 0x04. A read-side measurement observed 0x00 before and 0x04 after a
failed native read. Both vendor read attempts fail with EAGAIN when nonblocking.

The successful alternative uses ppdev register ioctls to perform software EPP
transfers while preserving the caller's INIT bit. Claim the port, clear controls,
and implement the following cycle from the public Linux EPP handshake. Do not
call native EPP read/write, since those change INIT on this host.

1. Set data direction (output for writes, input for reads).
2. Set idle control: preserve INIT, clear address/data strobes; set bit 0 for
   writes, clear bit 0 for reads. These are register bits, including PC polarity.
3. For a write, place one byte on the data register.
4. Assert address strobe (bit 3) or data strobe (bit 1).
5. Wait for status bit 7 to clear. For reads, sample the data register now.
6. Clear the selected strobe and wait for status bit 7 to set.
7. After a transfer, clear bit 0 and return the data direction to output.

Each wait must have a deadline and a finite poll limit. On error, deassert
strobes, return to output direction, and release the port. A partial packet is
an error and must not be decoded as a sample. This is process-level cleanup,
not a hardware force watchdog. No guarantees about real-time servo operation.

## Observed bootstrap subset

After claim and control clear, the successful reference run:

| Operation | Address | Data | Observation |
|---|---:|---|---|
| Write | 0x17 | 0x01 | Initial reference setup; full bit meaning unknown |
| Read | 0x0e | one byte | Actual reply 0x01 |
| Read | 0x00 | 16 bytes | Actual changing device packet |

The reference selects address zero twice before its first packet; retain that
sequence in the initial Rust readout. Later repeated acquisitions select it once.
The Rust tool implements only this prefix and packet acquisition. It does not
replay the later setup value 0x02 or any eight-byte output packet observed during
reference initialization/frame/shutdown. Their force/control semantics have not
been established. The setup write above is an active device operation, not a
passive host query. A successful response is a communication check, not a unique
model identification.

## Packet decoding

A successful reference run produced packet
`e4fb4c049a188f0385ff22ffe21f020c`. The SDK's corresponding encoder query returned
`[6298, -1052, 1100, 911, -123, -222]` with no API error.

The first twelve bytes are six little-endian signed 16-bit words. To match SDK
axis order, select word indexes `[2, 0, 1, 3, 4, 5]`. Later button observations
are described below; the force specification documents the subsequently observed
output acknowledgement. Other tail semantics remain unknown. Do not infer
calibration or force safety from this one packet. Exact encoder resolutions and calibration are still unverified.

## Scope and validation

The reference shim performs actual signal handshakes; it does not inject success
or substitute payloads. Its completed high-level transfers are captured by the
Rust recorder and underlying register ioctls by simultaneous strace. Offline
synthetic experiments are separate and not evidence for these device facts.

The initial SDK encoder experiment used the wrong query type and attempted
multiple frames without scheduler ticks; those API errors are not device errors.
The corrected single-frame long-integer query completed without errors.

Rust implementation must be independent of vendor libraries, use this observed
prefix, reject unexpected reply/short transfers, preserve bounded waits, and
release its claim. Validate signed word order with the observed SDK fixture and
exercise successful/failed handshake cleanup without hardware before a live run.

## Stylus button (P-0026)

The owner's three requested press/release cycles in the live Rust capture produce
three low intervals of bit 1 (mask 0x02) in zero-based byte 13: 38.210–38.625 s,
39.049–39.505 s, and 39.910–40.355 s. Byte 13 is 0x1f released and 0x1d pressed.
Decode this unit's button as `(packet[13] & 0x02) == 0`. Other tail bits remain
unspecified. This is physical observation, not an SDK button equivalence claim:
an offline single-bit replay sweep returned zero for all SDK button queries and
was inconclusive. A missing/stale stream must release application grabs.
