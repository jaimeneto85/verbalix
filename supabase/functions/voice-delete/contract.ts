export type DeleteRequest = {
  requestId: string;
  voiceProfileId: string;
};

export type ErrorCode =
  | "UNAUTHENTICATED"
  | "NOT_FOUND"
  | "INVALID_REQUEST"
  | "INTERNAL_ERROR";

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function isUuid(value: string): boolean {
  return UUID_RE.test(value);
}

export function parseDeleteRequest(value: unknown): DeleteRequest {
  if (!value || typeof value !== "object") throw new Error("INVALID_REQUEST");
  const c = value as Record<string, unknown>;
  if (
    typeof c.requestId !== "string" ||
    !isUuid(c.requestId) ||
    typeof c.voiceProfileId !== "string" ||
    !isUuid(c.voiceProfileId)
  ) {
    throw new Error("INVALID_REQUEST");
  }
  return {
    requestId: c.requestId as string,
    voiceProfileId: c.voiceProfileId as string,
  };
}
