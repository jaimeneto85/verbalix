export type TransformOperation = "translate" | "improve";
export type LengthPreference = "concise" | "balanced" | "detailed";
export type TonePreference = "neutral" | "friendly" | "assertive" | "technical";

export type TransformRequest = {
  requestId: string;
  operation: TransformOperation;
  text: string;
  preferences?: {
    formality: 1 | 2 | 3 | 4 | 5;
    length: LengthPreference;
    tone: TonePreference;
  };
};

export type TransformResponse = {
  requestId: string;
  sourceLanguage: string;
  targetLanguage: string | null;
  result: string;
};

export type ErrorCode =
  | "UNAUTHENTICATED"
  | "TEXT_TOO_LONG"
  | "RATE_LIMITED"
  | "PROVIDER_TIMEOUT"
  | "INVALID_RESPONSE"
  | "INTERNAL_ERROR";

export const MAX_TEXT_CHARACTERS = 12_000;
export const MAX_RESULT_CHARACTERS = 24_000;

const operations = new Set(["translate", "improve"]);
const lengths = new Set(["concise", "balanced", "detailed"]);
const tones = new Set(["neutral", "friendly", "assertive", "technical"]);

export function parseRequest(value: unknown): TransformRequest {
  if (!value || typeof value !== "object") throw new Error("INVALID_RESPONSE");
  const candidate = value as Record<string, unknown>;
  if (
    typeof candidate.requestId !== "string" ||
    !isUuid(candidate.requestId) ||
    typeof candidate.text !== "string" ||
    candidate.text.trim().length === 0 ||
    typeof candidate.operation !== "string" ||
    !operations.has(candidate.operation)
  ) {
    throw new Error("INVALID_RESPONSE");
  }
  if ([...candidate.text].length > MAX_TEXT_CHARACTERS) {
    throw new Error("TEXT_TOO_LONG");
  }
  if (candidate.operation === "improve") {
    const preferences = candidate.preferences as Record<string, unknown> | undefined;
    if (
      !preferences ||
      typeof preferences.formality !== "number" ||
      !Number.isInteger(preferences.formality) ||
      preferences.formality < 1 ||
      preferences.formality > 5 ||
      typeof preferences.length !== "string" ||
      !lengths.has(preferences.length) ||
      typeof preferences.tone !== "string" ||
      !tones.has(preferences.tone)
    ) {
      throw new Error("INVALID_RESPONSE");
    }
  }
  return candidate as TransformRequest;
}

export function validateResponse(
  request: TransformRequest,
  value: unknown
): TransformResponse {
  if (!value || typeof value !== "object") throw new Error("INVALID_RESPONSE");
  const candidate = value as Record<string, unknown>;
  if (
    candidate.requestId !== request.requestId ||
    typeof candidate.sourceLanguage !== "string" ||
    candidate.sourceLanguage.trim().length === 0 ||
    typeof candidate.result !== "string" ||
    candidate.result.trim().length === 0 ||
    [...candidate.result].length > MAX_RESULT_CHARACTERS
  ) {
    throw new Error("INVALID_RESPONSE");
  }
  if (
    request.operation === "translate" &&
    (typeof candidate.targetLanguage !== "string" ||
      candidate.targetLanguage.trim().length === 0)
  ) {
    throw new Error("INVALID_RESPONSE");
  }
  if (request.operation === "improve" && candidate.targetLanguage !== null) {
    throw new Error("INVALID_RESPONSE");
  }
  return candidate as TransformResponse;
}

function isUuid(value: string) {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
    value
  );
}
