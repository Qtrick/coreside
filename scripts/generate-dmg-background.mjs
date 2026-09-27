#!/usr/bin/env node
/**
 * Generate Coreside macOS DMG installer background.
 *
 * Source: design/branding/dmg-background.svg
 * Output: src-tauri/resources/dmg-background.png (1320x800 at 144 DPI for 660x400 window)
 *
 * Design tokens strictly match Coreside dark theme (tokens.css):
 * - Background: #141714
 * - Emerald accent: #69c994 / #2f8f63
 * - Warm accent: #e0a158
 * - Typography: SF Pro / -apple-system
 */

import { execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");

const svgPath = path.join(root, "design/branding/dmg-background.svg");
const pngPath = path.join(root, "src-tauri/resources/dmg-background.png");

// Logical window dimensions
const width = 660;
const height = 400;

// Coordinates
const appX = 180;
const appY = 190;
const appsX = 480;
const appsY = 190;

const svgContent = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}" width="${width}" height="${height}">
  <defs>
    <!-- Background Gradient -->
    <radialGradient id="ambientGlow" cx="50%" cy="40%" r="55%">
      <stop offset="0%" stop-color="#1e2920" stop-opacity="0.9" />
      <stop offset="50%" stop-color="#161c16" stop-opacity="0.8" />
      <stop offset="100%" stop-color="#141714" stop-opacity="1" />
    </radialGradient>

    <!-- Emerald Arrow Gradient -->
    <linearGradient id="arrowGrad" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#2f8f63" stop-opacity="0.3" />
      <stop offset="50%" stop-color="#69c994" stop-opacity="0.85" />
      <stop offset="100%" stop-color="#69c994" stop-opacity="1" />
    </linearGradient>

    <!-- Warm Accent Glow -->
    <linearGradient id="trackGrad" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#2f8f63" stop-opacity="0.2" />
      <stop offset="70%" stop-color="#69c994" stop-opacity="0.6" />
      <stop offset="100%" stop-color="#e0a158" stop-opacity="0.8" />
    </linearGradient>

    <!-- Soft blur filter -->
    <filter id="softGlow" x="-30%" y="-30%" width="160%" height="160%">
      <feGaussianBlur stdDeviation="6" result="blur" />
      <feComposite in="SourceGraphic" in2="blur" operator="over" />
    </filter>
  </defs>

  <!-- Base background -->
  <rect width="${width}" height="${height}" fill="#141714" />
  <rect width="${width}" height="${height}" fill="url(#ambientGlow)" />

  <!-- Subtle constellation / orbital grid lines (low opacity, refined) -->
  <g stroke="#ffffff" stroke-opacity="0.03" stroke-width="1">
    <line x1="0" y1="80" x2="${width}" y2="80" />
    <line x1="0" y1="190" x2="${width}" y2="190" stroke-dasharray="4 8" />
    <line x1="0" y1="300" x2="${width}" y2="300" />
    <line x1="180" y1="0" x2="180" y2="${height}" stroke-dasharray="4 8" />
    <line x1="480" y1="0" x2="480" y2="${height}" stroke-dasharray="4 8" />
  </g>

  <!-- Orbital connector arc between stations -->
  <path d="M 180 190 Q 330 140 480 190" fill="none" stroke="url(#trackGrad)" stroke-width="1.5" stroke-dasharray="3 6" opacity="0.45" />

  <!-- Icon Anchors / Pedestal Rings -->
  <!-- App Icon Anchor -->
  <g transform="translate(${appX}, ${appY})">
    <circle r="74" fill="none" stroke="#69c994" stroke-opacity="0.12" stroke-width="1.5" />
    <circle r="78" fill="none" stroke="#2f8f63" stroke-opacity="0.06" stroke-width="1" stroke-dasharray="2 4" />
    <circle r="6" fill="#69c994" fill-opacity="0.2" />
  </g>

  <!-- Applications Folder Anchor -->
  <g transform="translate(${appsX}, ${appsY})">
    <circle r="74" fill="none" stroke="#e0a158" stroke-opacity="0.12" stroke-width="1.5" />
    <circle r="78" fill="none" stroke="#e0a158" stroke-opacity="0.06" stroke-width="1" stroke-dasharray="2 4" />
    <circle r="6" fill="#e0a158" fill-opacity="0.2" />
  </g>

  <!-- Directional Flow Track and Arrow -->
  <!-- Track line with glowing gradient -->
  <g>
    <!-- Background glow for the arrow shaft -->
    <line x1="268" y1="190" x2="388" y2="190" stroke="#69c994" stroke-width="4" opacity="0.15" filter="url(#softGlow)" />
    
    <!-- Dotted progress lead-in -->
    <circle cx="268" cy="190" r="2.5" fill="#69c994" opacity="0.35" />
    <circle cx="280" cy="190" r="2.5" fill="#69c994" opacity="0.45" />
    <circle cx="292" cy="190" r="2.5" fill="#69c994" opacity="0.6" />
    
    <!-- Main solid arrow shaft -->
    <line x1="302" y1="190" x2="388" y2="190" stroke="url(#arrowGrad)" stroke-width="2.5" stroke-linecap="round" />

    <!-- Chevron Arrowhead -->
    <path d="M 378 181 L 391 190 L 378 199" fill="none" stroke="#69c994" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />

    <!-- Subtle arrow tip aura -->
    <circle cx="391" cy="190" r="8" fill="#69c994" opacity="0.12" filter="url(#softGlow)" />
  </g>

  <!-- Instruction Typography -->
  <g text-anchor="middle" font-family="-apple-system, BlinkMacSystemFont, 'SF Pro Display', 'DM Sans', sans-serif">
    <!-- Header title -->
    <text x="330" y="58" font-size="20" font-weight="600" fill="#eef2ec" letter-spacing="-0.015em">
      Drag Coreside to Applications
    </text>

    <!-- Subtitle -->
    <text x="330" y="82" font-size="13" font-weight="400" fill="#a6afa3" letter-spacing="-0.005em">
      to install in your personal software environment
    </text>

    <!-- Subtle Coreside Watermark at Bottom -->
    <text x="330" y="372" font-size="11" font-weight="500" fill="#69c994" fill-opacity="0.4" letter-spacing="0.08em">
      CORESIDE
    </text>
  </g>
</svg>
`;

fs.mkdirSync(path.dirname(svgPath), { recursive: true });
fs.mkdirSync(path.dirname(pngPath), { recursive: true });

fs.writeFileSync(svgPath, svgContent.trim() + "\n", "utf8");
console.log(`[dmg] wrote vector source: ${path.relative(root, svgPath)}`);

// Render to Retina 2x PNG (1320x800) with 144 DPI
const retinaWidth = width * 2;
execSync(`sips -s format png "${svgPath}" --resampleWidth ${retinaWidth} --out "${pngPath}"`);
execSync(`sips -s dpiWidth 144 -s dpiHeight 144 "${pngPath}"`);

const stat = fs.statSync(pngPath);
console.log(`[dmg] generated Retina PNG: ${path.relative(root, pngPath)} (${retinaWidth}x${height * 2}, ${stat.size} bytes, 144 DPI)`);
