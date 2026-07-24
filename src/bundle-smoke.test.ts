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
    const selection = readFileSync(
      "src-tauri/src/platform/macos_selection.rs",
      "utf8"
    );

    expect(geometry).toContain("AXBoundsForRange");
    expect(geometry).toContain('"AXFrame"');
    expect(geometry).toContain('"AXPosition"');
    expect(geometry).toContain('"AXSize"');
    expect(geometry).toContain("CGEventGetLocation");
    expect(geometry).not.toContain("objc2_app_kit");
    expect(geometry).not.toContain("NSEvent");
    expect(selection).toContain("macos_geometry::resolve");
    expect(selection).toContain("with_geometry_source");
  });

  it("keeps text-marker capture read-only on public AX APIs with owned values", () => {
    const accessibility = [
      "macos_accessibility",
      "macos_ax",
      "macos_selection",
      "macos_text_marker"
    ]
      .map((module) =>
        readFileSync(`src-tauri/src/platform/${module}.rs`, "utf8")
      )
      .join("\n");
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
    expect(restore).toContain(".writable");
    expect(restore).toContain("expected.element_identity.as_ref()");
    expect(restore).toContain('role == "AXSecureTextField"');
  });

  it("keeps fallback, mutation, bounds, and diagnostics fail-closed", () => {
    const replacement = readFileSync(
      "src-tauri/src/platform/macos_replace.rs",
      "utf8"
    );
    const selection = readFileSync(
      "src-tauri/src/platform/macos_selection.rs",
      "utf8"
    );
    const classicRange = readFileSync(
      "src-tauri/src/platform/macos_classic_range.rs",
      "utf8"
    );
    const geometry = readFileSync(
      "src-tauri/src/platform/macos_geometry.rs",
      "utf8"
    );
    const diagnostics = readFileSync(
      "src-tauri/src/diagnostics.rs",
      "utf8"
    );

    expect(selection).toMatch(
      /Err\(range_failure\)[\s\S]*marker_eligible_after_range\(range_failure\)[\s\S]*marker_selection\(element\)/
    );
    expect(selection).not.toContain("cf_range_selection(element).or_else");
    expect(classicRange).toContain(
      "marker_fallback_rejects_structural_and_cross_stage_failures"
    );

    const eligibilityCheck = replacement.indexOf("validate_expected(expected)?");
    const focusedElementLookup = replacement.indexOf(
      "macos_ax::focused_element_for_pid(expected.pid)",
      eligibilityCheck
    );
    const recapture = replacement.indexOf(
      "macos_selection::capture(&element)?",
      focusedElementLookup
    );
    const setter = replacement.indexOf(
      "macos_ax::set_selected_text(element.as_ref(), text)",
      recapture
    );
    expect(eligibilityCheck).toBeGreaterThan(-1);
    expect(focusedElementLookup).toBeGreaterThan(eligibilityCheck);
    expect(recapture).toBeGreaterThan(focusedElementLookup);
    expect(setter).toBeGreaterThan(recapture);

    const rectDecoder = geometry.slice(
      geometry.indexOf("pub(super) fn rect_from_value"),
      geometry.indexOf("fn read_point")
    );
    expect(rectDecoder.indexOf("has_ax_value_type")).toBeLessThan(
      rectDecoder.indexOf("AXValueGetValue")
    );

    const snapshotMetadata = diagnostics.slice(
      diagnostics.indexOf("fn snapshot_metadata"),
      diagnostics.indexOf("fn lifecycle_metadata")
    );
    expect(snapshotMetadata).not.toMatch(
      /snapshot\.pid|snapshot\.range|snapshot\.bounds/
    );
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

  it("keeps the runtime composition root within the file-size gate", () => {
    const runtime = readFileSync("src-tauri/src/lib.rs", "utf8");
    const state = readFileSync("src-tauri/src/runtime.rs", "utf8");

    expect(runtime.split("\n").length).toBeLessThanOrEqual(301);
    expect(state).toContain("pub(crate) struct AppRuntime");
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
    const commands = [
      readFileSync("src-tauri/src/commands.rs", "utf8"),
      readFileSync("src-tauri/src/commands_transform.rs", "utf8")
    ].join("\n");
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

    expect(dispatcher).toContain(
      "get_or_create(app, surface, 420.0, 220.0, sequence, readiness)"
    );
    expect(overlay).toContain('"Ação necessária"');
    expect(overlay).toContain("Abrir Verbalix");
    expect(commands).toContain('show_main_window(&app, "login_required")');
    expect(commands).toContain('ai_readiness("provider_unavailable")');
    expect(commands).toContain(".show_error(");
    expect(commands).toContain("begin_transform(snapshot.id, request.request_id)");
    expect(commands).toContain("abort_transform(request.request_id)");
    expect(overlay).not.toContain("native.aiReadiness()");
    expect(overlay).not.toMatch(/catch[\s\S]{0,80}openMainWindow/);
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

  it("inserts successful transformations into synchronized history", () => {
    const command = readFileSync(
      "src-tauri/src/commands_transform.rs",
      "utf8"
    );
    const transform = command.indexOf(".coordinator\n        .transform(");
    const success = command.indexOf(".await?;", transform);
    const historyGate = command.indexOf("if settings.history_enabled", success);
    const insert = command.indexOf(".insert(request, &response", historyGate);

    expect(transform).toBeGreaterThan(-1);
    expect(success).toBeGreaterThan(transform);
    expect(historyGate).toBeGreaterThan(success);
    expect(insert).toBeGreaterThan(historyGate);
  });
});
