import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultSettings } from "./types";

type NoteEvent = {
  payload: {
    mode: "result" | "preview" | "undo";
    requestId?: string;
    text: string;
  };
};

const mocks = vi.hoisted(() => ({
  applyPreview: vi.fn(),
  dismissOverlays: vi.fn(),
  listener: undefined as undefined | ((event: NoteEvent) => void),
  loadSettings: vi.fn(),
  transformSelection: vi.fn(),
  undoReplacement: vi.fn(),
  unlisten: vi.fn()
}));

vi.mock("./native", () => ({
  native: {
    applyPreview: mocks.applyPreview,
    dismissOverlays: mocks.dismissOverlays,
    loadSettings: mocks.loadSettings,
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
    mocks.applyPreview.mockResolvedValue("Improved");
    mocks.transformSelection.mockResolvedValue({});
    mocks.undoReplacement.mockResolvedValue(undefined);
  });

  it("starts translation and improvement only from explicit toolbar actions", async () => {
    const user = userEvent.setup();
    render(<Overlay kind="toolbar" />);

    expect(mocks.transformSelection).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: /Traduzir/ }));
    await user.click(screen.getByRole("button", { name: /Aprimorar/ }));

    expect(mocks.transformSelection.mock.calls).toEqual([
      ["translate", defaultSettings],
      ["improve", defaultSettings]
    ]);
  });

  it("renders transformation failures and dismisses with Escape", async () => {
    mocks.transformSelection.mockRejectedValue("provider unavailable");
    render(<Overlay kind="toolbar" />);

    fireEvent.click(screen.getByRole("button", { name: /Traduzir/ }));
    expect(await screen.findByText("provider unavailable")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(mocks.dismissOverlays).toHaveBeenCalledOnce();
  });

  it("applies a preview and then offers strict undo", async () => {
    const user = userEvent.setup();
    render(<Overlay kind="note" />);
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
    expect(mocks.dismissOverlays).toHaveBeenCalledOnce();
  });

  it("copies note results and dismisses from its close control", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText }
    });
    render(<Overlay kind="note" />);
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
