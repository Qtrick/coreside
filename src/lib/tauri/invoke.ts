import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { TauriCommandError, formatInvokeError } from "./errors";
import { isTauriRuntime } from "./runtime";

/**
 * Trusted desktop IPC. When Tauri is present, never falls back to mock persistence.
 * Outside Tauri (web preview / vitest), loads mocks via dynamic import so production
 * Tauri bundles can tree-shake the mock implementation out of the main chunk.
 */
export async function invoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (isTauriRuntime()) {
    try {
      return await tauriInvoke<T>(command, args);
    } catch (error) {
      const { message, code } = formatInvokeError(error);
      throw new TauriCommandError(message, code);
    }
  }

  const { mockInvoke } = await import("./mocks");
  return mockInvoke<T>(command, args);
}
