import { assertEquals, assertRejects } from "jsr:@std/assert";
import { runInterpretPipeline, runStreamPipeline } from "./stages.ts";

const VALID_AUDIO_BASE64 = btoa("fake-audio-data");

function makeTextResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function makeSttResponse(): Response {
  return makeTextResponse({ text: "Hello", language_code: "en" });
}

function makeTranslateResponse(): Response {
  return makeTextResponse({ output: [{ content: [{ text: "Olá" }] }] });
}

function makeStubbedFetcher(responses: Response[]) {
  let idx = 0;
  return async () => responses[idx++];
}

function makeAbortFetcher(): (_input: string | URL | Request, _init?: RequestInit) => Promise<Response> {
  return async () => {
    throw new DOMException("AbortError", "AbortError");
  };
}

Deno.test("runStreamPipeline - STT AbortError throws PROVIDER_TIMEOUT", async () => {
  await assertRejects(
    () => runStreamPipeline(
      VALID_AUDIO_BASE64,
      "pt",
      "voice-id",
      undefined,
      makeAbortFetcher(),
      "el-key",
      "oai-key",
      "model",
    ),
    Error,
    "PROVIDER_TIMEOUT",
  );
});

Deno.test("runStreamPipeline - translate AbortError throws PROVIDER_TIMEOUT", async () => {
  let callIdx = 0;
  const abortFetcher = async (): Promise<Response> => {
    if (callIdx++ === 0) return makeSttResponse();
    throw new DOMException("AbortError", "AbortError");
  };

  await assertRejects(
    () => runStreamPipeline(
      VALID_AUDIO_BASE64,
      "pt",
      "voice-id",
      undefined,
      abortFetcher,
      "el-key",
      "oai-key",
      "model",
    ),
    Error,
    "PROVIDER_TIMEOUT",
  );
});

Deno.test("runStreamPipeline - TTS AbortError throws PROVIDER_TIMEOUT", async () => {
  let callIdx = 0;
  const abortFetcher = async (): Promise<Response> => {
    if (callIdx === 0) { callIdx++; return makeSttResponse(); }
    if (callIdx === 1) { callIdx++; return makeTranslateResponse(); }
    throw new DOMException("AbortError", "AbortError");
  };

  await assertRejects(
    () => runStreamPipeline(
      VALID_AUDIO_BASE64,
      "pt",
      "voice-id",
      undefined,
      abortFetcher,
      "el-key",
      "oai-key",
      "model",
    ),
    Error,
    "PROVIDER_TIMEOUT",
  );
});

Deno.test("runStreamPipeline - STT non-ok throws STT_FAILED", async () => {
  await assertRejects(
    () => runStreamPipeline(
      VALID_AUDIO_BASE64,
      "pt",
      "voice-id",
      undefined,
      makeStubbedFetcher([makeTextResponse({ error: "fail" }, 500)]),
      "el-key",
      "oai-key",
      "model",
    ),
    Error,
    "STT_FAILED",
  );
});

Deno.test("runStreamPipeline - translate non-ok throws TRANSLATION_FAILED", async () => {
  await assertRejects(
    () => runStreamPipeline(
      VALID_AUDIO_BASE64,
      "pt",
      "voice-id",
      undefined,
      makeStubbedFetcher([
        makeSttResponse(),
        makeTextResponse({ error: "fail" }, 500),
      ]),
      "el-key",
      "oai-key",
      "model",
    ),
    Error,
    "TRANSLATION_FAILED",
  );
});

Deno.test("runStreamPipeline - TTS non-ok throws TTS_FAILED", async () => {
  await assertRejects(
    () => runStreamPipeline(
      VALID_AUDIO_BASE64,
      "pt",
      "voice-id",
      undefined,
      makeStubbedFetcher([
        makeSttResponse(),
        makeTranslateResponse(),
        makeTextResponse({ error: "fail" }, 503),
      ]),
      "el-key",
      "oai-key",
      "model",
    ),
    Error,
    "TTS_FAILED",
  );
});

Deno.test("runStreamPipeline - with context returns ttsBody stream and sourceText", async () => {
  const pcmBytes = new Uint8Array([1, 2, 3, 4]);
  const ttsResponse = new Response(pcmBytes.buffer as ArrayBuffer, {
    status: 200,
    headers: { "Content-Type": "audio/pcm" },
  });

  const result = await runStreamPipeline(
    VALID_AUDIO_BASE64,
    "pt",
    "voice-id",
    [{ source: "Previous utterance" }],
    makeStubbedFetcher([makeSttResponse(), makeTranslateResponse(), ttsResponse]),
    "el-key",
    "oai-key",
    "model",
  );

  assertEquals(result.sourceText, "Hello");
  assertEquals(result.detectedLanguage, "en");
  assertEquals(typeof result.sttMs, "number");
  assertEquals(typeof result.translateMs, "number");
  assertEquals(result.ttsBody instanceof ReadableStream, true);
});

Deno.test("runInterpretPipeline - STT AbortError throws PROVIDER_TIMEOUT", async () => {
  await assertRejects(
    () => runInterpretPipeline(
      VALID_AUDIO_BASE64,
      "pt",
      "voice-id",
      makeAbortFetcher(),
      "el-key",
      "oai-key",
      "model",
    ),
    Error,
    "PROVIDER_TIMEOUT",
  );
});

Deno.test("runInterpretPipeline - translate AbortError throws PROVIDER_TIMEOUT", async () => {
  let callIdx = 0;
  const abortFetcher = async (): Promise<Response> => {
    if (callIdx++ === 0) return makeSttResponse();
    throw new DOMException("AbortError", "AbortError");
  };

  await assertRejects(
    () => runInterpretPipeline(
      VALID_AUDIO_BASE64,
      "pt",
      "voice-id",
      abortFetcher,
      "el-key",
      "oai-key",
      "model",
    ),
    Error,
    "PROVIDER_TIMEOUT",
  );
});

Deno.test("runInterpretPipeline - TTS AbortError throws PROVIDER_TIMEOUT", async () => {
  let callIdx = 0;
  const abortFetcher = async (): Promise<Response> => {
    if (callIdx === 0) { callIdx++; return makeSttResponse(); }
    if (callIdx === 1) { callIdx++; return makeTranslateResponse(); }
    throw new DOMException("AbortError", "AbortError");
  };

  await assertRejects(
    () => runInterpretPipeline(
      VALID_AUDIO_BASE64,
      "pt",
      "voice-id",
      abortFetcher,
      "el-key",
      "oai-key",
      "model",
    ),
    Error,
    "PROVIDER_TIMEOUT",
  );
});

Deno.test("runInterpretPipeline - happy path returns detectedLanguage and audioBase64", async () => {
  const fakeAudio = new Uint8Array([0x49, 0x44, 0x33]);
  const ttsResponse = new Response(fakeAudio.buffer as ArrayBuffer, {
    status: 200,
    headers: { "Content-Type": "audio/mpeg" },
  });

  const result = await runInterpretPipeline(
    VALID_AUDIO_BASE64,
    "pt",
    "voice-id",
    makeStubbedFetcher([makeSttResponse(), makeTranslateResponse(), ttsResponse]),
    "el-key",
    "oai-key",
    "model",
  );

  assertEquals(result.detectedLanguage, "en");
  assertEquals(typeof result.audioBase64, "string");
  assertEquals(result.audioBase64.length > 0, true);
  assertEquals(typeof result.stageMs.stt, "number");
  assertEquals(typeof result.stageMs.translate, "number");
  assertEquals(typeof result.stageMs.tts, "number");
});
