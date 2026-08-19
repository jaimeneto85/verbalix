import { assertEquals, assertThrows } from "jsr:@std/assert";
import {
  parseRequest,
  MAX_AUDIO_BYTES,
  MAX_CONTEXT_ITEMS,
  MAX_CONTEXT_SOURCE_CHARS,
} from "./contract.ts";

const VALID_UUID = "12345678-1234-4123-a123-123456789abc";

Deno.test("parseRequest - valid request passes", () => {
  const result = parseRequest({
    requestId: VALID_UUID,
    targetLanguage: "en",
    audioBase64: btoa("hello"),
    mimeType: "audio/wav",
  });
  assertEquals(result.requestId, VALID_UUID);
  assertEquals(result.targetLanguage, "en");
  assertEquals(result.mimeType, "audio/wav");
  assertEquals(result.stream, false);
  assertEquals(result.context, undefined);
});

Deno.test("parseRequest - stream:true is parsed", () => {
  const result = parseRequest({
    requestId: VALID_UUID,
    targetLanguage: "en",
    audioBase64: btoa("hello"),
    mimeType: "audio/wav",
    stream: true,
  });
  assertEquals(result.stream, true);
});

Deno.test("parseRequest - stream absent defaults to false", () => {
  const result = parseRequest({
    requestId: VALID_UUID,
    targetLanguage: "en",
    audioBase64: btoa("hello"),
    mimeType: "audio/wav",
  });
  assertEquals(result.stream, false);
});

Deno.test("parseRequest - stream:false is parsed", () => {
  const result = parseRequest({
    requestId: VALID_UUID,
    targetLanguage: "en",
    audioBase64: btoa("hello"),
    mimeType: "audio/wav",
    stream: false,
  });
  assertEquals(result.stream, false);
});

Deno.test("parseRequest - valid context is parsed and returned", () => {
  const result = parseRequest({
    requestId: VALID_UUID,
    targetLanguage: "en",
    audioBase64: btoa("hello"),
    mimeType: "audio/wav",
    context: [{ source: "Hello world" }],
  });
  assertEquals(result.context?.length, 1);
  assertEquals(result.context?.[0].source, "Hello world");
});

Deno.test("parseRequest - context capped to MAX_CONTEXT_ITEMS items", () => {
  const result = parseRequest({
    requestId: VALID_UUID,
    targetLanguage: "en",
    audioBase64: btoa("hello"),
    mimeType: "audio/wav",
    context: [
      { source: "A" },
      { source: "B" },
      { source: "C" },
    ],
  });
  assertEquals(result.context?.length, MAX_CONTEXT_ITEMS);
  assertEquals(result.context?.[0].source, "A");
  assertEquals(result.context?.[1].source, "B");
});

Deno.test("parseRequest - context source truncated to MAX_CONTEXT_SOURCE_CHARS scalars", () => {
  const longSource = "あ".repeat(MAX_CONTEXT_SOURCE_CHARS + 50);
  const result = parseRequest({
    requestId: VALID_UUID,
    targetLanguage: "en",
    audioBase64: btoa("hello"),
    mimeType: "audio/wav",
    context: [{ source: longSource }],
  });
  const source = result.context?.[0].source ?? "";
  assertEquals([...source].length, MAX_CONTEXT_SOURCE_CHARS);
});

Deno.test("parseRequest - empty context array returns undefined", () => {
  const result = parseRequest({
    requestId: VALID_UUID,
    targetLanguage: "en",
    audioBase64: btoa("hello"),
    mimeType: "audio/wav",
    context: [],
  });
  assertEquals(result.context, undefined);
});

Deno.test("parseRequest - context absent returns undefined", () => {
  const result = parseRequest({
    requestId: VALID_UUID,
    targetLanguage: "en",
    audioBase64: btoa("hello"),
    mimeType: "audio/wav",
  });
  assertEquals(result.context, undefined);
});

Deno.test("parseRequest - context items with non-string source are skipped", () => {
  const result = parseRequest({
    requestId: VALID_UUID,
    targetLanguage: "en",
    audioBase64: btoa("hello"),
    mimeType: "audio/wav",
    context: [{ source: 42 }, { source: "valid" }],
  });
  assertEquals(result.context?.length, 1);
  assertEquals(result.context?.[0].source, "valid");
});

Deno.test("parseRequest - invalid body rejects with INVALID_REQUEST", () => {
  assertThrows(() => parseRequest(null), Error, "INVALID_REQUEST");
  assertThrows(() => parseRequest("string"), Error, "INVALID_REQUEST");
  assertThrows(() => parseRequest({}), Error, "INVALID_REQUEST");
});

Deno.test("parseRequest - missing audioBase64 rejects", () => {
  assertThrows(
    () =>
      parseRequest({
        requestId: VALID_UUID,
        targetLanguage: "en",
        mimeType: "audio/wav",
      }),
    Error,
    "INVALID_REQUEST",
  );
});

Deno.test("parseRequest - empty audioBase64 rejects", () => {
  assertThrows(
    () =>
      parseRequest({
        requestId: VALID_UUID,
        targetLanguage: "en",
        audioBase64: "",
        mimeType: "audio/wav",
      }),
    Error,
    "INVALID_REQUEST",
  );
});

Deno.test("parseRequest - unsupported language rejects with LANGUAGE_UNSUPPORTED", () => {
  assertThrows(
    () =>
      parseRequest({
        requestId: VALID_UUID,
        targetLanguage: "xx",
        audioBase64: btoa("hello"),
        mimeType: "audio/wav",
      }),
    Error,
    "LANGUAGE_UNSUPPORTED",
  );
});

Deno.test("parseRequest - oversized audio rejects with AUDIO_TOO_LARGE", () => {
  const oversizedBase64 = "A".repeat(
    Math.ceil(MAX_AUDIO_BYTES * (4 / 3)) + 100,
  );
  assertThrows(
    () =>
      parseRequest({
        requestId: VALID_UUID,
        targetLanguage: "pt",
        audioBase64: oversizedBase64,
        mimeType: "audio/wav",
      }),
    Error,
    "AUDIO_TOO_LARGE",
  );
});

Deno.test("parseRequest - invalid requestId rejects", () => {
  assertThrows(
    () =>
      parseRequest({
        requestId: "not-a-uuid",
        targetLanguage: "en",
        audioBase64: btoa("hello"),
        mimeType: "audio/wav",
      }),
    Error,
    "INVALID_REQUEST",
  );
});

Deno.test("parseRequest - invalid mimeType rejects", () => {
  assertThrows(
    () =>
      parseRequest({
        requestId: VALID_UUID,
        targetLanguage: "en",
        audioBase64: btoa("hello"),
        mimeType: "audio/mp3",
      }),
    Error,
    "INVALID_REQUEST",
  );
});

Deno.test("parseRequest - all BCP-47 supported languages pass", () => {
  const supported = ["en", "pt", "es", "fr", "de", "it", "ja", "ko", "zh"];
  for (const lang of supported) {
    const result = parseRequest({
      requestId: VALID_UUID,
      targetLanguage: lang,
      audioBase64: btoa("hello"),
      mimeType: "audio/wav",
    });
    assertEquals(result.targetLanguage, lang);
  }
});
