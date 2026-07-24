import type { TransformRequest } from "./contract.ts";
import { OpenAiProvider } from "./provider.ts";

const request: TransformRequest = {
  requestId: "b65c8888-fb0e-4a8f-9fee-95268995bf68",
  operation: "translate",
  text: "Use fetch safely.",
};

Deno.test("provider maps rate limiting", async () => {
  const provider = providerReturning(new Response(null, { status: 429 }));
  await assertRejects(
    () => provider.transform(request, new AbortController().signal),
    "RATE_LIMITED",
  );
});

Deno.test("provider maps network and malformed payload failures", async () => {
  const networkProvider = new OpenAiProvider(
    "key",
    "model",
    () => Promise.reject(new TypeError("network unavailable")),
  );
  await assertRejects(
    () => networkProvider.transform(request, new AbortController().signal),
    "INVALID_RESPONSE",
  );

  const malformedProvider = providerReturning(
    Response.json({
      output: [{ content: [{ type: "output_text", text: "not-json" }] }],
    }),
  );
  await assertRejects(
    () => malformedProvider.transform(request, new AbortController().signal),
    "INVALID_RESPONSE",
  );
});

Deno.test("provider maps upstream HTTP and incomplete envelopes", async () => {
  for (
    const response of [
      new Response(null, { status: 500 }),
      Response.json({ output: [] }),
      Response.json({
        output: [{ content: [{ type: "refusal", text: "Unavailable" }] }],
      }),
    ]
  ) {
    await assertRejects(
      () =>
        providerReturning(response).transform(
          request,
          new AbortController().signal,
        ),
      "INVALID_RESPONSE",
    );
  }
});

Deno.test("provider preserves AbortError for timeout mapping", async () => {
  const provider = new OpenAiProvider(
    "key",
    "model",
    () => Promise.reject(new DOMException("Aborted", "AbortError")),
  );
  await assertRejects(
    () => provider.transform(request, new AbortController().signal),
    "AbortError",
    true,
  );
});

Deno.test("provider validates structured output invariants", async () => {
  const provider = providerReturning(
    Response.json({
      output: [
        {
          content: [
            {
              type: "output_text",
              text: JSON.stringify({
                sourceLanguage: "English",
                targetLanguage: null,
                result: "Resultado",
              }),
            },
          ],
        },
      ],
    }),
  );
  await assertRejects(
    () => provider.transform(request, new AbortController().signal),
    "INVALID_RESPONSE",
  );
});

Deno.test("provider requests a bounded model output", async () => {
  let body = "";
  const provider = new OpenAiProvider(
    "key",
    "model",
    (_input, init) => {
      body = String((init as { body?: BodyInit } | undefined)?.body);
      return Promise.resolve(
        Response.json({
          output: [
            {
              content: [
                {
                  type: "output_text",
                  text: JSON.stringify({
                    sourceLanguage: "English",
                    targetLanguage: "Portuguese",
                    result: "Resultado",
                  }),
                },
              ],
            },
          ],
        }),
      );
    },
  );

  await provider.transform(request, new AbortController().signal);
  const payload = JSON.parse(body);
  if (payload.max_output_tokens !== 8_000) {
    throw new Error("expected bounded model output");
  }
});

function providerReturning(response: Response) {
  return new OpenAiProvider("key", "model", () => Promise.resolve(response));
}

async function assertRejects(
  callback: () => Promise<unknown>,
  expected: string,
  compareName = false,
) {
  let actual = "";
  try {
    await callback();
  } catch (reason) {
    actual = reason instanceof Error
      ? compareName ? reason.name : reason.message
      : "";
  }
  if (actual !== expected) {
    throw new Error(`expected ${expected}, received ${actual}`);
  }
}
