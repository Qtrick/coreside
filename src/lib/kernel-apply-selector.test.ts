import { describe, expect, it } from "vitest";
import { KERNEL_APPLY_SELECTOR } from "./kernel-apply-selector";

describe("kernel Apply selector contract", () => {
  it("matches data-proposal-apply and not preview btn-primary", () => {
    const root = document.createElement("div");
    root.innerHTML = `
      <aside class="change-proposal" aria-label="Proposed change">
        <div class="tool-preview">
          <button type="button" class="btn btn-primary">Add Task</button>
        </div>
        <div class="button-row">
          <button
            type="button"
            class="btn btn-primary"
            data-proposal-apply="true"
            aria-label="Apply proposed change"
          >Apply</button>
        </div>
      </aside>
    `;

    const apply = root.querySelector(KERNEL_APPLY_SELECTOR);
    expect(apply).not.toBeNull();
    expect(apply?.getAttribute("aria-label")).toBe("Apply proposed change");
    expect(apply?.textContent?.trim()).toBe("Apply");

    const previewPrimary = root.querySelector(".tool-preview .btn-primary");
    expect(previewPrimary).not.toBeNull();
    expect(previewPrimary).not.toBe(apply);
    expect(previewPrimary?.hasAttribute("data-proposal-apply")).toBe(false);

    // Bare .btn-primary under the proposal is ambiguous (preview + Apply).
    const allPrimary = root.querySelectorAll(
      '.change-proposal[aria-label="Proposed change"] .btn-primary',
    );
    expect(allPrimary.length).toBe(2);
  });

  it("does not match when only preview primary exists", () => {
    const root = document.createElement("div");
    root.innerHTML = `
      <aside class="change-proposal" aria-label="Proposed change">
        <button type="button" class="btn btn-primary">Add Task</button>
      </aside>
    `;
    expect(root.querySelector(KERNEL_APPLY_SELECTOR)).toBeNull();
  });
});
