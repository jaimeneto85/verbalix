import { assertEquals, assertThrows } from "jsr:@std/assert";
import { parseRequest, MAX_AUDIO_BYTES } from "./contract.ts";

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
});

Deno.test("parseRequest - invalid body rejects with INVALID_REQUEST", () => {
  assertThrows(
    () => parseRequest(null),
    Error,
    "INVALID_REQUEST",
  );
  assertThrows(
    () => parseRequest("string"),
    Error,
    "INVALID_REQUEST",
  );
  assertThrows(
    () => parseRequest({}),
    Error,
    "INVALID_REQUEST",
  );
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
