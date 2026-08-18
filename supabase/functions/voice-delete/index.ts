import { SupabaseUserAuthenticator } from "../transform/auth.ts";
import {
  createDeleteHandler,
  createDeleteServiceClient,
} from "./handler.ts";
import { createElevenLabsDeleter } from "./provider.ts";

const supabaseUrl = Deno.env.get("SUPABASE_URL") ?? "";
const anonKey = Deno.env.get("SUPABASE_ANON_KEY") ?? "";
const serviceKey = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") ?? "";
const elevenLabsKey = Deno.env.get("ELEVEN_LABS_KEY") ?? "";

const handler = createDeleteHandler({
  authenticator: new SupabaseUserAuthenticator(supabaseUrl, anonKey),
  deleter: createElevenLabsDeleter(elevenLabsKey),
  serviceClient: createDeleteServiceClient(supabaseUrl, serviceKey),
  timeout: {
    schedule: (cb, delay) => setTimeout(cb, delay),
    cancel: (h) => clearTimeout(h as number),
  },
});

Deno.serve(handler);
