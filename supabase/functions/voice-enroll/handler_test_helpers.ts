import type { UserAuthenticator } from "../transform/auth.ts";
import type { VoiceProfileView } from "./contract.ts";
import type {
  EnrollHandlerDeps,
  SupabaseServiceClient,
  UpsertResult,
} from "./handler.ts";
import type { VoiceProvider } from "./provider.ts";

export const REQUEST_ID = "b65c8888-fb0e-4a8f-9fee-95268995bf68";

export type StateOptions = {
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

export function createState(options: StateOptions = {}) {
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
    { voiceProfileId: REQUEST_ID, alreadyDone: false };

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

export function validRequest(body = validBody()) {
  return new Request("https://example.test", {
    method: "POST",
    headers: {
      Authorization: "Bearer user-token",
      "Content-Type": "application/json",
    },
    body,
  });
}

export function validBody() {
  return JSON.stringify({
    requestId: REQUEST_ID,
    displayName: "Minha Voz",
    sampleBase64: "c2FtcGxl",
    mimeType: "audio/wav",
  });
}

export async function assertError(
  response: Response,
  status: number,
  code: string,
) {
  assertEquals(response.status, status);
  assertEquals(response.headers.get("cache-control"), "no-store");
  const body = await response.json();
  assertEquals(body.error.code, code);
}

export function assertEquals(actual: unknown, expected: unknown) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(
      `expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`,
    );
  }
}
