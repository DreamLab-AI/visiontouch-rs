# Overload archaeology and calibration baseline — 2026-10-16

The owner recalls a release of resistance at excessive load and requests that
we identify all contributing conditions before interpreting collision tests.
Opposing pen/object motion is a test scenario, not proof of any one trip reason.

## Evidence boundaries

P-0036 is **DECOMPILE** evidence, admitted by D-0001. It is excluded from the
paper. Pseudocode and complete REA Evidence remain ignored; the following are
own-word static findings, not a copy of reference implementation or a physical
test of the owner's unit. Peak force ratings are not established trip thresholds.

The Windows PDD 5.1.9 user library exposes separate controls for command checks,
command saturation, rapid force changes, velocity and output enable. Targeted
analysis finds both per-axis checks and a combined three-axis command budget.
The combined budget is instantaneous across axes, not evidence of accumulated
load over time. The threshold policy can choose clipping or returning an error.
A function whose public name suggests zeroing saturation actually changes
threshold policy; its name alone does not prove that it zeros motor output.

The motor-warming path is separate and carries history. Static Windows analysis
finds a different return path for warming versus excessive command: the higher
I/O force pipeline explicitly disables on the former, while the latter can
propagate to a higher SDK/application layer. The exact error-to-release chain
must therefore be traced through the scheduler/demo before claiming how an
owner-experienced release occurs. It is not yet established as an external load
sensor or an amplifier-current measurement. Additional hardware protections and
their packet/status representation remain unresolved.

Both Windows EPP drivers and the user library are present. Bundled documentation
supports Premium 1.0 with a parallel interface, and model metadata identifies a
compatible family candidate; exact matching of this unit's 1.0A revision remains
unproved. The newer driver alone cannot establish every historical demo's policy.

## Acquired comparison targets

- [Official PDD 4.2.49 ZIP](https://s3.amazonaws.com/dl.3dsystems.com/binaries/Sensable/PDD4_2_49/PDD-v4.2.49.zip),
  SHA-256 `d3f5377d4dad1ebd723447b1d90e50556a13bf357dfef9b3cac14b3468bd8534`.
  Extracted Dice SHA-256 `5328610f76f35f2e5a17911c965a453c24c517551e2035bf161e1f671ea5a05e`.
  Its release notes mention a maximum-force error fix in that demo.
- [Official Windows SDK Academic 3.1](https://s3.amazonaws.com/dl.3dsystems.com/binaries/Sensable/OpenHaptics/OpenHaptics_Academic_3.1.exe),
  SHA-256 `492990f2a7ef6bfc19dc16c635f4e3c6d3e9bb27f44eacae10dcc4003b9a1580`.
  Selected prebuilt calibration/error/load examples are admitted by D-0002.
- [Historical academic reference](https://www.ida.liu.se/conferences/ssomc99/papers/Jansson.pdf)
  points to a Greatest Hits download formerly hosted by the vendor. The complete
  pack has not been recovered. Do not identify the requested balloon/water scenes
  as members without further evidence.

## Recovered demo lifecycle findings

P-0041 is **DECOMPILE** evidence under D-0002 and is excluded from the paper.
Targeted REA/Ghidra analysis of the recovered 2005 Dice executable finds that
its local force-send wrapper requests output disable after any nonzero send
result. This supplies a concrete application-level release action beyond the
low-level library's error return. Its higher-level rearm trigger remains
unresolved; the existence of an enable helper does not establish that trigger.

The newer calibration example distinguishes ready, update-required and
manual-input-required states. It updates the encoder reference and displays
position; it requests no force output. It therefore does not establish a safe
powered move to zero or an application working volume.

The newer error-handling example clears contact state for its classified force
errors. On a motor-warming error it explicitly disables force output, then
requests automatic recovery when no error is pending. The separate warming
prevention example instead attenuates demand according to the hottest reported
motor state, using distinct hot/cool transitions and gradual recovery. Both
examples perform scheduler/device cleanup after scheduler failure. These
different policies reinforce that contact reset, command rejection, output
disable and rearming each need an explicit owner in our application.

All four executable hashes, procedure addresses, complete Evidence IDs and
limitations are preserved in the ignored demo-research analysis index. No
vendor force model or demonstration thresholds were copied. No executable was
run and no new hardware test was performed; driver failure behaviour, exact
revision compatibility and the physical unit's limits remain unresolved.

## Independent implementation

Our calibration/effects code implements original, simple numerical models and
retains the existing 0.5 model N, 2 N/s and 96-count ceilings. A configurable lower
limit and optional 250 ms command-saturation release are our own design choices;
they are not reference-driver trip values. Release latches until explicit new
arming. Every application shares a single SDK unit derating gain. Its current
value is 1.0 times the existing conservative envelope; final adjustment awaits
logged observations rather than adopting historical product ratings.

One input-only 10-second baseline (P-0037) received 4175 samples. Median cycle
2.378 ms, worst 2.874 ms, no 25 ms deadline misses; one encoder varied one count
and the other five were constant. This records the unlabelled current pose,
without motor output, force measurement or a physical zero assignment. The unit
owner was then advised to power down while offline investigation continues;
no subsequent hardware experiment has been run.

## Next powered session

Use Calibration Studio's motor-off directions to record the bottom pose and all
six comfortable working-volume bounds. Enter desired working-coordinate limits;
zero is inverse-mapped into the observed model. Reconnection requires fresh
capture. Export records as evidence, never silently restore a physical origin.

Start with short held trials, logging raw/unwrapped encoders, timestamp intervals,
model/mapped position, unclamped demand, guarded commands, exact DAC offsets,
acknowledgements, effect state and stop reason. A requested force is not measured
delivered force. Record operator sound labels and link fixed-position/gain WAV
recordings. Audio comparison can expose repeatable spectral changes; no acoustic
overload classifier is validated yet.

For Pixel 9 Pro measurements use **Object temperature**, the same identified
motor-housing spot, the matching material setting and about 5 cm sensor distance.
[Google's object-temperature guide](https://support.google.com/pixelphone/answer/14103759?hl=en-uk)
describes the sensor and dependence on material, target size and distance.
Save surface readings with timestamps, target/material/distance and sensor notes.
Record ambient temperature separately if available; an IR surface reading is not
an air-temperature or winding-temperature measurement. No shutdown temperature
or duty-cycle limit has been established for this aged unit.
