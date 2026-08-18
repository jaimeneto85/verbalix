import { parseEnrollRequest } from "./contract.ts";
import {
  createEnrollHandler,
  createSupabaseServiceClient,
  type SupabaseServiceClient,
  type UpsertResult,
} from "./handler.ts";
import type { UserAuthenticator } from "../transform/auth.ts";
import type { VoiceProvider } from "./provider.ts";
import type { VoiceProfileView } from "./contract.ts";

const VALID_UUID = "b65c8888-fb0e-4a8f-9fee-95268995bf68";
const VALID_SAMPLE = btoa("fake-wav-data");

Deno.test("parseEnrollRequest rejects missing fields", () => {
  assertThrows(() => parseEnrollRequest({}), "INVALID_REQUEST");
});

Deno.test("parseEnrollRequest rejects invalid UUID", () => {
  assertThrows(
    () =>
      parseEnrollRequest({
        requestId: "not-a-uuid",
        displayName: "My Voice",
        sampleBase64: VALID_SAMPLE,
        mimeType: "audio/wav",
      }),
    "INVALID_REQUEST",
  );
});

Deno.test("parseEnrollRequest rejects oversized sampleBase64", () => {
  const oversized = "a".repeat(10 * 1024 * 1024 * 1.5 + 1);
  assertThrows(
    () =>
      parseEnrollRequest({
        requestId: VALID_UUID,
        displayName: "My Voice",
        sampleBase64: oversized,
        mimeType: "audio/wav",
      }),
    "SAMPLE_TOO_LARGE",
  );
});

Deno.test("parseEnrollRequest rejects invalid mimeType", () => {
  assertThrows(
    () =>
      parseEnrollRequest({
        requestId: VALID_UUID,
        displayName: "My Voice",
        sampleBase64: VALID_SAMPLE,
        mimeType: "audio/mp3",
      }),
    "INVALID_REQUEST",
  );
});

Deno.test("parseEnrollRequest returns parsed request for valid input", () => {
  const result = parseEnrollRequest({
    requestId: VALID_UUID,
    displayName: "  My Voice  ",
    sampleBase64: VALID_SAMPLE,
    mimeType: "audio/wav",
  });
  if (result.requestId !== VALID_UUID) throw new Error("requestId mismatch");
  if (result.displayName !== "My Voice") throw new Error("displayName not trimmed");
  if (result.mimeType !== "audio/wav") throw new Error("mimeType mismatch");
});

Deno.test("handler returns 401 for missing Authorization", async () => {
  const handler = createEnrollHandler(makeDeps());
  const response = await handler(new Request("http://localhost/", { method: "POST" }));
  if (response.status !== 401) throw new Error(`expected 401, got ${response.status}`);
});

Deno.test("handler returns 401 for anon role", async () => {
  const handler = createEnrollHandler(
    makeDeps({
      authenticator: {
        authenticate: async (_token: string) => ({ id: "user-1", role: "anon" }),
      },
    }),
  );
  const response = await handler(
    makeRequest({ Authorization: "Bearer anon-token" }),
  );
  if (response.status !== 401) throw new Error(`expected 401, got ${response.status}`);
});

Deno.test("handler returns 200 VoiceProfileView on success", async () => {
  const profile: VoiceProfileView = {
    voiceProfileId: VALID_UUID,
    status: "ready",
    displayName: "My Voice",
  };
  const handler = createEnrollHandler(
    makeDeps({
      authenticator: {
        authenticate: async (_token: string) => ({ id: "user-1", role: "authenticated" }),
      },
      provider: {
        enroll: async () => "elabs-voice-id",
        deleteVoice: async () => {},
      },
      serviceClient: makeServiceClient(profile),
    }),
  );
  const body = JSON.stringify({
    requestId: VALID_UUID,
    displayName: "My Voice",
    sampleBase64: VALID_SAMPLE,
    mimeType: "audio/wav",
  });
  const response = await handler(
    makeRequest({ Authorization: "Bearer user-token" }, body),
  );
  if (response.status !== 200) throw new Error(`expected 200, got ${response.status}`);
  const data = await response.json();
  if (data.voiceProfileId !== VALID_UUID) throw new Error("voiceProfileId mismatch");
});

Deno.test("handler returns 504 on provider timeout", async () => {
  const handler = createEnrollHandler(
    makeDeps({
      authenticator: {
        authenticate: async (_token: string) => ({ id: "user-1", role: "authenticated" }),
      },
      provider: {
        enroll: () => new Promise(() => {}),
        deleteVoice: async () => {},
      },
      serviceClient: makeServiceClient(null),
      timeout: {
        schedule: (cb: () => void, _delay: number) => {
          cb();
          return 0;
        },
        cancel: (_h: unknown) => {},
      },
    }),
  );
  const body = JSON.stringify({
    requestId: VALID_UUID,
    displayName: "My Voice",
    sampleBase64: VALID_SAMPLE,
    mimeType: "audio/wav",
  });
  const response = await handler(
    makeRequest({ Authorization: "Bearer user-token" }, body),
  );
  if (response.status !== 504) throw new Error(`expected 504, got ${response.status}`);
});

Deno.test("handler returns 200 for existing requestId (dedup)", async () => {
  const profile: VoiceProfileView = {
    voiceProfileId: VALID_UUID,
    status: "ready",
    displayName: "My Voice",
  };
  let upsertCallCount = 0;
  const handler = createEnrollHandler(
    makeDeps({
      authenticator: {
        authenticate: async (_token: string) => ({ id: "user-1", role: "authenticated" }),
      },
      provider: {
        enroll: async () => "elabs-voice-id",
        deleteVoice: async () => {},
      },
      serviceClient: {
        ...makeServiceClient(profile),
        upsertEnrolling: async (): Promise<UpsertResult> => {
          upsertCallCount++;
          return { voiceProfileId: VALID_UUID, alreadyDone: true };
        },
      },
    }),
  );
  const body = JSON.stringify({
    requestId: VALID_UUID,
    displayName: "My Voice",
    sampleBase64: VALID_SAMPLE,
    mimeType: "audio/wav",
  });
  const response = await handler(
    makeRequest({ Authorization: "Bearer user-token" }, body),
  );
  if (response.status !== 200) throw new Error(`expected 200, got ${response.status}`);
  const data = await response.json();
  if (data.voiceProfileId !== VALID_UUID) throw new Error("voiceProfileId mismatch");
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

function makeRequest(headers: Record<string, string>, body?: string): Request {
  return new Request("http://localhost/", {
    method: "POST",
    headers,
    body: body ?? "{}",
  });
}

function makeServiceClient(profile: VoiceProfileView | null): SupabaseServiceClient {
  const defaultUpsert: UpsertResult = {
    voiceProfileId: VALID_UUID,
    alreadyDone: false,
  };
  return {
    upsertEnrolling: async () => defaultUpsert,
    setReady: async () => {},
    setFailed: async () => {},
    getProfile: async () => profile,
    getPreviousProfile: async () => null,
    deleteProfile: async () => {},
  };
}

type PartialDeps = {
  authenticator?: UserAuthenticator;
  provider?: VoiceProvider;
  serviceClient?: SupabaseServiceClient;
  timeout?: { schedule: (cb: () => void, delay: number) => unknown; cancel: (h: unknown) => void };
};

function makeDeps(overrides: PartialDeps = {}) {
  return {
    authenticator: overrides.authenticator ?? {
      authenticate: async (_token: string) => null,
    },
    provider: overrides.provider ?? {
      enroll: async () => "voice-id",
      deleteVoice: async () => {},
    },
    getSecret: (_name: string) => undefined,
    serviceClient: overrides.serviceClient ?? makeServiceClient(null),
    timeout: overrides.timeout ?? {
      schedule: (_cb: () => void, _delay: number) => 0,
      cancel: (_h: unknown) => {},
    },
  };
}
