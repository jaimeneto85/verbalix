import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  microphonePermissionStatus: vi.fn(),
  requestMicrophonePermission: vi.fn(),
  beginVoiceEnrollment: vi.fn(),
  finishVoiceEnrollment: vi.fn(),
  cancelVoiceEnrollment: vi.fn(),
  deleteVoiceProfile: vi.fn(),
  voiceProfileStatus: vi.fn(),
  enterLive: vi.fn(),
  leaveLive: vi.fn(),
  onLiveStateChange: vi.fn(),
  listen: vi.fn()
}));

vi.mock("../native", () => ({
  native: {
    microphonePermissionStatus: mocks.microphonePermissionStatus,
    requestMicrophonePermission: mocks.requestMicrophonePermission,
    beginVoiceEnrollment: mocks.beginVoiceEnrollment,
    finishVoiceEnrollment: mocks.finishVoiceEnrollment,
    cancelVoiceEnrollment: mocks.cancelVoiceEnrollment,
    deleteVoiceProfile: mocks.deleteVoiceProfile,
    voiceProfileStatus: mocks.voiceProfileStatus,
    enterLive: mocks.enterLive,
    leaveLive: mocks.leaveLive,
    onLiveStateChange: mocks.onLiveStateChange
  }
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: mocks.listen
}));

import { InterpretationPanel } from "./InterpretationPanel";

describe("InterpretationPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.microphonePermissionStatus.mockResolvedValue("notDetermined");
    mocks.voiceProfileStatus.mockResolvedValue(null);
    mocks.listen.mockResolvedValue(() => undefined);
    mocks.onLiveStateChange.mockReturnValue(() => undefined);
  });

  it("prompts for login when unauthenticated and never touches native voice APIs", async () => {
    render(
      <InterpretationPanel
        authenticated={false}
        voiceProfileId={undefined}
        onVoiceProfileChange={vi.fn()}
      />
    );

    expect(
      screen.getByText("Faça login para usar o enrollment de voz.")
    ).toBeInTheDocument();
    expect(mocks.beginVoiceEnrollment).not.toHaveBeenCalled();
    expect(mocks.finishVoiceEnrollment).not.toHaveBeenCalled();
  });

  it("requires explicit consent before showing microphone permission and recording controls", async () => {
    render(
      <InterpretationPanel
        authenticated
        voiceProfileId={undefined}
        onVoiceProfileChange={vi.fn()}
      />
    );

    expect(
      screen.getByText(/vai gravar uma amostra de voz/)
    ).toBeInTheDocument();
    expect(screen.queryByText(/Microfone:/)).not.toBeInTheDocument();

    const user = userEvent.setup();
    await user.click(
      screen.getByRole("button", { name: "Entendi e concordo" })
    );

    expect(await screen.findByText(/Microfone:/)).toBeInTheDocument();
    expect(mocks.microphonePermissionStatus).toHaveBeenCalledOnce();
  });

  it("gates recording behind an undetermined/denied microphone permission", async () => {
    mocks.microphonePermissionStatus.mockResolvedValue("denied");
    const user = userEvent.setup();
    render(
      <InterpretationPanel
        authenticated
        voiceProfileId={undefined}
        onVoiceProfileChange={vi.fn()}
      />
    );

    await user.click(
      screen.getByRole("button", { name: "Entendi e concordo" })
    );

    expect(await screen.findByText("negado")).toBeInTheDocument();
    expect(
      screen.getByText("Abra Preferências do Sistema → Privacidade → Microfone")
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Gravar amostra/ })
    ).not.toBeInTheDocument();
    expect(mocks.beginVoiceEnrollment).not.toHaveBeenCalled();
  });

  it("requests microphone permission on demand when status is undetermined", async () => {
    mocks.microphonePermissionStatus.mockResolvedValue("notDetermined");
    mocks.requestMicrophonePermission.mockResolvedValue("authorized");
    const user = userEvent.setup();
    render(
      <InterpretationPanel
        authenticated
        voiceProfileId={undefined}
        onVoiceProfileChange={vi.fn()}
      />
    );

    await user.click(
      screen.getByRole("button", { name: "Entendi e concordo" })
    );
    await user.click(
      await screen.findByRole("button", { name: "Solicitar permissão" })
    );

    expect(mocks.requestMicrophonePermission).toHaveBeenCalledOnce();
  });

  it("records via native commands and never exposes raw audio through React state", async () => {
    mocks.microphonePermissionStatus.mockResolvedValue("authorized");
    mocks.beginVoiceEnrollment.mockResolvedValue(undefined);
    mocks.finishVoiceEnrollment.mockResolvedValue({
      voiceProfileId: "profile-id",
      status: "ready",
      displayName: "Minha voz"
    });
    const onVoiceProfileChange = vi.fn();
    const user = userEvent.setup();
    render(
      <InterpretationPanel
        authenticated
        voiceProfileId={undefined}
        onVoiceProfileChange={onVoiceProfileChange}
      />
    );

    await user.click(
      screen.getByRole("button", { name: "Entendi e concordo" })
    );
    await user.type(
      await screen.findByPlaceholderText("Nome do perfil de voz"),
      "Minha voz"
    );
    await user.click(
      screen.getByRole("button", { name: /Gravar amostra/ })
    );

    expect(mocks.beginVoiceEnrollment).toHaveBeenCalledOnce();
    expect(mocks.beginVoiceEnrollment).toHaveBeenCalledWith();

    await user.click(screen.getByRole("button", { name: "Parar" }));
    await user.click(screen.getByRole("button", { name: "Enviar" }));

    expect(mocks.finishVoiceEnrollment).toHaveBeenCalledWith("Minha voz");
    expect(await screen.findByText("Minha voz")).toBeInTheDocument();
    expect(onVoiceProfileChange).toHaveBeenCalledWith("profile-id");

    const bodyText = document.body.textContent ?? "";
    expect(bodyText).not.toMatch(/base64|wav|pcm/i);
  });

  it("cancels an in-progress recording without uploading anything", async () => {
    mocks.microphonePermissionStatus.mockResolvedValue("authorized");
    mocks.beginVoiceEnrollment.mockResolvedValue(undefined);
    mocks.cancelVoiceEnrollment.mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(
      <InterpretationPanel
        authenticated
        voiceProfileId={undefined}
        onVoiceProfileChange={vi.fn()}
      />
    );

    await user.click(
      screen.getByRole("button", { name: "Entendi e concordo" })
    );
    await user.click(
      screen.getByRole("button", { name: /Gravar amostra/ })
    );
    await user.click(screen.getByRole("button", { name: "Cancelar" }));

    expect(mocks.cancelVoiceEnrollment).toHaveBeenCalledOnce();
    expect(mocks.finishVoiceEnrollment).not.toHaveBeenCalled();
  });

  it("deletes an existing voice profile and reflects the empty state", async () => {
    mocks.microphonePermissionStatus.mockResolvedValue("authorized");
    mocks.voiceProfileStatus.mockResolvedValue({
      voiceProfileId: "profile-id",
      status: "ready",
      displayName: "Voz existente"
    });
    mocks.deleteVoiceProfile.mockResolvedValue(undefined);
    const onVoiceProfileChange = vi.fn();
    const user = userEvent.setup();
    render(
      <InterpretationPanel
        authenticated
        voiceProfileId="profile-id"
        onVoiceProfileChange={onVoiceProfileChange}
      />
    );

    await user.click(
      screen.getByRole("button", { name: "Entendi e concordo" })
    );

    expect(await screen.findByText("Voz existente")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Excluir voz" }));

    expect(mocks.deleteVoiceProfile).toHaveBeenCalledWith("profile-id");
    expect(onVoiceProfileChange).toHaveBeenCalledWith(undefined);
    expect(screen.queryByText("Voz existente")).not.toBeInTheDocument();
  });

  it("shows a friendly error and preserves state when upload fails", async () => {
    mocks.microphonePermissionStatus.mockResolvedValue("authorized");
    mocks.beginVoiceEnrollment.mockResolvedValue(undefined);
    mocks.finishVoiceEnrollment.mockRejectedValue(new Error("network"));
    const user = userEvent.setup();
    render(
      <InterpretationPanel
        authenticated
        voiceProfileId={undefined}
        onVoiceProfileChange={vi.fn()}
      />
    );

    await user.click(
      screen.getByRole("button", { name: "Entendi e concordo" })
    );
    await user.type(
      await screen.findByPlaceholderText("Nome do perfil de voz"),
      "Minha voz"
    );
    await user.click(
      screen.getByRole("button", { name: /Gravar amostra/ })
    );
    await user.click(screen.getByRole("button", { name: "Parar" }));
    await user.click(screen.getByRole("button", { name: "Enviar" }));

    expect(
      await screen.findByText(
        "Falha ao enviar amostra de voz. Tente novamente."
      )
    ).toBeInTheDocument();
  });

  it("requires a display name before allowing upload", async () => {
    mocks.microphonePermissionStatus.mockResolvedValue("authorized");
    mocks.beginVoiceEnrollment.mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(
      <InterpretationPanel
        authenticated
        voiceProfileId={undefined}
        onVoiceProfileChange={vi.fn()}
      />
    );

    await user.click(
      screen.getByRole("button", { name: "Entendi e concordo" })
    );
    await user.click(
      screen.getByRole("button", { name: /Gravar amostra/ })
    );
    await user.click(screen.getByRole("button", { name: "Parar" }));
    await user.click(screen.getByRole("button", { name: "Enviar" }));

    expect(
      await screen.findByText("Informe um nome para o perfil de voz.")
    ).toBeInTheDocument();
    expect(mocks.finishVoiceEnrollment).not.toHaveBeenCalled();
  });
});
