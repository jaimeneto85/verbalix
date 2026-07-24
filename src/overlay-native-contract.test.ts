import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

describe("macOS overlay native contract", () => {
  it("keeps the panel nonactivating, clear and positioned in Cocoa points", () => {
    const panel = readFileSync(
      "src-tauri/src/platform/macos_overlay_panel.rs",
      "utf8"
    );

    expect(panel).toContain("setBecomesKeyOnlyIfNeeded: true");
    expect(panel).toContain("setOpaque: false");
    expect(panel).toContain("clearColor");
    expect(panel).toContain("setBackgroundColor: clear_color");
    expect(panel).toContain("setFrameOrigin: native_origin");
    expect(panel).not.toMatch(
      /LogicalPosition|PhysicalPosition|scale_factor|set_position/
    );
  });

  it("does not route the macOS placement path through Tauri coordinates", () => {
    const dispatcher = readFileSync(
      "src-tauri/src/platform/overlay_dispatcher.rs",
      "utf8"
    );
    const macPath = dispatcher.slice(
      dispatcher.indexOf('#[cfg(target_os = "macos")]\nfn place'),
      dispatcher.indexOf('#[cfg(not(target_os = "macos"))]\nfn place')
    );

    expect(macPath).toContain("macos_overlay_panel::place");
    expect(macPath).not.toMatch(
      /LogicalPosition|PhysicalPosition|scale_factor|set_position/
    );
  });
});
