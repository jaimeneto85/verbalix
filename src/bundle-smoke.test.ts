import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

type TauriConfiguration = {
  identifier: string;
  build: { frontendDist: string };
  bundle: {
    active: boolean;
    targets: string[];
    macOS: { minimumSystemVersion: string };
  };
};

describe("macOS bundle smoke contract", () => {
  it("packages the production frontend for the supported macOS baseline", () => {
    const configuration = JSON.parse(
      readFileSync("src-tauri/tauri.conf.json", "utf8")
    ) as TauriConfiguration;

    expect(configuration.identifier).toBe("com.verbalix.desktop");
    expect(configuration.build.frontendDist).toBe("../dist");
    expect(configuration.bundle.active).toBe(true);
    expect(configuration.bundle.targets).toContain("app");
    expect(configuration.bundle.macOS.minimumSystemVersion).toBe("14.0");
  });

  it("declares the required desktop capability scope", () => {
    const capabilities = JSON.parse(
      readFileSync("src-tauri/capabilities/default.json", "utf8")
    ) as { windows: string[]; permissions: string[] };

    expect(capabilities.windows).toEqual(
      expect.arrayContaining(["main", "toolbar", "note"])
    );
    expect(capabilities.permissions).toContain("core:window:default");
  });
});
