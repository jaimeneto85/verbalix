import type { UserAuthenticator } from "../transform/auth.ts";
import {
  type ErrorCode,
  type InterpretResponse,
  MAX_AUDIO_BYTES,
  parseRequest,
} from "./contract.ts";
import type { Fetcher } from "./provider.ts";
import type { InterpretServiceClient } from "./service_client.ts";
import { runInterpretPipeline } from "./stages.ts";

export const MAX_INTERPRET_BODY_BYTES = Math.ceil(MAX_AUDIO_BYTES * (4 / 3)) +
  8 * 1024;

const responseHeaders = {
  "Content-Type": "application/json",
  "Cache-Control": "no-store",
};

export type InterpretHandlerDeps = {
  authenticator: UserAuthenticator;
  serviceClient: InterpretServiceClient;
  fetcher: Fetcher;
};

export async function handleInterpret(
  req: Request,
  deps: InterpretHandlerDeps,
): Promise<Response> {
  if (req.method !== "POST") {
    return errorResponse("INVALID_REQUEST", 405);
  }

  const token = bearerToken(req.headers.get("authorization"));
  if (!token) return errorResponse("UNAUTHENTICATED", 401);

  try {
    const user = await deps.authenticator.authenticate(token);
    if (!user || user.role === "anon" || user.role === "anonymous") {
      return errorResponse("UNAUTHENTICATED", 401);
    }

    const body = await readBoundedBody(req);
    let parsed: unknown;
    try {
      parsed = JSON.parse(
        new TextDecoder("utf-8", { fatal: true }).decode(body),
      );
    } catch {
      throw new Error("INVALID_REQUEST");
    }

    const interpretReq = parseRequest(parsed);

    const voiceProfile = await deps.serviceClient.getReadyVoiceProfile(
      user.id,
    );
    if (!voiceProfile) {
      return errorResponse("NO_VOICE_PROFILE", 404);
    }

    const pipelineResult = await runInterpretPipeline(
      interpretReq.audioBase64,
      interpretReq.targetLanguage,
      voiceProfile.providerVoiceId,
      deps.fetcher,
    );

    const response: InterpretResponse = {
      requestId: interpretReq.requestId,
      detectedLanguage: pipelineResult.detectedLanguage,
      targetLanguage: interpretReq.targetLanguage,
      audioBase64: pipelineResult.audioBase64,
      mimeType: "audio/mpeg",
      stageMs: pipelineResult.stageMs,
    };

    return new Response(JSON.stringify(response), {
      status: 200,
      headers: responseHeaders,
    });
  } catch (reason) {
    const code = normalizeError(reason);
    return errorResponse(code, statusFor(code));
  }
}

async function readBoundedBody(request: Request): Promise<Uint8Array> {
  const declaredLength = request.headers.get("content-length");
  if (
    declaredLength !== null &&
    (!/^\d+$/.test(declaredLength) ||
      Number(declaredLength) > MAX_INTERPRET_BODY_BYTES)
  ) {
    throw new Error("AUDIO_TOO_LARGE");
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
      if (length > MAX_INTERPRET_BODY_BYTES) {
        await reader.cancel();
        throw new Error("AUDIO_TOO_LARGE");
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
  const codes: string[] = [
    "UNAUTHENTICATED",
    "INVALID_REQUEST",
    "NO_VOICE_PROFILE",
    "LANGUAGE_UNSUPPORTED",
    "AUDIO_TOO_LARGE",
    "STT_FAILED",
    "TRANSLATION_FAILED",
    "TTS_FAILED",
    "PROVIDER_TIMEOUT",
    "INTERNAL_ERROR",
  ];
  return codes.includes(value);
}

function statusFor(code: ErrorCode): number {
  const statuses: Record<ErrorCode, number> = {
    UNAUTHENTICATED: 401,
    INVALID_REQUEST: 400,
    NO_VOICE_PROFILE: 404,
    LANGUAGE_UNSUPPORTED: 400,
    AUDIO_TOO_LARGE: 400,
    STT_FAILED: 502,
    TRANSLATION_FAILED: 502,
    TTS_FAILED: 502,
    PROVIDER_TIMEOUT: 502,
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
