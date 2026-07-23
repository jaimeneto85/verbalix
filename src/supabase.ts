import { createClient } from "@supabase/supabase-js";
import { native } from "./native";

let client: ReturnType<typeof createClient> | null | undefined;

export async function getSupabase() {
  if (client !== undefined) return client;
  const config = await native.publicBackendConfig();
  if (!config.configured) {
    client = null;
    return client;
  }
  client = createClient(config.supabaseUrl, config.anonymousKey, {
    auth: {
      persistSession: false,
      autoRefreshToken: true,
      detectSessionInUrl: true,
      flowType: "pkce"
    }
  });
  return client;
}
