import {
  closeToolCanvas,
  denyPendingApprovalIfPresent,
  E2E_NOTE_INPUT_ID,
  E2E_TOOL_ID,
  openPersonalTool,
  requireExistingSeed,
  waitForAppReady,
  waitForSeededTool,
  waitForToolCanvas,
} from "../helpers.js";

describe("Journey 5 — generated tool open + state persist", () => {
  it("opens the seeded tool and persists text input state across close/reopen", async () => {
    requireExistingSeed("Journey 5");

    await waitForAppReady();
    await denyPendingApprovalIfPresent();
    await waitForSeededTool();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    // The shell owns horizontal scrolling. Generated content and the header
    // must reflow inside this pane rather than widening the desktop window.
    const layout = await browser.execute(() => {
      const root = document.documentElement;
      const main = document.querySelector<HTMLElement>(".app-shell-main");
      const canvas = document.querySelector<HTMLElement>(".tool-canvas");
      const body = document.querySelector<HTMLElement>(".tool-canvas-body");
      const close = document.querySelector<HTMLElement>(
        '[aria-label="Close tool canvas"], [aria-label="Back to chat"]',
      );

      const offenders: Array<Record<string, unknown>> = [];
      if (main && main.scrollWidth > main.clientWidth + 2) {
        const mainRect = main.getBoundingClientRect();
        for (const node of Array.from(main.querySelectorAll<HTMLElement>("*"))) {
          const rect = node.getBoundingClientRect();
          if (rect.width <= 0 || rect.height <= 0) continue;
          const pastRight = rect.right - (mainRect.right + 1);
          const pastLeft = mainRect.left - 1 - rect.left;
          if (pastRight <= 0 && pastLeft <= 0) continue;
          const style = getComputedStyle(node);
          offenders.push({
            tag: node.tagName.toLowerCase(),
            className: String(node.className || "").slice(0, 120),
            id: node.id || null,
            pastRight: Math.round(pastRight),
            pastLeft: Math.round(pastLeft),
            width: Math.round(rect.width),
            scrollWidth: node.scrollWidth,
            clientWidth: node.clientWidth,
            minWidth: style.minWidth,
            overflowX: style.overflowX,
            position: style.position,
          });
        }
        offenders.sort(
          (a, b) =>
            Number(b.pastRight) +
            Number(b.pastLeft) -
            (Number(a.pastRight) + Number(a.pastLeft)),
        );
      }

      return {
        rootOverflow: root.scrollWidth > root.clientWidth + 2,
        mainOverflow: Boolean(main && main.scrollWidth > main.clientWidth + 2),
        canvasOverflow: Boolean(
          canvas && canvas.scrollWidth > canvas.clientWidth + 2,
        ),
        bodyOverflow: Boolean(body && body.scrollWidth > body.clientWidth + 2),
        closeVisible: Boolean(
          close &&
            close.getBoundingClientRect().left >= 0 &&
            close.getBoundingClientRect().right <= window.innerWidth + 1,
        ),
        mainClientWidth: main?.clientWidth ?? null,
        mainScrollWidth: main?.scrollWidth ?? null,
        windowInnerWidth: window.innerWidth,
        offenders: offenders.slice(0, 8),
      };
    });
    expect(layout).toMatchObject({
      rootOverflow: false,
      mainOverflow: false,
      canvasOverflow: false,
      bodyOverflow: false,
      closeVisible: true,
    });

    const input = await $(`#${E2E_NOTE_INPUT_ID}`);
    await input.waitForExist({ timeout: 10_000 });
    await input.setValue("E2E persisted note");

    await closeToolCanvas();
    await openPersonalTool();
    await waitForToolCanvas(E2E_TOOL_ID);

    const reopened = await $(`#${E2E_NOTE_INPUT_ID}`);
    await expect(reopened).toHaveValue("E2E persisted note");
  });
});
