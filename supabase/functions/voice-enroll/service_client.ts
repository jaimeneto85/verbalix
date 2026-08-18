import type { Fetcher } from "./provider.ts";
import type { VoiceProfileView } from "./contract.ts";

export type UpsertResult = {
  voiceProfileId: string;
  alreadyDone: boolean;
};

export interface SupabaseServiceClient {
  upsertEnrolling(
    userId: string,
    requestId: string,
    displayName: string,
  ): Promise<UpsertResult>;
  setReady(voiceProfileId: string, providerVoiceId: string): Promise<void>;
  setFailed(voiceProfileId: string): Promise<void>;
  getProfile(
    userId: string,
    voiceProfileId: string,
  ): Promise<VoiceProfileView | null>;
  getPreviousProfile(
    userId: string,
  ): Promise<
    | { voiceProfileId: string; requestId: string; providerVoiceId: string }
    | null
  >;
  deleteProfile(voiceProfileId: string): Promise<void>;
}

export function createSupabaseServiceClient(
  supabaseUrl: string,
  serviceKey: string,
  fetcher: Fetcher = fetch,
): SupabaseServiceClient {
  const baseUrl = `${supabaseUrl}/rest/v1`;
  const authHeaders = {
    Authorization: `Bearer ${serviceKey}`,
    apikey: serviceKey,
    "Content-Type": "application/json",
  };

  return {
    async upsertEnrolling(
      userId: string,
      requestId: string,
      displayName: string,
    ): Promise<UpsertResult> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?on_conflict=user_id,request_id&select=id,status`,
        {
          method: "POST",
          headers: {
            ...authHeaders,
            Prefer: "resolution=merge-duplicates,return=representation",
          },
          body: JSON.stringify({
            user_id: userId,
            request_id: requestId,
            display_name: displayName,
            status: "enrolling",
          }),
        },
      );

      if (!response.ok) throw new Error("INTERNAL_ERROR");

      const rows = (await response.json()) as Array<Record<string, unknown>>;
      if (!rows || rows.length === 0) throw new Error("INTERNAL_ERROR");

      const row = rows[0];
      const voiceProfileId = typeof row.id === "string" ? row.id : "";
      if (!voiceProfileId) throw new Error("INTERNAL_ERROR");

      return {
        voiceProfileId,
        alreadyDone: row.status !== "enrolling",
      };
    },

    async setReady(
      voiceProfileId: string,
      providerVoiceId: string,
    ): Promise<void> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?id=eq.${voiceProfileId}`,
        {
          method: "PATCH",
          headers: { ...authHeaders, Prefer: "return=minimal" },
          body: JSON.stringify({
            status: "ready",
            provider_voice_id: providerVoiceId,
          }),
        },
      );
      if (!response.ok) throw new Error("INTERNAL_ERROR");
    },

    async setFailed(voiceProfileId: string): Promise<void> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?id=eq.${voiceProfileId}`,
        {
          method: "PATCH",
          headers: { ...authHeaders, Prefer: "return=minimal" },
          body: JSON.stringify({ status: "failed" }),
        },
      );
      if (!response.ok) throw new Error("INTERNAL_ERROR");
    },

    async getProfile(
      userId: string,
      voiceProfileId: string,
    ): Promise<VoiceProfileView | null> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?id=eq.${voiceProfileId}&user_id=eq.${userId}&select=id,status,display_name`,
        { headers: authHeaders },
      );
      if (!response.ok) throw new Error("INTERNAL_ERROR");
      const rows = (await response.json()) as Array<Record<string, unknown>>;
      if (!rows || rows.length === 0) return null;
      const row = rows[0];
      return {
        voiceProfileId: row.id as string,
        status: row.status as VoiceProfileView["status"],
        displayName: row.display_name as string,
      };
    },

    async getPreviousProfile(
      userId: string,
    ): Promise<
      | { voiceProfileId: string; requestId: string; providerVoiceId: string }
      | null
    > {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?user_id=eq.${userId}&status=in.(ready,failed,enrolling,deleting)&select=id,request_id,provider_voice_id&order=created_at.desc&limit=1`,
        { headers: authHeaders },
      );
      if (!response.ok) throw new Error("INTERNAL_ERROR");
      const rows = (await response.json()) as Array<Record<string, unknown>>;
      if (!rows || rows.length === 0) return null;
      const row = rows[0];
      if (typeof row.provider_voice_id !== "string") return null;
      return {
        voiceProfileId: row.id as string,
        requestId: typeof row.request_id === "string" ? row.request_id : "",
        providerVoiceId: row.provider_voice_id,
      };
    },

    async deleteProfile(voiceProfileId: string): Promise<void> {
      const response = await fetcher(
        `${baseUrl}/voice_profiles?id=eq.${voiceProfileId}`,
        {
          method: "DELETE",
          headers: { ...authHeaders, Prefer: "return=minimal" },
        },
      );
      if (!response.ok) throw new Error("INTERNAL_ERROR");
    },
  };
}
