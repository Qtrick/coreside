import { render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LiveWallpaper } from "./LiveWallpaper";
import type { ResolvedWallpaper } from "@/types/wallpaper";

/**
 * Regression coverage for the Matrix wallpaper "solid #050805" CI failure
 * (Journey 13): resize() clears the canvas pixel buffer by reassigning
 * canvas.width/height, which must always be followed by a glyph repaint —
 * never left as a bare dark fill. See LiveWallpaper.tsx `resize()` /
 * `repaintMatrixAfterClear`.
 *
 * jsdom has no real 2D canvas, so `getContext` is stubbed with a fake
 * context that records calls. `requestAnimationFrame` is stubbed to run
 * synchronously so resize handling is deterministic (no timers, no sleeps).
 * Tests force `prefers-reduced-motion: reduce` so `draw()` never
 * self-reschedules another frame, keeping the synchronous rAF stub bounded.
 */

type FakeGradient = {
  addColorStop: ReturnType<typeof vi.fn>;
};

type FakeCtx = {
  fillStyle: string | FakeGradient;
  strokeStyle: string | FakeGradient;
  lineWidth: number;
  font: string;
  globalAlpha: number;
  fillRect: ReturnType<typeof vi.fn>;
  fillText: ReturnType<typeof vi.fn>;
  setTransform: ReturnType<typeof vi.fn>;
  clearRect: ReturnType<typeof vi.fn>;
  beginPath: ReturnType<typeof vi.fn>;
  closePath: ReturnType<typeof vi.fn>;
  moveTo: ReturnType<typeof vi.fn>;
  lineTo: ReturnType<typeof vi.fn>;
  stroke: ReturnType<typeof vi.fn>;
  arc: ReturnType<typeof vi.fn>;
  fill: ReturnType<typeof vi.fn>;
  drawImage: ReturnType<typeof vi.fn>;
  createLinearGradient: ReturnType<typeof vi.fn>;
  createRadialGradient: ReturnType<typeof vi.fn>;
};

function createFakeCtx(): FakeCtx {
  const gradient: FakeGradient = { addColorStop: vi.fn() };
  return {
    fillStyle: "",
    strokeStyle: "",
    lineWidth: 1,
    font: "",
    globalAlpha: 1,
    fillRect: vi.fn(),
    fillText: vi.fn(),
    setTransform: vi.fn(),
    clearRect: vi.fn(),
    beginPath: vi.fn(),
    closePath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    stroke: vi.fn(),
    arc: vi.fn(),
    fill: vi.fn(),
    drawImage: vi.fn(),
    createLinearGradient: vi.fn(() => gradient),
    createRadialGradient: vi.fn(() => gradient),
  };
}

function mockMatchMedia(matches: boolean) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })) as unknown as typeof window.matchMedia;
}

function setInnerWidth(width: number) {
  Object.defineProperty(window, "innerWidth", {
    value: width,
    writable: true,
    configurable: true,
  });
}

const matrixWallpaper: ResolvedWallpaper = {
  format: "legacy",
  config: {
    kind: "matrix",
    color: "#33ff66",
    secondaryColor: "#6ab0d4",
    speed: 1,
    density: 0.55,
    opacity: 0.5,
  },
};

describe("LiveWallpaper matrix canvas repaint-after-clear", () => {
  let fakeCtx: FakeCtx;
  const originalWebdriver = Object.getOwnPropertyDescriptor(
    window.navigator,
    "webdriver",
  );

  beforeEach(() => {
    fakeCtx = createFakeCtx();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(
      () => fakeCtx as unknown as CanvasRenderingContext2D,
    );
    // Reduced motion keeps draw() from self-rescheduling another rAF frame,
    // so the synchronous rAF stub below cannot recurse unboundedly.
    mockMatchMedia(true);
    vi.stubGlobal(
      "requestAnimationFrame",
      vi.fn((cb: FrameRequestCallback) => {
        cb(0);
        return 1;
      }),
    );
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    setInnerWidth(1024);
  });

  afterEach(() => {
    if (originalWebdriver) {
      Object.defineProperty(window.navigator, "webdriver", originalWebdriver);
    } else {
      delete (window.navigator as { webdriver?: boolean }).webdriver;
    }
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("repaints glyphs after a resize clears the canvas buffer, not just a solid fill", () => {
    render(<LiveWallpaper wallpaper={matrixWallpaper} />);

    // Mount already triggers an initial buffer-sized resize + forced paint.
    expect(fakeCtx.fillText.mock.calls.length).toBeGreaterThan(0);

    fakeCtx.fillRect.mockClear();
    fakeCtx.fillText.mockClear();

    // Simulate a real resize that changes the canvas pixel buffer (dpr*view
    // dimensions differ from the previously committed canvas.width/height).
    setInnerWidth(640);
    window.dispatchEvent(new Event("resize"));

    // The dark tint fill alone (solid #050805 CI regression) is not enough —
    // glyphs must be repainted in the same pass.
    expect(fakeCtx.fillRect.mock.calls.length).toBeGreaterThan(0);
    expect(fakeCtx.fillText.mock.calls.length).toBeGreaterThan(0);
  });

  it("keeps repainting glyphs after resize under WebDriver (CI) sessions", () => {
    Object.defineProperty(window.navigator, "webdriver", {
      value: true,
      configurable: true,
    });

    render(<LiveWallpaper wallpaper={matrixWallpaper} />);
    expect(fakeCtx.fillText.mock.calls.length).toBeGreaterThan(0);

    fakeCtx.fillText.mockClear();
    setInnerWidth(800);
    window.dispatchEvent(new Event("resize"));

    expect(fakeCtx.fillText.mock.calls.length).toBeGreaterThan(0);
  });

  it("cancels the animation loop and removes every listener it attached on unmount", () => {
    const windowAddSpy = vi.spyOn(window, "addEventListener");
    const windowRemoveSpy = vi.spyOn(window, "removeEventListener");
    const docAddSpy = vi.spyOn(document, "addEventListener");
    const docRemoveSpy = vi.spyOn(document, "removeEventListener");
    const cancelSpy = vi.fn();
    vi.stubGlobal("cancelAnimationFrame", cancelSpy);

    const { unmount } = render(<LiveWallpaper wallpaper={matrixWallpaper} />);

    const addedWindowTypes = windowAddSpy.mock.calls.map((call) => call[0]);
    const addedDocTypes = docAddSpy.mock.calls.map((call) => call[0]);
    // Sanity: the effect really attaches the listeners this test verifies get removed.
    expect(addedWindowTypes).toEqual(
      expect.arrayContaining(["resize", "pageshow", "pagehide", "blur", "focus"]),
    );
    expect(addedDocTypes).toEqual(expect.arrayContaining(["visibilitychange"]));

    unmount();

    // Every window/document listener type added by mount must be removed by cleanup —
    // otherwise detached wallpaper instances keep listening (memory leak).
    const removedWindowTypes = windowRemoveSpy.mock.calls.map((call) => call[0]);
    const removedDocTypes = docRemoveSpy.mock.calls.map((call) => call[0]);
    for (const type of new Set(addedWindowTypes)) {
      expect(removedWindowTypes).toContain(type);
    }
    for (const type of new Set(addedDocTypes)) {
      expect(removedDocTypes).toContain(type);
    }
    // The RAF loop (and any pending resize RAF) must be canceled, not left running
    // against an unmounted canvas.
    expect(cancelSpy).toHaveBeenCalled();
  });

  it("repaints particles wallpaper after resize clears canvas buffer under reduced-motion", () => {
    const particlesWallpaper: ResolvedWallpaper = {
      format: "legacy",
      config: {
        kind: "particles",
        color: "#60a5fa",
        secondaryColor: "#a78bfa",
        speed: 1,
        density: 0.5,
        opacity: 0.6,
      },
    };

    render(<LiveWallpaper wallpaper={particlesWallpaper} />);
    // Particles initial frame clears and draws paths/arcs
    expect(fakeCtx.clearRect.mock.calls.length).toBeGreaterThan(0);
    expect(fakeCtx.arc.mock.calls.length).toBeGreaterThan(0);

    fakeCtx.clearRect.mockClear();
    fakeCtx.arc.mockClear();

    setInnerWidth(768);
    window.dispatchEvent(new Event("resize"));

    // After buffer clear on resize, repaintAfterClear repaints particles
    expect(fakeCtx.clearRect.mock.calls.length).toBeGreaterThan(0);
    expect(fakeCtx.arc.mock.calls.length).toBeGreaterThan(0);
  });

  it("repaints rain wallpaper strokes after resize clears canvas buffer", () => {
    const rainWallpaper: ResolvedWallpaper = {
      format: "legacy",
      config: {
        kind: "rain",
        color: "#38bdf8",
        secondaryColor: "#818cf8",
        speed: 1.2,
        density: 0.6,
        opacity: 0.7,
      },
    };

    render(<LiveWallpaper wallpaper={rainWallpaper} />);
    expect(fakeCtx.stroke.mock.calls.length).toBeGreaterThan(0);

    fakeCtx.stroke.mockClear();
    setInnerWidth(800);
    window.dispatchEvent(new Event("resize"));

    // Rain stroke lines repainted after resize buffer clear
    expect(fakeCtx.stroke.mock.calls.length).toBeGreaterThan(0);
  });
});

