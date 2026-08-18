import type { UserAuthenticator } from "../transform/auth.ts";
import {
  type ErrorCode,
  MAX_SAMPLE_BYTES,
  parseEnrollRequest,
  type VoiceProfileView,
} from "./contract.ts";
import type { VoiceProvider } from "./provider.ts";
import {
  createSupabaseServiceClient,
  type SupabaseServiceClient,
  type UpsertResult,
} from "./service_client.ts";

export { createSupabaseServiceClient };
export type { SupabaseServiceClient, UpsertResult };

export const MAX_ENROLL_BODY_BYTES = Math.ceil(MAX_SAMPLE_BYTES * (4 / 3)) +
  8 * 1024;
export const ENROLL_TIMEOUT_MS = 60_000;

export type TimeoutScheduler = {
  schedule(callback: () => void, delay: number): unknown;
  cancel(handle: unknown): void;
};

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
      if (previous && previous.requestId === enrollReq.requestId) {
        const view = await deps.serviceClient.getProfile(
          user.id,
          previous.voiceProfileId,
        );
        if (!view) return errorResponse("INTERNAL_ERROR", 500);
        return new Response(JSON.stringify(view), {
          status: 200,
          headers: responseHeaders,
        });
      }

      if (previous) {
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

      try {
        await deps.serviceClient.setReady(voiceProfileId, providerVoiceId);
        const view = await deps.serviceClient.getProfile(
          user.id,
          voiceProfileId,
        );
        if (!view) return errorResponse("INTERNAL_ERROR", 500);
        return new Response(JSON.stringify(view), {
          status: 200,
          headers: responseHeaders,
        });
      } catch {
        await deps.provider.deleteVoice(providerVoiceId).catch(() => {});
        await deps.serviceClient.setFailed(voiceProfileId).catch(() => {});
        return errorResponse("INTERNAL_ERROR", 500);
      }
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
