import { expect, test } from "@playwright/test";

// This suite proves ONLY the IPC invoke sequence and rendered status for the
// "Microfone virtual" section, using a fully stubbed __TAURI_INTERNALS__ that
// also emits a synthetic "virtual-mic-status" event. It does NOT and CANNOT
// prove that the CoreAudio driver actually installs, that the install script
// runs with real sudo privileges, or that audio is genuinely routed to the
// virtual device — those require a signed bundle with the driver installed
// on a real machine and are documented as manual gates.

type Invocation = {
  cmd: string;
  args: Record<string, unknown>;
};

type VirtualMicStatus = "notInstalled" | "installed" | "incompatibleVersion";

function stubTauri(status: VirtualMicStatus) {
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
            voiceProfileId: "profile-id",
            outputToVirtualMic: false
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
        if (cmd === "live_status") return { status: "idle" };
        if (cmd === "virtual_mic_status") return { status };
        if (cmd === "save_settings") return undefined;
        if (cmd === "plugin:event|listen") return 1;
        return undefined;
      }
    }
  });
}

async function openInterpretationTab(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByRole("button", { name: /Interpretação/ }).click();
  await page.getByRole("button", { name: "Entendi e concordo" }).click();
  await expect(page.getByText("Minha voz")).toBeVisible();
}

async function getInvocations(page: import("@playwright/test").Page) {
  return page.evaluate(() => {
    const runtimeWindow = window as typeof window & {
      __verbalixInvocations: Invocation[];
    };
    return runtimeWindow.__verbalixInvocations;
  });
}

test.describe("virtual mic status: notInstalled", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(stubTauri, "notInstalled");
  });

  test("shows the not-installed status and the install CTA without ever invoking a sudo/script command", async ({
    page
  }) => {
    await openInterpretationTab(page);

    await expect(page.getByText("Não instalado", { exact: true })).toBeVisible();
    await expect(page.getByText(/Instale o driver executando/)).toBeVisible();
    await expect(page.getByText("scripts/install-virtual-mic.sh")).toBeVisible();

    const invocations = await getInvocations(page);
    expect(
      invocations.some(({ cmd }) => /sudo|install_virtual_mic|run_script/.test(cmd))
    ).toBe(false);
    expect(invocations.some(({ cmd }) => cmd === "virtual_mic_status")).toBe(true);
  });
});

test.describe("virtual mic status: installed", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(stubTauri, "installed");
  });

  test("shows the installed status and hides the install CTA", async ({ page }) => {
    await openInterpretationTab(page);

    await expect(page.getByText("Instalado", { exact: true })).toBeVisible();
    await expect(page.getByText(/Instale o driver executando/)).toHaveCount(0);
    await expect(page.getByText("scripts/install-virtual-mic.sh")).toHaveCount(0);
  });
});

test.describe("virtual mic status: incompatibleVersion", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(stubTauri, "incompatibleVersion");
  });

  test("shows the reinstall guidance for an incompatible driver version", async ({ page }) => {
    await openInterpretationTab(page);

    await expect(page.getByText("Versão incompatível", { exact: true })).toBeVisible();
    await expect(page.getByText(/Reinstale executando/)).toBeVisible();
    await expect(page.getByText("scripts/install-virtual-mic.sh")).toBeVisible();
  });
});

test.describe("virtual mic toggle", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(stubTauri, "installed");
  });

  test("persists the toggle via save_settings with outputToVirtualMic true", async ({ page }) => {
    await openInterpretationTab(page);

    const toggle = page.getByRole("checkbox");
    await expect(toggle).not.toBeChecked();
    await toggle.check();
    await expect(toggle).toBeChecked();

    const invocations = await getInvocations(page);
    const saveCalls = invocations.filter(({ cmd }) => cmd === "save_settings");
    expect(saveCalls.length).toBeGreaterThan(0);
    const lastSaveCall = saveCalls[saveCalls.length - 1];
    expect(lastSaveCall.args.settings).toMatchObject({ outputToVirtualMic: true });
  });
});
