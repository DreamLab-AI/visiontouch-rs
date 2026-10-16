# Decompilation log

Initially empty. See the dated entries below for any subsequent targeted work.

If that changes, each entry must record: date, file name + SHA-256 of the
binary examined, the specific interoperability question being answered, why
observation could not answer it, who performed it (spec team member), and the
resulting PROVENANCE.md row IDs. The output of the decompiler must not be
committed to this repository.

## D-0001 — 2026-10-16 — bounded force-release investigation

Owner explicitly requested examination of the reference stack's critical overload
release path, including the Windows parallel driver for the identified 1.0A.

Questions: does a peak or cumulative motor-command check zero, clamp, or disable
output; how is that condition surfaced; is it based on commanded output or a
measured physical load; which parts belong to user-space versus the EPP kernel
transport? Do not broaden this into copying force models or application code.

Files admitted for this question:

| Artefact | SHA-256 |
|---|---|
| Linux JUJU I/O library 4.3 | `35096489264a62e90f6ce87cb5fed69b68ffcb80e66f5f66f9be1cb4b26ca10b` |
| Linux SDK device library 3.0.0, only if needed to trace error propagation | `af1fbf2790d2f16f46033a025cb81a9d7846b3aaeef08364a055a1cf9befac71` |
| Windows PDD 5.1.9 user I/O DLL | `99bd8b09150a24fb5a86fd26e187df0ec80378c6f1bc7ce82c99266401663fac` |
| Windows x86 EPP kernel driver | `e55cc467cf1b9fd9bd7182c2a0e00e9666382b4f9812599683608e860ca4dafb` |
| Windows x64 EPP kernel driver | `8a42fd98cde2fcab61809cc0bf48f65a25f907ee3c23dca05906e1273b6da05c` |

Why observation is insufficient: packaged API documentation distinguishes force
clamping, excessive-command errors, velocity and force-change checks, but does
not establish the low-level peak/cumulative release state transitions for the
parallel unit. Prior live evidence is bounded/low-force and deliberately does
not trip a physical overload. New offline synthetic scheduler probes stall at
the first frame and therefore cannot establish the release action. Windows
kernel/runtime execution is unavailable on this Linux host. Deliberately driving
the actual unit to an unknown hardware limit is not needed to answer a static
interoperability question. First inspect symbols, then only the named check,
policy and zero/disable paths and their direct callers as necessary.

Analyst: coding agent acting in the specification role. Tools: REA with Ghidra;
all pseudocode, instruction listings, symbol maps and evidence bundles stay in
ignored `vendor-original/`. Static findings are labelled as such, never as live
physical measurements. Resulting provenance: P-0036. Bounded analysis completed;
private Linux and Windows reports retain exact artifact hashes and Evidence IDs.
The findings distinguish command clipping/errors, thermal-history checks and
explicit disable paths; no hardware overload threshold or safe kernel watchdog
was established. Any findings
derived from instruction analysis are DECOMPILE and excluded from the paper.

## D-0002 — 2026-10-16 — Windows demo safety and calibration lifecycle

Owner explicitly requests finding and decompiling the old Windows demo pack
using REA to understand safety boundaries, rather than importing its code.
Official packages were found and hashed; full inventory stays ignored in
`vendor-original/demo-research/`. The exact Greatest Hits collection remains
unlocated; the 2005 Dice binary is a recovered historical vendor demo.

Questions: which calibration state permits output; what happens to output and
contact/dynamics state after a force error; how does a motor-load example handle
warnings/recovery; does the old Dice app convert a peak-command error into
zero/disable? Limit analysis to these lifecycle/safety paths and their necessary
direct callees. Do not recover or reproduce demo force-model implementation.

| Artefact | SHA-256 |
|---|---|
| 2005 Dice Windows executable | `5328610f76f35f2e5a17911c965a453c24c517551e2035bf161e1f671ea5a05e` |
| Its matching 2005 user I/O DLL, only if needed for error propagation | `71197cb992f5ba3915a0a195278d42709c4ce376c0e2735227c7de897133e2fc` |
| Windows SDK 3.1 calibration example | `76f6d024fc8830d95a8199a3549a035a4bb9d925704e965587224f6ca3e70950` |
| Windows SDK 3.1 error-handling example | `0205bbf25185e0e58b08e0cbb243a38085786fafd1cb0f215fe4cd9e64741182` |
| Windows SDK 3.1 motor-warming example | `04b5b756581915479d9961cae625277e5ec0f1ed20216b856209f652ba098fce` |

Why observation is insufficient: the vendor documents and PE imports identify
the examples and interfaces but not the complete error-to-release/recovery
sequence. These are Windows-native binaries; no compatible runtime/device
environment has been established here. Running a driver on the live powered
unit to provoke a limit is unnecessary. Source implementation is deliberately
not used as Rust input; targeted read-only REA/Ghidra analysis answers the named
interoperability questions through a separately recorded specification.

Analyst: coding agent acting in specification role. Tool: REA/Ghidra, no target
execution. Keep all code/pseudocode/Evidence under ignored `vendor-original/`.
Own-word conclusions from this analysis are DECOMPILE, excluded from the paper,
and must not be represented as verified physical behaviour. Provenance: P-0041.

Outcome: bounded static analysis completed with REA 6.5.0 and Ghidra 12.1.2 for
the four admitted Windows x86 executables. The matching historical I/O DLL was
not instruction-analyzed. Dice's force-send wrapper explicitly requests output
disable after any nonzero send result. Its higher-level rearm trigger remains
unresolved. The newer calibration example queries/updates calibration and
displays position; it does not request force output. The error-handling example
resets contact for its classified force errors, disables on motor warming, and
requests automatic recovery when its foreground error handoff clears. The
motor-warming prevention example instead attenuates commands using reported
temperature state and cleans up on scheduler failure. These are different
application policies, not proof of a universal driver trip or physical sensor.

Complete small-function Evidence, target identities and scope limits are saved
under ignored `vendor-original/demo-research/analysis/`, indexed by
`D0002-evidence-index.json` and summarized in own words in `D0002-FINDINGS.md`.
Two larger Dice servo queries exceeded the MCP transport budget; their retained
session ended during the owner's pause. No conclusion depends on unretrieved
servo pseudocode. Native sessions were closed. No vendor executable or hardware
was run, and no force-model implementation was promoted into the Rust crates.
