import { parseDeleteRequest } from "./contract.ts";
import {
  createDeleteHandler,
  type DeleteHandlerDeps,
  type DeleteServiceClient,
} from "./handler.ts";
import type { UserAuthenticator } from "../transform/auth.ts";
import type { VoiceDeleter } from "./provider.ts";
import type { VoiceProfileView } from "../voice-enroll/contract.ts";

const VALID_UUID = "b65c8888-fb0e-4a8f-9fee-95268995bf68";
const ANOTHER_UUID = "ae63a86a-b5a2-4893-aea3-d78653385ed9";

Deno.test("parseDeleteRequest rejects invalid UUID for voiceProfileId", () => {
  assertThrows(
    () => parseDeleteRequest({ voiceProfileId: "not-a-uuid" }),
    "INVALID_REQUEST",
  );
});

Deno.test("parseDeleteRequest rejects missing fields", () => {
  assertThrows(() => parseDeleteRequest({}), "INVALID_REQUEST");
});

Deno.test("parseDeleteRequest accepts valid input", () => {
  const result = parseDeleteRequest({ voiceProfileId: VALID_UUID });
  if (result.voiceProfileId !== VALID_UUID) {
    throw new Error("voiceProfileId mismatch");
  }
});

Deno.test("handler returns 401 for missing Authorization", async () => {
  const handler = createDeleteHandler(makeDeps());
  const response = await handler(
    new Request("http://localhost/", { method: "POST", body: "{}" }),
  );
  if (response.status !== 401) throw new Error(`expected 401, got ${response.status}`);
});

Deno.test("handler returns 401 for anon role", async () => {
  const handler = createDeleteHandler(
    makeDeps({
      authenticator: {
        authenticate: async (_token: string) => ({ id: "user-1", role: "anon" }),
      },
    }),
  );
  const response = await handler(
    makeRequest({ Authorization: "Bearer anon-token" }, "{}"),
  );
  if (response.status !== 401) throw new Error(`expected 401, got ${response.status}`);
});

Deno.test("handler returns 404 for missing profile", async () => {
  const handler = createDeleteHandler(
    makeDeps({
      authenticator: {
        authenticate: async (_token: string) => ({ id: "user-1", role: "authenticated" }),
      },
      serviceClient: makeServiceClient(null, null),
    }),
  );
  const body = JSON.stringify({ voiceProfileId: ANOTHER_UUID });
  const response = await handler(
    makeRequest({ Authorization: "Bearer user-token" }, body),
  );
  if (response.status !== 404) throw new Error(`expected 404, got ${response.status}`);
});

Deno.test("handler returns 200 on successful delete", async () => {
  const profile: VoiceProfileView = {
    voiceProfileId: ANOTHER_UUID,
    status: "ready",
    displayName: "Test Voice",
  };
  const handler = createDeleteHandler(
    makeDeps({
      authenticator: {
        authenticate: async (_token: string) => ({ id: "user-1", role: "authenticated" }),
      },
      serviceClient: makeServiceClient(profile, "elabs-voice-id"),
      deleter: {
        deleteVoice: async (_id: string) => {},
      },
    }),
  );
  const body = JSON.stringify({ voiceProfileId: ANOTHER_UUID });
  const response = await handler(
    makeRequest({ Authorization: "Bearer user-token" }, body),
  );
  if (response.status !== 200) throw new Error(`expected 200, got ${response.status}`);
  const data = await response.json();
  if (data.voiceProfileId !== ANOTHER_UUID) throw new Error("voiceProfileId mismatch");
});

function assertThrows(fn: () => unknown, expectedCode: string): void {
  let actual = "";
  try {
    fn();
  } catch (reason) {
    actual = reason instanceof Error ? reason.message : "";
  }
  if (actual !== expectedCode) {
    throw new Error(`expected ${expectedCode}, received ${actual}`);
  }
}

function makeRequest(headers: Record<string, string>, body: string): Request {
  return new Request("http://localhost/", {
    method: "POST",
    headers,
    body,
  });
}

function makeServiceClient(
  profile: VoiceProfileView | null,
  providerVoiceId: string | null,
): DeleteServiceClient {
  return {
    markDeleting: async () => {},
    resolveProviderVoiceId: async () => providerVoiceId,
    deleteProfile: async () => {},
    getProfile: async () => profile,
  };
}

type PartialDeps = {
  authenticator?: UserAuthenticator;
  deleter?: VoiceDeleter;
  serviceClient?: DeleteServiceClient;
};

function makeDeps(overrides: PartialDeps = {}): DeleteHandlerDeps {
  return {
    authenticator: overrides.authenticator ?? {
      authenticate: async (_token: string) => null,
    },
    deleter: overrides.deleter ?? {
      deleteVoice: async (_id: string) => {},
    },
    serviceClient: overrides.serviceClient ?? makeServiceClient(null, null),
    timeout: {
      schedule: (_cb: () => void, _delay: number) => 0,
      cancel: (_h: unknown) => {},
    },
  };
}
