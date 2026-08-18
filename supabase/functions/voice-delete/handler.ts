import type { UserAuthenticator } from "../transform/auth.ts";
import { type ErrorCode, parseDeleteRequest } from "./contract.ts";
import type { Fetcher, VoiceDeleter } from "./provider.ts";
import type { VoiceProfileView } from "../voice-enroll/contract.ts";

export const DELETE_TIMEOUT_MS = 30_000;

export type TimeoutScheduler = {
  schedule(callback: () => void, delay: number): unknown;
  cancel(handle: unknown): void;
};

export interface DeleteServiceClient {
  markDeleting(voiceProfileId: string, userId: string): Promise<void>;
  resolveProviderVoiceId(
    voiceProfileId: string,
    userId: string,
  ): Promise<string | null>;
  deleteProfile(voiceProfileId: string): Promise<void>;
  getProfile(
    userId: string,
    voiceProfileId: string,
  ): Promise<VoiceProfileView | null>;
}

export type DeleteHandlerDeps = {
  authenticator: UserAuthenticator;
  deleter: VoiceDeleter;
  serviceClient: DeleteServiceClient;
  timeout: TimeoutScheduler;
};

const responseHeaders = {
  "Content-Type": "application/json",
  "Cache-Control": "no-store",
};

export function createDeleteHandler(deps: DeleteHandlerDeps) {
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

      const deleteReq = parseDeleteRequest(parsed);

      const profile = await deps.serviceClient.getProfile(
        user.id,
        deleteReq.voiceProfileId,
      );
      if (!profile) return errorResponse("NOT_FOUND", 404);

      await deps.serviceClient.markDeleting(deleteReq.voiceProfileId, user.id);

      const providerVoiceId = await deps.serviceClient.resolveProviderVoiceId(
        deleteReq.voiceProfileId,
        user.id,
      );

      if (providerVoiceId) {
        const controller = new AbortController();
        let rejectTimeout = (_reason: Error) => {};
        const timeoutPromise = new Promise<never>((_resolve, reject) => {
          rejectTimeout = reject;
        });
        const handle = deps.timeout.schedule(() => {
          controller.abort();
          rejectTimeout(new Error("INTERNAL_ERROR"));
        }, DELETE_TIMEOUT_MS);

        try {
          await Promise.race([
            deps.deleter.deleteVoice(providerVoiceId, controller.signal),
            timeoutPromise,
          ]);
        } finally {
          deps.timeout.cancel(handle);
        }
      }

      await deps.serviceClient.deleteProfile(deleteReq.voiceProfileId);

      return new Response(
        JSON.stringify({
          voiceProfileId: deleteReq.voiceProfileId,
          status: "deleting",
          displayName: profile.displayName,
        }),
        { status: 200, headers: responseHeaders },
      );
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

export function createDeleteServiceClient(
  supabaseUrl: string,
  serviceKey: string,
  fetcher: Fetcher = fetch,
): DeleteServiceClient {
  const baseUrl = `${supabaseUrl}/rest/v1`;
  const authHeaders = {
    Authorization: `Bearer ${serviceKey}`,
    apikey: serviceKey,
    "Content-Type": "application/json",
  };

  return {
    async markDeleting(voiceProfileId: string, _userId: string): Promise<void> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?id=eq.${voiceProfileId}`,
        {
          method: "PATCH",
          headers: { ...authHeaders, Prefer: "return=minimal" },
          body: JSON.stringify({ status: "deleting" }),
        },
      );
      if (!response.ok) throw new Error("INTERNAL_ERROR");
    },

    async resolveProviderVoiceId(
      voiceProfileId: string,
      _userId: string,
    ): Promise<string | null> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?id=eq.${voiceProfileId}&select=provider_voice_id`,
        { headers: authHeaders },
      );
      if (!response.ok) throw new Error("INTERNAL_ERROR");
      const rows = (await response.json()) as Array<Record<string, unknown>>;
      if (!rows || rows.length === 0) return null;
      const val = rows[0].provider_voice_id;
      return typeof val === "string" ? val : null;
    },

    async deleteProfile(voiceProfileId: string): Promise<void> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?id=eq.${voiceProfileId}`,
        {
          method: "DELETE",
          headers: { ...authHeaders, Prefer: "return=minimal" },
        },
      );
      if (!response.ok) throw new Error("INTERNAL_ERROR");
    },

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
