import { describe, expect, it } from "vitest";
import { verifySurfaceElement } from "@/lib/visual-verification";

describe("verifySurfaceElement", () => {
  it("flags unlabeled buttons", () => {
    const root = document.createElement("div");
    root.style.width = "400px";
    const btn = document.createElement("button");
    root.appendChild(btn);
    document.body.appendChild(root);
    const result = verifySurfaceElement(root, 400);
    const labels = result.checks.find((c) => c.id === "labels_present");
    expect(labels?.status).toBe("fail");
    document.body.removeChild(root);
  });

  it("passes labeled controls", () => {
    const root = document.createElement("div");
    const btn = document.createElement("button");
    btn.setAttribute("aria-label", "Save");
    btn.style.width = "48px";
    btn.style.height = "48px";
    root.appendChild(btn);
    document.body.appendChild(root);
    const result = verifySurfaceElement(root, 800);
    expect(result.implemented).toBe(true);
    expect(result.checks.find((c) => c.id === "labels_present")?.status).toBe(
      "pass",
    );
    document.body.removeChild(root);
  });
});
