import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/lib/tauri";
import type { AgentTurnEvent } from "@/lib/tauri";

/**
 * Eavesdropping denial (doc/unit note): tool-window listeners must ignore
 * text/action/error/operation on the global bus. Interactive sends deliver
 * those kinds on Channel instead — see docs/SCOPED_TURN_STREAMING.md.
 */
describe("scoped turn streaming (mock send path)", () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it("delivers text on the send_message onEvent callback (Channel stand-in)", async () => {
    const conversation = await api.createConversation();
    const events: AgentTurnEvent[] = [];

    const result = await api.sendMessage({
      conversationId: conversation.id,
      content: "hello",
      onEvent: (event) => {
        events.push(event);
      },
    });

    expect(result.assistantMessage.length).toBeGreaterThan(0);
    expect(events.some((e) => e.kind === "text")).toBe(true);
    const textEvent = events.find((e) => e.kind === "text");
    expect(textEvent).toMatchObject({
      kind: "text",
      conversationId: conversation.id,
      text: result.assistantMessage,
    });
    expect(typeof (textEvent && textEvent.kind === "text" ? textEvent.turnId : null)).toBe(
      "string",
    );
  });

  it("documents that Sync/Conflict remain on the global listenAgentTurn path", async () => {
    // attachAgentTurnSyncListener in app-store ignores private kinds; Channel
    // handlers ignore sync/conflict. This keeps multi-window refresh working
    // without putting assistant text on the process-wide bus.
    const { listenAgentTurn } = await import("@/lib/tauri/events");
    const stop = await listenAgentTurn(() => undefined);
    stop();
    expect(typeof listenAgentTurn).toBe("function");
  });

  it("emits operation preview before text for progressive op preview fixture", async () => {
    const conversation = await api.createConversation();
    const events: AgentTurnEvent[] = [];

    await api.sendMessage({
      conversationId: conversation.id,
      content: "please run progressive op preview now",
      onEvent: (event) => {
        events.push(event);
      },
    });

    const previewIdx = events.findIndex(
      (e) => e.kind === "operation" && e.status === "preview",
    );
    const textIdx = events.findIndex((e) => e.kind === "text");
    expect(previewIdx).toBeGreaterThanOrEqual(0);
    expect(textIdx).toBeGreaterThan(previewIdx);
    expect(events[previewIdx]).toMatchObject({
      kind: "operation",
      operationId: "op-progressive-preview",
      status: "preview",
    });
  });
});
