# Historical calibration and force-verification practice for the Premium 1.0

Research note, 2026-10-17. All sources are PUBLIC. Product names are used
referentially only (see `docs/clean-room/POLICY.md`). Everything below is in
our own words; the only quoted fragments are very short referential ones.

Conventions. `[Sn]` cites the bibliography in section 7. `DERIVED` means our
own arithmetic from cited numbers. `NOT FOUND` means a public value could not
be located and must not be assumed. `REPO` means a value taken from
`crates/device/src/lib.rs`, which is a reference model, not a published figure.

Evidence quality warning. The two best primary sources are about the larger
model 1.5, not the 1.0. The 2002 Presence paper [S2] and its Berkeley memo
[S3] give a full kinematic and dynamic model, but only with model 1.5
numbers. No public source located here gives the Premium 1.0 link lengths,
encoder counts per revolution, capstan ratios, motor torque constants, DAC
resolution or amplifier gain. Those gaps are listed in section 8.

## 1. The canonical calibration procedure (hardware reset of encoders)

### 1.1 Why a reset is needed

The three main-arm joints carry incremental optical encoders on the motor
shafts [S6]. An incremental encoder reports only change since power-up, so
the host cannot know absolute joint angles until the arm is placed in a known
pose and the counters are zeroed [S4]. The vendor's programming guide groups
devices into three calibration styles [S5]:

| Style | What the user does | Persistence | Applies to |
|---|---|---|---|
| Hardware reset of encoders | Manually place the arm in the reset pose, then run the calibration routine once | Until power is removed or another reset is done | All Premium models [S5] |
| Inkwell | Seat the stylus in a fixture that fixes position and orientation | Across sessions | The later desktop device with an inkwell, not Premium [S5] |
| Auto | Device refines calibration as it moves; may be partial per axis | Across sessions | Later devices [S5] |

Premium units have no inkwell, so the reset pose is reproduced by hand and is
described as "unpredictable" in one registration study [S8]. This matches the
Premium 1.0 on the owner's bench: reset is a manual, operator-skill step.

### 1.2 The reset pose, described physically

Three independent public descriptions agree:

1. The vendor programming guide says the reset position is the one in which
   all links are orthogonal [S5].
2. A Lappeenranta thesis and dissertation describe it as: first link
   horizontal and perpendicular to the front face of the base; second link
   vertical and pointing down from the first. The four rotary joints of the
   parallel linkage then form a rectangle, so lines between joints 1 and 3
   and joints 2 and 4 are horizontal, and lines between joints 1 and 2 and
   joints 3 and 4 are vertical [S4a, S4b].
3. A university laboratory guide for a Premium 1.0 gives the same zero
   configuration: upper arm horizontal, lower arm vertical, tip above the
   x axis; encoder counts and joint angles increase in the direction drawn on
   its schematic [S6].

The Berkeley memo uses the same pose as the kinematic zero configuration,
with axes w2 = w3 = [-1 0 0] and gravity along negative y [S3, S2]. In that
pose the model 1.5 tip sits straight ahead of the base, level with the
shoulder axis offset by the link geometry [S2, section 2.1].

Practical wording for this project (own words): hold the stylus end so that
the near arm is level and square to the front of the base, and the far arm
hangs plumb, with nothing touching the workspace limits. The vendor
diagnostic then performs the zero; we replicate the effect in our own code.

### 1.3 Joint angles assumed at the reset pose

- Model 1.5 (Berkeley/Presence convention): the zero configuration is also
  theta = (0, 0, 0). Joint 1 swings the whole arm about the vertical axis,
  joint 2 raises the near arm about a horizontal axis, joint 3 moves the far
  arm. All three are zero at reset [S2, S3].
- Premium 1.0 (laboratory guide): joint angles are zero at the same
  configuration, and positive counts are the drawn positive directions [S6].
- This repo's reference model: counts (0, 0, 0) give position (0, 0, 0). We
  checked `position_from_counts`: r = L(cos 0 + sin 0) = L, y = L(0 - 1 + 1)
  = 0, z = L - L = 0. So REPO treats reset as the origin, consistent with the
  published zero configuration. DERIVED.
- Sign conventions differ between sources and between host libraries. REPO
  negates counts 1 and 2 and swaps their order relative to the angles
  (angles = a, -c2, -c1). No public source defines the 1.0's per-axis count
  direction; it must be established by the sign check in section 4.

### 1.4 How the host zeroes the encoders

Public descriptions of the mechanism are consistent but thin:

- The user is prompted to place the arm, and the calibration call is made
  once [S5]. The vendor's test utility can do the same [S5].
- No public source describes the wire-level action. The Linux driver for the
  later PCI interface exposes raw register read and write only, with no reset
  or calibration commands; its meaning lives in the vendor library [S7].
  Conclusion: the zero is a host-side offset or counter clear, and its wire
  form for the parallel Premium must come from OBSERVE rows in this project,
  not from public documentation. NOT FOUND publicly.
- One 1.0 laboratory wrapper exposes a "zero encoders" call that is run
  rarely and interactively, which supports a host-side software offset [S6].

### 1.5 Consequences of a wrong reset pose

Because the counters are relative, every later angle inherits the reset error
as a constant offset on each joint:

1. Position error. Registration work reports total discrepancy of roughly
   10 to 60 mm and a few degrees at the stylus tip, which they attribute to
   both the hand-set reset and the nominal kinematic model [S8]. DERIVED
   rule of thumb for this unit: a 1 degree joint offset moves the tip about
   L times 0.01745 = 2.4 mm for L = 0.1397 m.
2. Force direction error. Cartesian force is mapped to torque through the
   Jacobian at the believed angles. A reset offset of delta rotates the
   commanded force direction by roughly delta; at 3 degrees a pure axial
   force picks up a sideways component of about 5 percent. DERIVED.
3. Gravity compensation is evaluated at the wrong angles, so a hands-off arm
   drifts or is pushed, worst at the workspace edges where the gravity torque
   changes fastest [S2, section 5.1].
4. Workspace and soft-limit checks become wrong: the arm can reach a
   mechanical stop while the model believes it has margin.
5. Anything that assumes the origin is the centre of the usable workspace
   (virtual walls, safe-box checks) is mis-placed.

Mitigation used in the literature: build a fixture that repeats the pose, or
calibrate kinematic parameters against a grid after reset [S8].

## 2. Published kinematic model and parameters

### 2.1 Model structure (model 1.5, applies in form to the 1.0)

The Berkeley memo and the Presence paper define the arm as three revolute
joints. Joint 1 is the base yaw. Joints 2 and 3 move a parallelogram linkage
in the plane the base joint rotates. The forward map in closed form [S2, eq. 2
area, S3]:

    x = sin(t1) (l1 cos t2 + l2 sin t3)
    y = l2 - l2 cos t3 + l1 sin t2
    z = -l1 + cos(t1) (l1 cos t2 + l2 sin t3)

with gravity along negative y. Joint 2 and joint 3 therefore act as shoulder
and elbow angles measured from the zero pose, and the tip is the sum of a
near-arm term (l1) and a far-arm term (l2). The reference repo model is the
special case l1 = l2 = L:

    r = L (cos b + sin c)
    x = -r sin a
    y = L (sin b - cos c + 1)
    z = r cos a - L

(`crates/device/src/lib.rs`, `position_from_counts`). Structure and zero
pose match the published one apart from sign conventions: REPO has x = -r sin
a where the memo has +sin(t1), i.e. the joint-1 count direction is reversed
relative to the memo's drawn direction. That is a convention choice, to be
fixed by the sign check, not an error.

### 2.2 Numeric parameters

| Quantity | Published value | Model | Cite |
|---|---|---|---|
| Link length l1 | 0.215 m | 1.5 | [S2 appendix, S3] |
| Link length l2 | 0.170 m | 1.5 | [S2 appendix, S3] |
| Offset l3 | 0.0325 m | 1.5 | [S2 appendix] |
| Cable transmission ratios | 11.6:1 and 13.3:1 | 1.5 (assumed in memo) | [S3] |
| Assumed motor | Maxon RE-025-055035 | 1.5 (assumption in memo) | [S3] |
| Motor, Premium 1.0 unit | Maxon 118743 brushed DC, one per main joint | 1.0 | [S6] |
| Encoder, Premium 1.0 unit | HEDM-5500-B02 optical, on motor shaft | 1.0 | [S6] |
| Encoder counts per revolution (any model) | NOT FOUND (500 lines would give 2000 decoded; unverified) | | |
| Main-joint link lengths for the 1.0 | NOT FOUND | 1.0 | |
| Nominal workspace | 13 x 18 x 25 cm (5 x 7 x 10 in) | 1.0 | [S1] |
| Nominal workspace | 381 x 267 x 191 mm | 1.5 | [S9] |
| Position resolution | about 0.03 mm at workspace centre | stated for the family | [S2] |

Unit check: l1 = 0.215 m is 8.46 in. For comparison, REPO L = 0.1397 m is
exactly 5.5 in. DERIVED.

### 2.3 Comparison with the repo reference model

- Workspace scaling is the only public hint for the 1.0 arm length. The 1.0
  to 1.5 workspace linear ratio is 19.5/13 = 1.5 [S1]. DERIVED: 0.215 m
  divided by 1.5 is 0.143 m, and 0.170 m divided by 1.5 is 0.113 m. REPO's
  L = 0.1397 m (5.5 in) lies within about 2.5 percent of the first, which is
  consistent with a 5.5 in near arm. This is a plausibility check only; it
  does not validate L, and does not show that the two links are equal.
- A US patent describing a similar arm lists 0.143 m and 0.178 m with speed
  ratios 9.97 and 9.94 [S10]. It does not name the model; treat as a
  neighbouring order of magnitude only.
- Per-count angle constants in REPO: 0.00018075138 rad per count on joint 1
  and 0.00021136618 on joints 2 and 3. Reciprocals give 34 761 and 29 727
  counts per full joint revolution, or 0.0104 and 0.0121 degrees per count.
  DERIVED. At L = 0.1397 m these are 0.0253 mm and 0.0295 mm per count at the
  tip, which brackets the published 0.03 mm resolution figure [S2]. That is
  a good sign for the constants; it is not proof of them.
- Check against motor-side hardware: with an assumed 2000 decoded counts per
  motor turn (unverified), the implied cable ratios are 17.4 and 14.9. This is
  above the 11.6 and 13.3 of the 1.5 memo [S3] but within the plausible range
  for a different model. Both the encoder cpr and ratio are NOT FOUND
  publicly for the 1.0, so this is only a flag for an OWNER inspection row.
- Gimbal. Registration work reports potentiometers rather than encoders in
  the standard gimbal [S8]; the owner's unit has a three-encoder gimbal, so
  that statement does not apply and OWNER evidence governs. A patent mentions
  256 counts per revolution gimbal encoders [S10]; model unknown.

## 3. Force output facts

### 3.1 Datasheet values

The family datasheet columns are 1.0, 1.5, 1.5 high force, ... , 3.0 [S1].
The extracted text of the table does not give a clean per-column mapping, so
only the following are safe for the 1.0 unless an OWNER row confirms them:

| Quantity | Value | Applies to | Cite |
|---|---|---|---|
| Maximum exertable force at nominal (orthogonal) pose | 8.5 N (1.9 lbf) | 1.5; the 1.0 shares the entry in the family table | [S1, S9] |
| Continuous force, 24 h | 1.4 N (0.3 lbf) | same entry | [S1, S11] |
| Backdrive friction | 0.04 N (0.15 oz) | same entry | [S1, S11] |
| Closed-loop stiffness | 3.5 N/mm (20 lbf/in) | same entry | [S1, S9] |
| Position precision | 0.03 mm | family | [S2] |

Caveat: these are marketing maxima at the nominal pose with arms orthogonal.
They are the only published ceilings, so use them as upper bounds for this
unit, not as measured values.

### 3.2 Measured characteristics from the literature (model 1.5)

| Item | Result | Cite |
|---|---|---|
| Servo rate used in the study | 1 kHz | [S2] |
| Velocity granularity at 1 kHz | 0.03 mm / 1 ms = 30 mm/s | [S2] |
| Lag of the stock 100-sample velocity average | about 50 ms | [S2] |
| Structural resonance | about 125 Hz | [S2] |
| Max stable wall stiffness, stock amplifier | 2.0 N/mm | [S2] |
| Max stable wall stiffness, linear amplifier | 2.7 N/mm | [S2] |
| Max stable viscous damping | 0.0050 to 0.011 N/(mm/s) with new amps and velocity estimator | [S2] |
| Gravity compensation | Works well across the workspace using the dynamic model | [S2] |

Reading: the usable virtual-wall stiffness (2.0 N/mm) is 57 percent of the
3.5 N/mm datasheet figure. DERIVED. For this project a wall test above
about 1 N/mm adds risk with no test value.

### 3.3 Amplifier, motor and DAC

- Stock amplifier: pulse-width modulated, with an update-rate and temperature
  control safety logic built in [S2]. Its internal gain is NOT FOUND.
- A laboratory driver for a 1.0 maps motor current commands in the range
  -1.0 to 1.0 A and warns when a torque request exceeds what that current can
  provide [S6]. So +/-1.0 A is a public upper bound on per-joint motor
  current in that setup; the amplifier model is not named.
- DAC resolution. The task statement says 12-bit; no public source located
  here confirms it. NOT FOUND. REPO treats the word as 2047-centred with
  MAX_DAC_OFFSET = 96.
- Motor torque constants for the Maxon 118743: not retrieved here. NOT FOUND
  in this research pass; a Maxon catalogue datasheet is PUBLIC and is the
  next lookup.

### 3.4 What 96 DAC counts means

Using a 2047-centred 12-bit word:

    96 / 2047 = 4.69 percent of full-scale deflection (96 / 2048 = 4.69).

Two different readings, because the repo's own torque scaling is an inferred
constant (825 counts per N m on joint 1, 960 on joints 2 and 3):

1. Using REPO scaling at the ideal pose (Jacobian column magnitude about L):
   96 counts = 0.1164 N m on joint 1 and 0.1000 N m on joints 2 and 3. At a
   moment arm of 0.1397 m this is 0.83 N and 0.72 N. DERIVED. Against the
   8.5 N published peak that is 8.4 to 9.8 percent.
2. Linear scaling from the published peak. If 2047 counts were 8.5 N, then
   96 counts would be 0.40 N (4.7 percent). DERIVED. The two readings
   disagree by about a factor of two. The disagreement itself is a finding:
   it implies REPO's counts-per-newton-metre gives a full-scale force of
   about 2047 / 960 / 0.1397 = 15 N, well above the 8.5 N family maximum,
   which is plausible only if the real DAC range is not fully usable or the
   scaling is wrong. This must be measured (section 4), not assumed.

Practical values for REPO scaling (ideal pose, 960 counts per N m, L =
0.1397 m): 0.1 N is about 13 counts; 0.5 N is about 67 counts; 0.716 N is
96 counts. DERIVED.

### 3.5 Feel relative to published friction

Backdrive friction is 0.04 N [S1]. DERIVED:

- 0.1 N is 2.5 times friction. A hand resting lightly on the stylus should
  just detect a steady push; it may not overcome stiction on every joint.
- 0.5 N is 12.5 times friction and 36 percent of the 1.4 N continuous rating.
  It is an obvious, comfortable push, safe to hold indefinitely at the
  nominal pose.
- 8.5 N peak is 212 times friction.

## 4. Community force-verification protocol

What the literature actually did, and what was not found:

- Stiffness ramp against a virtual wall until marginal stability, wall
  normal along +y at y = 0, no active damping, at a 1 kHz servo rate [S2].
  This is the one clearly validated experimental protocol. Marginal stability
  was defined as small sustained oscillations that do not grow [S2].
- Constant downward force of 2 N for 10 minutes with the arm constrained,
  measuring motor casing temperature rise [S2]. A thermal verification, not a
  force-accuracy check.
- Gravity compensation validated by observing that gravity effects vanish
  across the workspace [S2].
- A direct commanded-versus-measured force comparison using a load cell or
  scale, hanging masses, and per-axis sign checks: NOT FOUND in the PHANToM
  literature. A search for it returned only generic load-cell practice.
  Section 4.1 is therefore our own protocol, built from the generic method of
  comparing against an external reference [S12] and the experiments above.
- Passivity rationale for the rate and stiffness limits: a sampled virtual
  wall is passive only when device damping exceeds roughly K T / 2 (T the
  sample period), so the achievable stiffness at 1 kHz is bounded by physical
  damping [S13, S14]. The exact constant should be checked against the
  original paper; secondary sources disagree by a factor of two [S13].
  At T = 1 ms the bound on K is 2b/T.

### 4.1 Protocol adapted to this unit (our own design, DERIVED)

Preconditions: encoders reset per section 1; e-stop or host deadman armed;
ForceGuard limits in place; nothing in the workspace; log counts, commanded
offsets and a timestamp for every step; keep the arm near the reset pose
(orthogonal links) so the Jacobian is simplest.

Equipment: a digital hook or push-pull gauge (0.01 N resolution or better,
range 0 to 5 N) or a kitchen scale of 1 g resolution, a stiff rigid post, 50
g, 100 g and 200 g masses on a light cord or hook, a ruler.

Step 0. Free-motion baseline (motors off). Move the stylus by hand through
the central workspace. Record backdrive effort with the gauge pulled slowly.
Acceptance: about 0.04 N order of magnitude [S1]; more than 0.2 N means a
binding cable or a mis-set joint, stop and inspect.

Step 1. Sign check per axis (no load cell needed). With the stylus held
loosely, command +5 DAC counts on one joint at a time, then each Cartesian
axis through the Jacobian at 0.1 N. Confirm the arm pushes in the intended
direction and that encoder counts move the way the model predicts.
Acceptance: all three axes agree in sign; any mismatch is recorded as an
OBSERVE/BLACKBOX row and fixed in the model, never by ad hoc flips.

Step 2. Static gauge test along each axis. Fix the gauge between the stylus
tip and a post along +x, then -x, +y, -y, +z, -z. Step the commanded force
0.1, 0.25, 0.5, 1.0, 2.0 N, dwell 2 s per step, ramp up and down, and
record the gauge reading. Never exceed 2 N in this step until the preceding
steps pass; the first unit session should stop at 0.5 N.
Acceptance: measured force within 20 percent of commanded for forces at or
above 0.5 N; linear fit slope between 0.8 and 1.2; hysteresis below 0.05 N;
offset (zero-command reading) below 0.04 N. Report the slope per axis; this
gives the real counts-per-newton constant and resolves the factor-of-two
disagreement in section 3.4. Treat a single-point match as inconclusive.

Step 3. Hanging mass test (gravity check, motors enabled). Hang 50 g, then
100 g, then 200 g from the stylus tip and command an upward force equal to
the weight (0.49 N, 0.98 N, 1.96 N). Acceptance: the stylus holds within 2
mm for 10 s. Use only the 50 g and 100 g masses on the first run; stop
immediately if the arm walks. This is the cleanest absolute reference because the weight is
known to well under 1 percent.

Step 4. Gravity compensation check. With the stylus released and the
compensation torque enabled but no other force, the arm should hover at
rest at five poses: reset pose, left, right, high, low. Acceptance: drift
below 5 mm in 10 s at each pose. Use a conservative torque cap; the model
needs link masses, which are NOT FOUND for the 1.0, so expect to fit them.
The 1.5 values in [S3] are a starting shape only, not usable numbers.

Step 5. Stiffness ramp against a virtual wall. Implement a one-sided wall at
y = 0 (normal +y), no damping, hold the stylus loosely, penetrate by about 5
mm. Raise stiffness from 0.1 N/mm in steps of 0.1 N/mm, 3 s per step, until
sustained oscillation just appears [S2]. Back off 30 percent. Acceptance:
marginal stiffness is a measured fact for the log; plausible range 1 to 2.7
N/mm at 1 kHz [S2]. Hold the wall force below the 2 N test cap. If the
transport cannot sustain 1 kHz, record the actual rate and expect a
proportionally lower limit, since K scales with 1/T [S13, S14].

Step 6. Thermal check. Hold 1 N in the downward direction for 10 minutes
with the arm braced, and measure the casing temperature rise with a probe.
For reference, 2 N for 10 minutes gave +34.5 degrees C with the stock PWM
amplifier and +14.5 with a linear one [S2]; at half the force expect about
a quarter of the heat (I squared R), so a rise much above 10 degrees C
for 1 N is a warning. DERIVED.

Magnitude ladder for the first sessions: 0.1 N, then 0.25 N, 0.5 N, then
1 N; do not pass 2 N without a written pass of steps 0 to 3. The current
ceiling of 96 DAC counts (about 0.4 to 0.8 N, section 3.4) is a sound
first-session limit.

## 5. Safety practice

- Motor heating. Stock PWM drive heat is I squared R with duty cycle, so
  25 percent duty gives four times the heat of a linear drive at the same
  mean force [S2]. A reasonable finger force held for a long time with little
  motion caused a software fault from overheating in the stock rig [S2].
  Continuous rating at the nominal pose is 1.4 N for 24 hours [S1];
  peak 8.5 N is for short bursts only. Duty-cycle figures beyond these are
  NOT FOUND.
- Thermal protection. The stock amplifier box carries update-rate and
  temperature-control safety logic; replacing the box bypasses both, so a
  replacement driver must reimplement them [S2]. This is the strongest public
  statement of how protection is arranged. The trip temperature and time
  constant are NOT FOUND.
- Emergency stop. A laboratory 1.0 setup enables motor control only when the
  e-stop is pulled up, and advises testing code with it down (no forces) first
  [S6].
- Watchdog. The Linux driver for the later interface has no hardware
  watchdog or timeout logic; stopping the device is done by clearing the
  run bit or closing the file [S7]. The vendor box's rate-check is the only
  public reference to a watchdog [S2]. For our project the software deadman
  (25 ms command timeout in REPO) is therefore project design, not a
  documented vendor behaviour.
- Gravity with power off. Unpowered, the arm falls under gravity unless
  supported; hold the stylus while enabling.
- Velocity. Reduce velocity granularity (30 mm/s at 1 kHz [S2]) before using
  damping; a naive velocity-based damper can destabilise the arm.

## 6. Linux driver notes (kernel source)

`drivers/misc/phantom.c` [S7] targets a PCI card (PLX 9050 bridge) used by the
vendor, not the parallel Premium, so it documents only the shape of the
problem. It maps three regions, offers raw register read and write of up to
eight 32-bit words, a mode where an interrupt applies stored outputs and
toggles an amplifier bit, and a single-opener character device. It contains
no reset, calibration or watchdog logic, confirming that calibration lives in
the userspace library. The forsslund / vintagehaptics notes on parallel-port
Premiums were searched for and not located: NOT FOUND. Only a vendor brochure
stating that Premiums connect through the parallel EPP port was located [S15].

## 7. Bibliography

| Tag | Reference |
|---|---|
| S1 | Friebe, "PHANTOM Premium models 1.0, 1.5 and 3.0" datasheet reproduced in Technical Computing Prague 2005 papers. https://www.humusoft.cz/docs/papers/tcp2005/003_friebe.pdf |
| S2 | Cavusoglu, Feygin, Tendick, "A critical study of the mechanical and electrical properties of the PHANToM haptic interface and improvements for high-performance control", Presence 11(6), 555-568, 2002. doi:10.1162/105474602321050695. Author copy: http://engr.case.edu/cavusoglu_cenk/papers/PRES2002.pdf |
| S3 | Cavusoglu, Feygin, "Kinematics and dynamics of PHANToM(TM) model 1.5 haptic interface", UC Berkeley ERL Memo M01/15, 2001. https://www2.eecs.berkeley.edu/Pubs/TechRpts/2001/9579.html |
| S4a | Lappeenranta University of Technology, master's thesis on PHANTOM neutral position. https://lutpub.lut.fi/bitstream/handle/10024/72534/nbnfi-fe201111075801.pdf |
| S4b | Roozbahani, doctoral dissertation, LUT. https://lutpub.lut.fi/bitstream/handle/10024/111842/Roozbahani_Diss.pdf |
| S5 | OpenHaptics Toolkit Programmer's Guide, calibration and joint-space sections (calibration types; the "all links orthogonal" reset pose). Public copy: https://www.scribd.com/document/337066602/OpenHaptics-ProgGuide-pdf (referential use only) |
| S6 | University of Pennsylvania MEAM design lab, "SensAble PHANToM Premium 1.0" guide (motors, encoders, zero configuration, e-stop). https://medesign.seas.upenn.edu/index.php/Guides/Phantom |
| S7 | Linux kernel source, `drivers/misc/phantom.c`. https://github.com/torvalds/linux/blob/master/drivers/misc/phantom.c |
| S8 | Ikits, Brederson, Hansen, Hollerbach, "A comprehensive calibration and registration procedure for the visual haptic workbench", EGVE 2003. https://publications.sci.utah.edu/publications/ikits03/Ikits_EGVE_03.pdf |
| S9 | 3D Systems, Phantom Premium brochure. https://cdn-66b0dbd2c1ac1859509c5e7a.closte.com/wp-content/uploads/2017/01/Phantom_Premium_EN_Web.pdf |
| S10 | US patent "Robotic apparatus" (neighbouring-arm link lengths; patent number as returned by search, 6,786,896 or 8,123,740, to be confirmed). https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/6786896 |
| S11 | hvr.co.kr Phantom Premium comparison (continuous and friction values). http://hvr.co.kr/?page_id=1200 |
| S12 | NIST, sensor calibration draft (comparison against a reference load cell). https://www.nist.gov/document/sensor-calibration-v1-0docx |
| S13 | Colgate, Brown, "Factors affecting the Z-width of a haptic display", IEEE ICRA 1994, 3205-3210. https://www.robotics.northwestern.edu/research/publications/factors-affecting-the-z-width-of-a-haptic-display.html |
| S14 | Colgate, Schenkel, "Passivity of a class of sampled-data systems: application to haptic interfaces", J. Robotic Systems 14(1), 37-47, 1997. |
| S15 | 3D Systems, Phantom Premium 2P brochure (parallel EPP connection). https://fr.3dsystems.com/sites/default/files/2017-02/3DSystems-Haptics-PhantomPremium_2P_EN_Letter_WEB.pdf |
| S16 | Massie, Salisbury, "The PHANToM haptic interface: a device for probing virtual objects", ASME DSC 55-1, 295-300, 1994. Not retrieved in this pass; cited only by title. |

## 8. Gaps and what must come from OWNER / OBSERVE rows

NOT FOUND publicly: 1.0 link lengths; encoder counts per revolution; cable
ratios; motor torque constant; amplifier gain; DAC resolution; thermal trip
point; 1.0 link masses; exact 1.0 friction measurement; Massie and Salisbury
1994 text; forsslund/vintagehaptics notes.

## 9. Provenance rows to append (do not edit PROVENANCE.md here)

Suggested IDs follow the file's counter; renumber on append. Date 2026-10-17.

| ID | Date | Fact / artefact | Source class | Source detail | Who | Notes |
|----|------|-----------------|--------------|---------------|-----|-------|
| P-NEXT+0 | 2026-10-17 | Reset pose is the all-links-orthogonal pose; calibration persists until power is removed | PUBLIC | OpenHaptics Programmer's Guide calibration section [S5] | research | Referential; own wording |
| P-NEXT+1 | 2026-10-17 | Reset pose: near arm horizontal and square to base front, far arm vertical downward; joint rectangle | PUBLIC | LUT thesis and dissertation [S4a, S4b] | research | |
| P-NEXT+2 | 2026-10-17 | Premium 1.0 uses Maxon 118743 motors and HEDM-5500-B02 optical encoders; zero config upper arm horizontal, lower vertical | PUBLIC | UPenn MEAM guide [S6] | research | Encoder cpr not stated |
| P-NEXT+3 | 2026-10-17 | Model 1.5 kinematics: l1 0.215 m, l2 0.170 m, l3 0.0325 m; ratios 11.6:1, 13.3:1 | PUBLIC | Cavusoglu & Feygin ERL M01/15; Presence 2002 [S2, S3] | research | Model 1.5, not 1.0 |
| P-NEXT+4 | 2026-10-17 | Family spec: 8.5 N peak, 1.4 N continuous, 0.04 N friction, 3.5 N/mm, 0.03 mm resolution | PUBLIC | Datasheets [S1, S9, S11, S2] | research | Column mapping to 1.0 unconfirmed |
| P-NEXT+5 | 2026-10-17 | Max stable wall stiffness 2.0 N/mm stock, 2.7 N/mm linear amp, 1 kHz; 125 Hz resonance | PUBLIC | Presence 2002 [S2] | research | Model 1.5 |
| P-NEXT+6 | 2026-10-17 | 2 N for 10 min: casing +34.5 C (PWM) vs +14.5 C (linear); box carries update-rate and temperature safety logic | PUBLIC | Presence 2002 [S2] | research | |
| P-NEXT+7 | 2026-10-17 | Linux phantom.c is raw register passthrough with no reset, calibration or watchdog | PUBLIC | Mainline kernel source [S7] | research | PCI variant |
| P-NEXT+8 | 2026-10-17 | Registration error 10 to 60 mm from hand-set reset and nominal kinematics | PUBLIC | Ikits et al. 2003 [S8] | research | |
| P-NEXT+9 | 2026-10-17 | Sampled virtual-wall passivity bound roughly K < 2b/T | PUBLIC | Colgate & Brown 1994; Colgate & Schenkel 1997 [S13, S14] | research | Constant not confirmed from originals |
