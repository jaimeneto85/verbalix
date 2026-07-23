import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  SelectionSnapshot,
  TransformResult
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
  refreshSelection() {
    return invoke<SelectionSnapshot | null>("refresh_selection");
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
  undoReplacement(transformedText: string) {
    return invoke<void>("undo_replacement", { transformedText });
  },
  dismissOverlays() {
    return invoke<void>("dismiss_overlays");
  }
};
