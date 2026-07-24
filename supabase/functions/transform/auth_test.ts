import { SupabaseUserAuthenticator } from "./auth.ts";

Deno.test("authenticator accepts a real Supabase user", async () => {
  const authenticator = new SupabaseUserAuthenticator(
    "https://project.supabase.co",
    "public-key",
    (_input, init) => {
      assertEquals(
        new Headers(init?.headers).get("authorization"),
        "Bearer user-token",
      );
      return Promise.resolve(
        Response.json({ id: "user-id", role: "authenticated" }),
      );
    },
  );

  const user = await authenticator.authenticate("user-token");
  assertEquals(user?.id, "user-id");
  assertEquals(user?.role, "authenticated");
});

Deno.test("authenticator rejects the public anonymous key", async () => {
  let calls = 0;
  const authenticator = new SupabaseUserAuthenticator(
    "https://project.supabase.co",
    "public-key",
    () => {
      calls += 1;
      return Promise.resolve(
        Response.json({ id: "unexpected", role: "authenticated" }),
      );
    },
  );

  assertEquals(await authenticator.authenticate("public-key"), null);
  assertEquals(calls, 0);
});

Deno.test("authenticator rejects anonymous roles and invalid sessions", async () => {
  for (
    const response of [
      Response.json({ id: "user-id", role: "anon" }),
      Response.json({ id: "user-id", role: "anonymous" }),
      new Response(null, { status: 401 }),
      new Response("not-json"),
    ]
  ) {
    const authenticator = new SupabaseUserAuthenticator(
      "https://project.supabase.co",
      "public-key",
      () => Promise.resolve(response),
    );
    assertEquals(await authenticator.authenticate("user-token"), null);
  }
});

Deno.test("authenticator fails closed when configuration is absent", async () => {
  const authenticator = new SupabaseUserAuthenticator(
    "",
    "",
    () => Promise.reject(new Error("unexpected fetch")),
  );
  await assertRejects(
    () => authenticator.authenticate("user-token"),
    "INTERNAL_ERROR",
  );
});

function assertEquals(actual: unknown, expected: unknown) {
  if (actual !== expected) {
    throw new Error(`expected ${String(expected)}, received ${String(actual)}`);
  }
}

async function assertRejects(callback: () => Promise<unknown>, code: string) {
  let actual = "";
  try {
    await callback();
  } catch (reason) {
    actual = reason instanceof Error ? reason.message : "";
  }
  assertEquals(actual, code);
}
