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

  it("keeps selection geometry on thread-safe AX and Core Graphics boundaries", () => {
    const geometry = readFileSync(
      "src-tauri/src/platform/macos_geometry.rs",
      "utf8"
    );
    const accessibility = readFileSync(
      "src-tauri/src/platform/macos_accessibility.rs",
      "utf8"
    );

    expect(geometry).toContain("AXBoundsForRange");
    expect(geometry).toContain('"AXFrame"');
    expect(geometry).toContain('"AXPosition"');
    expect(geometry).toContain('"AXSize"');
    expect(geometry).toContain("CGEventGetLocation");
    expect(geometry).not.toContain("objc2_app_kit");
    expect(geometry).not.toContain("NSEvent");
    expect(accessibility).toContain("macos_geometry::resolve");
    expect(accessibility).toContain("with_geometry_source");
  });

  it("keeps text-marker capture read-only on public AX APIs with owned values", () => {
    const accessibility = readFileSync(
      "src-tauri/src/platform/macos_accessibility.rs",
      "utf8"
    );
    const restore = readFileSync(
      "src-tauri/src/platform/macos_restore.rs",
      "utf8"
    );

    expect(accessibility).toContain("AXUIElementCreateSystemWide");
    expect(accessibility).toContain('"AXFocusedUIElement"');
    expect(accessibility).toContain("AXTextMarkerRangeGetTypeID");
    expect(accessibility).toContain('"AXStringForTextMarkerRange"');
    expect(accessibility).toContain('"AXBoundsForTextMarkerRange"');
    expect(accessibility).toContain("AXTextMarkerRangeCopyStartMarker");
    expect(accessibility).toContain("AXTextMarkerRangeCopyEndMarker");
    expect(accessibility).toContain('"AXIndexForTextMarker"');
    expect(accessibility).toContain('"AXLengthForTextMarkerRange"');
    expect(accessibility).toMatch(
      /geometry_source:\s*GeometrySource::TextMarkerRange,\s*writable:\s*false/
    );
    expect(accessibility).toContain("impl Drop for OwnedAxElement");
    expect(accessibility).toContain("impl Drop for OwnedCfValue");
    expect(accessibility).not.toContain("AXFocusedApplication");
    expect(accessibility).not.toContain("copy_selection_preserving_clipboard");
    expect(restore).toContain("if !expected.writable");
    expect(restore).toContain('role == "AXSecureTextField"');
  });

  it("preserves the visible Regular lifecycle and close-reopen paths", () => {
    const runtime = readFileSync("src-tauri/src/lib.rs", "utf8");

    expect(runtime).toContain("ActivationPolicy::Regular");
    expect(runtime).toContain("WindowEvent::CloseRequested");
    expect(runtime).toContain("api.prevent_close()");
    expect(runtime).toContain("window.hide()");
    expect(runtime).toContain("RunEvent::Reopen");
    expect(runtime).toContain('show_main_window(app, "dock_reopen")');
  });

  it("uses a full note for actionable errors and a single public config policy", () => {
    const dispatcher = readFileSync(
      "src-tauri/src/platform/overlay_dispatcher.rs",
      "utf8"
    );
    const overlay = readFileSync("src/Overlay.tsx", "utf8");
    const config = readFileSync(
      "src-tauri/src/application/ai_readiness.rs",
      "utf8"
    );
    const buildScript = readFileSync("src-tauri/build.rs", "utf8");
    const commands = readFileSync("src-tauri/src/commands.rs", "utf8");
    const example = readFileSync(".env.example", "utf8");
    const gitignore = readFileSync(".gitignore", "utf8");
    const publicRuntime = [
      dispatcher,
      overlay,
      config,
      buildScript,
      commands,
      readFileSync("src/supabase.ts", "utf8")
    ].join("\n");

    expect(dispatcher).toContain('window(app, "note", 420.0, 220.0');
    expect(overlay).toContain('"Ação necessária"');
    expect(overlay).toContain("Abrir Verbalix");
    expect(commands).toContain('show_main_window(&app, "login_required")');
    expect(commands).toContain('ai_readiness("provider_unavailable")');
    expect(commands).toContain(".show_error(");
    expect(config).toContain('include!(concat!(env!("OUT_DIR")');
    expect(config).toContain(
      'process_pair("VITE_SUPABASE_URL", "VITE_SUPABASE_ANON_KEY")'
    );
    expect(config).toContain(
      'process_pair("VERBALIX_SUPABASE_URL", "VERBALIX_SUPABASE_ANON_KEY")'
    );
    expect(config).toContain("std::env::var(url_name)");
    expect(buildScript).toContain("dotenvy::from_path_iter");
    expect(buildScript).toContain("cargo:rerun-if-changed=../.env");
    expect(buildScript).toContain("cargo:rerun-if-env-changed={name}");
    expect(buildScript).toContain("verbalix_backend_config.rs");
    expect(buildScript).toContain("fs::write(output, contents)");
    expect(buildScript).not.toContain("cargo:rustc-env");
    expect(buildScript).not.toMatch(/cargo:warning|dbg!|eprintln!/);
    expect(example.trim().split("\n")).toEqual([
      "VITE_SUPABASE_URL=",
      "VITE_SUPABASE_ANON_KEY="
    ]);
    expect(gitignore.split("\n")).toContain(".env");
    expect(publicRuntime).not.toMatch(/OPENAI_API_KEY|SERVICE_ROLE/i);
  });
});
