//! Detect `@tool` mentions without treating email addresses as triggers.

export type MentionQuery = {
  /** Absolute start index of `@` in the full text. */
  start: number;
  /** End index of the current query (caret). */
  end: number;
  /** Text after `@` used for filtering. */
  query: string;
};

export type ToolMention = {
  mentionId: string;
  toolId: string;
  label: string;
  start: number;
  end: number;
};

/** True when `@` at `atIndex` is a mention trigger (start of text/word, not email). */
export function isMentionTrigger(text: string, atIndex: number): boolean {
  if (atIndex < 0 || atIndex >= text.length || text[atIndex] !== "@") {
    return false;
  }
  if (atIndex === 0) return true;
  const prev = text[atIndex - 1];
  // Email-like: word-char immediately before `@`
  if (/[A-Za-z0-9._%]/.test(prev)) return false;
  return /\s/.test(prev) || prev === "(" || prev === "[" || prev === "{";
}

/** Active mention query at caret, or null. */
export function getActiveMentionQuery(
  text: string,
  caret: number,
): MentionQuery | null {
  if (caret < 0 || caret > text.length) return null;
  const before = text.slice(0, caret);
  const at = before.lastIndexOf("@");
  if (at < 0) return null;
  if (!isMentionTrigger(text, at)) return null;
  const query = before.slice(at + 1);
  // Stop if whitespace / newline after `@` query started poorly
  if (/[\s\n]/.test(query)) return null;
  return { start: at, end: caret, query };
}

export function filterToolsByMentionQuery<
  T extends { id: string; name: string; description?: string | null },
>(tools: T[], query: string, recentIds: string[] = []): T[] {
  const q = query.trim().toLowerCase();
  const scored = tools.map((tool) => {
    const name = tool.name.toLowerCase();
    const desc = (tool.description ?? "").toLowerCase();
    let score = 0;
    if (!q) {
      score = 1;
    } else if (name.startsWith(q)) {
      score = 100;
    } else if (name.includes(q)) {
      score = 60;
    } else if (desc.includes(q)) {
      score = 30;
    } else {
      return null;
    }
    const recentBoost = recentIds.indexOf(tool.id);
    if (recentBoost >= 0) score += 20 - Math.min(recentBoost, 10);
    return { tool, score };
  });
  return scored
    .filter((x): x is { tool: T; score: number } => x !== null)
    .sort((a, b) => b.score - a.score || a.tool.name.localeCompare(b.tool.name))
    .map((x) => x.tool);
}

/** Replace `@query` range with `@Label` and return structured mention. */
export function applyMentionInsertion(
  text: string,
  range: { start: number; end: number },
  tool: { id: string; name: string },
): { text: string; mention: ToolMention; caret: number } {
  const label = tool.name.trim() || "Tool";
  const token = `@${label}`;
  const next = `${text.slice(0, range.start)}${token}${text.slice(range.end)}`;
  const mention: ToolMention = {
    mentionId: `m-${tool.id}-${range.start}`,
    toolId: tool.id,
    label,
    start: range.start,
    end: range.start + token.length,
  };
  return { text: next, mention, caret: mention.end };
}

/** Resolve `@Label` tokens against known tools (longest name first). */
export function resolveMentionsInText(
  text: string,
  tools: Array<{ id: string; name: string }>,
): ToolMention[] {
  const sorted = [...tools].sort((a, b) => b.name.length - a.name.length);
  const mentions: ToolMention[] = [];
  let i = 0;
  while (i < text.length) {
    if (text[i] === "@" && isMentionTrigger(text, i)) {
      let matched: { id: string; name: string } | null = null;
      for (const tool of sorted) {
        const candidate = `@${tool.name}`;
        if (text.slice(i, i + candidate.length) === candidate) {
          const after = text[i + candidate.length];
          if (after === undefined || /[\s,.!?;:)\]}]/.test(after)) {
            matched = tool;
            break;
          }
        }
      }
      if (matched) {
        const token = `@${matched.name}`;
        mentions.push({
          mentionId: `m-${matched.id}-${i}`,
          toolId: matched.id,
          label: matched.name,
          start: i,
          end: i + token.length,
        });
        i += token.length;
        continue;
      }
    }
    i += 1;
  }
  return mentions;
}

/**
 * Text spans win; pending menu selections overlay stable toolIds when labels/positions match.
 */
export function mergeResolvedMentions(
  resolved: ToolMention[],
  pending: ToolMention[],
): ToolMention[] {
  if (pending.length === 0) return resolved;
  const usedPending = new Set<number>();
  return resolved.map((r) => {
    let matchIdx = pending.findIndex(
      (p, idx) =>
        !usedPending.has(idx) &&
        p.start === r.start &&
        p.label.toLowerCase() === r.label.toLowerCase(),
    );
    if (matchIdx < 0) {
      matchIdx = pending.findIndex(
        (p, idx) =>
          !usedPending.has(idx) &&
          p.label.toLowerCase() === r.label.toLowerCase(),
      );
    }
    if (matchIdx < 0) return r;
    usedPending.add(matchIdx);
    const match = pending[matchIdx];
    return {
      ...r,
      toolId: match.toolId,
      mentionId: `m-${match.toolId}-${r.start}`,
    };
  });
}
