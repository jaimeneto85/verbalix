export type Fetcher = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>;

export async function transcribe(
  audioBase64: string,
  fetcher: Fetcher,
): Promise<{ text: string; detectedLanguage: string; sttMs: number }> {
  const apiKey = Deno.env.get("ELEVENLABS_API_KEY") ?? "";
  const start = Date.now();

  const binary = atob(audioBase64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  const audioBlob = new Blob([bytes.buffer as ArrayBuffer], {
    type: "audio/wav",
  });

  const form = new FormData();
  form.append("file", audioBlob, "audio.wav");
  form.append("model_id", "scribe_v1");

  const response = await fetcher(
    "https://api.elevenlabs.io/v1/speech-to-text",
    {
      method: "POST",
      headers: { "xi-api-key": apiKey },
      body: form,
    },
  );

  if (!response.ok) {
    throw new Error("STT_FAILED");
  }

  let data: unknown;
  try {
    data = await response.json();
  } catch {
    throw new Error("STT_FAILED");
  }

  if (!data || typeof data !== "object") throw new Error("STT_FAILED");
  const record = data as Record<string, unknown>;

  if (
    typeof record.text !== "string" ||
    typeof record.language_code !== "string"
  ) {
    throw new Error("STT_FAILED");
  }

  return {
    text: record.text,
    detectedLanguage: record.language_code,
    sttMs: Date.now() - start,
  };
}

export async function translate(
  text: string,
  targetLanguage: string,
  fetcher: Fetcher,
): Promise<{ translatedText: string; translateMs: number }> {
  const apiKey = Deno.env.get("OPENAI_API_KEY") ?? "";
  const model = Deno.env.get("OPENAI_MODEL") ?? "gpt-4o-mini";
  const start = Date.now();

  const prompt =
    `Translate the following text to ${targetLanguage}. Return only the translated text without any explanation or additional commentary.\n\nText: ${text}`;

  const response = await fetcher("https://api.openai.com/v1/responses", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${apiKey}`,
    },
    body: JSON.stringify({
      model,
      input: prompt,
    }),
  });

  if (!response.ok) {
    throw new Error("TRANSLATION_FAILED");
  }

  let data: unknown;
  try {
    data = await response.json();
  } catch {
    throw new Error("TRANSLATION_FAILED");
  }

  if (!data || typeof data !== "object") throw new Error("TRANSLATION_FAILED");
  const record = data as Record<string, unknown>;

  const output = record.output;
  if (!Array.isArray(output) || output.length === 0) {
    throw new Error("TRANSLATION_FAILED");
  }

  const firstOutput = output[0] as Record<string, unknown>;
  const content = firstOutput?.content;
  if (!Array.isArray(content) || content.length === 0) {
    throw new Error("TRANSLATION_FAILED");
  }

  const firstContent = content[0] as Record<string, unknown>;
  if (typeof firstContent?.text !== "string") {
    throw new Error("TRANSLATION_FAILED");
  }

  return {
    translatedText: firstContent.text,
    translateMs: Date.now() - start,
  };
}

export async function synthesize(
  text: string,
  voiceId: string,
  fetcher: Fetcher,
): Promise<{ audioBase64: string; ttsMs: number }> {
  const apiKey = Deno.env.get("ELEVENLABS_API_KEY") ?? "";
  const start = Date.now();

  const response = await fetcher(
    `https://api.elevenlabs.io/v1/text-to-speech/${voiceId}`,
    {
      method: "POST",
      headers: {
        "xi-api-key": apiKey,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        text,
        model_id: "eleven_multilingual_v2",
        output_format: "mp3_44100_128",
      }),
    },
  );

  if (!response.ok) {
    throw new Error("TTS_FAILED");
  }

  const arrayBuffer = await response.arrayBuffer();
  const bytes = new Uint8Array(arrayBuffer);
  let binary = "";
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  const audioBase64 = btoa(binary);

  return {
    audioBase64,
    ttsMs: Date.now() - start,
  };
}
