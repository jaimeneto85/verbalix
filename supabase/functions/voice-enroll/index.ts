import { SupabaseUserAuthenticator } from "../transform/auth.ts";
import { createEnrollHandler, createSupabaseServiceClient } from "./handler.ts";
import { createElevenLabsProvider } from "./provider.ts";

const supabaseUrl = Deno.env.get("SUPABASE_URL") ?? "";
const anonKey = Deno.env.get("SUPABASE_ANON_KEY") ?? "";
const serviceKey = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") ?? "";
const elevenLabsKey = Deno.env.get("ELEVEN_LABS_KEY") ?? "";

const handler = createEnrollHandler({
  authenticator: new SupabaseUserAuthenticator(supabaseUrl, anonKey),
  provider: createElevenLabsProvider(elevenLabsKey),
  getSecret: (name) => Deno.env.get(name),
  serviceClient: createSupabaseServiceClient(supabaseUrl, serviceKey),
  timeout: {
    schedule: (cb, delay) => setTimeout(cb, delay),
    cancel: (h) => clearTimeout(h as number),
  },
});

Deno.serve(handler);
