import { SupabaseUserAuthenticator } from "../transform/auth.ts";
import { handleInterpret } from "./handler.ts";
import { createInterpretServiceClient } from "./service_client.ts";

const supabaseUrl = Deno.env.get("SUPABASE_URL") ?? "";
const anonKey = Deno.env.get("SUPABASE_ANON_KEY") ?? "";
const serviceKey = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") ?? "";
const elevenLabsKey = Deno.env.get("ELEVENLABS_API_KEY") ?? "";
const openAiKey = Deno.env.get("OPENAI_API_KEY") ?? "";
const openAiModel = Deno.env.get("OPENAI_MODEL") ?? "gpt-4o-mini";

const authenticator = new SupabaseUserAuthenticator(supabaseUrl, anonKey);
const serviceClient = createInterpretServiceClient(supabaseUrl, serviceKey);

const INTERPRET_TIMEOUT_MS = 45_000;

Deno.serve(async (req: Request): Promise<Response> => {
  const controller = new AbortController();
  const handle = setTimeout(() => controller.abort(), INTERPRET_TIMEOUT_MS);

  try {
    return await handleInterpret(req, {
      authenticator,
      serviceClient,
      fetcher: (input, init) =>
        fetch(input, { ...init, signal: controller.signal }),
      elevenLabsKey,
      openAiKey,
      openAiModel,
    });
  } finally {
    clearTimeout(handle);
  }
});
