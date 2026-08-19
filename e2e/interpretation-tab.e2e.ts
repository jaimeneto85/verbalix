import { expect, test } from "@playwright/test";

// This suite proves ONLY tab routing and IPC invoke sequencing for the
// "Interpretação" tab, using a fully stubbed __TAURI_INTERNALS__.invoke.
// It does NOT and CANNOT prove real microphone permission/TCC behaviour,
// real audio capture, or a real ElevenLabs enrollment round trip — those
// require a signed bundle with Accessibility/Microphone permission granted
// by the user and are documented as manual gates.

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
          if (cmd === "accessibility_status") return true;
          if (cmd === "has_session") return true;
          if (cmd === "microphone_permission_status") return "notDetermined";
          if (cmd === "voice_profile_status") return null;
          if (cmd === "list_history") return [];
          if (cmd === "plugin:event|listen") return 1;
          return undefined;
        }
      }
    });
  });
});

test("routes to the Interpretação tab behind consent and never invokes recording/upload commands", async ({
  page
}) => {
  await page.goto("/");

  await page.getByRole("button", { name: /Interpretação/ }).click();

  await expect(
    page.getByText("O Verbalix vai gravar uma amostra de voz")
  ).toBeVisible();
  await expect(page.getByText(/Microfone:/)).toHaveCount(0);

  const invocations = await page.evaluate(() => {
    const runtimeWindow = window as typeof window & {
      __verbalixInvocations: Invocation[];
    };
    return runtimeWindow.__verbalixInvocations;
  });

  expect(
    invocations.some(({ cmd }) => cmd === "begin_voice_enrollment")
  ).toBe(false);
  expect(
    invocations.some(({ cmd }) => cmd === "finish_voice_enrollment")
  ).toBe(false);
});

test("gates recording on the microphone permission returned by the status command", async ({
  page
}) => {
  await page.goto("/");

  await page.getByRole("button", { name: /Interpretação/ }).click();
  await page.getByRole("button", { name: "Entendi e concordo" }).click();

  await expect(page.getByText("não determinado")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Solicitar permissão" })
  ).toBeVisible();

  const invocations = await page.evaluate(() => {
    const runtimeWindow = window as typeof window & {
      __verbalixInvocations: Invocation[];
    };
    return runtimeWindow.__verbalixInvocations;
  });

  expect(
    invocations.some(({ cmd }) => cmd === "microphone_permission_status")
  ).toBe(true);
  expect(
    invocations.some(({ cmd }) => cmd === "begin_voice_enrollment")
  ).toBe(false);
});
