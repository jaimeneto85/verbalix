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
          if (cmd === "accessibility_status" || cmd === "has_session") {
            return false;
          }
          if (cmd === "plugin:event|listen") {
            return 1;
          }
          return undefined;
        }
      }
    });
  });
});

test("shows stale TCC recovery and rechecks the simulated denied state", async ({
  page
}) => {
  await page.goto("/");

  await expect(page.getByText("Permissão pendente")).toBeVisible();
  await expect(
    page.getByText("Se o Verbalix já aparece habilitado:")
  ).toBeVisible();
  await expect(page.getByText(/Remova a entrada antiga/)).toBeVisible();
  await expect(
    page.getByText(/Apple Development ou Developer ID/)
  ).toBeVisible();

  const statusChecksBeforePrompt = await page.evaluate(() => {
    const runtimeWindow = window as typeof window & {
      __verbalixInvocations: Invocation[];
    };
    return runtimeWindow.__verbalixInvocations.filter(
      ({ cmd }) => cmd === "accessibility_status"
    );
  });
  expect(statusChecksBeforePrompt.length).toBeGreaterThan(0);
  expect(statusChecksBeforePrompt.every(({ args }) => args.prompt === false)).toBe(
    true
  );

  await page.getByRole("button", { name: "Abrir permissão" }).click();

  await expect
    .poll(() =>
      page.evaluate(() => {
        const runtimeWindow = window as typeof window & {
          __verbalixInvocations: Invocation[];
        };
        return runtimeWindow.__verbalixInvocations.filter(
          ({ cmd }) => cmd === "accessibility_status"
        );
      })
    )
    .toContainEqual({
      cmd: "accessibility_status",
      args: { prompt: true }
    });
  await expect(page.getByText("Permissão pendente")).toBeVisible();
});
