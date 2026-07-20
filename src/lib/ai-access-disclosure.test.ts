import { describe, expect, it } from "vitest";

/**
 * Mirrors Rust `resolve_access_presentation` disclosure for developer_environment.
 * Keep in sync with src-tauri/src/ai/access_mode.rs
 */
function disclosureFor(
  source: "connection" | "env" | "none",
  hasKey: boolean,
  developerMode: boolean,
) {
  if (source === "connection" && hasKey) {
    return { showProviderIdentity: true, showModelIdentity: true };
  }
  if (source === "env" && hasKey) {
    return {
      showProviderIdentity: developerMode,
      showModelIdentity: developerMode,
      consumerDisplayName: "AI Access",
    };
  }
  return { showProviderIdentity: false, showModelIdentity: false };
}

describe("AI access disclosure", () => {
  it("hides OpenRouter/model for env credentials without Developer Mode", () => {
    const d = disclosureFor("env", true, false);
    expect(d.showProviderIdentity).toBe(false);
    expect(d.showModelIdentity).toBe(false);
    expect(d.consumerDisplayName).toBe("AI Access");
  });

  it("shows provider for BYOK", () => {
    const d = disclosureFor("connection", true, false);
    expect(d.showProviderIdentity).toBe(true);
    expect(d.showModelIdentity).toBe(true);
  });

  it("reveals env details only in Developer Mode", () => {
    const d = disclosureFor("env", true, true);
    expect(d.showProviderIdentity).toBe(true);
  });
});
