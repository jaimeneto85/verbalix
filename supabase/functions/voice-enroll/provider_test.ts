import { ElevenLabsProvider } from "./provider.ts";

Deno.test("ElevenLabsProvider.enroll returns voice_id on success", async () => {
  const fakeFetcher = async (_input: string | URL | Request, _init?: RequestInit) => {
    return new Response(JSON.stringify({ voice_id: "test_voice_id" }), {
      status: 200,
    });
  };

  const provider = new ElevenLabsProvider("fake-api-key", fakeFetcher);
  const wavBlob = new Blob([new Uint8Array([0, 1, 2, 3])], { type: "audio/wav" });
  const voiceId = await provider.enroll("Test Voice", wavBlob);

  if (voiceId !== "test_voice_id") {
    throw new Error(`expected test_voice_id, got ${voiceId}`);
  }
});

Deno.test("ElevenLabsProvider.deleteVoice treats 404 as success", async () => {
  const fakeFetcher = async (_input: string | URL | Request, _init?: RequestInit) => {
    return new Response(null, { status: 404 });
  };

  const provider = new ElevenLabsProvider("fake-api-key", fakeFetcher);
  await provider.deleteVoice("non-existent-voice-id");
});

Deno.test("ElevenLabsProvider.enroll throws PROVIDER_REJECTED on non-ok response", async () => {
  const fakeFetcher = async (_input: string | URL | Request, _init?: RequestInit) => {
    return new Response(JSON.stringify({ error: "Unauthorized" }), {
      status: 401,
    });
  };

  const provider = new ElevenLabsProvider("fake-api-key", fakeFetcher);
  const wavBlob = new Blob([new Uint8Array([0, 1, 2, 3])], { type: "audio/wav" });

  let threw = false;
  try {
    await provider.enroll("Test Voice", wavBlob);
  } catch (err) {
    threw = err instanceof Error && err.message === "PROVIDER_REJECTED";
  }
  if (!threw) throw new Error("expected PROVIDER_REJECTED");
});

Deno.test("ElevenLabsProvider.deleteVoice throws PROVIDER_REJECTED on server error", async () => {
  const fakeFetcher = async (_input: string | URL | Request, _init?: RequestInit) => {
    return new Response(null, { status: 500 });
  };

  const provider = new ElevenLabsProvider("fake-api-key", fakeFetcher);

  let threw = false;
  try {
    await provider.deleteVoice("some-voice-id");
  } catch (err) {
    threw = err instanceof Error && err.message === "PROVIDER_REJECTED";
  }
  if (!threw) throw new Error("expected PROVIDER_REJECTED");
});
