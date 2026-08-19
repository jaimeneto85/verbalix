import type { Fetcher } from "./provider.ts";

export interface InterpretServiceClient {
  getReadyVoiceProfile(
    userId: string,
  ): Promise<{ providerVoiceId: string } | null>;
}

export function createInterpretServiceClient(
  supabaseUrl: string,
  serviceKey: string,
  fetcher: Fetcher = fetch,
): InterpretServiceClient {
  const baseUrl = `${supabaseUrl}/rest/v1`;
  const authHeaders = {
    Authorization: `Bearer ${serviceKey}`,
    apikey: serviceKey,
    "Content-Type": "application/json",
  };

  return {
    async getReadyVoiceProfile(
      userId: string,
    ): Promise<{ providerVoiceId: string } | null> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?user_id=eq.${userId}&status=eq.ready&order=created_at.desc&limit=1&select=provider_voice_id`,
        { headers: authHeaders },
      );

      if (!response.ok) throw new Error("NO_VOICE_PROFILE");

      const rows = (await response.json()) as Array<Record<string, unknown>>;
      if (!rows || rows.length === 0) return null;

      const row = rows[0];
      if (typeof row.provider_voice_id !== "string") return null;

      return { providerVoiceId: row.provider_voice_id };
    },
  };
}
