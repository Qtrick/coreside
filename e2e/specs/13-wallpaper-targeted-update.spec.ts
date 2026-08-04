import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import {
  E2E_REPO_ROOT,
  evidenceIdentity,
  openSettings,
  openSettingsCategory,
  waitForAppReady,
} from "../helpers.js";

/**
 * Journey 13 — wallpaper selection + transparency commit + canvas pixel sampling.
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock npx wdio run e2e/wdio.conf.ts --suite wallpaper-targeted
 */
const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/wallpaper-visual-results.json",
);
const screenshotPath = path.resolve(
  E2E_REPO_ROOT,
  "e2e/.artifacts/wallpaper-visual-screenshot.png",
);

describe("Journey 13 — wallpaper targeted update", () => {
  it("applies Matrix and commits transparency with non-uniform canvas pixels", async () => {
    await waitForAppReady();
    await openSettings();

    // Reliable path: Appearance → "Open wallpaper templates" → Added/Templates.
    try {
      await openSettingsCategory("Appearance");
    } catch {
      // Fall through.
    }
    const openTemplates = await $("button*=Open wallpaper templates");
    await openTemplates.waitForExist({ timeout: 10_000 });
    await openTemplates.click();

    const wallpapersHeading = await $("#wallpaper-heading");
    await wallpapersHeading.waitForExist({
      timeout: 15_000,
      timeoutMsg: "wallpaper-heading not found after Open wallpaper templates",
    });
    await wallpapersHeading.scrollIntoView();

    const matrix = await $("button.wallpaper-preset-card*=Matrix");
    await matrix.waitForExist({ timeout: 10_000 });
    await matrix.click();

    await browser.waitUntil(
      async () => (await matrix.getAttribute("aria-pressed")) === "true",
      { timeout: 10_000, timeoutMsg: "Matrix preset not selected" },
    );

    const slider = await $('input[type="range"][aria-valuemax="60"]');
    await slider.waitForExist({ timeout: 5_000 });
    await slider.click();
    await browser.execute((el) => {
      const input = el as HTMLInputElement;
      input.value = "40";
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new Event("change", { bubbles: true }));
      input.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
    }, slider);

    await browser.waitUntil(
      async () => (await slider.getAttribute("aria-valuenow")) === "40",
      { timeout: 8_000, timeoutMsg: "transparency did not commit to 40" },
    );

    const live = await $(".live-wallpaper");
    await live.waitForExist({ timeout: 10_000 });

    const canvas = await $(".live-wallpaper-canvas");
    await canvas.waitForExist({ timeout: 10_000 });

    // Matrix rain starts as a solid clear (#050805) then paints glyphs.
    // Poll a dense sample grid until unique colors >= 2 and luminance span >= 8.
    let finalPixels: {
      width: number;
      height: number;
      sampleCount: number;
      uniqueColors: number;
      lumSpan: number;
      samples: Array<{
        x: number;
        y: number;
        r: number;
        g: number;
        b: number;
        a: number;
        lum: number;
      }>;
      panelAlpha: number | null;
    } | null = null;

    await browser.waitUntil(
      async () => {
        const result = await browser.execute(() => {
          const el = document.querySelector(
            ".live-wallpaper-canvas",
          ) as HTMLCanvasElement | null;
          if (!el) return null;
          const ctx = el.getContext("2d", { willReadFrequently: true });
          if (!ctx) return null;
          const w = el.width || el.clientWidth;
          const h = el.height || el.clientHeight;
          if (w < 40 || h < 40) return null;
          const stepX = Math.max(8, Math.floor(w / 12));
          const stepY = Math.max(8, Math.floor(h / 12));
          const samples: Array<{
            x: number;
            y: number;
            r: number;
            g: number;
            b: number;
            a: number;
            lum: number;
          }> = [];
          for (let y = 4; y < h; y += stepY) {
            for (let x = 4; x < w; x += stepX) {
              const data = ctx.getImageData(x, y, 1, 1).data;
              const r = data[0] ?? 0;
              const g = data[1] ?? 0;
              const b = data[2] ?? 0;
              const a = data[3] ?? 0;
              samples.push({
                x,
                y,
                r,
                g,
                b,
                a,
                lum: 0.2126 * r + 0.7152 * g + 0.0722 * b,
              });
            }
          }
          const keys = new Set(
            samples.map((s) => `${s.r},${s.g},${s.b},${s.a}`),
          );
          const lums = samples.map((s) => s.lum);
          const lumSpan = Math.max(...lums) - Math.min(...lums);
          const panelAlphaRaw = getComputedStyle(document.documentElement)
            .getPropertyValue("--core-panel-alpha")
            .trim();
          return {
            width: w,
            height: h,
            sampleCount: samples.length,
            uniqueColors: keys.size,
            lumSpan,
            // Prefer mixed samples for the report (not only the top-left clear band).
            samples: [
              ...samples.filter((s) => s.lum > 20).slice(0, 6),
              ...samples.slice(0, 6),
            ].slice(0, 12),
            panelAlpha: panelAlphaRaw ? Number(panelAlphaRaw) : null,
          };
        });
        if (
          result &&
          result.uniqueColors >= 2 &&
          result.lumSpan >= 8 &&
          result.sampleCount >= 16
        ) {
          finalPixels = result;
          return true;
        }
        return false;
      },
      {
        timeout: 12_000,
        interval: 200,
        timeoutMsg:
          "Matrix canvas never showed measurable pixel variance (glyphs vs clear)",
      },
    );

    expect(finalPixels).toBeTruthy();
    expect(finalPixels!.uniqueColors).toBeGreaterThanOrEqual(2);
    expect(finalPixels!.lumSpan).toBeGreaterThanOrEqual(8);
    expect(finalPixels!.sampleCount).toBeGreaterThanOrEqual(16);
    // uniqueColors/lumSpan already prove non-uniformity across the full grid;
    // do not re-assert on a truncated preview slice.

    const valueNow = await slider.getAttribute("aria-valuenow");
    expect(valueNow).toBe("40");

    let screenshotHash: string | null = null;
    try {
      fs.mkdirSync(path.dirname(screenshotPath), { recursive: true });
      await browser.saveScreenshot(screenshotPath);
      if (fs.existsSync(screenshotPath)) {
        screenshotHash = createHash("sha256")
          .update(fs.readFileSync(screenshotPath))
          .digest("hex");
      }
    } catch {
      screenshotHash = null;
    }

    const identity = evidenceIdentity();
    const evidence = {
      schemaVersion: 1,
      product: "Coreside",
      generatedAt: new Date().toISOString(),
      commit: identity.commit,
      dirty: identity.dirty,
      platform: identity.platform,
      architecture: identity.arch,
      command: "e2e:journey-13-wallpaper-targeted",
      result: "passed",
      evidenceLevel: "Desktop Verified",
      e2eSpec: "e2e/specs/13-wallpaper-targeted-update.spec.ts",
      assertions: {
        matrixSelected: "passed",
        transparencyCommitted40: "passed",
        liveWallpaperPresent: "passed",
        canvasPresent: "passed",
        desktopPixelSampling: "passed",
        pixelVariance: "passed",
        uniqueColorsAtLeast2: "passed",
        lumSpanAtLeast8: "passed",
      },
      transparency: {
        ariaValueNow: valueNow,
        panelAlpha: finalPixels!.panelAlpha,
      },
      canvas: {
        width: finalPixels!.width,
        height: finalPixels!.height,
        sampleCount: finalPixels!.sampleCount,
        uniqueColors: finalPixels!.uniqueColors,
        lumSpan: finalPixels!.lumSpan,
        samples: finalPixels!.samples,
      },
      screenshotHash,
      publicBeta: "Not ready",
    };

    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2) + "\n");
  });
});
