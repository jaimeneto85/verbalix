import { describe, expect, it } from "vitest";
import { defaultSettings } from "./types";

describe("default settings", () => {
  it("starts with the technical balanced profile", () => {
    expect(defaultSettings).toMatchObject({
      formality: 3,
      length: "balanced",
      tone: "technical",
      confirmBeforeReplace: false
    });
  });

  it("does not retain selected text in settings", () => {
    expect(Object.keys(defaultSettings)).not.toContain("text");
  });
});
