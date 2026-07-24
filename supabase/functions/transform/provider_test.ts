import type { TransformRequest } from "./contract.ts";
import { OpenAiProvider, outputTokenBudget } from "./provider.ts";

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
    Response.json(completedPayload("not-json")),
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
    Response.json(
      completedPayload(
        JSON.stringify({
          sourceLanguage: "English",
          targetLanguage: null,
          result: "Resultado",
        }),
      ),
    ),
  );
  await assertRejects(
    () => provider.transform(request, new AbortController().signal),
    "INVALID_RESPONSE",
  );
});

Deno.test("provider requests low-latency reasoning and a bounded output", async () => {
  let body = "";
  const provider = new OpenAiProvider(
    "key",
    "model",
    (_input, init) => {
      body = String((init as { body?: BodyInit } | undefined)?.body);
      return Promise.resolve(
        Response.json(
          completedPayload(
            JSON.stringify({
              sourceLanguage: "English",
              targetLanguage: "Portuguese",
              result: "Resultado",
            }),
          ),
        ),
      );
    },
  );

  await provider.transform(request, new AbortController().signal);
  const payload = JSON.parse(body);
  if (
    payload.model !== "model" ||
    payload.reasoning?.effort !== "none" ||
    payload.max_output_tokens !== 500
  ) {
    throw new Error("expected bounded model output");
  }
});

Deno.test("output budget holds exact floor and proportional boundaries", () => {
  assertEquals(outputTokenBudget(""), 500);
  assertEquals(outputTokenBudget("a".repeat(558)), 500);
  assertEquals(outputTokenBudget("a".repeat(559)), 501);
  assertEquals(outputTokenBudget("a".repeat(1_000)), 795);
});

Deno.test("output budget counts Unicode scalars and holds exact cap boundaries", () => {
  assertEquals(outputTokenBudget("😀".repeat(1_000)), 795);
  assertEquals(outputTokenBudget("a".repeat(11_806)), 7_999);
  assertEquals(outputTokenBudget("a".repeat(11_807)), 8_000);
  assertEquals(outputTokenBudget("a".repeat(11_809)), 8_000);
  assertEquals(outputTokenBudget("a".repeat(12_000)), 8_000);
});

Deno.test("provider rejects incomplete and unknown response envelopes", async () => {
  const validOutput = structuredOutput();
  for (
    const envelope of [
      {
        status: "incomplete",
        incomplete_details: { reason: "max_output_tokens" },
        output: validOutput,
      },
      {
        status: "incomplete",
        incomplete_details: { reason: "content_filter" },
        output: validOutput,
      },
      { status: "incomplete", output: validOutput },
      { status: "unknown", output: validOutput },
      { output: validOutput },
      completedPayload(validOutputText(), { reason: "max_output_tokens" }),
    ]
  ) {
    await assertRejects(
      () =>
        providerReturning(Response.json(envelope)).transform(
          request,
          new AbortController().signal,
        ),
      "INVALID_RESPONSE",
    );
  }
});

Deno.test("provider accepts completed envelopes with null or absent details", async () => {
  for (
    const envelope of [
      completedPayload(validOutputText()),
      { status: "completed", output: structuredOutput() },
    ]
  ) {
    const result = await providerReturning(Response.json(envelope)).transform(
      request,
      new AbortController().signal,
    );
    assertEquals(result.result, "Resultado");
  }
});

function validOutputText() {
  return JSON.stringify({
    sourceLanguage: "English",
    targetLanguage: "Portuguese",
    result: "Resultado",
  });
}

function structuredOutput() {
  return [{ content: [{ type: "output_text", text: validOutputText() }] }];
}

function completedPayload(
  text: string,
  incompleteDetails: { reason?: string } | null = null,
) {
  return {
    status: "completed",
    incomplete_details: incompleteDetails,
    output: [{ content: [{ type: "output_text", text }] }],
  };
}

function providerReturning(response: Response) {
  return new OpenAiProvider("key", "model", () => Promise.resolve(response));
}

function assertEquals(actual: unknown, expected: unknown) {
  if (actual !== expected) {
    throw new Error(`expected ${expected}, received ${actual}`);
  }
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
