import { SupabaseUserAuthenticator } from "./auth.ts";
import { createTransformHandler } from "./handler.ts";
import { OpenAiProvider } from "./provider.ts";

const handler = createTransformHandler({
  providerFactory: (apiKey, model) => new OpenAiProvider(apiKey, model),
  authenticator: new SupabaseUserAuthenticator(
    Deno.env.get("SUPABASE_URL") ?? "",
    Deno.env.get("SUPABASE_ANON_KEY") ?? "",
  ),
  getSecret: (name) => Deno.env.get(name),
  timeout: {
    schedule: (callback, delay) => setTimeout(callback, delay),
    cancel: (handle) => clearTimeout(handle as number),
  },
});

Deno.serve(handler);
