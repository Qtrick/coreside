import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

/** Cheap brace-balance gate so orphaned CSS declarations fail in unit CI. */
describe("global.css token contract", () => {
  it("parses with balanced braces (no orphan declarations near wallpaper filters)", () => {
    const css = fs.readFileSync(path.join(root, "src/styles/global.css"), "utf8");
    let depth = 0;
    for (const ch of css) {
      if (ch === "{") depth += 1;
      if (ch === "}") depth -= 1;
      expect(depth).toBeGreaterThanOrEqual(0);
    }
    expect(depth).toBe(0);
    expect(css).toMatch(
      /\.wallpaper-filter-presets\s*\{[^}]*display:\s*flex;[^}]*\}/,
    );
  });
});
