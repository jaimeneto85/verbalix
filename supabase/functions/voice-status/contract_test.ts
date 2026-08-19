import { parseStatusRequest } from "./contract.ts";
import {
  createStatusHandler,
  type StatusHandlerDeps,
  type StatusServiceClient,
} from "./handler.ts";
import type { UserAuthenticator } from "../transform/auth.ts";
import type { VoiceProfileView } from "../voice-enroll/contract.ts";

const VALID_UUID = "b65c8888-fb0e-4a8f-9fee-95268995bf68";

Deno.test("parseStatusRequest rejects missing voiceProfileId", () => {
  assertThrows(() => parseStatusRequest({}), "INVALID_REQUEST");
});

Deno.test("parseStatusRequest rejects invalid UUID", () => {
  assertThrows(
    () => parseStatusRequest({ voiceProfileId: "not-a-uuid" }),
    "INVALID_REQUEST",
  );
});

Deno.test("parseStatusRequest accepts valid UUID", () => {
  const result = parseStatusRequest({ voiceProfileId: VALID_UUID });
  if (result.voiceProfileId !== VALID_UUID) {
    throw new Error("voiceProfileId mismatch");
  }
});

Deno.test("handler returns 401 for missing Authorization", async () => {
  const handler = createStatusHandler(makeDeps());
  const response = await handler(
    new Request("http://localhost/", { method: "POST", body: "{}" }),
  );
  if (response.status !== 401) throw new Error(`expected 401, got ${response.status}`);
});

Deno.test("handler returns 401 for anon role", async () => {
  const handler = createStatusHandler(
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

Deno.test("handler returns 404 when profile not found", async () => {
  const handler = createStatusHandler(
    makeDeps({
      authenticator: {
        authenticate: async (_token: string) => ({ id: "user-1", role: "authenticated" }),
      },
      serviceClient: { getProfile: async () => null },
    }),
  );
  const body = JSON.stringify({ voiceProfileId: VALID_UUID });
  const response = await handler(
    makeRequest({ Authorization: "Bearer user-token" }, body),
  );
  if (response.status !== 404) throw new Error(`expected 404, got ${response.status}`);
});

Deno.test("handler returns 200 with VoiceProfileView when found", async () => {
  const profile: VoiceProfileView = {
    voiceProfileId: VALID_UUID,
    status: "ready",
    displayName: "My Voice",
  };
  const handler = createStatusHandler(
    makeDeps({
      authenticator: {
        authenticate: async (_token: string) => ({ id: "user-1", role: "authenticated" }),
      },
      serviceClient: { getProfile: async () => profile },
    }),
  );
  const body = JSON.stringify({ voiceProfileId: VALID_UUID });
  const response = await handler(
    makeRequest({ Authorization: "Bearer user-token" }, body),
  );
  if (response.status !== 200) throw new Error(`expected 200, got ${response.status}`);
  const data = await response.json();
  if (data.voiceProfileId !== VALID_UUID) throw new Error("voiceProfileId mismatch");
  if (data.status !== "ready") throw new Error("status mismatch");
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

type PartialDeps = {
  authenticator?: UserAuthenticator;
  serviceClient?: StatusServiceClient;
};

function makeDeps(overrides: PartialDeps = {}): StatusHandlerDeps {
  return {
    authenticator: overrides.authenticator ?? {
      authenticate: async (_token: string) => null,
    },
    serviceClient: overrides.serviceClient ?? {
      getProfile: async () => null,
    },
  };
}
