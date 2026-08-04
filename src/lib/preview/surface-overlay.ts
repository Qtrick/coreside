/**
 * Speculative preview surface overlay — Channel `previewSurface` events paint
 * ToolCanvas before durable Sync/commit. Not persisted.
 */

import type { ToolDefinition } from "@/types/tool";
import { validateToolDefinition } from "@/lib/tool-schema";

export type PreviewSurfaceEventPayload = {
  conversationId: string;
  turnId: string;
  toolId?: string | null;
  surfaceId: string;
  applicationId?: string | null;
  definitionJson: unknown;
  stateJson: unknown;
  revision: number;
  sequence: number;
};

export type PreviewSurfaceOverlay = {
  conversationId: string;
  turnId: string;
  toolId: string | null;
  surfaceId: string;
  applicationId: string | null;
  tool: ToolDefinition;
  state: Record<string, unknown>;
  revision: number;
  sequence: number;
};

/** Key for overlay map: prefer toolId, else surfaceId. */
export function previewOverlayKey(input: {
  toolId?: string | null;
  surfaceId: string;
}): string {
  const toolId = (input.toolId ?? "").trim();
  if (toolId) return toolId;
  return input.surfaceId.trim();
}

export function applyPreviewSurfaceOverlay(
  current: Record<string, PreviewSurfaceOverlay>,
  event: PreviewSurfaceEventPayload,
): Record<string, PreviewSurfaceOverlay> {
  const conversationId = (event.conversationId ?? "").trim();
  if (!conversationId) return current;

  const validated = validateToolDefinition(event.definitionJson);
  if (!validated.success) {
    return current;
  }
  const key = previewOverlayKey({
    toolId: event.toolId ?? validated.data.id,
    surfaceId: event.surfaceId,
  });
  if (!key) return current;

  const prev = current[key];
  // Ignore stale/out-of-order paint frames for the same key + turn.
  if (prev && prev.sequence >= event.sequence && prev.turnId === event.turnId) {
    return current;
  }

  const state =
    event.stateJson &&
    typeof event.stateJson === "object" &&
    !Array.isArray(event.stateJson)
      ? (event.stateJson as Record<string, unknown>)
      : {};

  return {
    ...current,
    [key]: {
      conversationId,
      turnId: event.turnId,
      toolId: event.toolId ?? validated.data.id,
      surfaceId: event.surfaceId,
      applicationId: event.applicationId ?? event.toolId ?? validated.data.id,
      tool: validated.data,
      state,
      revision: event.revision,
      sequence: event.sequence,
    },
  };
}

export function clearPreviewSurfaceOverlays(
  current: Record<string, PreviewSurfaceOverlay>,
): Record<string, PreviewSurfaceOverlay> {
  if (Object.keys(current).length === 0) return current;
  return {};
}

/** Drop overlays belonging to one conversation (interrupt / error / turn end). */
export function clearPreviewOverlaysForConversation(
  current: Record<string, PreviewSurfaceOverlay>,
  conversationId: string | null | undefined,
): Record<string, PreviewSurfaceOverlay> {
  const conv = (conversationId ?? "").trim();
  if (!conv || Object.keys(current).length === 0) return current;
  let changed = false;
  const next: Record<string, PreviewSurfaceOverlay> = {};
  for (const [key, overlay] of Object.entries(current)) {
    if (overlay.conversationId === conv) {
      changed = true;
      continue;
    }
    next[key] = overlay;
  }
  return changed ? next : current;
}

/**
 * Clear overlays after Sync / durable commit.
 *
 * Prefer tool/surface targeting when present. Otherwise clear by conversationId.
 * Never wipe the entire map on empty targeting — that leaked clears across chats.
 */
export function clearPreviewOverlaysMatching(
  current: Record<string, PreviewSurfaceOverlay>,
  match: {
    conversationId?: string | null;
    toolIds?: string[];
    surfaceIds?: string[];
  },
): Record<string, PreviewSurfaceOverlay> {
  const toolIds = new Set((match.toolIds ?? []).map((s) => s.trim()).filter(Boolean));
  const surfaceIds = new Set(
    (match.surfaceIds ?? []).map((s) => s.trim()).filter(Boolean),
  );
  const conversationId = (match.conversationId ?? "").trim();

  if (toolIds.size === 0 && surfaceIds.size === 0) {
    if (conversationId) {
      return clearPreviewOverlaysForConversation(current, conversationId);
    }
    // Unscoped Sync must not clear speculative paint for unrelated chats.
    return current;
  }

  let changed = false;
  const next: Record<string, PreviewSurfaceOverlay> = {};
  for (const [key, overlay] of Object.entries(current)) {
    const hit =
      (overlay.toolId && toolIds.has(overlay.toolId)) ||
      toolIds.has(key) ||
      surfaceIds.has(overlay.surfaceId);
    if (hit) {
      changed = true;
      continue;
    }
    next[key] = overlay;
  }
  return changed ? next : current;
}

/**
 * Resolve overlay for the active tool. Requires matching conversation so a
 * background chat cannot paint over another conversation's open app.
 */
export function getPreviewOverlayForTool(
  overlays: Record<string, PreviewSurfaceOverlay>,
  toolId: string | null | undefined,
  conversationId?: string | null,
): PreviewSurfaceOverlay | null {
  const id = (toolId ?? "").trim();
  if (!id) return null;
  const overlay = overlays[id] ?? overlays[`surf-${id}`] ?? null;
  if (!overlay) return null;
  const conv = (conversationId ?? "").trim();
  if (conv && overlay.conversationId !== conv) return null;
  return overlay;
}
