import { assertEquals, assertRejects } from "jsr:@std/assert";
import { transcribe, translate, synthesize } from "./provider.ts";

function makeTextResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function makeAudioResponse(bytes: Uint8Array, status = 200): Response {
  return new Response(bytes.buffer as ArrayBuffer, {
    status,
    headers: { "Content-Type": "audio/mpeg" },
  });
}

Deno.test("transcribe - sends multipart form with audio and model", async () => {
  let capturedRequest: Request | undefined;

  const stubbedFetch = async (
    input: string | URL | Request,
    init?: RequestInit,
  ): Promise<Response> => {
    capturedRequest = new Request(input, init);
    return makeTextResponse({
      text: "Hello world",
      language_code: "en",
    });
  };

  const audioBase64 = btoa("fake-wav-data");
  const result = await transcribe(audioBase64, stubbedFetch);

  assertEquals(result.text, "Hello world");
  assertEquals(result.detectedLanguage, "en");
  assertEquals(typeof result.sttMs, "number");

  assertEquals(
    capturedRequest!.url,
    "https://api.elevenlabs.io/v1/speech-to-text",
  );
  assertEquals(capturedRequest!.method, "POST");
});

Deno.test("transcribe - throws STT_FAILED on non-ok response", async () => {
  const stubbedFetch = async (): Promise<Response> => {
    return makeTextResponse({ error: "bad request" }, 400);
  };

  await assertRejects(
    () => transcribe(btoa("data"), stubbedFetch),
    Error,
    "STT_FAILED",
  );
});

Deno.test("transcribe - throws STT_FAILED when response missing fields", async () => {
  const stubbedFetch = async (): Promise<Response> => {
    return makeTextResponse({ text: "hello" });
  };

  await assertRejects(
    () => transcribe(btoa("data"), stubbedFetch),
    Error,
    "STT_FAILED",
  );
});

Deno.test("translate - sends correct prompt with explicit target language", async () => {
  let capturedBody: unknown;

  const stubbedFetch = async (
    _input: string | URL | Request,
    init?: RequestInit,
  ): Promise<Response> => {
    capturedBody = JSON.parse(init?.body as string);
    return makeTextResponse({
      output: [
        {
          content: [{ text: "Olá mundo" }],
        },
      ],
    });
  };

  const result = await translate("Hello world", "pt", stubbedFetch);

  assertEquals(result.translatedText, "Olá mundo");
  assertEquals(typeof result.translateMs, "number");

  const body = capturedBody as Record<string, unknown>;
  assertEquals(typeof body.input, "string");
  const input = body.input as string;
  assertEquals(input.includes("pt"), true);
  assertEquals(input.includes("Hello world"), true);
});

Deno.test("translate - throws TRANSLATION_FAILED on non-ok response", async () => {
  const stubbedFetch = async (): Promise<Response> => {
    return makeTextResponse({ error: "bad request" }, 400);
  };

  await assertRejects(
    () => translate("hello", "pt", stubbedFetch),
    Error,
    "TRANSLATION_FAILED",
  );
});

Deno.test("translate - throws TRANSLATION_FAILED on unexpected response shape", async () => {
  const stubbedFetch = async (): Promise<Response> => {
    return makeTextResponse({ result: "something" });
  };

  await assertRejects(
    () => translate("hello", "pt", stubbedFetch),
    Error,
    "TRANSLATION_FAILED",
  );
});

Deno.test("synthesize - returns audio as base64", async () => {
  const fakeAudio = new Uint8Array([0x49, 0x44, 0x33]);
  const stubbedFetch = async (): Promise<Response> => {
    return makeAudioResponse(fakeAudio);
  };

  const result = await synthesize("Hello", "voice123", stubbedFetch);

  assertEquals(typeof result.audioBase64, "string");
  assertEquals(result.audioBase64.length > 0, true);
  assertEquals(typeof result.ttsMs, "number");

  const decoded = atob(result.audioBase64);
  const decodedBytes = new Uint8Array(decoded.length);
  for (let i = 0; i < decoded.length; i++) {
    decodedBytes[i] = decoded.charCodeAt(i);
  }
  assertEquals(decodedBytes, fakeAudio);
});

Deno.test("synthesize - throws TTS_FAILED on non-ok response", async () => {
  const stubbedFetch = async (): Promise<Response> => {
    return makeTextResponse({ error: "bad request" }, 400);
  };

  await assertRejects(
    () => synthesize("hello", "voice123", stubbedFetch),
    Error,
    "TTS_FAILED",
  );
});

Deno.test("synthesize - calls correct ElevenLabs TTS endpoint with voiceId", async () => {
  let capturedUrl: string | undefined;

  const stubbedFetch = async (
    input: string | URL | Request,
  ): Promise<Response> => {
    capturedUrl = typeof input === "string" ? input : input.toString();
    return makeAudioResponse(new Uint8Array([1, 2, 3]));
  };

  await synthesize("Hello", "my-voice-id", stubbedFetch);

  assertEquals(
    capturedUrl,
    "https://api.elevenlabs.io/v1/text-to-speech/my-voice-id",
  );
});
