import { assertEquals } from "jsr:@std/assert";
import { handleInterpret } from "./handler.ts";
import type { InterpretHandlerDeps } from "./handler.ts";
import type { UserAuthenticator, AuthenticatedUser } from "../transform/auth.ts";
import type { InterpretServiceClient } from "./service_client.ts";
import { MAX_INTERPRET_BODY_BYTES } from "./handler.ts";

const VALID_UUID = "12345678-1234-4123-a123-123456789abc";

function makeValidAudioBase64(): string {
  return btoa("x".repeat(100));
}

function makeRequest(body: unknown, token?: string): Request {
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

class ThrowingAuthenticator implements UserAuthenticator {
  constructor(private readonly err: unknown) {}
  async authenticate(_token: string): Promise<AuthenticatedUser | null> {
    throw this.err;
  }
}

class MockServiceClient implements InterpretServiceClient {
  constructor(private readonly profile: { providerVoiceId: string } | null) {}
  async getReadyVoiceProfile(_userId: string): Promise<{ providerVoiceId: string } | null> {
    return this.profile;
  }
}

function makeDefaultDeps(overrides: Partial<InterpretHandlerDeps> = {}): InterpretHandlerDeps {
  return {
    authenticator: new MockAuthenticator({ id: "user-1", role: "authenticated" }),
    serviceClient: new MockServiceClient({ providerVoiceId: "voice-id-1" }),
    fetcher: async () => new Response("{}", { status: 200 }),
    elevenLabsKey: "",
    openAiKey: "",
    openAiModel: "test",
    ...overrides,
  };
}

Deno.test("handleInterpret - returns 401 when user role is anon", async () => {
  const deps = makeDefaultDeps({
    authenticator: new MockAuthenticator({ id: "user-1", role: "anon" }),
  });
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav" },
    "some-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 401);
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "UNAUTHENTICATED");
});

Deno.test("handleInterpret - returns 401 when user role is anonymous", async () => {
  const deps = makeDefaultDeps({
    authenticator: new MockAuthenticator({ id: "user-1", role: "anonymous" }),
  });
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav" },
    "some-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 401);
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "UNAUTHENTICATED");
});

Deno.test("handleInterpret - PROVIDER_TIMEOUT when authenticator throws DOMException AbortError", async () => {
  const abortErr = new DOMException("Request timed out", "AbortError");
  const deps = makeDefaultDeps({
    authenticator: new ThrowingAuthenticator(abortErr),
  });
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav" },
    "some-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 502);
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "PROVIDER_TIMEOUT");
});

Deno.test("handleInterpret - INTERNAL_ERROR when authenticator throws unknown error", async () => {
  const deps = makeDefaultDeps({
    authenticator: new ThrowingAuthenticator(new Error("unexpected database failure")),
  });
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav" },
    "some-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 500);
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "INTERNAL_ERROR");
});

Deno.test("handleInterpret - AUDIO_TOO_LARGE when content-length header exceeds limit", async () => {
  const deps = makeDefaultDeps();
  const req = new Request("http://localhost/interpret", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: "Bearer some-token",
      "Content-Length": String(MAX_INTERPRET_BODY_BYTES + 1),
    },
    body: JSON.stringify({
      requestId: VALID_UUID,
      targetLanguage: "pt",
      audioBase64: makeValidAudioBase64(),
      mimeType: "audio/wav",
    }),
  });

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 400);
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "AUDIO_TOO_LARGE");
});

Deno.test("handleInterpret - AUDIO_TOO_LARGE when content-length header is not a digit string", async () => {
  const deps = makeDefaultDeps();
  const req = new Request("http://localhost/interpret", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: "Bearer some-token",
      "Content-Length": "invalid-length",
    },
    body: JSON.stringify({
      requestId: VALID_UUID,
      targetLanguage: "pt",
      audioBase64: makeValidAudioBase64(),
      mimeType: "audio/wav",
    }),
  });

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 400);
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "AUDIO_TOO_LARGE");
});

Deno.test("handleInterpret - INTERNAL_ERROR when non-Error object is thrown", async () => {
  const deps = makeDefaultDeps({
    authenticator: new ThrowingAuthenticator("string error not an Error object"),
  });
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav" },
    "some-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 500);
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "INTERNAL_ERROR");
});

Deno.test("CA01 - stream:true with context included still returns streaming response", async () => {
  const sttResponse = new Response(
    JSON.stringify({ text: "Hello", language_code: "en" }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
  const translateResponse = new Response(
    JSON.stringify({ output: [{ content: [{ text: "Olá" }] }] }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
  const pcmBytes = new Uint8Array([5, 10, 15]);
  const ttsResponse = new Response(pcmBytes.buffer as ArrayBuffer, {
    status: 200,
    headers: { "Content-Type": "audio/pcm" },
  });

  let fetchIdx = 0;
  const responses = [sttResponse, translateResponse, ttsResponse];
  const deps = makeDefaultDeps({
    fetcher: async () => responses[fetchIdx++],
  });

  const req = makeRequest(
    {
      requestId: VALID_UUID,
      targetLanguage: "pt",
      audioBase64: makeValidAudioBase64(),
      mimeType: "audio/wav",
      stream: true,
      context: [{ source: "Previous segment text" }],
    },
    "valid-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 200);
  assertEquals(response.headers.get("Content-Type"), "application/octet-stream");
  assertEquals(response.headers.get("X-Verbalix-Detected-Language"), "en");
  assertEquals(response.headers.get("X-Verbalix-Audio-Format"), "pcm_24000");

  const bodyBytes = new Uint8Array(await response.arrayBuffer());
  assertEquals(bodyBytes[0], 0x56);
  assertEquals(bodyBytes[1], 0x4c);
  assertEquals(bodyBytes[2], 0x42);
  assertEquals(bodyBytes[3], 0x58);

  const jsonLen = new DataView(bodyBytes.buffer).getUint32(4, false);
  const jsonBytes = bodyBytes.slice(8, 8 + jsonLen);
  const meta = JSON.parse(new TextDecoder().decode(jsonBytes)) as { sourceText: string };
  assertEquals(meta.sourceText, "Hello");
});
