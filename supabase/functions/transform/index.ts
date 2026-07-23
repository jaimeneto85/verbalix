import { parseRequest, type ErrorCode } from "./contract.ts";
import { OpenAiProvider } from "./provider.ts";

const headers = {
  "Content-Type": "application/json",
  "Cache-Control": "no-store"
};

Deno.serve(async (request) => {
  if (request.method !== "POST") {
    return errorResponse("INVALID_RESPONSE", 405);
  }
  const authorization = request.headers.get("authorization");
  if (!authorization?.startsWith("Bearer ")) {
    return errorResponse("UNAUTHENTICATED", 401);
  }

  try {
    const input = parseRequest(await request.json());
    const apiKey = Deno.env.get("OPENAI_API_KEY");
    const model = Deno.env.get("OPENAI_MODEL");
    if (!apiKey || !model) return errorResponse("INTERNAL_ERROR", 500);

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 20_000);
    try {
      const provider = new OpenAiProvider(apiKey, model);
      const result = await provider.transform(input, controller.signal);
      return new Response(JSON.stringify(result), { status: 200, headers });
    } finally {
      clearTimeout(timer);
    }
  } catch (reason) {
    const code = normalizeError(reason);
    const status =
      code === "UNAUTHENTICATED"
        ? 401
        : code === "TEXT_TOO_LONG"
          ? 413
          : code === "RATE_LIMITED"
            ? 429
            : code === "PROVIDER_TIMEOUT"
              ? 504
              : code === "INVALID_RESPONSE"
                ? 422
                : 500;
    return errorResponse(code, status);
  }
});

function normalizeError(reason: unknown): ErrorCode {
  if (reason instanceof DOMException && reason.name === "AbortError") {
    return "PROVIDER_TIMEOUT";
  }
  if (reason instanceof Error) {
    const codes: ErrorCode[] = [
      "UNAUTHENTICATED",
      "TEXT_TOO_LONG",
      "RATE_LIMITED",
      "PROVIDER_TIMEOUT",
      "INVALID_RESPONSE",
      "INTERNAL_ERROR"
    ];
    if (codes.includes(reason.message as ErrorCode)) {
      return reason.message as ErrorCode;
    }
  }
  return "INTERNAL_ERROR";
}

function errorResponse(code: ErrorCode, status: number) {
  return new Response(JSON.stringify({ error: { code } }), { status, headers });
}
