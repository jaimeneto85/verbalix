import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  virtualMicStatus: vi.fn(),
  onVirtualMicStatusChange: vi.fn()
}));

vi.mock("../native", () => ({
  native: {
    virtualMicStatus: mocks.virtualMicStatus,
    onVirtualMicStatusChange: mocks.onVirtualMicStatusChange
  }
}));

import { VirtualMicSection } from "./VirtualMicSection";

describe("VirtualMicSection", () => {
  let statusListener: ((event: { status: string }) => void) | null;

  beforeEach(() => {
    vi.clearAllMocks();
    statusListener = null;
    mocks.onVirtualMicStatusChange.mockImplementation((callback) => {
      statusListener = callback;
      return () => {
        statusListener = null;
      };
    });
  });

  it("shows the not-installed status with an install hint pointing at the script", async () => {
    mocks.virtualMicStatus.mockResolvedValue({ status: "notInstalled" });

    render(
      <VirtualMicSection
        outputToVirtualMic={false}
        onOutputToVirtualMicChange={vi.fn()}
        onAir={false}
        fallbackWarning={false}
      />
    );

    expect(await screen.findByText("Não instalado")).toBeInTheDocument();
    expect(screen.getByText("scripts/install-virtual-mic.sh")).toBeInTheDocument();
    expect(screen.getByText("Alto-falante (monitor)", { exact: false })).toBeInTheDocument();
  });

  it("shows the installed status without an install hint", async () => {
    mocks.virtualMicStatus.mockResolvedValue({ status: "installed" });

    render(
      <VirtualMicSection
        outputToVirtualMic={false}
        onOutputToVirtualMicChange={vi.fn()}
        onAir={false}
        fallbackWarning={false}
      />
    );

    expect(await screen.findByText("Instalado")).toBeInTheDocument();
    expect(screen.queryByText("scripts/install-virtual-mic.sh")).not.toBeInTheDocument();
  });

  it("shows the incompatible status with a reinstall hint", async () => {
    mocks.virtualMicStatus.mockResolvedValue({ status: "incompatibleVersion" });

    render(
      <VirtualMicSection
        outputToVirtualMic={false}
        onOutputToVirtualMicChange={vi.fn()}
        onAir={false}
        fallbackWarning={false}
      />
    );

    expect(await screen.findByText("Versão incompatível")).toBeInTheDocument();
    expect(screen.getByText(/Reinstale executando/)).toBeInTheDocument();
  });

  it("toggles output to the virtual mic via native settings persistence", async () => {
    mocks.virtualMicStatus.mockResolvedValue({ status: "installed" });
    const onOutputToVirtualMicChange = vi.fn();
    const user = userEvent.setup();

    render(
      <VirtualMicSection
        outputToVirtualMic={false}
        onOutputToVirtualMicChange={onOutputToVirtualMicChange}
        onAir={false}
        fallbackWarning={false}
      />
    );

    await screen.findByText("Instalado");
    await user.click(
      screen.getByRole("checkbox", { name: /Enviar áudio traduzido/ })
    );

    expect(onOutputToVirtualMicChange).toHaveBeenCalledWith(true);
  });

  it("shows the virtual mic as the current destination while on air with the toggle enabled", async () => {
    mocks.virtualMicStatus.mockResolvedValue({ status: "installed" });

    render(
      <VirtualMicSection
        outputToVirtualMic
        onOutputToVirtualMicChange={vi.fn()}
        onAir
        fallbackWarning={false}
      />
    );

    await screen.findByText("Instalado");
    expect(screen.getByText("Verbalix Microphone")).toBeInTheDocument();
  });

  it("reflects a plug/unplug status change from the backend event", async () => {
    mocks.virtualMicStatus.mockResolvedValue({ status: "notInstalled" });

    render(
      <VirtualMicSection
        outputToVirtualMic={false}
        onOutputToVirtualMicChange={vi.fn()}
        onAir={false}
        fallbackWarning={false}
      />
    );

    await screen.findByText("Não instalado");

    act(() => {
      statusListener?.({ status: "installed" });
    });

    expect(await screen.findByText("Instalado")).toBeInTheDocument();
  });

  it("shows a fallback warning without hiding the driver status", async () => {
    mocks.virtualMicStatus.mockResolvedValue({ status: "installed" });

    render(
      <VirtualMicSection
        outputToVirtualMic
        onOutputToVirtualMicChange={vi.fn()}
        onAir
        fallbackWarning
      />
    );

    await screen.findByText("Instalado");
    expect(
      screen.getByText(/não foi possível rotear para o microfone virtual/)
    ).toBeInTheDocument();
    expect(screen.getByText("Alto-falante (monitor)", { exact: false })).toBeInTheDocument();
  });

  it("unsubscribes from the virtual-mic-status listener on unmount", async () => {
    mocks.virtualMicStatus.mockResolvedValue({ status: "notInstalled" });
    const unlisten = vi.fn();
    mocks.onVirtualMicStatusChange.mockReturnValue(unlisten);

    const { unmount } = render(
      <VirtualMicSection
        outputToVirtualMic={false}
        onOutputToVirtualMicChange={vi.fn()}
        onAir={false}
        fallbackWarning={false}
      />
    );

    await screen.findByText("Não instalado");
    unmount();

    expect(unlisten).toHaveBeenCalledOnce();
  });
});
