import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultSettings } from "./types";

const { invoke, listen } = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn()
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

import { native } from "./native";

describe("native command contract", () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockReset();
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

  it("enters live with the camelCase target language and voice profile payload", async () => {
    invoke.mockResolvedValue(undefined);

    await native.enterLive({ targetLanguage: "pt", voiceProfileId: "profile-id" });

    expect(invoke).toHaveBeenCalledWith("enter_live", {
      payload: { targetLanguage: "pt", voiceProfileId: "profile-id" }
    });
  });

  it("leaves live without arguments", async () => {
    invoke.mockResolvedValue(undefined);

    await native.leaveLive();

    expect(invoke).toHaveBeenCalledWith("leave_live");
  });

  it("reads live status without arguments", async () => {
    invoke.mockResolvedValue({ status: "idle" });

    await expect(native.liveStatus()).resolves.toEqual({ status: "idle" });

    expect(invoke).toHaveBeenCalledWith("live_status");
  });

  it("subscribes to live-state-changed and forwards only the event payload", async () => {
    let deliver: ((event: { payload: unknown }) => void) | undefined;
    listen.mockImplementation((eventName, handler) => {
      expect(eventName).toBe("live-state-changed");
      deliver = handler;
      return Promise.resolve(() => undefined);
    });
    const callback = vi.fn();

    native.onLiveStateChange(callback);
    await Promise.resolve();
    await Promise.resolve();

    deliver?.({ payload: { status: "speaking", lastLatencyMs: 1200 } });

    expect(callback).toHaveBeenCalledWith({ status: "speaking", lastLatencyMs: 1200 });
  });

  it("unsubscribes from live-state-changed once the underlying listener resolves", async () => {
    const unlisten = vi.fn();
    listen.mockResolvedValue(unlisten);

    const dispose = native.onLiveStateChange(vi.fn());
    await Promise.resolve();
    await Promise.resolve();
    dispose();

    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("is a no-op to unsubscribe from live-state-changed before the listener resolves", () => {
    let resolveListen: ((value: () => void) => void) | undefined;
    listen.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveListen = resolve;
        })
    );

    const dispose = native.onLiveStateChange(vi.fn());

    expect(() => dispose()).not.toThrow();
    resolveListen?.(() => undefined);
  });

  it("reads virtual mic status without arguments", async () => {
    invoke.mockResolvedValue({ status: "installed" });

    await expect(native.virtualMicStatus()).resolves.toEqual({
      status: "installed"
    });

    expect(invoke).toHaveBeenCalledWith("virtual_mic_status");
  });

  it("subscribes to virtual-mic-status and forwards only the event payload", async () => {
    let deliver: ((event: { payload: unknown }) => void) | undefined;
    listen.mockImplementation((eventName, handler) => {
      expect(eventName).toBe("virtual-mic-status");
      deliver = handler;
      return Promise.resolve(() => undefined);
    });
    const callback = vi.fn();

    native.onVirtualMicStatusChange(callback);
    await Promise.resolve();
    await Promise.resolve();

    deliver?.({ payload: { status: "incompatibleVersion" } });

    expect(callback).toHaveBeenCalledWith({ status: "incompatibleVersion" });
  });

  it("unsubscribes from virtual-mic-status once the underlying listener resolves", async () => {
    const unlisten = vi.fn();
    listen.mockResolvedValue(unlisten);

    const dispose = native.onVirtualMicStatusChange(vi.fn());
    await Promise.resolve();
    await Promise.resolve();
    dispose();

    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("is a no-op to unsubscribe from virtual-mic-status before the listener resolves", () => {
    let resolveListen: ((value: () => void) => void) | undefined;
    listen.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveListen = resolve;
        })
    );

    const dispose = native.onVirtualMicStatusChange(vi.fn());

    expect(() => dispose()).not.toThrow();
    resolveListen?.(() => undefined);
  });
});
