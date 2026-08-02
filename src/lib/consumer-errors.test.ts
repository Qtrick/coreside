import { describe, expect, it } from "vitest";
import { consumerErrorMessage } from "./consumer-errors";

describe("consumerErrorMessage", () => {
  it("rewrites wallpaper schemaVersion noise", () => {
    const { message, technical } = consumerErrorMessage(
      'wallpaper JSON invalid: missing field `schemaVersion` at line 1 column 15',
    );
    expect(message).toMatch(/could not apply that wallpaper/i);
    expect(technical).toMatch(/schemaVersion/);
  });
});
