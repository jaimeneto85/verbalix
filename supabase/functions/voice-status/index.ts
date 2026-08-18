import { SupabaseUserAuthenticator } from "../transform/auth.ts";
import { createStatusHandler, createStatusServiceClient } from "./handler.ts";

const supabaseUrl = Deno.env.get("SUPABASE_URL") ?? "";
const anonKey = Deno.env.get("SUPABASE_ANON_KEY") ?? "";
const serviceKey = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") ?? "";

const handler = createStatusHandler({
  authenticator: new SupabaseUserAuthenticator(supabaseUrl, anonKey),
  serviceClient: createStatusServiceClient(supabaseUrl, serviceKey),
});

Deno.serve(handler);
