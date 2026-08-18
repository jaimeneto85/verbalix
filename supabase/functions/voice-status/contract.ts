export type StatusRequest = {
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

export function parseStatusRequest(value: unknown): StatusRequest {
  if (!value || typeof value !== "object") throw new Error("INVALID_REQUEST");
  const c = value as Record<string, unknown>;
  if (typeof c.voiceProfileId !== "string" || !isUuid(c.voiceProfileId)) {
    throw new Error("INVALID_REQUEST");
  }
  return { voiceProfileId: c.voiceProfileId as string };
}
