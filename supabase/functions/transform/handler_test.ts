import type { UserAuthenticator } from "./auth.ts";
import type { TransformResponse } from "./contract.ts";
import {
  createTransformHandler,
  type HandlerDependencies,
  MAX_BODY_BYTES,
  PROVIDER_TIMEOUT_MS,
} from "./handler.ts";
import type { AiProvider } from "./provider.ts";

const requestId = "b65c8888-fb0e-4a8f-9fee-95268995bf68";

Deno.test("handler rejects unsupported methods before dependencies", async () => {
  const state = createState();
  const response = await createTransformHandler(state.dependencies)(
    new Request("https://example.test", { method: "GET" }),
  );
  await assertError(response, 405, "INVALID_RESPONSE");
  assertEquals(state.authCalls, 0);
  assertEquals(state.providerCalls, 0);
});

Deno.test("handler requires a valid user bearer", async () => {
  const missing = createState();
  await assertError(
    await createTransformHandler(missing.dependencies)(
      new Request("https://example.test", {
        method: "POST",
        body: validBody(),
      }),
    ),
    401,
    "UNAUTHENTICATED",
  );

  const anonymous = createState({ authenticated: false });
  await assertError(
    await createTransformHandler(anonymous.dependencies)(validRequest()),
    401,
    "UNAUTHENTICATED",
  );
  assertEquals(anonymous.providerCalls, 0);
});

Deno.test("handler caps body bytes before parsing or provider use", async () => {
  const state = createState();
  const response = await createTransformHandler(state.dependencies)(
    validRequest("界".repeat(Math.ceil(MAX_BODY_BYTES / 3) + 1)),
  );
  await assertError(response, 413, "TEXT_TOO_LONG");
  assertEquals(state.providerCalls, 0);
});

Deno.test("handler accepts body exactly at the byte limit", async () => {
  const state = createState();
  const body = validBody();
  const byteLength = new TextEncoder().encode(body).byteLength;
  const response = await createTransformHandler(state.dependencies)(
    validRequest(body + " ".repeat(MAX_BODY_BYTES - byteLength)),
  );
  assertEquals(response.status, 200);
  assertEquals(state.providerCalls, 1);
});

Deno.test("handler rejects declared oversized body before provider use", async () => {
  const state = createState();
  const response = await createTransformHandler(state.dependencies)(
    new Request("https://example.test", {
      method: "POST",
      headers: {
        Authorization: "Bearer user-token",
        "Content-Length": String(MAX_BODY_BYTES + 1),
      },
      body: validBody(),
    }),
  );
  await assertError(response, 413, "TEXT_TOO_LONG");
  assertEquals(state.providerCalls, 0);
});

Deno.test("handler rejects malformed payload and fractional formality", async () => {
  const malformed = createState();
  await assertError(
    await createTransformHandler(malformed.dependencies)(
      validRequest("{not-json"),
    ),
    422,
    "INVALID_RESPONSE",
  );

  const fractional = createState();
  await assertError(
    await createTransformHandler(fractional.dependencies)(
      validRequest(
        JSON.stringify({
          requestId,
          operation: "improve",
          text: "Improve this",
          preferences: {
            formality: 2.5,
            length: "balanced",
            tone: "technical",
          },
        }),
      ),
    ),
    422,
    "INVALID_RESPONSE",
  );
  assertEquals(fractional.providerCalls, 0);
});

Deno.test("handler fails closed when provider secrets are absent", async () => {
  for (
    const secrets of [
      { OPENAI_MODEL: "model" },
      { OPENAI_API_KEY: "key" },
    ]
  ) {
    const state = createState({ secrets });
    await assertError(
      await createTransformHandler(state.dependencies)(validRequest()),
      500,
      "INTERNAL_ERROR",
    );
    assertEquals(state.providerCalls, 0);
  }
});

Deno.test("handler maps provider rate limits and invalid output", async () => {
  const limited = createState({ providerError: new Error("RATE_LIMITED") });
  await assertError(
    await createTransformHandler(limited.dependencies)(validRequest()),
    429,
    "RATE_LIMITED",
  );
  assertEquals(limited.cancelCalls, 1);

  const invalid = createState({
    result: {
      requestId: "different-request",
      sourceLanguage: "English",
      targetLanguage: "Portuguese",
      result: "Resultado",
    },
  });
  await assertError(
    await createTransformHandler(invalid.dependencies)(validRequest()),
    422,
    "INVALID_RESPONSE",
  );
});

Deno.test("handler aborts provider at the total timeout and cancels timer", async () => {
  let scheduledDelay = 0;
  let cancelled = false;
  const state = createState({
    transform: () => new Promise(() => {}),
    timeout: {
      schedule(callback, delay) {
        scheduledDelay = delay;
        queueMicrotask(callback);
        return "timer";
      },
      cancel(handle) {
        cancelled = handle === "timer";
      },
    },
  });

  const response = await createTransformHandler(state.dependencies)(
    validRequest(),
  );
  await assertError(response, 504, "PROVIDER_TIMEOUT");
  assertEquals(scheduledDelay, PROVIDER_TIMEOUT_MS);
  assertEquals(cancelled, true);
});

Deno.test("handler returns validated output without caching", async () => {
  const state = createState();
  const response = await createTransformHandler(state.dependencies)(
    validRequest(),
  );
  assertEquals(response.status, 200);
  assertEquals(response.headers.get("cache-control"), "no-store");
  const body = await response.json();
  assertEquals(body.requestId, requestId);
  assertEquals(body.result, "Use fetch safely.");
  assertEquals(state.cancelCalls, 1);
});

type StateOptions = {
  authenticated?: boolean;
  secrets?: Partial<Record<string, string>>;
  providerError?: Error;
  result?: TransformResponse;
  transform?: AiProvider["transform"];
  timeout?: HandlerDependencies["timeout"];
};

function createState(options: StateOptions = {}) {
  const state = {
    authCalls: 0,
    providerCalls: 0,
    cancelCalls: 0,
    dependencies: {} as HandlerDependencies,
  };
  const authenticator: UserAuthenticator = {
    authenticate() {
      state.authCalls += 1;
      return Promise.resolve(
        options.authenticated === false
          ? null
          : { id: "user-id", role: "authenticated" },
      );
    },
  };
  const result = options.result ?? {
    requestId,
    sourceLanguage: "English",
    targetLanguage: "Portuguese",
    result: "Use fetch safely.",
  };
  state.dependencies = {
    authenticator,
    getSecret: (name) =>
      (options.secrets ?? {
        OPENAI_API_KEY: "key",
        OPENAI_MODEL: "model",
      })[name],
    providerFactory: () => {
      state.providerCalls += 1;
      return {
        transform: options.transform ??
          (() =>
            options.providerError
              ? Promise.reject(options.providerError)
              : Promise.resolve(result)),
      };
    },
    timeout: options.timeout ?? {
      schedule: () => "timer",
      cancel: () => {
        state.cancelCalls += 1;
      },
    },
  };
  return state;
}

function validRequest(body = validBody()) {
  return new Request("https://example.test", {
    method: "POST",
    headers: {
      Authorization: "Bearer user-token",
      "Content-Type": "application/json",
    },
    body,
  });
}

function validBody() {
  return JSON.stringify({
    requestId,
    operation: "translate",
    text: "Use fetch safely.",
  });
}

async function assertError(response: Response, status: number, code: string) {
  assertEquals(response.status, status);
  assertEquals(response.headers.get("cache-control"), "no-store");
  const body = await response.json();
  assertEquals(body.error.code, code);
}

function assertEquals(actual: unknown, expected: unknown) {
  if (actual !== expected) {
    throw new Error(`expected ${String(expected)}, received ${String(actual)}`);
  }
}
