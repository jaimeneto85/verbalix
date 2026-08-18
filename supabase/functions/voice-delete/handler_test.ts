import type { UserAuthenticator } from "../transform/auth.ts";
import type { VoiceProfileView } from "../voice-enroll/contract.ts";
import {
  createDeleteHandler,
  DELETE_TIMEOUT_MS,
  type DeleteHandlerDeps,
  type DeleteServiceClient,
} from "./handler.ts";
import type { VoiceDeleter } from "./provider.ts";

const voiceProfileId = "0d3f2c40-0e2a-4d3a-9c9a-2a4a6e9b0f11";

Deno.test("handler rejects unsupported methods before dependencies", async () => {
  const state = createState();
  const response = await createDeleteHandler(state.dependencies)(
    new Request("https://example.test", { method: "GET" }),
  );
  await assertError(response, 405, "INVALID_REQUEST");
  assertEquals(state.authCalls, 0);
});

Deno.test("handler requires a valid user bearer", async () => {
  const missing = createState();
  await assertError(
    await createDeleteHandler(missing.dependencies)(
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
    await createDeleteHandler(anonymous.dependencies)(validRequest()),
    401,
    "UNAUTHENTICATED",
  );
  assertEquals(anonymous.deleteCalls, 0);
});

Deno.test("handler rejects malformed json payload", async () => {
  const state = createState();
  const response = await createDeleteHandler(state.dependencies)(
    validRequest("{not-json"),
  );
  await assertError(response, 422, "INVALID_REQUEST");
});

Deno.test("handler returns not found for a profile owned by another user", async () => {
  const state = createState({ profile: null });
  const response = await createDeleteHandler(state.dependencies)(
    validRequest(),
  );
  await assertError(response, 404, "NOT_FOUND");
  assertEquals(state.markDeletingCalls, 0);
});

Deno.test("handler is idempotent when the voice is already absent at the provider", async () => {
  const state = createState({
    deleteVoice: () => Promise.resolve(),
    providerVoiceId: null,
  });
  const response = await createDeleteHandler(state.dependencies)(
    validRequest(),
  );
  assertEquals(response.status, 200);
  assertEquals(state.deleteCalls, 0);
  assertEquals(state.deleteProfileCalls, 1);
});

Deno.test("handler marks deleting before calling the provider and reconciles on partial failure", async () => {
  const state = createState({
    deleteVoice: () => Promise.reject(new Error("boom")),
  });
  const response = await createDeleteHandler(state.dependencies)(
    validRequest(),
  );
  await assertError(response, 500, "INTERNAL_ERROR");
  assertEquals(state.markDeletingCalls, 1);
  assertEquals(state.deleteProfileCalls, 0);
});

Deno.test("handler aborts the provider call at the timeout and leaves status deleting", async () => {
  let scheduledDelay = 0;
  let cancelled = false;
  const state = createState({
    deleteVoice: () => new Promise(() => {}),
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

  const response = await createDeleteHandler(state.dependencies)(
    validRequest(),
  );
  await assertError(response, 500, "INTERNAL_ERROR");
  assertEquals(scheduledDelay, DELETE_TIMEOUT_MS);
  assertEquals(cancelled, true);
  assertEquals(state.deleteProfileCalls, 0);
});

Deno.test("handler deletes the row and returns the safe deleting view on success", async () => {
  const state = createState();
  const response = await createDeleteHandler(state.dependencies)(
    validRequest(),
  );
  assertEquals(response.status, 200);
  assertEquals(response.headers.get("cache-control"), "no-store");
  const body = await response.json();
  assertEquals(body.voiceProfileId, voiceProfileId);
  assertEquals(body.status, "deleting");
  assertEquals(body.displayName, "Minha Voz");
  assertEquals(state.deleteCalls, 1);
  assertEquals(state.deleteProfileCalls, 1);
});

type StateOptions = {
  authenticated?: boolean;
  profile?: VoiceProfileView | null;
  providerVoiceId?: string | null;
  deleteVoice?: VoiceDeleter["deleteVoice"];
  timeout?: DeleteHandlerDeps["timeout"];
};

function createState(options: StateOptions = {}) {
  const state = {
    authCalls: 0,
    deleteCalls: 0,
    markDeletingCalls: 0,
    deleteProfileCalls: 0,
    dependencies: {} as DeleteHandlerDeps,
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

  const deleter: VoiceDeleter = {
    deleteVoice: (providerVoiceId, signal) => {
      state.deleteCalls += 1;
      return (options.deleteVoice ?? (() => Promise.resolve()))(
        providerVoiceId,
        signal,
      );
    },
  };

  const profile = options.profile === undefined
    ? {
      voiceProfileId,
      status: "ready" as const,
      displayName: "Minha Voz",
    }
    : options.profile;

  const providerVoiceId = options.providerVoiceId === undefined
    ? "provider-voice-1"
    : options.providerVoiceId;

  const serviceClient: DeleteServiceClient = {
    markDeleting: (_voiceProfileId, _userId) => {
      state.markDeletingCalls += 1;
      return Promise.resolve();
    },
    resolveProviderVoiceId: (_voiceProfileId, _userId) => {
      return Promise.resolve(providerVoiceId);
    },
    deleteProfile: (_voiceProfileId) => {
      state.deleteProfileCalls += 1;
      return Promise.resolve();
    },
    getProfile: (_userId, _voiceProfileId) => {
      return Promise.resolve(profile);
    },
  };

  state.dependencies = {
    authenticator,
    deleter,
    serviceClient,
    timeout: options.timeout ?? {
      schedule: () => "timer",
      cancel: () => {},
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
  return JSON.stringify({ voiceProfileId });
}

async function assertError(response: Response, status: number, code: string) {
  assertEquals(response.status, status);
  assertEquals(response.headers.get("cache-control"), "no-store");
  const body = await response.json();
  assertEquals(body.error.code, code);
}

function assertEquals(actual: unknown, expected: unknown) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(
      `expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`,
    );
  }
}
