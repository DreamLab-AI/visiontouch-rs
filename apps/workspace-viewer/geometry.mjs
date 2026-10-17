/** Reference-model kinematics, shared convention with visiontouch-device.
 * This is a visualization, not an independent physical calibration. */
export function armPose(counts, upper = 0.1397, lower = 0.1397) {
  if (
    !Array.isArray(counts) ||
    counts.length < 3 ||
    counts.some((x) => !Number.isFinite(x))
  )
    throw Error("Invalid encoder pose");
  const a = counts[0] * 0.00018075138,
    b = -counts[2] * 0.00021136618,
    c = -counts[1] * 0.00021136618;
  const shoulder = [0, lower, -upper];
  const elbow = [
    -upper * Math.cos(b) * Math.sin(a),
    lower + upper * Math.sin(b),
    upper * Math.cos(b) * Math.cos(a) - upper,
  ];
  const tip = [
    elbow[0] - lower * Math.sin(c) * Math.sin(a),
    elbow[1] - lower * Math.cos(c),
    elbow[2] + lower * Math.sin(c) * Math.cos(a),
  ];
  return { shoulder, elbow, tip, angles: [a, b, c] };
}
export function validateCapture(data) {
  if (
    data?.schema !== 1 ||
    !Array.isArray(data.points) ||
    data.points.length < 1 ||
    data.points.length > 40000
  )
    throw Error("Expected a workspace capture with 1–40,000 points");
  for (const p of data.points)
    if (
      !Array.isArray(p) ||
      p.length !== 3 ||
      p.some((v) => !Number.isFinite(v) || Math.abs(v) > 10)
    )
      throw Error("Invalid model point");
  if (!Array.isArray(data.frames) || data.frames.length > 40000)
    throw Error("Invalid pose frames");
  for (const f of data.frames) {
    if (!Number.isFinite(f.t) || f.t < 0) throw Error("Invalid frame time");
    armPose(f.counts);
  }
  return data;
}
export function voxelSurface(points, size = 0.006) {
  if (!Number.isFinite(size) || size < 0.002 || size > 0.03)
    throw Error("Cell size outside 2–30 mm");
  const cells = new Map();
  for (const p of points) {
    const c = p.map((v) => Math.floor(v / size));
    cells.set(c.join(","), c);
  }
  // Cube faces only on observed-cell boundaries; unknown cells are not filled.
  const faces = [
    [
      [1, 0, 0],
      [
        [1, 0, 0],
        [1, 1, 0],
        [1, 1, 1],
        [1, 0, 1],
      ],
    ],
    [
      [-1, 0, 0],
      [
        [0, 0, 1],
        [0, 1, 1],
        [0, 1, 0],
        [0, 0, 0],
      ],
    ],
    [
      [0, 1, 0],
      [
        [0, 1, 1],
        [1, 1, 1],
        [1, 1, 0],
        [0, 1, 0],
      ],
    ],
    [
      [0, -1, 0],
      [
        [0, 0, 0],
        [1, 0, 0],
        [1, 0, 1],
        [0, 0, 1],
      ],
    ],
    [
      [0, 0, 1],
      [
        [1, 0, 1],
        [1, 1, 1],
        [0, 1, 1],
        [0, 0, 1],
      ],
    ],
    [
      [0, 0, -1],
      [
        [0, 0, 0],
        [0, 1, 0],
        [1, 1, 0],
        [1, 0, 0],
      ],
    ],
  ];
  const vertices = [];
  for (const c of cells.values())
    for (const [delta, corners] of faces) {
      if (cells.has(c.map((v, i) => v + delta[i]).join(","))) continue;
      for (const i of [0, 1, 2, 0, 2, 3])
        for (let j = 0; j < 3; j++)
          vertices.push((c[j] + corners[i][j]) * size);
    }
  return {
    vertices,
    cells: cells.size,
    observedCellVolumeMl: cells.size * size ** 3 * 1e6,
  };
}
