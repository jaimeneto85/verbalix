import type { UserAuthenticator } from "./auth.ts";
import {
  type ErrorCode,
  parseRequest,
  type TransformRequest,
  validateResponse,
} from "./contract.ts";
import type { AiProvider } from "./provider.ts";

export const MAX_BODY_BYTES = 64 * 1024;
export const PROVIDER_TIMEOUT_MS = 20_000;

export type ProviderFactory = (
  apiKey: string,
  model: string,
) => AiProvider;

export type TimeoutScheduler = {
  schedule(callback: () => void, delay: number): unknown;
  cancel(handle: unknown): void;
};

export type HandlerDependencies = {
  providerFactory: ProviderFactory;
  authenticator: UserAuthenticator;
  getSecret(name: string): string | undefined;
  timeout: TimeoutScheduler;
};

const responseHeaders = {
  "Content-Type": "application/json",
  "Cache-Control": "no-store",
};

export function createTransformHandler(dependencies: HandlerDependencies) {
  return async (request: Request): Promise<Response> => {
    if (request.method !== "POST") {
      return errorResponse("INVALID_RESPONSE", 405);
    }

    const token = bearerToken(request.headers.get("authorization"));
    if (!token) return errorResponse("UNAUTHENTICATED", 401);

    try {
      const user = await dependencies.authenticator.authenticate(token);
      if (!user || user.role === "anon" || user.role === "anonymous") {
        return errorResponse("UNAUTHENTICATED", 401);
      }

      const input = parseRequest(await readJsonBody(request));
      const apiKey = dependencies.getSecret("OPENAI_API_KEY");
      const model = dependencies.getSecret("OPENAI_MODEL");
      if (!apiKey || !model) return errorResponse("INTERNAL_ERROR", 500);

      const result = await runProvider(dependencies, apiKey, model, input);
      return new Response(JSON.stringify(validateResponse(input, result)), {
        status: 200,
        headers: responseHeaders,
      });
    } catch (reason) {
      const code = normalizeError(reason);
      return errorResponse(code, statusFor(code));
    }
  };
}

async function readJsonBody(request: Request): Promise<unknown> {
  const declaredLength = request.headers.get("content-length");
  if (
    declaredLength !== null &&
    (/^\d+$/.test(declaredLength) === false ||
      Number(declaredLength) > MAX_BODY_BYTES)
  ) {
    throw new Error("TEXT_TOO_LONG");
  }

  const body = await readBoundedBody(request);
  try {
    return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(body));
  } catch {
    throw new Error("INVALID_RESPONSE");
  }
}

async function readBoundedBody(request: Request) {
  if (!request.body) return new Uint8Array();
  const reader = request.body.getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      length += value.byteLength;
      if (length > MAX_BODY_BYTES) {
        await reader.cancel();
        throw new Error("TEXT_TOO_LONG");
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

async function runProvider(
  dependencies: HandlerDependencies,
  apiKey: string,
  model: string,
  request: TransformRequest,
) {
  const controller = new AbortController();
  let rejectTimeout = (_reason: Error) => {};
  const timeout = new Promise<never>((_resolve, reject) => {
    rejectTimeout = reject;
  });
  const handle = dependencies.timeout.schedule(
    () => {
      controller.abort();
      rejectTimeout(new Error("PROVIDER_TIMEOUT"));
    },
    PROVIDER_TIMEOUT_MS,
  );
  try {
    const provider = dependencies.providerFactory(apiKey, model);
    return await Promise.race([
      provider.transform(request, controller.signal),
      timeout,
    ]);
  } finally {
    dependencies.timeout.cancel(handle);
  }
}

function bearerToken(authorization: string | null) {
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
  return [
    "UNAUTHENTICATED",
    "TEXT_TOO_LONG",
    "RATE_LIMITED",
    "PROVIDER_TIMEOUT",
    "INVALID_RESPONSE",
    "INTERNAL_ERROR",
  ].includes(value);
}

function statusFor(code: ErrorCode) {
  const statuses: Record<ErrorCode, number> = {
    UNAUTHENTICATED: 401,
    TEXT_TOO_LONG: 413,
    RATE_LIMITED: 429,
    PROVIDER_TIMEOUT: 504,
    INVALID_RESPONSE: 422,
    INTERNAL_ERROR: 500,
  };
  return statuses[code];
}

function errorResponse(code: ErrorCode, status: number) {
  return new Response(JSON.stringify({ error: { code } }), {
    status,
    headers: responseHeaders,
  });
}
