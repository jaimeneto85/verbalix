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

  it("reads currently synced preferences without arguments", async () => {
    invoke.mockResolvedValue(null);

    await expect(native.currentSyncedPreferences()).resolves.toBeNull();

    expect(invoke).toHaveBeenCalledWith("current_synced_preferences");
  });

  it("reads microphone permission status without arguments", async () => {
    invoke.mockResolvedValue("authorized");

    await expect(native.microphonePermissionStatus()).resolves.toBe(
      "authorized"
    );

    expect(invoke).toHaveBeenCalledWith("microphone_permission_status");
  });

  it("requests microphone permission without arguments", async () => {
    invoke.mockResolvedValue("notDetermined");

    await expect(native.requestMicrophonePermission()).resolves.toBe(
      "notDetermined"
    );

    expect(invoke).toHaveBeenCalledWith("request_microphone_permission");
  });

  it("begins voice enrollment without arguments", async () => {
    invoke.mockResolvedValue(undefined);

    await native.beginVoiceEnrollment();

    expect(invoke).toHaveBeenCalledWith("begin_voice_enrollment");
  });

  it("finishes voice enrollment with the trimmed camelCase display name", async () => {
    const view = {
      voiceProfileId: "profile-id",
      status: "enrolling",
      displayName: "Minha voz"
    };
    invoke.mockResolvedValue(view);

    await expect(
      native.finishVoiceEnrollment("Minha voz")
    ).resolves.toEqual(view);

    expect(invoke).toHaveBeenCalledWith("finish_voice_enrollment", {
      displayName: "Minha voz"
    });
  });

  it("cancels voice enrollment without arguments", async () => {
    invoke.mockResolvedValue(undefined);

    await native.cancelVoiceEnrollment();

    expect(invoke).toHaveBeenCalledWith("cancel_voice_enrollment");
  });

  it("deletes a voice profile by camelCase id", async () => {
    invoke.mockResolvedValue(undefined);

    await native.deleteVoiceProfile("profile-id");

    expect(invoke).toHaveBeenCalledWith("delete_voice_profile", {
      voiceProfileId: "profile-id"
    });
  });

  it("reads voice profile status by camelCase id and may resolve null", async () => {
    invoke.mockResolvedValue(null);

    await expect(
      native.voiceProfileStatus("profile-id")
    ).resolves.toBeNull();

    expect(invoke).toHaveBeenCalledWith("voice_profile_status", {
      voiceProfileId: "profile-id"
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
