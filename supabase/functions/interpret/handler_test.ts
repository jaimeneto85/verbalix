import { assertEquals } from "jsr:@std/assert";
import { handleInterpret } from "./handler.ts";
import type { InterpretHandlerDeps } from "./handler.ts";
import type { UserAuthenticator, AuthenticatedUser } from "../transform/auth.ts";
import type { InterpretServiceClient } from "./service_client.ts";
import { METADATA_MAGIC } from "./streaming.ts";

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

class MockServiceClient implements InterpretServiceClient {
  constructor(private readonly profile: { providerVoiceId: string } | null) {}
  async getReadyVoiceProfile(_userId: string): Promise<{ providerVoiceId: string } | null> {
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
    JSON.stringify({ output: [{ content: [{ text: "Olá" }] }] }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
}

function makeTtsResponse(): Response {
  return new Response(new Uint8Array([1, 2, 3]), {
    status: 200,
    headers: { "Content-Type": "audio/mpeg" },
  });
}

function makePcmStreamResponse(bytes: Uint8Array = new Uint8Array([10, 20, 30])): Response {
  return new Response(bytes.buffer as ArrayBuffer, {
    status: 200,
    headers: { "Content-Type": "audio/pcm" },
  });
}

function makeDefaultDeps(fetcher?: ReturnType<typeof makeStubbedFetcher>): InterpretHandlerDeps {
  return {
    authenticator: new MockAuthenticator({ id: "user-1", role: "authenticated" }),
    serviceClient: new MockServiceClient({ providerVoiceId: "voice-id-1" }),
    fetcher: fetcher ?? makeStubbedFetcher([]),
    elevenLabsKey: "",
    openAiKey: "",
    openAiModel: "test",
  };
}

Deno.test("handleInterpret - returns 401 when no JWT", async () => {
  const deps = makeDefaultDeps();
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
  const deps = { ...makeDefaultDeps(), authenticator: new MockAuthenticator(null) };
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav" },
    "some-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 401);
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "UNAUTHENTICATED");
});

Deno.test("handleInterpret - returns 404 when no voice profile", async () => {
  const deps = { ...makeDefaultDeps(), serviceClient: new MockServiceClient(null) };
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav" },
    "valid-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 404);
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "NO_VOICE_PROFILE");
});

Deno.test("CA02 - no stream returns JSON response identical to M2 contract", async () => {
  const deps = makeDefaultDeps(makeStubbedFetcher([
    makeSttResponse(),
    makeTranslateResponse(),
    makeTtsResponse(),
  ]));
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav" },
    "valid-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 200);
  assertEquals(response.headers.get("Content-Type"), "application/json");

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

Deno.test("CA02 - stream:false returns JSON identical to no-stream", async () => {
  const deps = makeDefaultDeps(makeStubbedFetcher([
    makeSttResponse(),
    makeTranslateResponse(),
    makeTtsResponse(),
  ]));
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav", stream: false },
    "valid-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 200);
  assertEquals(response.headers.get("Content-Type"), "application/json");
  const body = await response.json() as { mimeType: string };
  assertEquals(body.mimeType, "audio/mpeg");
});

Deno.test("CA01 - stream:true returns streaming response with X-Verbalix-* headers", async () => {
  const pcmBytes = new Uint8Array([100, 200, 150]);
  const deps = makeDefaultDeps(makeStubbedFetcher([
    makeSttResponse(),
    makeTranslateResponse(),
    makePcmStreamResponse(pcmBytes),
  ]));
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav", stream: true },
    "valid-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 200);
  assertEquals(response.headers.get("Content-Type"), "application/octet-stream");
  assertEquals(response.headers.get("X-Verbalix-Detected-Language"), "en");
  assertEquals(response.headers.get("X-Verbalix-Target-Language"), "pt");
  assertEquals(response.headers.get("X-Verbalix-Audio-Format"), "pcm_24000");
  assertEquals(typeof response.headers.get("X-Verbalix-Stt-Ms"), "string");
  assertEquals(typeof response.headers.get("X-Verbalix-Translate-Ms"), "string");
});

Deno.test("CA01 - stream body starts with VLBX magic and metadata frame", async () => {
  const pcmBytes = new Uint8Array([10, 20, 30, 40]);
  const deps = makeDefaultDeps(makeStubbedFetcher([
    makeSttResponse(),
    makeTranslateResponse(),
    makePcmStreamResponse(pcmBytes),
  ]));
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav", stream: true },
    "valid-token",
  );

  const response = await handleInterpret(req, deps);
  const bodyBytes = new Uint8Array(await response.arrayBuffer());

  assertEquals(bodyBytes[0], METADATA_MAGIC[0]);
  assertEquals(bodyBytes[1], METADATA_MAGIC[1]);
  assertEquals(bodyBytes[2], METADATA_MAGIC[2]);
  assertEquals(bodyBytes[3], METADATA_MAGIC[3]);

  const jsonLen = new DataView(bodyBytes.buffer).getUint32(4, false);
  assertEquals(jsonLen > 0, true);

  const jsonBytes = bodyBytes.slice(8, 8 + jsonLen);
  const meta = JSON.parse(new TextDecoder().decode(jsonBytes)) as { sourceText: string };
  assertEquals(typeof meta.sourceText, "string");
  assertEquals(meta.sourceText, "Hello");

  const pcmSection = bodyBytes.slice(8 + jsonLen);
  assertEquals(pcmSection, pcmBytes);
});

Deno.test("CA03 - STT failure returns JSON error before any streaming bytes", async () => {
  const deps = makeDefaultDeps(makeStubbedFetcher([
    new Response(JSON.stringify({ error: "fail" }), { status: 500 }),
  ]));
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav", stream: true },
    "valid-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 502);
  assertEquals(response.headers.get("Content-Type"), "application/json");
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "STT_FAILED");
});

Deno.test("CA04 - translate failure returns JSON error before streaming", async () => {
  const deps = makeDefaultDeps(makeStubbedFetcher([
    makeSttResponse(),
    new Response(JSON.stringify({ error: "fail" }), { status: 500 }),
  ]));
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav", stream: true },
    "valid-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 502);
  assertEquals(response.headers.get("Content-Type"), "application/json");
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "TRANSLATION_FAILED");
});

Deno.test("CA05 - TTS non-ok response returns JSON error before streaming", async () => {
  const deps = makeDefaultDeps(makeStubbedFetcher([
    makeSttResponse(),
    makeTranslateResponse(),
    new Response(JSON.stringify({ error: "fail" }), { status: 503 }),
  ]));
  const req = makeRequest(
    { requestId: VALID_UUID, targetLanguage: "pt", audioBase64: makeValidAudioBase64(), mimeType: "audio/wav", stream: true },
    "valid-token",
  );

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 502);
  assertEquals(response.headers.get("Content-Type"), "application/json");
  const body = await response.json() as { error: { code: string } };
  assertEquals(body.error.code, "TTS_FAILED");
});

Deno.test("handleInterpret - returns 400 for invalid request", async () => {
  const deps = makeDefaultDeps();
  const req = makeRequest({ invalid: "body" }, "valid-token");

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 400);
});

Deno.test("handleInterpret - returns 405 for non-POST methods", async () => {
  const deps = makeDefaultDeps();
  const req = new Request("http://localhost/interpret", {
    method: "GET",
    headers: { Authorization: "Bearer token" },
  });

  const response = await handleInterpret(req, deps);
  assertEquals(response.status, 405);
});
