import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  enterLive: vi.fn(),
  leaveLive: vi.fn(),
  liveStatus: vi.fn(),
  onLiveStateChange: vi.fn()
}));

vi.mock("../native", () => ({
  native: {
    enterLive: mocks.enterLive,
    leaveLive: mocks.leaveLive,
    liveStatus: mocks.liveStatus,
    onLiveStateChange: mocks.onLiveStateChange
  }
}));

import { LivePanel } from "./LivePanel";

describe("LivePanel", () => {
  let liveStateListener: ((event: { status: string; lastLatencyMs?: number; lastDropped?: boolean; errorMessage?: string }) => void) | null;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    liveStateListener = null;
    mocks.onLiveStateChange.mockImplementation((callback) => {
      liveStateListener = callback;
      return () => {
        liveStateListener = null;
      };
    });
    mocks.enterLive.mockResolvedValue(undefined);
    mocks.leaveLive.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows a notice and never touches native live commands without a voice profile", () => {
    render(<LivePanel voiceProfileId={undefined} />);

    expect(
      screen.getByText("Configure seu perfil de voz para usar a interpretação ao vivo")
    ).toBeInTheDocument();
    expect(mocks.enterLive).not.toHaveBeenCalled();
    expect(mocks.leaveLive).not.toHaveBeenCalled();
  });

  it("renders the target language selector with the supported languages", () => {
    render(<LivePanel voiceProfileId="profile-id" />);

    const select = screen.getByLabelText("Idioma destino") as HTMLSelectElement;
    expect(select.value).toBe("en");
    expect(screen.getByRole("option", { name: "Português" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "English" })).toBeInTheDocument();
    expect(mocks.onLiveStateChange).toHaveBeenCalledOnce();
  });

  it("enters live with the selected target language and voice profile, disabling the selector while on air", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<LivePanel voiceProfileId="profile-id" />);

    const select = screen.getByLabelText("Idioma destino") as HTMLSelectElement;
    await user.selectOptions(select, "pt");
    expect(select.value).toBe("pt");

    await user.click(screen.getByRole("button", { name: "Entrar no ar" }));

    expect(mocks.enterLive).toHaveBeenCalledWith({
      targetLanguage: "pt",
      voiceProfileId: "profile-id"
    });
    expect(screen.getByRole("button", { name: "Sair do ar" })).toBeInTheDocument();
    expect(select).toBeDisabled();
  });

  it("leaves live, stops native capture and resets state to idle", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<LivePanel voiceProfileId="profile-id" />);

    await user.click(screen.getByRole("button", { name: "Entrar no ar" }));
    await user.click(screen.getByRole("button", { name: "Sair do ar" }));

    expect(mocks.leaveLive).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "Entrar no ar" })).toBeInTheDocument();
    expect(screen.getByText("Inativo")).toBeInTheDocument();
  });

  it("still resets to idle when the native leave_live command rejects", async () => {
    mocks.leaveLive.mockRejectedValue(new Error("native failure"));
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<LivePanel voiceProfileId="profile-id" />);

    await user.click(screen.getByRole("button", { name: "Entrar no ar" }));
    await user.click(screen.getByRole("button", { name: "Sair do ar" }));

    expect(
      await screen.findByRole("button", { name: "Entrar no ar" })
    ).toBeInTheDocument();
    expect(screen.getByText("Inativo")).toBeInTheDocument();
    expect(mocks.leaveLive).toHaveBeenCalledOnce();
  });

  it("shows a sanitized error and never surfaces the raw failure when enter_live rejects", async () => {
    mocks.enterLive.mockRejectedValue(new Error("STT_FAILED: audio contents leaked"));
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<LivePanel voiceProfileId="profile-id" />);

    await user.click(screen.getByRole("button", { name: "Entrar no ar" }));

    expect(
      await screen.findByText("Falha ao iniciar interpretação ao vivo")
    ).toBeInTheDocument();
    const bodyText = document.body.textContent ?? "";
    expect(bodyText).not.toMatch(/audio contents leaked/i);
    expect(screen.getByRole("button", { name: "Entrar no ar" })).toBeInTheDocument();
  });

  it.each([
    ["listening", "Escutando"],
    ["processing", "Processando"],
    ["speaking", "Reproduzindo"]
  ] as const)("renders the %s state label from the live-state-changed event", async (status, label) => {
    render(<LivePanel voiceProfileId="profile-id" />);

    act(() => {
      liveStateListener?.({ status });
    });

    expect(await screen.findByText(label)).toBeInTheDocument();
  });

  it("disables enter/leave controls while processing or speaking", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<LivePanel voiceProfileId="profile-id" />);

    await user.click(screen.getByRole("button", { name: "Entrar no ar" }));

    act(() => {
      liveStateListener?.({ status: "processing" });
    });

    expect(screen.getByRole("button", { name: "Sair do ar" })).toBeDisabled();
  });

  it("shows the last utterance latency in milliseconds", async () => {
    render(<LivePanel voiceProfileId="profile-id" />);

    act(() => {
      liveStateListener?.({ status: "speaking", lastLatencyMs: 3200 });
    });

    expect(await screen.findByText("3200 ms")).toBeInTheDocument();
  });

  it("shows a dropped-segment indicator without disclosing segment content", async () => {
    render(<LivePanel voiceProfileId="profile-id" />);

    act(() => {
      liveStateListener?.({ status: "listening", lastDropped: true });
    });

    expect(await screen.findByText("Segmento descartado")).toBeInTheDocument();
  });

  it("surfaces a backend error event, resets on-air state and clears it automatically after 5s", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<LivePanel voiceProfileId="profile-id" />);

    await user.click(screen.getByRole("button", { name: "Entrar no ar" }));

    act(() => {
      liveStateListener?.({ status: "error", errorMessage: "Falha ao processar segmento" });
    });

    expect(await screen.findByText("Falha ao processar segmento")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Entrar no ar" })).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(screen.getByText("Inativo")).toBeInTheDocument();
    expect(screen.queryByText("Falha ao processar segmento")).not.toBeInTheDocument();
  });

  it("unsubscribes from the live-state-changed listener and clears pending timers on unmount", () => {
    const unlisten = vi.fn();
    mocks.onLiveStateChange.mockReturnValue(unlisten);

    const { unmount } = render(<LivePanel voiceProfileId="profile-id" />);

    act(() => {
      liveStateListener?.({ status: "error", errorMessage: "erro" });
    });

    unmount();

    expect(unlisten).toHaveBeenCalledOnce();
  });
});
