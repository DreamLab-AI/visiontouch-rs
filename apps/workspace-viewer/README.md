# Workspace Observatory

Offline Three.js arm reconstruction and workspace evidence viewer. It has no
port, motor, browser-device or live-control interface. The included owner sweep
is the 31,271-sample input-only recording from 2026-10-16. The final resting pose
was described by the owner as bottom dead centre "ish", not an exact origin.

From the repository root, `scripts/run-workspace-viewer` installs locked dependencies
when needed, extracts the three owner photographs from the uploaded ZIP, and
starts the loopback-only server. Or run manually:

```sh
cd apps/workspace-viewer
npm ci
npm start
# Open http://127.0.0.1:8765
```

The loopback-only server serves this viewer directory. No account or hosted
service is required. `data/latest.json` takes priority over the bundled evidence
example. To convert another input-only recorder file from the repository root:

```sh
scripts/import-workspace-recording path/to/input.csv
# Also accepts a gzip-compressed CSV.
```

Use **Open capture JSON** for native Calibration Studio sweep exports or the
converted replay bundle. Native exports show the observed cloud but do not
invent encoder trajectories. The CSV converter retains every spatial 2 mm cell
and every tenth timestamped encoder sample, including the final sample; raw data
remains the authoritative source. Replay uses continuous encoder counts.

The model contains a base, yaw pivot, upright frame, approximate drive housings,
paired links, distal link, orthogonal gimbal housings and stylus. The owner photos
supply visible topology. The 139.7 mm link defaults and angle convention come from
the existing reference model in `visiontouch-device`; changing lengths displays model
disagreement, not an independently measured fit. Cosmetic dimensions, cable
routing, joint offsets and motor assignments remain unverified. The last three
encoder channels are preserved but are not assigned wrist angles without evidence.
The model's tool point is the SDK reference point, not a calibrated stylus tip.

- Mint dots: measured encoder-derived model positions.
- Observed-cell surface: boundaries of cells touched by the trajectory. Unknown
  cells remain empty; changing cell size changes displayed coverage volume.
- Convex outer bound: a comparison envelope that fills concavities and may include
  unreachable positions. It is disabled by default and never grants motor authority.
- Orange marker: the final two-second median resting pose, explicitly provisional.
- Gold joints: reference-model armature. Play or scrub the recorded sweep; orbit,
  zoom, change link lengths, and export the visible reconstruction as GLB.

Photos are served from the optional local `photos` link. To recreate it from the
owner archive `docs/evidence/owner-photos-20261016.zip`, extract its three JPGs under `captures/device-photos/`
and link that directory here as `photos`. No photographs or SDK assets are
embedded in generated geometry. Missing photos do not prevent replay.

`npm test` checks kinematics against an SDK fixture, observed-cell topology,
input validation and native export translation. `npm run test:browser` checks
rendering, replay, scene toggles, imported evidence and GLB export with a running
local server and `/usr/bin/chromium` (override `CHROMIUM_PATH` if needed).

Primary reference context: the [manufacturer's Premium guide](https://s3.amazonaws.com/dl.3dsystems.com/binaries/Sensable/UserGuide/Premium1.0_1.5_HF_Device_guide.pdf).
The owner's photographs take precedence for this unit's visible arrangement;
family documentation does not identify its exact hardware revision.
