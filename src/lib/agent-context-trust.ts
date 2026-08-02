/**
 * Prompt-injection / trust-boundary fixtures.
 * Defense-in-depth: untrusted content must not be treated as authority.
 */

export type ContextTrust =
  | "trusted_system"
  | "trusted_user_instruction"
  | "trusted_project_instruction"
  | "untrusted_web_search"
  | "untrusted_crawled_page"
  | "untrusted_tool_output"
  | "untrusted_project_retrieval";

export type ContextSegment = {
  trust: ContextTrust;
  sourceId: string;
  content: string;
  mayContainInstructions: boolean;
};

const PRIVILEGED: ReadonlySet<ContextTrust> = new Set([
  "trusted_system",
  "trusted_user_instruction",
  "trusted_project_instruction",
]);

/** External / tool / retrieved content is never privileged authority. */
export function isPrivilegedTrust(trust: ContextTrust): boolean {
  return PRIVILEGED.has(trust);
}

/**
 * Wrap untrusted content so delimiter-like text cannot close a privileged
 * section. Callers must still keep this out of system-instruction slots.
 */
export function envelopeUntrustedContent(
  trust: Exclude<
    ContextTrust,
    "trusted_system" | "trusted_user_instruction" | "trusted_project_instruction"
  >,
  sourceId: string,
  content: string,
): ContextSegment {
  const escaped = content
    .split("")
    .filter((ch) => ch.charCodeAt(0) !== 0)
    .join("")
    .replace(/<\/?system>/gi, "[filtered]")
    .slice(0, 50_000);
  return {
    trust,
    sourceId,
    content: escaped,
    mayContainInstructions: false,
  };
}

/** Detect common injection phrases for diagnostics only — not a security gate. */
export function looksLikeInjectionAttempt(text: string): boolean {
  const lower = text.toLowerCase();
  return (
    lower.includes("ignore previous instructions") ||
    lower.includes("ignore all instructions") ||
    lower.includes("bypass approval") ||
    lower.includes("export all chats") ||
    lower.includes("reveal the system prompt") ||
    lower.includes("shell.exec")
  );
}

/**
 * Whether a segment may authorize privileged tool selection.
 * Only trusted instruction classes may.
 */
export function mayAuthorizePrivilegedActions(segment: ContextSegment): boolean {
  return isPrivilegedTrust(segment.trust) && segment.mayContainInstructions;
}
