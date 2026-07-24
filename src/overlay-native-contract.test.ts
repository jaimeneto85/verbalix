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
    expect(panel).toContain("zero_screen_max_y");
    expect(panel).toContain("firstObject()");
    expect(panel).not.toContain("mainScreen");
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

  it("keeps a new overlay hidden until the document readiness handshake", () => {
    const dispatcher = readFileSync(
      "src-tauri/src/platform/overlay_dispatcher.rs",
      "utf8"
    );
    const initialToolbar = dispatcher.slice(
      dispatcher.indexOf("OverlayCommand::ShowToolbar"),
      dispatcher.indexOf("OverlayCommand::ShowResult")
    );
    const ready = dispatcher.slice(
      dispatcher.indexOf("async fn surface_ready"),
      dispatcher.indexOf("impl OverlayCommand")
    );
    const readyStart = dispatcher.indexOf("fn execute_surface_ready");
    const readyExecution = dispatcher.slice(
      readyStart,
      dispatcher.indexOf('#[cfg(target_os = "macos")]', readyStart)
    );

    expect(initialToolbar).toContain("show_if_ready");
    expect(initialToolbar).not.toContain("show_and_confirm");
    expect(ready).toContain("tokio::sync::oneshot::channel");
    expect(ready).toContain("receiver.await");
    expect(ready).not.toContain("show_and_confirm");
    expect(readyExecution).toContain("mark_ready");
    expect(readyExecution).toContain("generation");
    expect(readyExecution).toContain("stale_surface");
    expect(readyExecution).toContain("show_if_ready");
    expect(readyExecution).toContain("Ok(true)");
  });

  it("binds a readiness ack to the exact caller and Rust-issued generation", () => {
    const command = readFileSync(
      "src-tauri/src/overlay_commands.rs",
      "utf8"
    );
    const window = readFileSync(
      "src-tauri/src/platform/overlay_window.rs",
      "utf8"
    );

    expect(command).toContain("window: WebviewWindow");
    expect(command).toContain("generation: uuid::Uuid");
    expect(command).toContain("is_current_caller");
    expect(window).toContain("begin_document");
    expect(window).toContain("&generation=");
    expect(window).toContain("caller.ns_view()");
    expect(window).toContain("current.ns_view()");
    expect(window).toContain("PageLoadEvent::Started");
    expect(window).toContain("reload_readiness.invalidate_document");
    expect(window).toContain("window.destroy()");
    expect(window).toContain("reload_invalidation_failed");
    expect(window).toContain("reload_destroy_failed");
    expect(window).toContain("reload_hide_failed");
    expect(window).toContain("invalid_window_found");
  });
});
