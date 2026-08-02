/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_APP_TITLE?: string;
  readonly VITE_SUPABASE_URL?: string;
  readonly VITE_SUPABASE_PUBLISHABLE_KEY?: string;
  /** Set to "1" only for desktop E2E builds that embed @wdio/tauri-plugin. */
  readonly VITE_E2E?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

// Optional E2E-only dependency — not installed for normal app builds.
declare module "@wdio/tauri-plugin";
