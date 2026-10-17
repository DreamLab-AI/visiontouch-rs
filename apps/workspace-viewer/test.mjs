import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { armPose, voxelSurface, validateCapture } from "./geometry.mjs";
test("arm endpoint matches SDK reference fixture", () => {
  const tip = armPose([6298, -1052, 1100, 0, 0, 0]).tip;
  const expected = [-0.1514001923, -0.0287494202, -0.0698199844];
  tip.forEach((v, i) => assert.ok(Math.abs(v - expected[i]) < 1e-8));
});
test("voxel mesh preserves unknown concavity instead of filling a box", () => {
  const v = voxelSurface(
    [
      [0, 0, 0],
      [0.012, 0, 0],
    ],
    0.006,
  );
  assert.equal(v.cells, 2);
  assert.equal(v.vertices.length, 2 * 6 * 6 * 3);
  assert.ok(Math.abs(v.observedCellVolumeMl - 0.432) < 1e-9);
});
test("shared voxel faces are removed", () =>
  assert.equal(
    voxelSurface(
      [
        [0, 0, 0],
        [0.006, 0, 0],
      ],
      0.006,
    ).vertices.length,
    10 * 6 * 3,
  ));
test("reject corrupt imports and oversized geometry parameters", () => {
  assert.throws(() =>
    validateCapture({ schema: 1, points: [[NaN, 0, 0]], frames: [] }),
  );
  assert.throws(() => voxelSurface([], 0));
  assert.throws(() => armPose([1, 2]));
});

test("coverage exports validate as plain schema 1 captures", () => {
  const d = validateCapture({
    schema: 1,
    points: [
      [0, 0, 0],
      [0.01, 0.02, 0.03],
    ],
    frames: [],
  });
  assert.equal(d.points.length, 2);
  assert.equal(d.frames.length, 0);
});

test("the calibration studio coverage fixture validates", () => {
  const fixture = JSON.parse(
    readFileSync(
      new URL("./examples/calibration-coverage.json", import.meta.url),
    ),
  );
  const d = validateCapture(fixture);
  assert.equal(d.points.length, 2);
  assert.equal(d.frames.length, 0);
});
