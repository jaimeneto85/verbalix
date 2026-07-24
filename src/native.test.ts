import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultSettings } from "./types";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { native } from "./native";

describe("native command contract", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("requests accessibility without prompting by default", async () => {
    invoke.mockResolvedValue(true);

    await native.accessibilityStatus();

    expect(invoke).toHaveBeenCalledWith("accessibility_status", {
      prompt: false
    });
  });

  it("sends both session tokens to the Keychain boundary", async () => {
    invoke.mockResolvedValue(undefined);

    await native.saveSession("access-secret", "refresh-secret");

    expect(invoke).toHaveBeenCalledWith("save_session", {
      accessToken: "access-secret",
      refreshToken: "refresh-secret"
    });
  });

  it.each(["translate", "improve"] as const)(
    "sends the %s operation and only transformation preferences",
    async (operation) => {
      invoke.mockResolvedValue({
        requestId: "request",
        sourceLanguage: "pt",
        targetLanguage: "en",
        result: "Result"
      });

      await native.transformSelection(operation, defaultSettings);

      expect(invoke).toHaveBeenCalledTimes(1);
      expect(invoke).toHaveBeenCalledWith("transform_selection", {
        operation,
        preferences: {
          formality: 3,
          length: "balanced",
          tone: "technical"
        }
      });
    }
  );

  it("passes transformed content to strict undo revalidation", async () => {
    invoke.mockResolvedValue(undefined);

    await native.undoReplacement("transformed");

    expect(invoke).toHaveBeenCalledWith("undo_replacement", {
      transformedText: "transformed"
    });
  });

  it("returns the native readiness acknowledgment", async () => {
    invoke.mockResolvedValue(true);

    await expect(
      native.overlaySurfaceReady("note", "note-generation")
    ).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith("overlay_surface_ready", {
      overlay: "note",
      generation: "note-generation"
    });
  });

  it("maps settings, session, selection and dismissal commands exactly", async () => {
    invoke.mockResolvedValue(undefined);

    await native.loadSettings();
    await native.saveSettings(defaultSettings);
    await native.hasSession();
    await native.clearSession();
    await native.currentSelection();
    await native.currentNoteResult();
    await native.publicBackendConfig();
    await native.aiReadiness();
    await native.openMainWindow();
    await native.refreshSelection();
    await native.overlaySurfaceReady("toolbar", "toolbar-generation");
    await native.dismissOverlays();
    await native.applyPreview("request-id");
    await native.listHistory();
    await native.deleteHistory("history-id");
    await native.deleteHistory();

    expect(invoke.mock.calls).toEqual([
      ["load_settings"],
      ["save_settings", { settings: defaultSettings }],
      ["has_session"],
      ["clear_session"],
      ["current_selection"],
      ["current_note_result"],
      ["public_backend_config"],
      ["ai_readiness"],
      ["open_main_window"],
      ["refresh_selection"],
      [
        "overlay_surface_ready",
        { overlay: "toolbar", generation: "toolbar-generation" }
      ],
      ["dismiss_overlays"],
      ["apply_preview", { requestId: "request-id" }],
      ["list_history"],
      ["delete_history", { id: "history-id" }],
      ["delete_history", { id: null }]
    ]);
  });
});
