import { useEffect, useRef, useState } from "react";
import type { WallpaperConfig } from "@/types/agent";
import { WallpaperKindSchema } from "@/types/agent";
import type { ResolvedWallpaper, SchemaWallpaperConfig } from "@/types/wallpaper";
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

function resolveKind(raw: string | undefined) {
  const parsed = WallpaperKindSchema.safeParse(raw ?? "none");
  return parsed.success ? parsed.data : "none";
}

function hexColor(raw: string | null | undefined, fallback: string) {
  if (!raw) return fallback;
  return /^#([0-9a-fA-F]{6}|[0-9a-fA-F]{3})$/.test(raw) ? raw : fallback;
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

    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const w = window.innerWidth;
      const h = window.innerHeight;
      canvas.width = Math.floor(w * dpr);
      canvas.height = Math.floor(h * dpr);
      canvas.style.width = `${w}px`;
      canvas.style.height = `${h}px`;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    resize();

    if (kind === "matrix") {
      const fontSize = Math.max(12, Math.round(16 - density * 4));
      let columns = Math.ceil(window.innerWidth / fontSize);
      let drops = Array.from({ length: columns }, () => Math.random() * -40);

      const rebuild = () => {
        columns = Math.max(8, Math.ceil(window.innerWidth / fontSize));
        drops = Array.from({ length: columns }, (_, i) =>
          drops[i] ?? Math.random() * -40,
        );
      };

      const onResize = () => {
        resize();
        rebuild();
      };
      window.addEventListener("resize", onResize);

      const draw = () => {
        if (!running) return;
        const w = window.innerWidth;
        const h = window.innerHeight;
        ctx.fillStyle = `rgba(0, 0, 0, ${0.05 + (1 - opacity) * 0.08})`;
        ctx.fillRect(0, 0, w, h);
        ctx.font = `${fontSize}px "IBM Plex Mono", ui-monospace, monospace`;
        ctx.fillStyle = color;
        ctx.globalAlpha = opacity;
        for (let i = 0; i < drops.length; i++) {
          const ch =
            MATRIX_GLYPHS[Math.floor(Math.random() * MATRIX_GLYPHS.length)]!;
          const x = i * fontSize;
          const y = drops[i]! * fontSize;
          ctx.fillText(ch, x, y);
          if (y > h && Math.random() > 0.975) drops[i] = 0;
          drops[i]! += reduced ? 0.15 * speed : speed;
        }
        ctx.globalAlpha = 1;
        raf = requestAnimationFrame(draw);
      };

      ctx.fillStyle = "#050805";
      ctx.fillRect(0, 0, window.innerWidth, window.innerHeight);
      raf = requestAnimationFrame(draw);

      return () => {
        running = false;
        cancelAnimationFrame(raf);
        window.removeEventListener("resize", onResize);
      };
    }

    type Particle = { x: number; y: number; vx: number; vy: number; r: number; a: number };
    let particles: Particle[] = [];
    const t0 = performance.now();

    const spawn = () => {
      const count = Math.round(40 + density * 120);
      particles = Array.from({ length: count }, () => ({
        x: Math.random() * window.innerWidth,
        y: Math.random() * window.innerHeight,
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

    const onResize = () => {
      resize();
      spawn();
    };
    window.addEventListener("resize", onResize);

    const drawFrame = (now: number) => {
      if (!running) return;
      const w = window.innerWidth;
      const h = window.innerHeight;
      const t = (now - t0) / 1000;
      ctx.clearRect(0, 0, w, h);
      ctx.globalAlpha = opacity;

      if (kind === "aurora") {
        const g1 = ctx.createRadialGradient(
          w * (0.3 + Math.sin(t * 0.4 * speed) * 0.15),
          h * 0.2,
          40,
          w * 0.35,
          h * 0.35,
          w * 0.55,
        );
        g1.addColorStop(0, color);
        g1.addColorStop(1, "transparent");
        const g2 = ctx.createRadialGradient(
          w * (0.7 + Math.cos(t * 0.35 * speed) * 0.12),
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
        const pulse = 0.5 + 0.5 * Math.sin(t * 1.2 * speed);
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
      raf = requestAnimationFrame(drawFrame);
    };

    raf = requestAnimationFrame(drawFrame);
    return () => {
      running = false;
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", onResize);
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
      <div className="live-wallpaper" aria-hidden>
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
      <div className="live-wallpaper" aria-hidden>
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
      <div className="live-wallpaper" aria-hidden>
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
      <div className="live-wallpaper" aria-hidden>
        <MediaWallpaperLayer config={config} />
      </div>
    );
  }

  return null;
}
