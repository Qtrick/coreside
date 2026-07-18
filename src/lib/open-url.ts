import { open } from "@tauri-apps/plugin-shell";

export async function openExternalUrl(url: string): Promise<void> {
  const trimmed = url.trim();
  if (!trimmed.startsWith("http://") && !trimmed.startsWith("https://")) {
    return;
  }
  try {
    await open(trimmed);
  } catch {
    window.open(trimmed, "_blank", "noopener,noreferrer");
  }
}
