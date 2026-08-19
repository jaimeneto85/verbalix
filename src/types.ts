export type LengthPreference = "concise" | "balanced" | "detailed";
export type TonePreference = "neutral" | "friendly" | "assertive" | "technical";

export type VoiceProfileStatus = "enrolling" | "ready" | "failed" | "deleting";

export type VoiceProfileView = {
  voiceProfileId: string;
  status: VoiceProfileStatus;
  displayName: string;
};

export type MicrophonePermission = "authorized" | "denied" | "notDetermined" | "restricted";

export type AppSettings = {
  formality: number;
  length: LengthPreference;
  tone: TonePreference;
  confirmBeforeReplace: boolean;
  historyEnabled: boolean;
  automaticToolbar: boolean;
  shortcut: string;
  voiceProfileId?: string;
  outputToVirtualMic?: boolean;
};

export type SelectionSnapshot = {
  id: string;
  pid: number;
  bundleId: string;
  text: string;
  range: { location: number; length: number };
  bounds: { x: number; y: number; width: number; height: number };
  geometrySource?: "selected_range" | "text_marker_range" | "focused_element" | "cursor";
  extractionStrategy?:
    | "selected_text"
    | "string_for_range"
    | "value_range"
    | "text_marker";
  writable: boolean;
  capturedAtMs: number;
};

export type TransformResult = {
  requestId: string;
  sourceLanguage: string;
  targetLanguage: string | null;
  result: string;
};

export type PublicBackendConfig = {
  supabaseUrl: string;
  anonymousKey: string;
  configured: boolean;
};

export type AiReadiness = {
  status: "ready" | "login_required" | "provider_not_configured";
  message: string;
};

export type NoteResult = {
  mode: "error" | "result" | "preview" | "undo";
  requestId?: string;
  text: string;
};

export type HistoryItem = {
  id: string;
  operation: "translate" | "improve";
  source_text: string;
  result_text: string;
  source_language: string;
  target_language: string | null;
  created_at: string;
};

export type LiveStatus = 'idle' | 'listening' | 'processing' | 'speaking' | 'error';

export interface LiveStateEvent {
  status: LiveStatus;
  lastLatencyMs?: number;
  lastDropped?: boolean;
  errorMessage?: string;
  sessionId?: string;
}

export interface EnterLivePayload {
  targetLanguage: string;
  voiceProfileId: string;
}

export interface LiveSettings {
  targetLanguage: string;
}

export type VirtualMicStatus = "notInstalled" | "installed" | "incompatibleVersion";

export interface VirtualMicStatusEvent {
  status: VirtualMicStatus;
}

export const defaultSettings: AppSettings = {
  formality: 3,
  length: "balanced",
  tone: "technical",
  confirmBeforeReplace: false,
  historyEnabled: false,
  automaticToolbar: true,
  shortcut: "Option+Shift+Space"
};
