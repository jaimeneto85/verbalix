export type Fetcher = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>;

export interface VoiceProvider {
  enroll(
    displayName: string,
    wavBlob: Blob,
    signal?: AbortSignal,
  ): Promise<string>;
  deleteVoice(providerVoiceId: string, signal?: AbortSignal): Promise<void>;
}

export class ElevenLabsProvider implements VoiceProvider {
  constructor(
    private readonly apiKey: string,
    private readonly fetcher: Fetcher = fetch,
  ) {}

  async enroll(
    displayName: string,
    wavBlob: Blob,
    signal?: AbortSignal,
  ): Promise<string> {
    const form = new FormData();
    form.append("name", displayName);
    form.append("files", wavBlob, "sample.wav");

    const response = await this.fetcher(
      "https://api.elevenlabs.io/v1/voices/add",
      {
        method: "POST",
        headers: { "xi-api-key": this.apiKey },
        body: form,
        signal,
      },
    );

    if (!response.ok) {
      throw new Error("PROVIDER_REJECTED");
    }

    let data: unknown;
    try {
      data = await response.json();
    } catch {
      throw new Error("PROVIDER_REJECTED");
    }

    if (!data || typeof data !== "object") throw new Error("PROVIDER_REJECTED");
    const record = data as Record<string, unknown>;
    if (typeof record.voice_id !== "string" || record.voice_id.length === 0) {
      throw new Error("PROVIDER_REJECTED");
    }
    return record.voice_id;
  }

  async deleteVoice(
    providerVoiceId: string,
    signal?: AbortSignal,
  ): Promise<void> {
    const response = await this.fetcher(
      `https://api.elevenlabs.io/v1/voices/${providerVoiceId}`,
      {
        method: "DELETE",
        headers: { "xi-api-key": this.apiKey },
        signal,
      },
    );

    if (!response.ok && response.status !== 404) {
      throw new Error("PROVIDER_REJECTED");
    }
  }
}

export function createElevenLabsProvider(
  apiKey: string,
  fetcher?: Fetcher,
): VoiceProvider {
  return new ElevenLabsProvider(apiKey, fetcher);
}
