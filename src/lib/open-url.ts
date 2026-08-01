import { api } from "@/lib/tauri";

/**
 * Open an external http(s) URL via the trusted Rust command.
 * Frontend must not call the shell plugin or window.open for untrusted URLs —
 * SSRF and scheme checks live in Rust.
 */
export async function openExternalUrl(url: string): Promise<void> {
  const trimmed = url.trim();
  if (!trimmed) return;
  await api.openExternalUrl(trimmed);
}
