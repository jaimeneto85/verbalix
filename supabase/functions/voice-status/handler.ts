import type { UserAuthenticator } from "../transform/auth.ts";
import { type ErrorCode, parseStatusRequest } from "./contract.ts";
import type { VoiceProfileView } from "../voice-enroll/contract.ts";

export type Fetcher = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>;

export interface StatusServiceClient {
  getProfile(
    userId: string,
    voiceProfileId: string,
  ): Promise<VoiceProfileView | null>;
}

export type StatusHandlerDeps = {
  authenticator: UserAuthenticator;
  serviceClient: StatusServiceClient;
};

const responseHeaders = {
  "Content-Type": "application/json",
  "Cache-Control": "no-store",
};

export function createStatusHandler(deps: StatusHandlerDeps) {
  return async (request: Request): Promise<Response> => {
    if (request.method !== "POST") {
      return errorResponse("INVALID_REQUEST", 405);
    }

    const token = bearerToken(request.headers.get("authorization"));
    if (!token) return errorResponse("UNAUTHENTICATED", 401);

    try {
      const user = await deps.authenticator.authenticate(token);
      if (!user || user.role === "anon" || user.role === "anonymous") {
        return errorResponse("UNAUTHENTICATED", 401);
      }

      let parsed: unknown;
      try {
        const text = await request.text();
        parsed = JSON.parse(text);
      } catch {
        throw new Error("INVALID_REQUEST");
      }

      const statusReq = parseStatusRequest(parsed);

      const profile = await deps.serviceClient.getProfile(
        user.id,
        statusReq.voiceProfileId,
      );

      if (!profile) return errorResponse("NOT_FOUND", 404);

      return new Response(JSON.stringify(profile), {
        status: 200,
        headers: responseHeaders,
      });
    } catch (reason) {
      const code = normalizeError(reason);
      return errorResponse(code, statusFor(code));
    }
  };
}

function bearerToken(authorization: string | null): string | null {
  const match = authorization?.match(/^Bearer ([^\s]+)$/);
  return match?.[1] ?? null;
}

function normalizeError(reason: unknown): ErrorCode {
  if (reason instanceof Error && isErrorCode(reason.message)) {
    return reason.message;
  }
  return "INTERNAL_ERROR";
}

function isErrorCode(value: string): value is ErrorCode {
  return (["UNAUTHENTICATED", "NOT_FOUND", "INVALID_REQUEST", "INTERNAL_ERROR"] as string[]).includes(value);
}

function statusFor(code: ErrorCode): number {
  const statuses: Record<ErrorCode, number> = {
    UNAUTHENTICATED: 401,
    NOT_FOUND: 404,
    INVALID_REQUEST: 422,
    INTERNAL_ERROR: 500,
  };
  return statuses[code];
}

function errorResponse(code: ErrorCode, status: number): Response {
  return new Response(JSON.stringify({ error: { code } }), {
    status,
    headers: responseHeaders,
  });
}

export function createStatusServiceClient(
  supabaseUrl: string,
  serviceKey: string,
  fetcher: Fetcher = fetch,
): StatusServiceClient {
  const baseUrl = `${supabaseUrl}/rest/v1`;
  const authHeaders = {
    Authorization: `Bearer ${serviceKey}`,
    apikey: serviceKey,
    "Content-Type": "application/json",
  };

  return {
    async getProfile(
      userId: string,
      voiceProfileId: string,
    ): Promise<VoiceProfileView | null> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?id=eq.${voiceProfileId}&user_id=eq.${userId}&select=id,status,display_name`,
        { headers: authHeaders },
      );
      if (!response.ok) throw new Error("INTERNAL_ERROR");
      const rows = (await response.json()) as Array<Record<string, unknown>>;
      if (!rows || rows.length === 0) return null;
      const row = rows[0];
      return {
        voiceProfileId: row.id as string,
        status: row.status as VoiceProfileView["status"],
        displayName: row.display_name as string,
      };
    },
  };
}
