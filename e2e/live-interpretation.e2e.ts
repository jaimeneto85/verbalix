import { expect, test } from "@playwright/test";

// This suite proves ONLY the IPC invoke sequence for the "Ao vivo" section
// (select target language -> enter_live -> live_status -> leave_live), using
// a fully stubbed __TAURI_INTERNALS__ that also emits a synthetic
// "live-state-changed" event. It does NOT and CANNOT prove real microphone
// capture, real audio playback, real VAD segmentation, or a real round trip
// through the `interpret` Edge Function — those require a signed bundle with
// Accessibility/Microphone permission granted by the user and a deployed
// backend, and are documented as manual gates.
//
// firstAudioMs (fala→primeiro áudio latency): the stub invoke always returns
// { status: "idle" } for live_status and never emits live-state-changed events
// with real timing data. The field is verified at the unit level (LivePanel.test.tsx
// and native.test.ts). A real latency measurement requires: real audio capture,
// a deployed interpret Edge Function, and a running Tauri bundle — all manual gates.

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
              shortcut: "Option+Shift+Space",
              voiceProfileId: "profile-id"
            };
          }
          if (cmd === "accessibility_status") return true;
          if (cmd === "has_session") return true;
          if (cmd === "microphone_permission_status") return "authorized";
          if (cmd === "voice_profile_status") {
            return {
              voiceProfileId: "profile-id",
              status: "ready",
              displayName: "Minha voz"
            };
          }
          if (cmd === "list_history") return [];
          if (cmd === "enter_live") return undefined;
          if (cmd === "leave_live") return undefined;
          if (cmd === "live_status") return { status: "idle" };
          if (cmd === "plugin:event|listen") return 1;
          return undefined;
        }
      }
    });
  });
});

test("selects a target language then enters and leaves live in sequence via the native commands", async ({
  page
}) => {
  await page.goto("/");

  await page.getByRole("button", { name: /Interpretação/ }).click();
  await page.getByRole("button", { name: "Entendi e concordo" }).click();

  await expect(page.getByText("Minha voz")).toBeVisible();
  await expect(page.getByLabel("Idioma destino")).toBeVisible();

  await page.getByLabel("Idioma destino").selectOption("pt");
  await page.getByRole("button", { name: "Entrar no ar" }).click();

  await expect(page.getByRole("button", { name: "Sair do ar" })).toBeVisible();

  await page.getByRole("button", { name: "Sair do ar" }).click();

  await expect(page.getByRole("button", { name: "Entrar no ar" })).toBeVisible();

  const invocations = await page.evaluate(() => {
    const runtimeWindow = window as typeof window & {
      __verbalixInvocations: Invocation[];
    };
    return runtimeWindow.__verbalixInvocations;
  });

  const relevant = invocations.filter(({ cmd }) =>
    ["enter_live", "leave_live"].includes(cmd)
  );
  expect(relevant.map(({ cmd }) => cmd)).toEqual(["enter_live", "leave_live"]);

  const enterCall = invocations.find(({ cmd }) => cmd === "enter_live");
  expect(enterCall?.args).toEqual({
    payload: { targetLanguage: "pt", voiceProfileId: "profile-id" }
  });
});

test("never invokes enter_live before the user selects a target language and clicks enter", async ({
  page
}) => {
  await page.goto("/");

  await page.getByRole("button", { name: /Interpretação/ }).click();
  await page.getByRole("button", { name: "Entendi e concordo" }).click();

  await expect(page.getByLabel("Idioma destino")).toBeVisible();

  const invocations = await page.evaluate(() => {
    const runtimeWindow = window as typeof window & {
      __verbalixInvocations: Invocation[];
    };
    return runtimeWindow.__verbalixInvocations;
  });

  expect(invocations.some(({ cmd }) => cmd === "enter_live")).toBe(false);
  expect(invocations.some(({ cmd }) => cmd === "leave_live")).toBe(false);
});
