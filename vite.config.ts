import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "node:path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
    },
  },
  clearScreen: false,
  // Pre-bundle the chat shell deps so `tauri dev` / `dev:web` cold starts skip discovery.
  optimizeDeps: {
    include: [
      "react",
      "react-dom",
      "react/jsx-runtime",
      "zustand",
      "zod",
      "lucide-react",
      "@tauri-apps/api",
      "@tauri-apps/api/core",
      "@tauri-apps/api/event",
      "@tauri-apps/plugin-shell",
      "react-markdown",
      "remark-gfm",
    ],
  },
  server: {
    port: 1422,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**", "**/.reference/**"],
    },
    warmup: {
      clientFiles: ["./src/main.tsx", "./src/app/App.tsx"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    // Report timing in CI logs without changing emit.
    reportCompressedSize: false,
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./tests/setup.ts"],
    exclude: [
      "**/node_modules/**",
      "**/dist/**",
      "**/.reference/**",
      "**/src-tauri/**",
      "**/e2e/**",
      // Extracted research archives must never enter the product Vitest graph.
      "**/reports/.artifacts/**",
      // Node built-in test runner (npm run test:dev-dispatcher), not Vitest.
      "**/scripts/**/*.test.mjs",
    ],
  },
});
