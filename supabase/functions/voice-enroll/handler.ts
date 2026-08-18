import type { UserAuthenticator } from "../transform/auth.ts";
import {
  type ErrorCode,
  parseEnrollRequest,
  type VoiceProfileView,
} from "./contract.ts";
import type { Fetcher, VoiceProvider } from "./provider.ts";

export const MAX_ENROLL_BODY_BYTES = 14 * 1024 * 1024;
export const ENROLL_TIMEOUT_MS = 60_000;

export type TimeoutScheduler = {
  schedule(callback: () => void, delay: number): unknown;
  cancel(handle: unknown): void;
};

export type UpsertResult = {
  voiceProfileId: string;
  alreadyDone: boolean;
};

export interface SupabaseServiceClient {
  upsertEnrolling(
    userId: string,
    requestId: string,
    displayName: string,
  ): Promise<UpsertResult>;
  setReady(voiceProfileId: string, providerVoiceId: string): Promise<void>;
  setFailed(voiceProfileId: string): Promise<void>;
  getProfile(
    userId: string,
    voiceProfileId: string,
  ): Promise<VoiceProfileView | null>;
  getPreviousProfile(
    userId: string,
  ): Promise<{ voiceProfileId: string; providerVoiceId: string } | null>;
  deleteProfile(voiceProfileId: string): Promise<void>;
}

export type EnrollHandlerDeps = {
  authenticator: UserAuthenticator;
  provider: VoiceProvider;
  getSecret(name: string): string | undefined;
  serviceClient: SupabaseServiceClient;
  timeout: TimeoutScheduler;
};

const responseHeaders = {
  "Content-Type": "application/json",
  "Cache-Control": "no-store",
};

export function createEnrollHandler(deps: EnrollHandlerDeps) {
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

      const body = await readBoundedBody(request);
      let parsed: unknown;
      try {
        parsed = JSON.parse(
          new TextDecoder("utf-8", { fatal: true }).decode(body),
        );
      } catch {
        throw new Error("INVALID_REQUEST");
      }

      const enrollReq = parseEnrollRequest(parsed);

      const previous = await deps.serviceClient.getPreviousProfile(user.id);
      if (previous && previous.voiceProfileId !== enrollReq.requestId) {
        try {
          await deps.provider.deleteVoice(previous.providerVoiceId);
        } catch {
        }
        await deps.serviceClient.deleteProfile(previous.voiceProfileId);
      }

      const upserted = await deps.serviceClient.upsertEnrolling(
        user.id,
        enrollReq.requestId,
        enrollReq.displayName,
      );

      if (upserted.alreadyDone) {
        const view = await deps.serviceClient.getProfile(
          user.id,
          upserted.voiceProfileId,
        );
        if (!view) return errorResponse("INTERNAL_ERROR", 500);
        return new Response(JSON.stringify(view), {
          status: 200,
          headers: responseHeaders,
        });
      }

      const { voiceProfileId } = upserted;

      const wavBytes = base64ToBytes(enrollReq.sampleBase64);
      const wavBlob = new Blob([wavBytes.buffer as ArrayBuffer], {
        type: "audio/wav",
      });

      const controller = new AbortController();
      let rejectTimeout = (_reason: Error) => {};
      const timeoutPromise = new Promise<never>((_resolve, reject) => {
        rejectTimeout = reject;
      });
      const handle = deps.timeout.schedule(() => {
        controller.abort();
        rejectTimeout(new Error("PROVIDER_TIMEOUT"));
      }, ENROLL_TIMEOUT_MS);

      let providerVoiceId: string;
      try {
        providerVoiceId = await Promise.race([
          deps.provider.enroll(
            enrollReq.displayName,
            wavBlob,
            controller.signal,
          ),
          timeoutPromise,
        ]);
        deps.timeout.cancel(handle);
      } catch (reason) {
        deps.timeout.cancel(handle);
        await deps.serviceClient.setFailed(voiceProfileId).catch(() => {});
        const code = normalizeError(reason);
        return errorResponse(code, statusFor(code));
      }

      await deps.serviceClient.setReady(voiceProfileId, providerVoiceId);

      const view = await deps.serviceClient.getProfile(user.id, voiceProfileId);
      if (!view) return errorResponse("INTERNAL_ERROR", 500);

      return new Response(JSON.stringify(view), {
        status: 200,
        headers: responseHeaders,
      });
    } catch (reason) {
      const code = normalizeError(reason);
      return errorResponse(code, statusFor(code));
    }
  };
}

function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

async function readBoundedBody(request: Request): Promise<Uint8Array> {
  const declaredLength = request.headers.get("content-length");
  if (
    declaredLength !== null &&
    (!/^\d+$/.test(declaredLength) ||
      Number(declaredLength) > MAX_ENROLL_BODY_BYTES)
  ) {
    throw new Error("SAMPLE_TOO_LARGE");
  }

  if (!request.body) return new Uint8Array();
  const reader = request.body.getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      length += value.byteLength;
      if (length > MAX_ENROLL_BODY_BYTES) {
        await reader.cancel();
        throw new Error("SAMPLE_TOO_LARGE");
      }
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }

  const body = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    body.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return body;
}

function bearerToken(authorization: string | null): string | null {
  const match = authorization?.match(/^Bearer ([^\s]+)$/);
  return match?.[1] ?? null;
}

function normalizeError(reason: unknown): ErrorCode {
  if (reason instanceof DOMException && reason.name === "AbortError") {
    return "PROVIDER_TIMEOUT";
  }
  if (reason instanceof Error && isErrorCode(reason.message)) {
    return reason.message;
  }
  return "INTERNAL_ERROR";
}

function isErrorCode(value: string): value is ErrorCode {
  return (
    [
      "UNAUTHENTICATED",
      "SAMPLE_TOO_LARGE",
      "INVALID_REQUEST",
      "PROVIDER_TIMEOUT",
      "PROVIDER_REJECTED",
      "INTERNAL_ERROR",
    ] as string[]
  ).includes(value);
}

function statusFor(code: ErrorCode): number {
  const statuses: Record<ErrorCode, number> = {
    UNAUTHENTICATED: 401,
    SAMPLE_TOO_LARGE: 413,
    INVALID_REQUEST: 422,
    PROVIDER_TIMEOUT: 504,
    PROVIDER_REJECTED: 502,
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

export function createSupabaseServiceClient(
  supabaseUrl: string,
  serviceKey: string,
  fetcher: Fetcher = fetch,
): SupabaseServiceClient {
  const baseUrl = `${supabaseUrl}/rest/v1`;
  const authHeaders = {
    Authorization: `Bearer ${serviceKey}`,
    apikey: serviceKey,
    "Content-Type": "application/json",
  };

  return {
    async upsertEnrolling(
      userId: string,
      requestId: string,
      displayName: string,
    ): Promise<UpsertResult> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?on_conflict=user_id,request_id&select=id,status`,
        {
          method: "POST",
          headers: {
            ...authHeaders,
            Prefer: "resolution=merge-duplicates,return=representation",
          },
          body: JSON.stringify({
            user_id: userId,
            request_id: requestId,
            display_name: displayName,
            status: "enrolling",
          }),
        },
      );

      if (!response.ok) throw new Error("INTERNAL_ERROR");

      const rows = (await response.json()) as Array<Record<string, unknown>>;
      if (!rows || rows.length === 0) throw new Error("INTERNAL_ERROR");

      const row = rows[0];
      const voiceProfileId = typeof row.id === "string" ? row.id : "";
      if (!voiceProfileId) throw new Error("INTERNAL_ERROR");

      return {
        voiceProfileId,
        alreadyDone: row.status !== "enrolling",
      };
    },

    async setReady(
      voiceProfileId: string,
      providerVoiceId: string,
    ): Promise<void> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?id=eq.${voiceProfileId}`,
        {
          method: "PATCH",
          headers: { ...authHeaders, Prefer: "return=minimal" },
          body: JSON.stringify({
            status: "ready",
            provider_voice_id: providerVoiceId,
          }),
        },
      );
      if (!response.ok) throw new Error("INTERNAL_ERROR");
    },

    async setFailed(voiceProfileId: string): Promise<void> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?id=eq.${voiceProfileId}`,
        {
          method: "PATCH",
          headers: { ...authHeaders, Prefer: "return=minimal" },
          body: JSON.stringify({ status: "failed" }),
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

    async getPreviousProfile(
      userId: string,
    ): Promise<{ voiceProfileId: string; providerVoiceId: string } | null> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?user_id=eq.${userId}&status=in.(ready,failed,enrolling,deleting)&select=id,provider_voice_id&order=created_at.desc&limit=1`,
        { headers: authHeaders },
      );
      if (!response.ok) throw new Error("INTERNAL_ERROR");
      const rows = (await response.json()) as Array<Record<string, unknown>>;
      if (!rows || rows.length === 0) return null;
      const row = rows[0];
      if (typeof row.provider_voice_id !== "string") return null;
      return {
        voiceProfileId: row.id as string,
        providerVoiceId: row.provider_voice_id,
      };
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
  };
}
