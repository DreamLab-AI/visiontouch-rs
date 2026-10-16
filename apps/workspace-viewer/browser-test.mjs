import { chromium } from "playwright";
import assert from "node:assert/strict";
import { mkdtemp, stat, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
const output = await mkdtemp(join(tmpdir(), "haptics-viewer-"));
const browser = await chromium.launch({
  executablePath: process.env.CHROMIUM_PATH ?? "/usr/bin/chromium",
  headless: true,
  args: ["--no-sandbox", "--enable-unsafe-swiftshader"],
});
try {
  const page = await browser.newPage({
    viewport: { width: 1480, height: 960 },
  });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(process.env.VIEWER_URL ?? "http://127.0.0.1:8765");
  await page.waitForFunction(
    () => window.__captureReady && window.__renderedFrames > 4,
  );
  await page.locator("#timeline").fill("0");
  await page.locator("#timeline").dispatchEvent("input");
  await page.locator("#play").click();
  await page.waitForFunction(
    () => parseFloat(document.getElementById("time").textContent) > 0.1,
  );
  await page.locator("#play").click();
  await page.locator("#voxels").check();
  await page.locator("#hull").check();
  await page.locator("#upper").fill("150");
  await page.locator("#upper").dispatchEvent("input");
  assert.match(await page.locator("#upperValue").textContent(), /150/);
  const pending = page.waitForEvent("download");
  await page.locator("#export").click();
  const download = await pending;
  await download.saveAs(join(output, "scene.glb"));
  assert.equal(await download.failure(), null);
  assert.ok((await stat(join(output, "scene.glb"))).size > 1000);
  await page.locator("#file").setInputFiles({
    name: "native.json",
    mimeType: "application/json",
    buffer: Buffer.from(
      JSON.stringify({
        schema: 1,
        reference_model_m: [0, 0, 0],
        sweep: {
          points_model_m: [
            [0, 0, 0],
            [0.1, 0, 0],
            [0, 0.1, 0],
            [0, 0, 0.1],
          ],
        },
      }),
    ),
  });
  await page.waitForFunction(
    () =>
      document.getElementById("captureTitle").textContent ===
      "Calibration sweep export",
  );
  assert.equal(await page.locator("#play").isDisabled(), true);
  assert.deepEqual(errors, []);
  console.log(
    "Browser checks passed: rendering, playback, geometry toggles, link adjustment, GLB and native import.",
  );
} finally {
  await browser.close();
  await rm(output, { recursive: true, force: true });
}
