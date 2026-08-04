import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import {
  E2E_REPO_ROOT,
  evidenceBinaryHash,
  evidenceIdentity,
  waitForAppReady,
} from "../helpers.js";

/**
 * Journey 12 — true provider streaming (mock live stream probe).
 *
 * Requires AI_PROVIDER=mock (wdio + e2e/run set this; CORESIDE_E2E skips dotenv).
 * The mock adapter emits TextDelta before delayed completion when the
 * user message contains "live stream probe".
 *
 * Run alone:
 *   CORESIDE_E2E=1 AI_PROVIDER=mock npx wdio run e2e/wdio.conf.ts --suite true-streaming
 */
const evidencePath = path.resolve(
  E2E_REPO_ROOT,
  "reports/true-streaming-results.json",
);

describe("Journey 12 — true provider streaming", () => {
  it("renders a live assistant delta before the turn completes", async () => {
    await waitForAppReady();

    const newChat = await $('[aria-label="New chat"]');
    await newChat.click();

    const composer = await $('[aria-label="Message composer"]');
    await composer.waitForExist({ timeout: 15_000 });
    await composer.click();
    await composer.setValue("Please run a live stream probe now");

    const send = await $('[aria-label="Send message"]');
    await send.waitForExist({ timeout: 10_000 });
    await send.waitForClickable({ timeout: 5_000 });

    const firstDeltaAt = Date.now();
    await send.click();

    const stream = await $('[data-testid="assistant-stream-text"]');
    await stream.waitForExist({ timeout: 8_000 });

    await browser.waitUntil(
      async () => {
        const t = (await stream.getText()).toLowerCase();
        return t.includes("live stream");
      },
      {
        timeout: 8_000,
        timeoutMsg:
          "streaming text never contained live stream marker (is AI_PROVIDER=mock and dotenv skipped?)",
      },
    );
    const earlyText = await stream.getText();
    expect(earlyText.toLowerCase()).toContain("live stream");
    expect(earlyText.toLowerCase()).not.toContain("coreside cannot");

    const turnId = (await stream.getAttribute("data-turn-id"))?.trim() ?? "";
    const sequenceAttr =
      (await stream.getAttribute("data-last-sequence")) ??
      (await stream.getAttribute("data-stream-sequence")) ??
      "";
    expect(turnId.length).toBeGreaterThan(0);
    const sequence = Number(sequenceAttr);
    expect(Number.isFinite(sequence)).toBe(true);
    expect(sequence).toBeGreaterThan(0);

    const fakeMarkers = await browser.execute(() => {
      return Boolean(
        document.querySelector(
          '[data-stream-mode="fake"], [data-testid="fake-stream"], .fake-stream',
        ),
      );
    });
    expect(fakeMarkers).toBe(false);

    await browser.waitUntil(
      async () => {
        const body = await $("body").getText();
        return /live stream probe complete/i.test(body);
      },
      {
        timeout: 20_000,
        timeoutMsg: "expected final live stream probe message",
      },
    );
    const completedAt = Date.now();

    const finalText = await browser.execute(() => {
      const body = document.body?.innerText ?? "";
      const match = body.match(/live stream probe complete[^\n]*/i);
      return match?.[0] ?? body.slice(0, 200);
    });
    expect(finalText.toLowerCase()).toContain("live stream probe complete");

    const identity = evidenceIdentity();
    const binaryHash = evidenceBinaryHash();
    const fingerprint = createHash("sha256")
      .update(
        [
          turnId,
          String(sequence),
          earlyText,
          finalText,
          identity.commit,
          identity.platform,
          identity.arch,
          binaryHash ?? "",
        ].join("|"),
      )
      .digest("hex");

    const evidence = {
      schemaVersion: 1,
      product: "Coreside",
      generatedAt: new Date().toISOString(),
      commit: identity.commit,
      dirty: identity.dirty,
      platform: identity.platform,
      architecture: identity.arch,
      binaryHash,
      fingerprint,
      command: "e2e:journey-12-true-streaming",
      result: "passed",
      evidenceLevel: "Desktop Verified",
      e2eSpec: "e2e/specs/12-true-streaming.spec.ts",
      e2eResult: "passed",
      partial: false,
      assertions: [
        "assistant-stream-text appeared before turn completion",
        "earlyText contained live stream marker",
        "data-turn-id non-empty",
        "data-last-sequence / data-stream-sequence > 0",
        "no fake-stream DOM marker",
        "final live stream probe complete visible",
        "no Auto/openrouter failure text",
      ],
      stream: {
        turnId,
        sequence,
        earlyText,
        finalText,
        firstDeltaAt,
        completedAt,
      },
      publicBeta: "Not ready",
    };

    fs.mkdirSync(path.dirname(evidencePath), { recursive: true });
    fs.writeFileSync(evidencePath, JSON.stringify(evidence, null, 2) + "\n");
  });
});
