import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AppSettings,
  AiReadiness,
  EnterLivePayload,
  HistoryItem,
  LiveStateEvent,
  MicrophonePermission,
  NoteResult,
  PublicBackendConfig,
  SelectionSnapshot,
  TransformResult,
  VirtualMicStatus,
  VirtualMicStatusEvent,
  VoiceProfileView
} from "./types";

export const native = {
  accessibilityStatus(prompt = false) {
    return invoke<boolean>("accessibility_status", { prompt });
  },
  loadSettings() {
    return invoke<AppSettings>("load_settings");
  },
  saveSettings(settings: AppSettings) {
    return invoke<void>("save_settings", { settings });
  },
  saveSession(accessToken: string, refreshToken: string) {
    return invoke<void>("save_session", { accessToken, refreshToken });
  },
  hasSession() {
    return invoke<boolean>("has_session");
  },
  clearSession() {
    return invoke<void>("clear_session");
  },
  currentSelection() {
    return invoke<SelectionSnapshot | null>("current_selection");
  },
  currentNoteResult() {
    return invoke<NoteResult | null>("current_note_result");
  },
  publicBackendConfig() {
    return invoke<PublicBackendConfig>("public_backend_config");
  },
  aiReadiness() {
    return invoke<AiReadiness>("ai_readiness");
  },
  openMainWindow() {
    return invoke<void>("open_main_window");
  },
  refreshSelection() {
    return invoke<SelectionSnapshot | null>("refresh_selection");
  },
  overlaySurfaceReady(overlay: "toolbar" | "note", generation: string) {
    return invoke<boolean>("overlay_surface_ready", { overlay, generation });
  },
  transformSelection(
    operation: "translate" | "improve",
    settings: AppSettings
  ) {
    return invoke<TransformResult>("transform_selection", {
      operation,
      preferences: {
        formality: settings.formality,
        length: settings.length,
        tone: settings.tone
      }
    });
  },
  applyPreview(requestId: string) {
    return invoke<string>("apply_preview", { requestId });
  },
  undoReplacement(transformedText: string) {
    return invoke<void>("undo_replacement", { transformedText });
  },
  dismissOverlays() {
    return invoke<void>("dismiss_overlays");
  },
  listHistory() {
    return invoke<HistoryItem[]>("list_history");
  },
  deleteHistory(id?: string) {
    return invoke<void>("delete_history", { id: id ?? null });
  },
  currentSyncedPreferences() {
    return invoke<AppSettings | null>("current_synced_preferences");
  },
  microphonePermissionStatus() {
    return invoke<MicrophonePermission>("microphone_permission_status");
  },
  requestMicrophonePermission() {
    return invoke<MicrophonePermission>("request_microphone_permission");
  },
  beginVoiceEnrollment() {
    return invoke<void>("begin_voice_enrollment");
  },
  finishVoiceEnrollment(displayName: string) {
    return invoke<VoiceProfileView>("finish_voice_enrollment", { displayName });
  },
  cancelVoiceEnrollment() {
    return invoke<void>("cancel_voice_enrollment");
  },
  deleteVoiceProfile(voiceProfileId: string) {
    return invoke<void>("delete_voice_profile", { voiceProfileId });
  },
  voiceProfileStatus(voiceProfileId: string) {
    return invoke<VoiceProfileView | null>("voice_profile_status", { voiceProfileId });
  },
  enterLive(payload: EnterLivePayload) {
    return invoke<void>("enter_live", { payload });
  },
  leaveLive() {
    return invoke<void>("leave_live");
  },
  liveStatus() {
    return invoke<LiveStateEvent>("live_status");
  },
  onLiveStateChange(callback: (event: LiveStateEvent) => void): () => void {
    let unlisten: (() => void) | undefined;
    listen<LiveStateEvent>("live-state-changed", (event) => {
      callback(event.payload);
    }).then((dispose) => {
      unlisten = dispose;
    });
    return () => {
      unlisten?.();
    };
  },
  virtualMicStatus() {
    return invoke<{ status: VirtualMicStatus }>("virtual_mic_status");
  },
  onVirtualMicStatusChange(callback: (event: VirtualMicStatusEvent) => void): () => void {
    let unlisten: (() => void) | undefined;
    listen<VirtualMicStatusEvent>("virtual-mic-status", (event) => {
      callback(event.payload);
    }).then((dispose) => {
      unlisten = dispose;
    });
    return () => {
      unlisten?.();
    };
  }
};
