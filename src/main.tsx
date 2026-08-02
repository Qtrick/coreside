import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "@/app/App";
import "@/styles/global.css";

async function boot() {
  // Tree-shaken out of production builds unless VITE_E2E=1.
  if (import.meta.env.VITE_E2E === "1") {
    await import("@wdio/tauri-plugin");
  }

  const root = document.getElementById("root");
  if (!root) {
    throw new Error("Root element #root not found");
  }

  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}

void boot();
