import { assertEquals, assertRejects } from "jsr:@std/assert";
import { synthesizeStream } from "./provider.ts";

function makeTextResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

Deno.test("synthesizeStream - returns ReadableStream on ok response", async () => {
  const pcmBytes = new Uint8Array([0, 1, 2, 3, 4, 5]);

  const stubbedFetch = async (): Promise<Response> => {
    return new Response(pcmBytes.buffer as ArrayBuffer, {
      status: 200,
      headers: { "Content-Type": "audio/pcm" },
    });
  };

  const stream = await synthesizeStream("Hello", "voice-id", stubbedFetch, "key");
  assertEquals(stream instanceof ReadableStream, true);

  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
  }
  const total = new Uint8Array(chunks.reduce((a, c) => a + c.byteLength, 0));
  let off = 0;
  for (const c of chunks) { total.set(c, off); off += c.byteLength; }
  assertEquals(total, pcmBytes);
});

Deno.test("synthesizeStream - throws TTS_FAILED on non-ok response", async () => {
  const stubbedFetch = async (): Promise<Response> => {
    return makeTextResponse({ error: "bad request" }, 503);
  };

  await assertRejects(
    () => synthesizeStream("hello", "voice123", stubbedFetch, "key"),
    Error,
    "TTS_FAILED",
  );
});

Deno.test("synthesizeStream - requests pcm_24000 output format", async () => {
  let capturedBody: unknown;

  const stubbedFetch = async (
    _input: string | URL | Request,
    init?: RequestInit,
  ): Promise<Response> => {
    capturedBody = JSON.parse(init?.body as string);
    return new Response(new Uint8Array([1, 2]).buffer as ArrayBuffer, {
      status: 200,
    });
  };

  await synthesizeStream("Text", "voice-id", stubbedFetch, "key");

  const body = capturedBody as Record<string, unknown>;
  assertEquals(body.output_format, "pcm_24000");
});

Deno.test("synthesizeStream - throws TTS_FAILED when response body is null", async () => {
  const stubbedFetch = async (): Promise<Response> => {
    return new Response(null, { status: 200 });
  };

  await assertRejects(
    () => synthesizeStream("hello", "voice123", stubbedFetch, "key"),
    Error,
    "TTS_FAILED",
  );
});

Deno.test("synthesizeStream - calls correct ElevenLabs TTS endpoint with voiceId", async () => {
  let capturedUrl: string | undefined;

  const stubbedFetch = async (
    input: string | URL | Request,
  ): Promise<Response> => {
    capturedUrl = typeof input === "string" ? input : input.toString();
    return new Response(new Uint8Array([1]).buffer as ArrayBuffer, { status: 200 });
  };

  await synthesizeStream("Hello", "stream-voice", stubbedFetch, "key");

  assertEquals(
    capturedUrl,
    "https://api.elevenlabs.io/v1/text-to-speech/stream-voice",
  );
});
