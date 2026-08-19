export type Fetcher = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>;

export interface VoiceDeleter {
  deleteVoice(providerVoiceId: string, signal?: AbortSignal): Promise<void>;
}

export class ElevenLabsDeleter implements VoiceDeleter {
  constructor(
    private readonly apiKey: string,
    private readonly fetcher: Fetcher = fetch,
  ) {}

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
      throw new Error("INTERNAL_ERROR");
    }
  }
}

export function createElevenLabsDeleter(
  apiKey: string,
  fetcher?: Fetcher,
): VoiceDeleter {
  return new ElevenLabsDeleter(apiKey, fetcher);
}
