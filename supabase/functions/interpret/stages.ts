import type { Fetcher } from "./provider.ts";
import { transcribe, translate, synthesize } from "./provider.ts";

export async function runInterpretPipeline(
  audioBase64: string,
  targetLanguage: string,
  providerVoiceId: string,
  fetcher: Fetcher,
  elevenLabsKey: string,
  openAiKey: string,
  openAiModel: string,
): Promise<{
  detectedLanguage: string;
  audioBase64: string;
  stageMs: { stt: number; translate: number; tts: number };
}> {
  let sttResult: { text: string; detectedLanguage: string; sttMs: number };
  try {
    sttResult = await transcribe(audioBase64, fetcher, elevenLabsKey);
  } catch (reason) {
    if (reason instanceof DOMException && reason.name === "AbortError") {
      throw new Error("PROVIDER_TIMEOUT");
    }
    throw new Error("STT_FAILED");
  }

  let translateResult: { translatedText: string; translateMs: number };
  try {
    translateResult = await translate(
      sttResult.text,
      targetLanguage,
      fetcher,
      openAiKey,
      openAiModel,
    );
  } catch (reason) {
    if (reason instanceof DOMException && reason.name === "AbortError") {
      throw new Error("PROVIDER_TIMEOUT");
    }
    throw new Error("TRANSLATION_FAILED");
  }

  let synthesizeResult: { audioBase64: string; ttsMs: number };
  try {
    synthesizeResult = await synthesize(
      translateResult.translatedText,
      providerVoiceId,
      fetcher,
      elevenLabsKey,
    );
  } catch (reason) {
    if (reason instanceof DOMException && reason.name === "AbortError") {
      throw new Error("PROVIDER_TIMEOUT");
    }
    throw new Error("TTS_FAILED");
  }

  return {
    detectedLanguage: sttResult.detectedLanguage,
    audioBase64: synthesizeResult.audioBase64,
    stageMs: {
      stt: sttResult.sttMs,
      translate: translateResult.translateMs,
      tts: synthesizeResult.ttsMs,
    },
  };
}
