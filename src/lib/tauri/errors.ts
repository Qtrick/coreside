export class TauriCommandError extends Error {
  readonly code?: string;

  constructor(message: string, code?: string) {
    super(message);
    this.name = "TauriCommandError";
    this.code = code;
  }
}

export function formatInvokeError(error: unknown): { message: string; code?: string } {
  if (typeof error === "string") {
    return { message: error || "Command failed" };
  }
  if (error instanceof Error) {
    return { message: error.message || "Command failed" };
  }
  if (error && typeof error === "object") {
    const obj = error as Record<string, unknown>;
    const code = typeof obj.code === "string" ? obj.code : undefined;
    if (typeof obj.message === "string" && obj.message.trim()) {
      return { message: obj.message, code };
    }
    // Tauri sometimes nests the payload.
    const nested = obj.error;
    if (nested && typeof nested === "object") {
      const inner = nested as Record<string, unknown>;
      const nestedCode =
        typeof inner.code === "string" ? inner.code : code;
      if (typeof inner.message === "string" && inner.message.trim()) {
        return { message: inner.message, code: nestedCode };
      }
    }
    try {
      return { message: JSON.stringify(error), code };
    } catch {
      return { message: "Command failed", code };
    }
  }
  return { message: "Command failed" };
}
