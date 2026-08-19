export type InterpretRequest = {
  requestId: string;
  targetLanguage: string;
  audioBase64: string;
  mimeType: "audio/wav";
};

export type InterpretResponse = {
  requestId: string;
  detectedLanguage: string;
  targetLanguage: string;
  audioBase64: string;
  mimeType: string;
  stageMs: {
    stt: number;
    translate: number;
    tts: number;
  };
};

export type ErrorCode =
  | "UNAUTHENTICATED"
  | "INVALID_REQUEST"
  | "NO_VOICE_PROFILE"
  | "LANGUAGE_UNSUPPORTED"
  | "AUDIO_TOO_LARGE"
  | "STT_FAILED"
  | "TRANSLATION_FAILED"
  | "TTS_FAILED"
  | "PROVIDER_TIMEOUT"
  | "INTERNAL_ERROR";

export const MAX_AUDIO_BYTES = 2_621_440;

const ALLOWED_LANGUAGES = new Set([
  "en",
  "pt",
  "es",
  "fr",
  "de",
  "it",
  "ja",
  "ko",
  "zh",
]);

const ALLOWED_MIME = new Set(["audio/wav"]);

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function parseRequest(body: unknown): InterpretRequest {
  if (!body || typeof body !== "object") throw new Error("INVALID_REQUEST");
  const c = body as Record<string, unknown>;

  if (
    typeof c.requestId !== "string" ||
    !UUID_RE.test(c.requestId) ||
    typeof c.audioBase64 !== "string" ||
    c.audioBase64.length === 0 ||
    typeof c.mimeType !== "string" ||
    !ALLOWED_MIME.has(c.mimeType) ||
    typeof c.targetLanguage !== "string" ||
    c.targetLanguage.trim().length === 0
  ) {
    throw new Error("INVALID_REQUEST");
  }

  if (!ALLOWED_LANGUAGES.has(c.targetLanguage)) {
    throw new Error("LANGUAGE_UNSUPPORTED");
  }

  if (c.audioBase64.length > Math.ceil(MAX_AUDIO_BYTES * (4 / 3))) {
    throw new Error("AUDIO_TOO_LARGE");
  }

  return {
    requestId: c.requestId as string,
    targetLanguage: c.targetLanguage as string,
    audioBase64: c.audioBase64 as string,
    mimeType: "audio/wav",
  };
}
