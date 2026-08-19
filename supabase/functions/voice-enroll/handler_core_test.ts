import {
  createEnrollHandler,
  ENROLL_TIMEOUT_MS,
  MAX_ENROLL_BODY_BYTES,
} from "./handler.ts";
import {
  assertError,
  assertEquals,
  createState,
  REQUEST_ID,
  validBody,
  validRequest,
} from "./handler_test_helpers.ts";

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
  assertEquals(body.voiceProfileId, REQUEST_ID);
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
