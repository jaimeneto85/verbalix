export type LengthPreference = "concise" | "balanced" | "detailed";
export type TonePreference = "neutral" | "friendly" | "assertive" | "technical";

export type AppSettings = {
  formality: number;
  length: LengthPreference;
  tone: TonePreference;
  confirmBeforeReplace: boolean;
  historyEnabled: boolean;
  automaticToolbar: boolean;
  shortcut: string;
};

export type SelectionSnapshot = {
  id: string;
  pid: number;
  bundleId: string;
  text: string;
  range: { location: number; length: number };
  bounds: { x: number; y: number; width: number; height: number };
  writable: boolean;
  capturedAtMs: number;
};

export type TransformResult = {
  requestId: string;
  sourceLanguage: string;
  targetLanguage: string | null;
  result: string;
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

export const defaultSettings: AppSettings = {
  formality: 3,
  length: "balanced",
  tone: "technical",
  confirmBeforeReplace: false,
  historyEnabled: false,
  automaticToolbar: true,
  shortcut: "Option+Shift+Space"
};
