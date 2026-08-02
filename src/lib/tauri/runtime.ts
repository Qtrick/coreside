/** True when running inside the Tauri desktop webview. */
export function isTauriRuntime(): boolean {
  return (
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
  );
}

/** Explicit browser/Vite preview (no desktop shell). Never enable via query params. */
export function isWebPreview(): boolean {
  return !isTauriRuntime();
}
