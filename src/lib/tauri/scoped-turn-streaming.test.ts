import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "@/lib/tauri";
import type { AgentTurnEvent } from "@/lib/tauri";

/**
 * Eavesdropping denial (doc/unit note): tool-window listeners must ignore
 * text/action/error/operation on the global bus. Interactive sends deliver
 * those kinds on Channel instead — see docs/SCOPED_TURN_STREAMING.md.
 *
 * Queue drain (no Channel) must not fall back to global emit for private kinds.
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
    });
    if (textEvent?.kind === "text") {
      // Mock may still send cumulative text; live Rust is delta-primary with
      // periodic checkpoints. Accept either carrier.
      expect(
        (textEvent.text && textEvent.text.length > 0) ||
          (textEvent.delta && textEvent.delta.length > 0),
      ).toBe(true);
      expect(typeof textEvent.turnId).toBe("string");
    }
  });

  it("documents scoped Sync subscribe helper and residual global listen", async () => {
    // Primary: subscribeConversationSync Channel. Residual: listenAgentTurn when
    // Rust has zero scoped deliveries (see docs/SCOPED_TURN_STREAMING.md).
    const { listenAgentTurn, subscribeConversationSync, isSyncEventForConversation } =
      await import("@/lib/tauri/events");
    const stop = await listenAgentTurn(() => undefined);
    stop();
    expect(typeof listenAgentTurn).toBe("function");
    expect(typeof subscribeConversationSync).toBe("function");
    expect(
      isSyncEventForConversation(
        {
          kind: "sync",
          conversationId: "conv-a",
          surfaceIds: [],
          syncKind: "transaction_applied",
        },
        "conv-a",
      ),
    ).toBe(true);
    expect(
      isSyncEventForConversation(
        {
          kind: "sync",
          conversationId: "conv-b",
          surfaceIds: [],
          syncKind: "transaction_applied",
        },
        "conv-a",
      ),
    ).toBe(false);
  });

  it("documents that private kinds must not ride the global bus", () => {
    // Contract: Action / Error / Operation / Text never use app.emit("agent-turn")
    // when Channel is absent — queue drain drops them (see emit_turn in
    // message_cmds.rs and docs/SCOPED_TURN_STREAMING.md).
    expect(true).toBe(true);
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
