export type VoiceProfileStatus = "enrolling" | "ready" | "failed" | "deleting";

export type VoiceProfileView = {
  voiceProfileId: string;
  status: VoiceProfileStatus;
  displayName: string;
};

export type EnrollRequest = {
  requestId: string;
  displayName: string;
  sampleBase64: string;
  mimeType: "audio/wav";
};

export type ErrorCode =
  | "UNAUTHENTICATED"
  | "SAMPLE_TOO_LARGE"
  | "INVALID_REQUEST"
  | "PROVIDER_TIMEOUT"
  | "PROVIDER_REJECTED"
  | "INTERNAL_ERROR";

export const MAX_SAMPLE_BYTES = 10 * 1024 * 1024;
export const MAX_DISPLAY_NAME_CHARS = 64;
const ALLOWED_MIME = new Set(["audio/wav"]);
const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function isUuid(value: string): boolean {
  return UUID_RE.test(value);
}

export function parseEnrollRequest(value: unknown): EnrollRequest {
  if (!value || typeof value !== "object") throw new Error("INVALID_REQUEST");
  const c = value as Record<string, unknown>;
  if (
    typeof c.requestId !== "string" ||
    !isUuid(c.requestId) ||
    typeof c.displayName !== "string" ||
    c.displayName.trim().length === 0 ||
    [...c.displayName].length > MAX_DISPLAY_NAME_CHARS ||
    typeof c.sampleBase64 !== "string" ||
    c.sampleBase64.length === 0 ||
    typeof c.mimeType !== "string" ||
    !ALLOWED_MIME.has(c.mimeType)
  ) {
    throw new Error("INVALID_REQUEST");
  }
  if (c.sampleBase64.length > MAX_SAMPLE_BYTES * 1.5) {
    throw new Error("SAMPLE_TOO_LARGE");
  }
  return {
    requestId: c.requestId as string,
    displayName: (c.displayName as string).trim(),
    sampleBase64: c.sampleBase64 as string,
    mimeType: "audio/wav",
  };
}
