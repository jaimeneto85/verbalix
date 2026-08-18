import type { UserAuthenticator } from "../transform/auth.ts";
import type { VoiceProfileView } from "./contract.ts";
import {
  createEnrollHandler,
  ENROLL_TIMEOUT_MS,
  type EnrollHandlerDeps,
  MAX_ENROLL_BODY_BYTES,
  type SupabaseServiceClient,
  type UpsertResult,
} from "./handler.ts";
import type { VoiceProvider } from "./provider.ts";

const requestId = "b65c8888-fb0e-4a8f-9fee-95268995bf68";

Deno.test("handler rejects unsupported methods before dependencies", async () => {
  const state = createState();
  const response = await createEnrollHandler(state.dependencies)(
    new Request("https://example.test", { method: "GET" }),
  );
  await assertError(response, 405, "INVALID_REQUEST");
  assertEquals(state.authCalls, 0);
});

Deno.test("handler requires a valid user bearer", async () => {
  const missing = createState();
  await assertError(
    await createEnrollHandler(missing.dependencies)(
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
    await createEnrollHandler(anonymous.dependencies)(validRequest()),
    401,
    "UNAUTHENTICATED",
  );
  assertEquals(anonymous.enrollCalls, 0);
});

Deno.test("handler rejects declared oversized body before touching the provider", async () => {
  const state = createState();
  const response = await createEnrollHandler(state.dependencies)(
    new Request("https://example.test", {
      method: "POST",
      headers: {
        Authorization: "Bearer user-token",
        "Content-Length": String(MAX_ENROLL_BODY_BYTES + 1),
      },
      body: validBody(),
    }),
  );
  await assertError(response, 413, "SAMPLE_TOO_LARGE");
  assertEquals(state.enrollCalls, 0);
});

Deno.test("handler rejects malformed json payload", async () => {
  const state = createState();
  const response = await createEnrollHandler(state.dependencies)(
    validRequest("{not-json"),
  );
  await assertError(response, 422, "INVALID_REQUEST");
  assertEquals(state.enrollCalls, 0);
});

Deno.test("handler is idempotent for a request_id already processed", async () => {
  const state = createState({
    upsertResult: { voiceProfileId: "profile-1", alreadyDone: true },
  });
  const response = await createEnrollHandler(state.dependencies)(
    validRequest(),
  );
  assertEquals(response.status, 200);
  const body = await response.json();
  assertEquals(body.voiceProfileId, "profile-1");
  assertEquals(state.enrollCalls, 0);
  assertEquals(state.setReadyCalls, 0);
});

Deno.test("handler replaces an existing profile before finishing a new enroll", async () => {
  const state = createState({
    previousProfile: {
      voiceProfileId: "previous-profile",
      requestId: "00000000-0000-4000-8000-000000000001",
      providerVoiceId: "previous-provider-voice",
    },
  });
  const response = await createEnrollHandler(state.dependencies)(
    validRequest(),
  );
  assertEquals(response.status, 200);
  assertEquals(state.deletedProviderVoiceIds, ["previous-provider-voice"]);
  assertEquals(state.deletedProfileIds, ["previous-profile"]);
  assertEquals(state.enrollCalls, 1);
});

Deno.test("handler keeps the current request id and skips replace when previous is the same row", async () => {
  const state = createState({
    previousProfile: {
      voiceProfileId: "00000000-0000-4000-8000-000000000002",
      requestId: requestId,
      providerVoiceId: "same-provider-voice",
    },
  });
  await createEnrollHandler(state.dependencies)(validRequest());
  assertEquals(state.deletedProviderVoiceIds.length, 0);
  assertEquals(state.deletedProfileIds.length, 0);
});

Deno.test("handler marks the profile failed and never orphans a voice on provider error", async () => {
  const state = createState({ providerError: new Error("PROVIDER_REJECTED") });
  const response = await createEnrollHandler(state.dependencies)(
    validRequest(),
  );
  await assertError(response, 502, "PROVIDER_REJECTED");
  assertEquals(state.setFailedCalls, 1);
  assertEquals(state.setReadyCalls, 0);
});

Deno.test("handler aborts the provider at the total timeout and cancels the timer", async () => {
  let scheduledDelay = 0;
  let cancelled = false;
  const state = createState({
    enroll: () => new Promise(() => {}),
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

  const response = await createEnrollHandler(state.dependencies)(
    validRequest(),
  );
  await assertError(response, 504, "PROVIDER_TIMEOUT");
  assertEquals(scheduledDelay, ENROLL_TIMEOUT_MS);
  assertEquals(cancelled, true);
  assertEquals(state.setFailedCalls, 1);
});

Deno.test("handler persists the ready profile and returns the safe view only", async () => {
  const state = createState();
  const response = await createEnrollHandler(state.dependencies)(
    validRequest(),
  );
  assertEquals(response.status, 200);
  assertEquals(response.headers.get("cache-control"), "no-store");
  const body = await response.json();
  assertEquals(body.voiceProfileId, requestId);
  assertEquals(body.displayName, "Minha Voz");
  assertEquals(body.status, "ready");
  assertEquals(JSON.stringify(body).includes("provider-voice"), false);
  assertEquals(state.setReadyCalls, 1);
});

Deno.test("handler cleans up orphan voice and marks failed when setReady fails after provider success", async () => {
  const state = createState({ setReadyError: new Error("db write failed") });
  const response = await createEnrollHandler(state.dependencies)(
    validRequest(),
  );
  await assertError(response, 500, "INTERNAL_ERROR");
  assertEquals(state.enrollCalls, 1);
  assertEquals(state.setReadyCalls, 1);
  assertEquals(state.setFailedCalls, 1);
  assertEquals(state.deletedProviderVoiceIds, ["provider-voice-id"]);
});

Deno.test("handler returns internal error when upsertEnrolling fails due to concurrent conflict", async () => {
  const state = createState({
    upsertEnrollingError: new Error("INTERNAL_ERROR"),
  });
  const response = await createEnrollHandler(state.dependencies)(
    validRequest(),
  );
  await assertError(response, 500, "INTERNAL_ERROR");
  assertEquals(state.enrollCalls, 0);
  assertEquals(state.setReadyCalls, 0);
});

type StateOptions = {
  authenticated?: boolean;
  enroll?: VoiceProvider["enroll"];
  providerError?: Error;
  upsertResult?: UpsertResult;
  upsertEnrollingError?: Error;
  setReadyError?: Error;
  previousProfile?: {
    voiceProfileId: string;
    requestId: string;
    providerVoiceId: string;
  };
  timeout?: EnrollHandlerDeps["timeout"];
};

function createState(options: StateOptions = {}) {
  const state = {
    authCalls: 0,
    enrollCalls: 0,
    setReadyCalls: 0,
    setFailedCalls: 0,
    deletedProviderVoiceIds: [] as string[],
    deletedProfileIds: [] as string[],
    dependencies: {} as EnrollHandlerDeps,
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

  const provider: VoiceProvider = {
    enroll: options.enroll ??
      ((_displayName, _blob, _signal) => {
        state.enrollCalls += 1;
        return options.providerError
          ? Promise.reject(options.providerError)
          : Promise.resolve("provider-voice-id");
      }),
    deleteVoice: (providerVoiceId) => {
      state.deletedProviderVoiceIds.push(providerVoiceId);
      return Promise.resolve();
    },
  };

  const upsertResult = options.upsertResult ??
    { voiceProfileId: requestId, alreadyDone: false };

  let profileStatus: VoiceProfileView["status"] = "enrolling";

  const serviceClient: SupabaseServiceClient = {
    upsertEnrolling: (_userId, _requestId, _displayName) => {
      if (options.upsertEnrollingError) {
        return Promise.reject(options.upsertEnrollingError);
      }
      return Promise.resolve(upsertResult);
    },
    setReady: (_voiceProfileId, _providerVoiceId) => {
      state.setReadyCalls += 1;
      if (options.setReadyError) {
        return Promise.reject(options.setReadyError);
      }
      profileStatus = "ready";
      return Promise.resolve();
    },
    setFailed: (_voiceProfileId) => {
      state.setFailedCalls += 1;
      profileStatus = "failed";
      return Promise.resolve();
    },
    getProfile: (_userId, voiceProfileId) => {
      return Promise.resolve({
        voiceProfileId,
        status: profileStatus,
        displayName: "Minha Voz",
      });
    },
    getPreviousProfile: (_userId) => {
      return Promise.resolve(options.previousProfile ?? null);
    },
    deleteProfile: (voiceProfileId) => {
      state.deletedProfileIds.push(voiceProfileId);
      return Promise.resolve();
    },
  };

  state.dependencies = {
    authenticator,
    provider,
    getSecret: () => "secret",
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
  return JSON.stringify({
    requestId,
    displayName: "Minha Voz",
    sampleBase64: "c2FtcGxl",
    mimeType: "audio/wav",
  });
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
