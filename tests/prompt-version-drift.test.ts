import { describe, expect, it } from "vitest";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

describe("Prompt version drift verification", () => {
  const CANONICAL_VERSION = "coreside-prompt-v1";
  const promptsDir = join(process.cwd(), "src-tauri", "prompts");

  it("verifies all prompt files specify canonical coreside-prompt-v1", () => {
    const files = readdirSync(promptsDir).filter((f) => f.endsWith(".md"));
    expect(files.length).toBeGreaterThan(0);

    for (const file of files) {
      const content = readFileSync(join(promptsDir, file), "utf-8");
      // Must not reference an unanchored prompt version like coreside-prompt-v2
      expect(content).not.toMatch(/coreside-prompt-v[2-9]/);
    }
  });

  it("verifies system prompt explicitly anchors canonical version", () => {
    const systemMd = readFileSync(join(promptsDir, "system.md"), "utf-8");
    expect(systemMd).toContain(`**${CANONICAL_VERSION}**`);
  });

  it("verifies Rust prompt_builder constant matches canonical version", () => {
    const promptBuilderRs = readFileSync(
      join(process.cwd(), "src-tauri", "src", "ai", "prompt_builder.rs"),
      "utf-8",
    );
    expect(promptBuilderRs).toContain(
      `pub const PROMPT_VERSION: &str = "${CANONICAL_VERSION}";`,
    );
  });
});
