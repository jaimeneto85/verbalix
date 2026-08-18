import { createEnrollHandler } from "./handler.ts";
import {
  assertError,
  assertEquals,
  createState,
  REQUEST_ID,
  validRequest,
} from "./handler_test_helpers.ts";

Deno.test("handler is idempotent for a request_id already processed", async () => {
  const state = createState({
    previousProfile: {
      voiceProfileId: "profile-1",
      requestId: REQUEST_ID,
      providerVoiceId: "provider-voice-id",
    },
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

Deno.test("handler keeps the current request id and skips replace when previous is the same row", async () => {
  const state = createState({
    previousProfile: {
      voiceProfileId: "00000000-0000-4000-8000-000000000002",
      requestId: REQUEST_ID,
      providerVoiceId: "same-provider-voice",
    },
  });
  await createEnrollHandler(state.dependencies)(validRequest());
  assertEquals(state.deletedProviderVoiceIds.length, 0);
  assertEquals(state.deletedProfileIds.length, 0);
});

Deno.test("handler returns INTERNAL_ERROR when profile cannot be found on idempotent retry", async () => {
  const state = createState({
    previousProfile: {
      voiceProfileId: "missing-profile",
      requestId: REQUEST_ID,
      providerVoiceId: "some-voice",
    },
  });
  state.dependencies.serviceClient.getProfile = () => Promise.resolve(null);
  const response = await createEnrollHandler(state.dependencies)(
    validRequest(),
  );
  await assertError(response, 500, "INTERNAL_ERROR");
  assertEquals(state.enrollCalls, 0);
});
