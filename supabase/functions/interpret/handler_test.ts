import { assertEquals } from "jsr:@std/assert";
import { handleInterpret } from "./handler.ts";
import type { InterpretHandlerDeps } from "./handler.ts";
import type { UserAuthenticator, AuthenticatedUser } from "../transform/auth.ts";
import type { InterpretServiceClient } from "./service_client.ts";

const VALID_UUID = "12345678-1234-4123-a123-123456789abc";

function makeValidAudioBase64(): string {
  return btoa("x".repeat(100));
}

function makeRequest(
  body: unknown,
  token?: string,
): Request {
  return new Request("http://localhost/interpret", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(body),
  });
}

class MockAuthenticator implements UserAuthenticator {
  constructor(private readonly user: AuthenticatedUser | null) {}
  async authenticate(_token: string): Promise<AuthenticatedUser | null> {
    return this.user;
  }
}

class MockServiceClient implements InterpretServiceClient {
  constructor(
    private readonly profile: { providerVoiceId: string } | null,
  ) {}
  async getReadyVoiceProfile(
    _userId: string,
  ): Promise<{ providerVoiceId: string } | null> {
    return this.profile;
  }
}

function makeStubbedFetcher(responses: Response[]): (input: string | URL | Request, init?: RequestInit) => Promise<Response> {
  let idx = 0;
  return async () => responses[idx++];
}

function makeSttResponse(): Response {
  return new Response(
    JSON.stringify({ text: "Hello", language_code: "en" }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
}

function makeTranslateResponse(): Response {
  return new Response(
    JSON.stringify({
      output: [{ content: [{ text: "Olá" }] }],
    }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
}

function makeTtsResponse(): Response {
  return new Response(new Uint8Array([1, 2, 3]), {
    status: 200,
    headers: { "Content-Type": "audio/mpeg" },
  });
}

Deno.test("handleInterpret - returns 401 when no JWT", async () => {
  const deps: InterpretHandlerDeps = {
    authenticator: new MockAuthenticator({ id: "user-1", role: "authenticated" }),
    serviceClient: new MockServiceClient({ providerVoiceId: "voice-id-1" }),
    fetcher: makeStubbedFetcher([]),
    elevenLabsKey: "",
    openAiKey: "",
    openAiModel: "test",
  };

  const req = new Request("http://localhost/interpret", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      requestId: VALID_UUID,
      targetLanguage: "pt",
      audioBase64: makeValidAudioBase64(),
      mimeType: "audio/wav",
    }),
  });

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 401);
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "UNAUTHENTICATED");
});

Deno.test("handleInterpret - returns 401 when authenticator returns null", async () => {
  const deps: InterpretHandlerDeps = {
    authenticator: new MockAuthenticator(null),
    serviceClient: new MockServiceClient({ providerVoiceId: "voice-id-1" }),
    fetcher: makeStubbedFetcher([]),
    elevenLabsKey: "",
    openAiKey: "",
    openAiModel: "test",
  };

  const req = makeRequest(
    {
      requestId: VALID_UUID,
      targetLanguage: "pt",
      audioBase64: makeValidAudioBase64(),
      mimeType: "audio/wav",
    },
    "some-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 401);
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "UNAUTHENTICATED");
});

Deno.test("handleInterpret - returns 404 when no voice profile", async () => {
  const deps: InterpretHandlerDeps = {
    authenticator: new MockAuthenticator({ id: "user-1", role: "authenticated" }),
    serviceClient: new MockServiceClient(null),
    fetcher: makeStubbedFetcher([]),
    elevenLabsKey: "",
    openAiKey: "",
    openAiModel: "test",
  };

  const req = makeRequest(
    {
      requestId: VALID_UUID,
      targetLanguage: "pt",
      audioBase64: makeValidAudioBase64(),
      mimeType: "audio/wav",
    },
    "valid-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 404);
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "NO_VOICE_PROFILE");
});

Deno.test("handleInterpret - calls pipeline on success and returns InterpretResponse", async () => {
  const deps: InterpretHandlerDeps = {
    authenticator: new MockAuthenticator({ id: "user-1", role: "authenticated" }),
    serviceClient: new MockServiceClient({ providerVoiceId: "voice-id-1" }),
    fetcher: makeStubbedFetcher([
      makeSttResponse(),
      makeTranslateResponse(),
      makeTtsResponse(),
    ]),
    elevenLabsKey: "",
    openAiKey: "",
    openAiModel: "test",
  };

  const req = makeRequest(
    {
      requestId: VALID_UUID,
      targetLanguage: "pt",
      audioBase64: makeValidAudioBase64(),
      mimeType: "audio/wav",
    },
    "valid-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 200);

  const body = await response.json() as {
    requestId: string;
    detectedLanguage: string;
    targetLanguage: string;
    audioBase64: string;
    mimeType: string;
    stageMs: { stt: number; translate: number; tts: number };
  };
  assertEquals(body.requestId, VALID_UUID);
  assertEquals(body.targetLanguage, "pt");
  assertEquals(body.detectedLanguage, "en");
  assertEquals(body.mimeType, "audio/mpeg");
  assertEquals(typeof body.audioBase64, "string");
  assertEquals(typeof body.stageMs.stt, "number");
  assertEquals(typeof body.stageMs.translate, "number");
  assertEquals(typeof body.stageMs.tts, "number");
});

Deno.test("handleInterpret - returns 400 for invalid request", async () => {
  const deps: InterpretHandlerDeps = {
    authenticator: new MockAuthenticator({ id: "user-1", role: "authenticated" }),
    serviceClient: new MockServiceClient({ providerVoiceId: "voice-id-1" }),
    fetcher: makeStubbedFetcher([]),
    elevenLabsKey: "",
    openAiKey: "",
    openAiModel: "test",
  };

  const req = makeRequest({ invalid: "body" }, "valid-token");

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 400);
});

Deno.test("handleInterpret - returns 405 for non-POST methods", async () => {
  const deps: InterpretHandlerDeps = {
    authenticator: new MockAuthenticator({ id: "user-1", role: "authenticated" }),
    serviceClient: new MockServiceClient({ providerVoiceId: "voice-id-1" }),
    fetcher: makeStubbedFetcher([]),
    elevenLabsKey: "",
    openAiKey: "",
    openAiModel: "test",
  };

  const req = new Request("http://localhost/interpret", {
    method: "GET",
    headers: { Authorization: "Bearer token" },
  });

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 405);
});
