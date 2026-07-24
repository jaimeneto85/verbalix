import { expect, test } from "@playwright/test";

type Invocation = {
  cmd: string;
  args: Record<string, unknown>;
};

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const callbacks = new Map<number, (...args: unknown[]) => unknown>();
    let nextCallback = 1;
    const invocations: Invocation[] = [];
    Object.assign(window, {
      __verbalixInvocations: invocations,
      __TAURI_INTERNALS__: {
        transformCallback(callback: (...args: unknown[]) => unknown) {
          const id = nextCallback++;
          callbacks.set(id, callback);
          return id;
        },
        unregisterCallback(id: number) {
          callbacks.delete(id);
        },
        async invoke(cmd: string, args: Record<string, unknown> = {}) {
          invocations.push({ cmd, args });
          if (cmd === "load_settings") {
            return {
              formality: 3,
              length: "balanced",
              tone: "technical",
              confirmBeforeReplace: false,
              historyEnabled: false,
              automaticToolbar: true,
              shortcut: "Option+Shift+Space"
            };
          }
          if (cmd === "transform_selection") {
            throw new Error("Remote authentication is required");
          }
          if (cmd === "current_note_result") {
            return {
              mode: "error",
              text: "Entre no Verbalix para usar tradução e aprimoramento."
            };
          }
          if (cmd === "plugin:event|listen") return 1;
          return undefined;
        }
      }
    });
  });
});

test("delegates login gating to one native transformation command", async ({
  page
}) => {
  await page.goto(
    "/?overlay=toolbar&generation=123e4567-e89b-42d3-a456-426614174000"
  );

  await page.getByRole("button", { name: /Traduzir/ }).click();

  const invocations = await page.evaluate(() => {
    const runtimeWindow = window as typeof window & {
      __verbalixInvocations: Invocation[];
    };
    return runtimeWindow.__verbalixInvocations;
  });
  expect(invocations.some(({ cmd }) => cmd === "ai_readiness")).toBe(false);
  expect(
    invocations.filter(({ cmd }) => cmd === "transform_selection")
  ).toHaveLength(1);
});

test("shows the complete actionable login error inside the note", async ({
  page
}) => {
  await page.goto(
    "/?overlay=note&generation=123e4567-e89b-42d3-a456-426614174000"
  );

  const message = page.getByText(
    "Entre no Verbalix para usar tradução e aprimoramento."
  );
  await expect(page.getByText("Ação necessária")).toBeVisible();
  await expect(message).toBeVisible();
  const bounds = await message.boundingBox();
  const viewport = page.viewportSize();
  expect(bounds).not.toBeNull();
  expect(viewport).not.toBeNull();
  expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(viewport!.height);

  await page.getByRole("button", { name: "Abrir Verbalix" }).click();
  await expect
    .poll(() =>
      page.evaluate(() => {
        const runtimeWindow = window as typeof window & {
          __verbalixInvocations: Invocation[];
        };
        return runtimeWindow.__verbalixInvocations.some(
          ({ cmd }) => cmd === "open_main_window"
        );
      })
    )
    .toBe(true);
});
