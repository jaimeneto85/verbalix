import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultSettings } from "./types";

type NoteEvent = {
  payload: {
    mode: "error" | "result" | "preview" | "undo";
    requestId?: string;
    text: string;
  };
};

const mocks = vi.hoisted(() => ({
  applyPreview: vi.fn(),
  currentNoteResult: vi.fn(),
  dismissOverlays: vi.fn(),
  listener: undefined as undefined | ((event: NoteEvent) => void),
  loadSettings: vi.fn(),
  openMainWindow: vi.fn(),
  overlaySurfaceReady: vi.fn(),
  transformSelection: vi.fn(),
  undoReplacement: vi.fn(),
  unlisten: vi.fn()
}));

vi.mock("./native", () => ({
  native: {
    applyPreview: mocks.applyPreview,
    currentNoteResult: mocks.currentNoteResult,
    dismissOverlays: mocks.dismissOverlays,
    loadSettings: mocks.loadSettings,
    openMainWindow: mocks.openMainWindow,
    overlaySurfaceReady: mocks.overlaySurfaceReady,
    transformSelection: mocks.transformSelection,
    undoReplacement: mocks.undoReplacement
  }
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_event, listener) => {
    mocks.listener = listener;
    return Promise.resolve(mocks.unlisten);
  })
}));

import { Overlay } from "./Overlay";

describe("selection overlays", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listener = undefined;
    mocks.loadSettings.mockResolvedValue(defaultSettings);
    mocks.openMainWindow.mockResolvedValue(undefined);
    mocks.overlaySurfaceReady.mockResolvedValue(true);
    mocks.applyPreview.mockResolvedValue("Improved");
    mocks.currentNoteResult.mockResolvedValue(null);
    mocks.transformSelection.mockResolvedValue({});
    mocks.undoReplacement.mockResolvedValue(undefined);
  });

  it("starts translation and improvement only from explicit toolbar actions", async () => {
    const user = userEvent.setup();
    render(<Overlay kind="toolbar" generation="toolbar-generation" />);

    expect(mocks.transformSelection).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: /Traduzir/ }));
    await user.click(screen.getByRole("button", { name: /Aprimorar/ }));

    expect(mocks.transformSelection.mock.calls).toEqual([
      ["translate", defaultSettings],
      ["improve", defaultSettings]
    ]);
  });

  it("leaves typed native failures to the native feedback surface", async () => {
    mocks.transformSelection.mockRejectedValue("provider unavailable");
    render(<Overlay kind="toolbar" generation="toolbar-generation" />);

    const translate = screen.getByRole("button", { name: /Traduzir/ });
    await waitFor(() => expect(translate).toBeEnabled());
    fireEvent.click(translate);
    await waitFor(() => expect(translate).toBeEnabled());
    expect(mocks.transformSelection).toHaveBeenCalledOnce();
    expect(mocks.openMainWindow).not.toHaveBeenCalled();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(mocks.dismissOverlays).toHaveBeenCalledOnce();
  });

  it("waits for settings and delegates readiness to the native command", async () => {
    let resolveSettings: (settings: typeof defaultSettings) => void = () => undefined;
    mocks.loadSettings.mockReturnValue(
      new Promise((resolve) => {
        resolveSettings = resolve;
      })
    );
    render(<Overlay kind="toolbar" generation="toolbar-generation" />);

    const improve = screen.getByRole("button", { name: /Aprimorar/ });
    expect(improve).toBeDisabled();
    fireEvent.click(improve);
    expect(mocks.transformSelection).not.toHaveBeenCalled();

    resolveSettings(defaultSettings);
    await waitFor(() => expect(improve).toBeEnabled());
    fireEvent.click(improve);

    await waitFor(() => expect(mocks.transformSelection).toHaveBeenCalledOnce());
    expect(mocks.transformSelection).toHaveBeenCalledWith(
      "improve",
      defaultSettings
    );
  });

  it("submits at most one action while the native request is pending", async () => {
    let resolveTransform: (value: object) => void = () => undefined;
    mocks.transformSelection.mockReturnValue(
      new Promise((resolve) => {
        resolveTransform = resolve;
      })
    );
    render(<Overlay kind="toolbar" generation="toolbar-generation" />);

    const translate = screen.getByRole("button", { name: /Traduzir/ });
    const improve = screen.getByRole("button", { name: /Aprimorar/ });
    await waitFor(() => expect(translate).toBeEnabled());
    fireEvent.click(translate);
    fireEvent.click(translate);
    fireEvent.click(improve);

    expect(mocks.transformSelection).toHaveBeenCalledOnce();
    expect(translate).toBeDisabled();
    expect(improve).toBeDisabled();
    resolveTransform({});
    await waitFor(() => expect(translate).toBeEnabled());
  });

  it("renders actionable errors in the full-size note surface", async () => {
    const user = userEvent.setup();
    render(<Overlay kind="note" generation="note-generation" />);
    await waitFor(() => expect(mocks.listener).toBeTypeOf("function"));
    act(() => {
      mocks.listener!({
        payload: {
          mode: "error",
          text: "Este build não contém a configuração pública do backend."
        }
      });
    });

    expect(await screen.findByText("Ação necessária")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Abrir Verbalix" }));
    expect(mocks.openMainWindow).toHaveBeenCalledOnce();
    expect(screen.queryByRole("button", { name: "Copiar" })).not.toBeInTheDocument();
  });

  it("applies a preview and then offers strict undo", async () => {
    const user = userEvent.setup();
    render(<Overlay kind="note" generation="note-generation" />);
    await waitFor(() => expect(mocks.listener).toBeTypeOf("function"));
    act(() => {
      mocks.listener!({
        payload: {
          mode: "preview",
          requestId: "request-1",
          text: "Improved text"
        }
      });
    });

    await user.click(screen.getByRole("button", { name: "Aplicar" }));
    expect(mocks.applyPreview).toHaveBeenCalledWith("request-1");
    await user.click(screen.getByRole("button", { name: "Desfazer" }));

    expect(mocks.undoReplacement).toHaveBeenCalledWith("Improved text");
    expect(mocks.dismissOverlays).not.toHaveBeenCalled();
  });

  it("keeps apply and undo failures native and blocks repeated pending actions", async () => {
    const user = userEvent.setup();
    let rejectApply: (reason?: unknown) => void = () => undefined;
    mocks.applyPreview.mockReturnValue(
      new Promise((_resolve, reject) => {
        rejectApply = reject;
      })
    );
    render(<Overlay kind="note" generation="note-generation" />);
    await waitFor(() => expect(mocks.listener).toBeTypeOf("function"));
    act(() => {
      mocks.listener!({
        payload: {
          mode: "preview",
          requestId: "request-1",
          text: "Improved text"
        }
      });
    });

    const apply = screen.getByRole("button", { name: "Aplicar" });
    await user.click(apply);
    await user.click(apply);
    expect(mocks.applyPreview).toHaveBeenCalledOnce();
    expect(apply).toBeDisabled();
    rejectApply("stale preview");
    await waitFor(() => expect(apply).toBeEnabled());
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeInTheDocument();
    expect(mocks.dismissOverlays).not.toHaveBeenCalled();
  });

  it("keeps a rejected undo visible and prevents duplicate native calls", async () => {
    const user = userEvent.setup();
    let rejectUndo: (reason?: unknown) => void = () => undefined;
    mocks.undoReplacement.mockReturnValue(
      new Promise((_resolve, reject) => {
        rejectUndo = reject;
      })
    );
    render(<Overlay kind="note" generation="note-generation" />);
    await waitFor(() => expect(mocks.listener).toBeTypeOf("function"));
    act(() => {
      mocks.listener!({
        payload: { mode: "undo", text: "Improved text" }
      });
    });

    const undo = screen.getByRole("button", { name: "Desfazer" });
    await user.click(undo);
    await user.click(undo);
    expect(mocks.undoReplacement).toHaveBeenCalledOnce();
    expect(undo).toBeDisabled();
    rejectUndo("stale undo");
    await waitFor(() => expect(undo).toBeEnabled());
    expect(screen.getByText("Improved text")).toBeInTheDocument();
    expect(mocks.dismissOverlays).not.toHaveBeenCalled();
  });

  it("recovers the first note result published before its listener was ready", async () => {
    mocks.currentNoteResult.mockResolvedValue({
      mode: "result",
      text: "Already translated"
    });

    render(<Overlay kind="note" generation="note-generation" />);

    expect(await screen.findByText("Already translated")).toBeInTheDocument();
    expect(screen.queryByText("Processando…")).not.toBeInTheDocument();
  });

  it("receives the first note result published after its listener was ready", async () => {
    render(<Overlay kind="note" generation="note-generation" />);
    await waitFor(() => expect(mocks.listener).toBeTypeOf("function"));

    act(() => {
      mocks.listener!({
        payload: { mode: "result", text: "Listener result" }
      });
    });

    expect(await screen.findByText("Listener result")).toBeInTheDocument();
    expect(mocks.currentNoteResult).toHaveBeenCalledOnce();
  });

  it("copies note results and dismisses from its close control", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText }
    });
    render(<Overlay kind="note" generation="note-generation" />);
    await waitFor(() => expect(mocks.listener).toBeTypeOf("function"));
    act(() => {
      mocks.listener!({
        payload: { mode: "result", text: "Translated result" }
      });
    });

    fireEvent.click(screen.getByRole("button", { name: "Copiar" }));
    fireEvent.click(screen.getByRole("button", { name: "×" }));

    expect(writeText).toHaveBeenCalledWith("Translated result");
    expect(mocks.dismissOverlays).toHaveBeenCalledOnce();
  });
});
