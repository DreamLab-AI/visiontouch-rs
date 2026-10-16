import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { ConvexGeometry } from "three/addons/geometries/ConvexGeometry.js";
import { GLTFExporter } from "three/addons/exporters/GLTFExporter.js";
import { armPose, validateCapture, voxelSurface } from "./geometry.mjs";
const $ = (id) => document.getElementById(id),
  vec = (p) => new THREE.Vector3(...p);
const scene = new THREE.Scene();
const renderer = new THREE.WebGLRenderer({
  canvas: $("scene"),
  antialias: true,
  alpha: true,
});
renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
renderer.outputColorSpace = THREE.SRGBColorSpace;
const camera = new THREE.PerspectiveCamera(42, 1, 0.002, 10);
const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;
scene.add(new THREE.HemisphereLight(0xd7edff, 0x26313c, 2));
const light = new THREE.DirectionalLight(0xffe6bc, 3);
light.position.set(-0.4, 1, 0.5);
scene.add(light);
const fill = new THREE.DirectionalLight(0x8bbcff, 2);
fill.position.set(0.5, 0.4, -0.5);
scene.add(fill);
const grid = new THREE.GridHelper(0.9, 30, 0x385365, 0x253949);
grid.position.y = -0.15;
scene.add(grid);
const axes = new THREE.AxesHelper(0.07);
axes.position.set(-0.25, -0.149, 0.2);
scene.add(axes);
const arm = new THREE.Group(),
  cloud = new THREE.Group();
scene.add(arm, cloud);
let data = null,
  playing = false,
  lastTime = 0,
  poseTime = 0,
  pointMesh,
  voxelMesh,
  hullMesh,
  currentFrame = 0;
const materials = {
  dark: new THREE.MeshStandardMaterial({
    color: 0x292d34,
    metalness: 0.6,
    roughness: 0.38,
  }),
  silver: new THREE.MeshStandardMaterial({
    color: 0xadb8c5,
    metalness: 0.85,
    roughness: 0.25,
  }),
  gold: new THREE.MeshStandardMaterial({
    color: 0xdab577,
    metalness: 0.65,
    roughness: 0.3,
  }),
  black: new THREE.MeshStandardMaterial({
    color: 0x11191e,
    metalness: 0.3,
    roughness: 0.6,
  }),
};
function clear(group) {
  while (group.children.length) {
    const x = group.children[0];
    group.remove(x);
    x.traverse((o) => {
      o.geometry?.dispose();
      if (o.material && !Object.values(materials).includes(o.material))
        o.material.dispose?.();
    });
  }
}
function box(parent, size, p, mat = materials.dark) {
  const m = new THREE.Mesh(new THREE.BoxGeometry(...size), mat);
  m.position.copy(vec(p));
  parent.add(m);
  return m;
}
function rod(parent, a, b, r, mat = materials.silver) {
  a = vec(a);
  b = vec(b);
  const delta = b.clone().sub(a);
  const m = new THREE.Mesh(
    new THREE.CylinderGeometry(r, r, delta.length(), 20),
    mat,
  );
  m.position.copy(a.clone().add(b).multiplyScalar(0.5));
  m.quaternion.setFromUnitVectors(
    new THREE.Vector3(0, 1, 0),
    delta.normalize(),
  );
  parent.add(m);
  return m;
}
function sphere(parent, p, r, mat) {
  const m = new THREE.Mesh(new THREE.SphereGeometry(r, 20, 12), mat);
  m.position.copy(vec(p));
  parent.add(m);
  return m;
}
const add = (a, b) => a.map((x, i) => x + b[i]);
function buildArm() {
  clear(arm);
  const counts = data?.frames[currentFrame]?.counts ?? [
    0, -1200, 1800, 0, 0, 0,
  ];
  const upper = +$("upper").value / 1000,
    lower = +$("lower").value / 1000;
  const p = armPose(counts, upper, lower),
    s = p.shoulder,
    e = p.elbow,
    t = p.tip;
  const yaw = p.angles[0];
  // Base/yaw assembly and U-shaped frame are photo-informed visual approximations.
  box(arm, [0.32, 0.075, 0.25], [0, -0.112, -0.13]);
  rod(arm, [0, -0.075, -upper], [0, 0.005, -upper], 0.024, materials.dark);
  const frame = new THREE.Group();
  frame.position.set(0, 0.005, -upper);
  frame.rotation.y = -yaw;
  arm.add(frame);
  box(frame, [0.15, 0.018, 0.05], [0, 0.005, 0]);
  for (const x of [-0.067, 0.067])
    box(frame, [0.013, lower - 0.012, 0.035], [x, (lower - 0.012) / 2, 0]);
  for (const y of [lower - 0.025, lower + 0.022])
    rod(frame, [-0.055, y, -0.018], [0.045, y, -0.018], 0.019, materials.black);
  rod(
    frame,
    [0.055, lower, -0.005],
    [0.07, lower, -0.005],
    0.044,
    materials.dark,
  );
  rod(
    frame,
    [-0.065, lower, -0.005],
    [-0.05, lower, -0.005],
    0.037,
    materials.dark,
  );
  // The two visible parallel rods and coupling brackets; exact cable mechanics unresolved.
  const sideways = [Math.cos(yaw) * 0.013, 0, Math.sin(yaw) * 0.013];
  rod(arm, add(s, sideways), add(e, sideways), 0.007, materials.dark);
  rod(
    arm,
    add(
      s,
      sideways.map((v) => -v),
    ),
    add(
      e,
      sideways.map((v) => -v),
    ),
    0.004,
    materials.silver,
  );
  rod(
    arm,
    add(
      e,
      sideways.map((v) => -1.8 * v),
    ),
    add(
      e,
      sideways.map((v) => 1.8 * v),
    ),
    0.008,
    materials.silver,
  );
  rod(arm, e, t, 0.006, materials.dark);
  sphere(arm, s, 0.012, materials.gold);
  sphere(arm, e, 0.011, materials.gold);
  // Orthogonal gimbal housings are visible; active/passive actuator roles are unknown.
  rod(
    arm,
    add(t, [0, 0.012, 0]),
    add(t, [0, -0.012, 0]),
    0.012,
    materials.dark,
  );
  rod(
    arm,
    add(t, [-0.018, -0.018, 0]),
    add(t, [0.018, -0.018, 0]),
    0.011,
    materials.dark,
  );
  rod(
    arm,
    add(t, [0, -0.018, -0.025]),
    add(t, [0, -0.018, 0.115]),
    0.004,
    materials.black,
  );
  sphere(arm, t, 0.005, materials.gold);
  arm.visible = $("arm").checked;
  $("angles").textContent =
    `Model angles: ${p.angles.map((x) => ((x * 180) / Math.PI).toFixed(1) + "°").join(" / ")}`;
  const expected = armPose(counts).tip;
  const residual = Math.hypot(...t.map((x, i) => x - expected[i])) * 1000;
  $("residual").textContent =
    `Current tip shift from reference model: ${residual.toFixed(2)} mm. This is model disagreement, not measured physical error.`;
  $("time").textContent =
    (data?.frames[currentFrame]?.t ?? 0).toFixed(2) + " s";
  $("poseLabel").textContent = data
    ? `${data.reference_label ?? "Unlabelled reference"} · ${data.frames[currentFrame]?.counts.length ?? 0} encoder channels retained`
    : "Photo-informed reconstruction · dimensions unverified";
}
function rebuildCloud() {
  clear(cloud);
  if (!data) return;
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute(
    "position",
    new THREE.Float32BufferAttribute(data.points.flat(), 3),
  );
  pointMesh = new THREE.Points(
    geometry,
    new THREE.PointsMaterial({
      color: 0x7de5bd,
      size: 0.0023,
      sizeAttenuation: true,
      transparent: true,
      opacity: 0.8,
    }),
  );
  cloud.add(pointMesh);
  const v = voxelSurface(data.points, +$("cell").value / 1000);
  const vg = new THREE.BufferGeometry();
  vg.setAttribute("position", new THREE.Float32BufferAttribute(v.vertices, 3));
  vg.computeVertexNormals();
  voxelMesh = new THREE.Mesh(
    vg,
    new THREE.MeshStandardMaterial({
      color: 0x60c8ae,
      transparent: true,
      opacity: 0.3,
      side: THREE.DoubleSide,
      depthWrite: false,
    }),
  );
  cloud.add(voxelMesh);
  hullMesh = null;
  if (data.points.length >= 4) {
    try {
      const h = new ConvexGeometry(data.points.map(vec));
      if (h.getAttribute("position").count > 0) {
        hullMesh = new THREE.Mesh(
          h,
          new THREE.MeshBasicMaterial({
            color: 0xeac27c,
            transparent: true,
            opacity: 0.1,
            side: THREE.DoubleSide,
            depthWrite: false,
          }),
        );
        cloud.add(hullMesh);
      } else h.dispose();
    } catch {
      $("message").textContent =
        "A 3D convex bound needs non-coplanar samples.";
    }
  }
  $("coverage").textContent =
    `${v.cells.toLocaleString()} observed cells · ${v.observedCellVolumeMl.toFixed(0)} mL of cell coverage. Coverage changes with cell size; it is not workspace capacity.`;
  if (data.reference_model_m) {
    const marker = sphere(
      cloud,
      data.reference_model_m,
      0.006,
      new THREE.MeshBasicMaterial({ color: 0xffa57c }),
    );
    marker.name = "Provisional resting reference";
  }
  visibility();
}
function visibility() {
  arm.visible = $("arm").checked;
  if (pointMesh) pointMesh.visible = $("points").checked;
  if (voxelMesh) voxelMesh.visible = $("voxels").checked;
  if (hullMesh) hullMesh.visible = $("hull").checked;
}
function resetView() {
  camera.position.set(0.58, 0.34, 0.63);
  controls.target.set(0, 0.065, -0.025);
  controls.update();
}
function loadCapture(json) {
  data = validateCapture(json);
  currentFrame = Math.max(0, data.frames.length - 1);
  poseTime = data.frames[currentFrame]?.t ?? 0;
  playing = false;
  $("play").textContent = "Play";
  $("timeline").max = Math.max(0, data.frames.length - 1);
  $("timeline").value = currentFrame;
  $("play").disabled = data.frames.length === 0;
  $("captureTitle").textContent = data.name ?? "Workspace sweep";
  $("summary").textContent =
    `${(data.sample_count ?? data.frames.length).toLocaleString()} samples · ${(data.duration_s ?? 0).toFixed(1)} s · model spans ${(data.span_mm ?? []).map((x) => x.toFixed(0)).join(" × ")} mm`;
  $("message").textContent =
    "Recorded coordinates are model-derived. No measured force or physical dimensions are implied.";
  rebuildCloud();
  buildArm();
  window.__captureReady = true;
}
async function latest() {
  try {
    let r = await fetch("./data/latest.json");
    if (!r.ok) r = await fetch("./examples/owner-sweep.json");
    if (!r.ok)
      throw Error(
        "No capture available. Import one with tools/import-workspace-recording.",
      );
    loadCapture(await r.json());
  } catch (e) {
    $("message").textContent = e.message;
  }
}
$("latest").onclick = latest;
$("file").onchange = async (e) => {
  try {
    const f = e.target.files[0];
    if (!f) return;
    if (f.size > 30_000_000) throw Error("Capture file exceeds 30 MB");
    loadCapture(JSON.parse(await f.text()));
  } catch (e) {
    $("message").textContent = e.message;
  }
};
$("play").onclick = () => {
  playing = !playing;
  $("play").textContent = playing ? "Pause" : "Play";
  lastTime = performance.now();
};
$("timeline").oninput = () => {
  currentFrame = +$("timeline").value;
  poseTime = data?.frames[currentFrame]?.t ?? 0;
  buildArm();
};
for (const id of ["arm", "points", "voxels", "hull"])
  $(id).onchange = visibility;
for (const id of ["upper", "lower"])
  $(id).oninput = () => {
    $(id + "Value").textContent = (+$(id).value).toFixed(1) + " mm";
    buildArm();
  };
$("cell").onchange = () => {
  $("cellValue").textContent = $("cell").value + " mm";
  rebuildCloud();
};
$("reset").onclick = () => {
  for (const id of ["upper", "lower"]) {
    $(id).value = 139.7;
    $(id + "Value").textContent = "139.7 mm";
  }
  resetView();
  buildArm();
};
$("export").onclick = () => {
  const out = new THREE.Group();
  out.add(arm.clone());
  out.add(cloud.clone());
  out.userData = {
    purpose: "offline visual reconstruction; not motor control",
    source: "owner photographs and reference kinematics",
    dimensions: "unverified",
    capture: data?.name ?? null,
  };
  new GLTFExporter().parse(
    out,
    (result) => {
      const url = URL.createObjectURL(
        new Blob([result], { type: "model/gltf-binary" }),
      );
      const a = document.createElement("a");
      a.href = url;
      a.download = "phantom-workspace.glb";
      a.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    },
    (error) => {
      $("message").textContent = error.message;
    },
    { binary: true },
  );
};
function resize() {
  const r = $("viewport").getBoundingClientRect();
  renderer.setSize(r.width, r.height, false);
  camera.aspect = r.width / r.height;
  camera.updateProjectionMatrix();
}
new ResizeObserver(resize).observe($("viewport"));
function animate(now) {
  requestAnimationFrame(animate);
  if (playing && data?.frames.length) {
    poseTime += (now - lastTime) / 1000;
    const end = data.frames.at(-1).t;
    if (poseTime > end) {
      poseTime = 0;
      currentFrame = 0;
    }
    while (
      currentFrame + 1 < data.frames.length &&
      data.frames[currentFrame + 1].t <= poseTime
    )
      currentFrame++;
    $("timeline").value = currentFrame;
    buildArm();
  }
  lastTime = now;
  controls.update();
  renderer.render(scene, camera);
  window.__renderedFrames = (window.__renderedFrames ?? 0) + 1;
}
resetView();
buildArm();
resize();
requestAnimationFrame(animate);
latest();
