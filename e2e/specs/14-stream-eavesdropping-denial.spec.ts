import fs from "node:fs";
import path from "node:path";
import {
  denyPendingApprovalIfPresent,
  E2E_REPO_ROOT,
  E2E_TOOL_ID,
  evidenceIdentity,
  listTauriWindows,
  openPersonalTool,
  openToolInWindow,
  requireExistingSeed,
  switchTauriWindow,
  waitForAppReady,
  waitForSeededTool,
  waitForToolCanvas,
} from "../helpers.js";

/**
 * Journey 14 — tool-window must not receive global agent-turn Text.
 *
 * Text is Channel-scoped (never on the process-wide bus). Sync/Conflict still
 * use app.emit("agent-turn"), so tool-window.json keeps core:event:default —
 * removing it would break Sync. This journey proves Text privacy: the tool
 * window may listen, but must receive zero Text payloads during a live stream.
 *
 * Run alone:
 *   CORESIDE_E2E_SEED=existing npx wdio run e2e/wdio.conf.ts --suite existing-eavesdrop
 */
const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/stream-eavesdropping-results.json",
);

type CollectedEvent = {
  kind?: string;
  text?: string;
  conversationId?: string;
};

describe("Journey 14 — stream eavesdropping denial", () => {
  it("tool window receives zero Text events while main streams", async () => {
    requireExistingSeed("Journey 14");

    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await waitForSeededTool();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);
    await openToolInWindow();

    const label = `tool-${E2E_TOOL_ID}`;
    await browser.waitUntil(
      async () => (await listTauriWindows()).includes(label),
      {
        timeout: 20_000,
        timeoutMsg: `Secondary window ${label} never appeared`,
      },
    );

    await switchTauriWindow(label);

    const listenSetup = await browser.execute(() => {
      const w = window as Window & {
        __e2eAgentTurnEvents?: CollectedEvent[];
        __e2eAgentTurnListenOk?: boolean;
        __e2eAgentTurnListenError?: string;
        __TAURI__?: {
          event?: {
            listen?: (
              event: string,
              handler: (event: { payload: CollectedEvent }) => void,
            ) => Promise<unknown>;
          };
        };
      };
      w.__e2eAgentTurnEvents = [];
      w.__e2eAgentTurnListenOk = false;
      w.__e2eAgentTurnListenError = undefined;
      const listen = w.__TAURI__?.event?.listen;
      if (!listen) {
        w.__e2eAgentTurnListenError = "__TAURI__.event.listen unavailable";
        return {
          ok: false as const,
          error: w.__e2eAgentTurnListenError,
        };
      }
      return listen("agent-turn", (event) => {
        w.__e2eAgentTurnEvents = w.__e2eAgentTurnEvents ?? [];
        w.__e2eAgentTurnEvents.push(event.payload ?? {});
      }).then(
        () => {
          w.__e2eAgentTurnListenOk = true;
          return { ok: true as const };
        },
        (err: unknown) => {
          const message =
            err instanceof Error ? err.message : String(err);
          w.__e2eAgentTurnListenError = message;
          return { ok: false as const, error: message };
        },
      );
    });

    expect(listenSetup.ok).toBe(true);

    await switchTauriWindow("main");

    const newChat = await $('[aria-label="New chat"]');
    await newChat.click();

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue("Please run a live stream probe now");

    const send = await $('[aria-label="Send message"]');
    await send.waitForExist({ timeout: 10_000 });
    await send.waitForClickable({ timeout: 5_000 });
    await send.click();

    const stream = await $('[data-testid="assistant-stream-text"]');
    await stream.waitForExist({ timeout: 10_000 });
    const earlyText = await stream.getText();
    expect(earlyText.toLowerCase()).toContain("live stream");

    // Collect for several seconds while the stream is live.
    await browser.pause(4_000);

    await browser.waitUntil(
      async () => {
        const body = await $("body").getText();
        return /live stream probe complete/i.test(body);
      },
      {
        timeout: 20_000,
        timeoutMsg: "expected final live stream probe message on main",
      },
    );

    await switchTauriWindow(label);
    // Brief settle so any late global emits would land.
    await browser.pause(500);

    const collected = await browser.execute(() => {
      const w = window as Window & {
        __e2eAgentTurnEvents?: CollectedEvent[];
        __e2eAgentTurnListenOk?: boolean;
        __e2eAgentTurnListenError?: string;
      };
      return {
        listenOk: Boolean(w.__e2eAgentTurnListenOk),
        listenError: w.__e2eAgentTurnListenError ?? null,
        events: w.__e2eAgentTurnEvents ?? [],
      };
    });

    expect(collected.listenOk).toBe(true);

    const textEvents = collected.events.filter((e) => {
      const kind = (e.kind ?? "").toLowerCase();
      if (kind === "text") return true;
      // Fail closed if assistant conversation content appears under any kind.
      const text = typeof e.text === "string" ? e.text : "";
      return /live stream/i.test(text);
    });

    // FAIL if Text arrives — that would prove the P1 eavesdropping hole.
    // PASS when Text is never globally emitted (Channel-only) even though
    // core:event:default remains for Sync/Conflict.
    expect(textEvents.length).toBe(0);

    const identity = evidenceIdentity();
    const evidence = {
      schemaVersion: 1,
      product: "Coreside",
      generatedAt: new Date().toISOString(),
      commit: identity.commit,
      dirty: identity.dirty,
      platform: identity.platform,
      architecture: identity.arch,
      command: "e2e:journey-14-stream-eavesdropping-denial",
      result: "passed",
      evidenceLevel: "Desktop Verified",
      e2eSpec: "e2e/specs/14-stream-eavesdropping-denial.spec.ts",
      window: label,
      documentation: {
        coreEventDefaultKept: true,
        reason:
          "Sync/Conflict still use app.emit('agent-turn'); removing core:event:default would break tool Sync. Text is Channel-only and must never appear on the global bus.",
        proofMode:
          "tool window listened on agent-turn during main live stream; zero Text payloads expected and asserted",
      },
      listenOk: collected.listenOk,
      eventsReceived: collected.events.length,
      textEventsReceived: textEvents.length,
      nonTextKinds: [
        ...new Set(
          collected.events
            .map((e) => e.kind)
            .filter(
              (k): k is string =>
                typeof k === "string" && k.toLowerCase() !== "text",
            ),
        ),
      ],
      assertions: [
        "tool window can listen for agent-turn (core:event:default present)",
        "zero Text payloads with assistant conversation content",
        "main window completed live stream probe",
      ],
      publicBeta: "Not ready",
    };

    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2) + "\n");
  });
});
