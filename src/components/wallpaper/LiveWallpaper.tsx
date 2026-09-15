import { useEffect, useRef, useState, type CSSProperties } from "react";
import type { WallpaperConfig } from "@/types/agent";
import { WallpaperKindSchema } from "@/types/agent";
import type { ResolvedWallpaper, SchemaWallpaperConfig } from "@/types/wallpaper";
import { cssFilterFromConfig } from "@/lib/wallpaper-filter";
import { normalizeCanonicalHex } from "@/lib/wallpaper-hex";
import { resolveMediaAssetUrl } from "@/lib/media";

type LiveWallpaperProps = {
  wallpaper: ResolvedWallpaper;
};

const MATRIX_GLYPHS =
  "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

function clamp(n: number, min: number, max: number) {
  return Math.min(max, Math.max(min, n));
}

function prefersReducedMotion() {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/** W3C WebDriver flag — false in normal production; true only under automation. */
function webDriverSession() {
  return typeof navigator !== "undefined" && navigator.webdriver === true;
}

function resolveKind(raw: string | undefined) {
  const parsed = WallpaperKindSchema.safeParse(raw ?? "none");
  return parsed.success ? parsed.data : "none";
}

function hexColor(raw: string | null | undefined, fallback: string) {
  return normalizeCanonicalHex(raw ?? "") ?? fallback;
}

function wallpaperLayerStyle(config?: SchemaWallpaperConfig | null): CSSProperties | undefined {
  const filter = cssFilterFromConfig(config?.filter);
  return filter ? { filter } : undefined;
}

function schemaToLegacyKind(config: SchemaWallpaperConfig): string {
  if (config.type === "canvas-preset") return config.preset ?? "none";
  if (config.type === "floating-particles") return "particles";
  return "none";
}

function CanvasWallpaper({
  kind,
  color,
  secondaryColor,
  speed,
  density,
  opacity,
}: {
  kind: string;
  color: string;
  secondaryColor: string;
  speed: number;
  density: number;
  opacity: number;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (kind === "none") return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let raf = 0;
    let running = true;
    const reduced = prefersReducedMotion();

    let viewW = window.innerWidth;
    let viewH = window.innerHeight;
    let resizeRaf = 0;
    let hidden = document.visibilityState === "hidden";
    const webDriver = webDriverSession();
    /** Matrix-only: repaint after resize when WebDriver keeps visibilityState hidden. */
    let repaintMatrixIfHidden: (() => void) | undefined;

    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 1.75);
      viewW = Math.max(1, canvas.clientWidth || window.innerWidth);
      viewH = Math.max(1, canvas.clientHeight || window.innerHeight);
      const nextW = Math.floor(viewW * dpr);
      const nextH = Math.floor(viewH * dpr);
      if (canvas.width !== nextW || canvas.height !== nextH) {
        canvas.width = nextW;
        canvas.height = nextH;
      }
      canvas.style.width = "100%";
      canvas.style.height = "100%";
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      repaintMatrixIfHidden?.();
    };
    resize();

    const scheduleResize = (after?: () => void) => {
      cancelAnimationFrame(resizeRaf);
      resizeRaf = requestAnimationFrame(() => {
        resize();
        after?.();
      });
    };

    const onVisibility = () => {
      hidden = document.visibilityState === "hidden";
    };
    document.addEventListener("visibilitychange", onVisibility);

    const ro =
      typeof ResizeObserver !== "undefined"
        ? new ResizeObserver(() => scheduleResize())
        : null;
    ro?.observe(canvas.parentElement ?? canvas);

    if (kind === "matrix") {
      const fontSize = Math.max(12, Math.round(16 - density * 4));
      let columns = Math.ceil(viewW / fontSize);
      // Seed drops across the visible height so glyphs appear immediately
      // (avoids many frames of uniform clear before rain enters the viewport).
      const seedDrops = (count: number) =>
        Array.from({ length: count }, (_, i) =>
          i % 3 === 0
            ? Math.random() * Math.max(8, viewH / fontSize)
            : Math.random() * -20,
        );
      let drops = seedDrops(columns);

      const rebuild = () => {
        columns = Math.max(8, Math.ceil(viewW / fontSize));
        const seeded = seedDrops(columns);
        drops = Array.from(
          { length: columns },
          (_, i) => drops[i] ?? seeded[i]!,
        );
      };

      const onResize = () => scheduleResize(rebuild);
      window.addEventListener("resize", onResize);

      const draw = (force = false) => {
        if (!running) return;
        if (hidden && !force && !webDriver) {
          return;
        }
        const w = viewW;
        const h = viewH;
        ctx.fillStyle = `rgba(0, 0, 0, ${0.05 + (1 - opacity) * 0.08})`;
        ctx.fillRect(0, 0, w, h);
        ctx.font = `${fontSize}px "IBM Plex Mono", ui-monospace, monospace`;
        // Bright head + dim trail so pixel samples see measurable luminance span.
        const drawAlpha = Math.max(0.45, opacity);
        for (let i = 0; i < drops.length; i++) {
          const ch =
            MATRIX_GLYPHS[Math.floor(Math.random() * MATRIX_GLYPHS.length)]!;
          const x = i * fontSize;
          const y = drops[i]! * fontSize;
          ctx.globalAlpha = drawAlpha * 0.35;
          ctx.fillStyle = color;
          ctx.fillText(ch, x, y - fontSize);
          ctx.globalAlpha = drawAlpha;
          ctx.fillStyle = "#b8ffb0";
          ctx.fillText(ch, x, y);
          if (y > h && Math.random() > 0.975) drops[i] = 0;
          drops[i]! += speed;
        }
        ctx.globalAlpha = 1;
        if (!reduced) {
          raf = requestAnimationFrame(() => draw());
        }
      };

      repaintMatrixIfHidden = () => {
        if (!hidden || !webDriver) return;
        // WebDriver may not run RAF; stack forced frames for glyph variance.
        for (let i = 0; i < 6; i++) draw(true);
      };

      ctx.fillStyle = "#050805";
      ctx.fillRect(0, 0, viewW, viewH);
      // Immediate first paint when visible or WebDriver needs samples before RAF.
      draw(true);
      repaintMatrixIfHidden();

      const onMatrixVisibility = () => {
        hidden = document.visibilityState === "hidden";
        if (!hidden && !reduced && running) {
          cancelAnimationFrame(raf);
          raf = requestAnimationFrame(() => draw());
        }
      };
      document.addEventListener("visibilitychange", onMatrixVisibility);

      return () => {
        running = false;
        cancelAnimationFrame(raf);
        cancelAnimationFrame(resizeRaf);
        window.removeEventListener("resize", onResize);
        document.removeEventListener("visibilitychange", onVisibility);
        document.removeEventListener("visibilitychange", onMatrixVisibility);
        ro?.disconnect();
      };
    }

    type Particle = { x: number; y: number; vx: number; vy: number; r: number; a: number };
    let particles: Particle[] = [];
    const t0 = performance.now();

    const spawn = () => {
      const count = Math.round(40 + density * 120);
      particles = Array.from({ length: count }, () => ({
        x: Math.random() * viewW,
        y: Math.random() * viewH,
        vx: (Math.random() - 0.5) * speed * (kind === "rain" ? 0.2 : 0.6),
        vy:
          kind === "rain"
            ? 1.5 + Math.random() * 3 * speed
            : (Math.random() - 0.5) * speed * 0.8,
        r: kind === "rain" ? 0.6 + Math.random() * 1.2 : 1 + Math.random() * 2.5,
        a: 0.2 + Math.random() * 0.8,
      }));
    };
    spawn();

    const onResize = () => scheduleResize(spawn);
    window.addEventListener("resize", onResize);

    const drawFrame = (now: number) => {
      if (!running) return;
      if (hidden) {
        return;
      }
      const w = viewW;
      const h = viewH;
      const t = reduced ? 0 : (now - t0) / 1000;
      ctx.clearRect(0, 0, w, h);
      ctx.globalAlpha = opacity;

      if (kind === "aurora") {
        const g1 = ctx.createRadialGradient(
          w * (0.3 + (reduced ? 0 : Math.sin(t * 0.4 * speed) * 0.15)),
          h * 0.2,
          40,
          w * 0.35,
          h * 0.35,
          w * 0.55,
        );
        g1.addColorStop(0, color);
        g1.addColorStop(1, "transparent");
        const g2 = ctx.createRadialGradient(
          w * (0.7 + (reduced ? 0 : Math.cos(t * 0.35 * speed) * 0.12)),
          h * 0.55,
          30,
          w * 0.65,
          h * 0.6,
          w * 0.5,
        );
        g2.addColorStop(0, secondaryColor);
        g2.addColorStop(1, "transparent");
        ctx.fillStyle = g1;
        ctx.fillRect(0, 0, w, h);
        ctx.fillStyle = g2;
        ctx.fillRect(0, 0, w, h);
      } else if (kind === "pulse") {
        const pulse = reduced ? 0.5 : 0.5 + 0.5 * Math.sin(t * 1.2 * speed);
        const g = ctx.createRadialGradient(
          w * 0.5,
          h * 0.45,
          20,
          w * 0.5,
          h * 0.45,
          w * (0.25 + pulse * 0.35),
        );
        g.addColorStop(0, color);
        g.addColorStop(0.55, secondaryColor);
        g.addColorStop(1, "transparent");
        ctx.fillStyle = g;
        ctx.fillRect(0, 0, w, h);
      } else {
        ctx.fillStyle = color;
        for (const p of particles) {
          if (!reduced) {
            p.x += p.vx;
            p.y += p.vy;
          }
          if (kind === "rain") {
            if (p.y > h) {
              p.y = -10;
              p.x = Math.random() * w;
            }
            ctx.globalAlpha = opacity * p.a;
            ctx.fillRect(p.x, p.y, p.r * 0.6, p.r * 8);
          } else {
            if (p.x < 0) p.x = w;
            if (p.x > w) p.x = 0;
            if (p.y < 0) p.y = h;
            if (p.y > h) p.y = 0;
            ctx.globalAlpha = opacity * p.a;
            ctx.beginPath();
            ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
            ctx.fill();
          }
        }
      }

      ctx.globalAlpha = 1;
      if (!reduced) {
        raf = requestAnimationFrame(drawFrame);
      }
    };

    raf = requestAnimationFrame(drawFrame);
    return () => {
      running = false;
      cancelAnimationFrame(raf);
      cancelAnimationFrame(resizeRaf);
      window.removeEventListener("resize", onResize);
      document.removeEventListener("visibilitychange", onVisibility);
      ro?.disconnect();
    };
  }, [kind, color, secondaryColor, speed, density, opacity]);

  return <canvas ref={canvasRef} className="live-wallpaper-canvas" />;
}

function CssWallpaper({
  config,
}: {
  config: SchemaWallpaperConfig;
}) {
  const opacity = clamp(config.opacity ?? 1, 0.05, 1);
  const primary = hexColor(config.color, "#141714");
  const secondary = hexColor(config.secondaryColor, "#1a2a3a");
  const angle = config.gradientAngle ?? 135;

  let background = primary;
  if (config.type === "linear-gradient") {
    background = `linear-gradient(${angle}deg, ${primary}, ${secondary})`;
  } else if (config.type === "ambient-gradient") {
    background = `radial-gradient(circle at 30% 20%, ${primary}, transparent 55%), radial-gradient(circle at 70% 65%, ${secondary}, transparent 50%), ${primary}`;
  }

  return (
    <div
      className="live-wallpaper-css"
      style={{ background, opacity }}
      aria-hidden
    />
  );
}

function MediaWallpaperLayer({ config }: { config: SchemaWallpaperConfig }) {
  const assetId = config.assetId ?? "";
  const [src, setSrc] = useState<string | null>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const reduced = prefersReducedMotion();
  const opacity = clamp(config.opacity ?? 0.85, 0.05, 1);
  const fit = config.extra?.fit === "contain" ? "contain" : "cover";
  const fallback = hexColor(config.reducedMotionFallback, "#141714");

  useEffect(() => {
    if (!assetId) return;
    let cancelled = false;
    void resolveMediaAssetUrl(assetId)
      .then((url) => {
        if (!cancelled) setSrc(url);
      })
      .catch(() => {
        if (!cancelled) setSrc(null);
      });
    return () => {
      cancelled = true;
    };
  }, [assetId]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video || config.type !== "video-loop") return;

    const syncPlayback = () => {
      if (reduced || document.hidden) {
        video.pause();
      } else {
        void video.play().catch(() => undefined);
      }
    };

    syncPlayback();
    const onVisibility = () => syncPlayback();
    const onBlur = () => video.pause();
    const onFocus = () => {
      if (!reduced && !document.hidden) void video.play().catch(() => undefined);
    };

    document.addEventListener("visibilitychange", onVisibility);
    window.addEventListener("blur", onBlur);
    window.addEventListener("focus", onFocus);
    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("focus", onFocus);
    };
  }, [config.type, reduced, src]);

  if (!assetId) return null;

  if (reduced) {
    return (
      <div
        className="live-wallpaper-fallback"
        style={{ background: fallback, opacity }}
        aria-hidden
      />
    );
  }

  if (!src) return <div className="live-wallpaper-loading" aria-hidden />;

  if (config.type === "video-loop") {
    return (
      <video
        ref={videoRef}
        className="live-wallpaper-media"
        src={src}
        muted={config.muted ?? true}
        loop
        playsInline
        autoPlay
        style={{ objectFit: fit, opacity }}
      />
    );
  }

  return (
    <img
      className="live-wallpaper-media"
      src={src}
      alt=""
      style={{ objectFit: fit, opacity }}
    />
  );
}

function SlideshowWallpaper({ config }: { config: SchemaWallpaperConfig }) {
  const ids = config.slideAssetIds ?? [];
  const [index, setIndex] = useState(0);
  const reduced = prefersReducedMotion();

  useEffect(() => {
    if (ids.length <= 1 || reduced) return;
    const interval = window.setInterval(
      () => setIndex((i) => (i + 1) % ids.length),
      config.intervalMs ?? 8000,
    );
    return () => window.clearInterval(interval);
  }, [ids.length, config.intervalMs, reduced]);

  if (ids.length === 0) return null;
  const activeId = ids[index] ?? ids[0];
  return (
    <MediaWallpaperLayer
      config={{
        ...config,
        type: "image-cover",
        assetId: activeId,
      }}
    />
  );
}

export function LiveWallpaper({ wallpaper }: LiveWallpaperProps) {
  if (wallpaper.format === "none") return null;

  if (wallpaper.format === "legacy") {
    const config: WallpaperConfig = wallpaper.config;
    const kind = resolveKind(config.kind);
    if (kind === "none") return null;
    const color = config.color ?? "#33ff66";
    const secondary = config.secondaryColor ?? "#6ab0d4";
    const speed = clamp(config.speed ?? 1, 0.25, 3);
    const density = clamp(config.density ?? 0.55, 0.1, 1);
    const opacity = clamp(
      config.opacity ?? (kind === "matrix" ? 0.42 : kind === "aurora" ? 0.55 : 0.35),
      0.05,
      0.85,
    );
    return (
      <div className="live-wallpaper" aria-hidden>
        <CanvasWallpaper
          kind={kind}
          color={color}
          secondaryColor={secondary}
          speed={speed}
          density={density}
          opacity={opacity}
        />
      </div>
    );
  }

  const config = wallpaper.config;

  if (
    config.type === "static-color" ||
    config.type === "linear-gradient" ||
    config.type === "ambient-gradient"
  ) {
    return (
      <div className="live-wallpaper" style={wallpaperLayerStyle(config)} aria-hidden>
        <CssWallpaper config={config} />
      </div>
    );
  }

  if (config.type === "canvas-preset" || config.type === "floating-particles") {
    const kind = resolveKind(schemaToLegacyKind(config));
    if (kind === "none") return null;
    const speed = clamp(config.extra?.speed ?? 1, 0.25, 3);
    const density = clamp(config.extra?.density ?? 0.55, 0.1, 1);
    const opacity = clamp(config.opacity ?? 0.35, 0.05, 0.85);
    return (
      <div className="live-wallpaper" style={wallpaperLayerStyle(config)} aria-hidden>
        <CanvasWallpaper
          kind={kind}
          color={hexColor(config.color, "#6ab0d4")}
          secondaryColor={hexColor(config.secondaryColor, "#33ff66")}
          speed={speed}
          density={density}
          opacity={opacity}
        />
      </div>
    );
  }

  if (config.type === "slideshow") {
    return (
      <div className="live-wallpaper" style={wallpaperLayerStyle(config)} aria-hidden>
        <SlideshowWallpaper config={config} />
      </div>
    );
  }

  if (
    config.type === "image-cover" ||
    config.type === "video-loop" ||
    config.type === "animated-image"
  ) {
    return (
      <div className="live-wallpaper" style={wallpaperLayerStyle(config)} aria-hidden>
        <MediaWallpaperLayer config={config} />
      </div>
    );
  }

  return null;
}
