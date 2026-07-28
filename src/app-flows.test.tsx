import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultSettings } from "./types";

const mocks = vi.hoisted(() => ({
  accessibilityStatus: vi.fn(),
  currentSyncedPreferences: vi.fn(),
  deleteHistory: vi.fn(),
  deepLinkHandler: undefined as undefined | ((urls: string[]) => Promise<void>),
  exchangeCodeForSession: vi.fn(),
  hasSession: vi.fn(),
  listHistory: vi.fn(),
  listenDispose: vi.fn(),
  loadSettings: vi.fn(),
  onAuthStateChange: vi.fn(),
  saveSession: vi.fn(),
  saveSettings: vi.fn()
}));

vi.mock("./native", () => ({
  native: {
    accessibilityStatus: mocks.accessibilityStatus,
    currentSyncedPreferences: mocks.currentSyncedPreferences,
    deleteHistory: mocks.deleteHistory,
    hasSession: mocks.hasSession,
    listHistory: mocks.listHistory,
    loadSettings: mocks.loadSettings,
    saveSession: mocks.saveSession,
    saveSettings: mocks.saveSettings
  }
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(mocks.listenDispose))
}));

vi.mock("./supabase", () => ({
  getSupabase: vi.fn(async () => ({
    auth: {
      exchangeCodeForSession: mocks.exchangeCodeForSession,
      onAuthStateChange: mocks.onAuthStateChange
    }
  }))
}));

vi.mock("@tauri-apps/plugin-deep-link", () => ({
  onOpenUrl: vi.fn((handler: (urls: string[]) => Promise<void>) => {
    mocks.deepLinkHandler = handler;
    return Promise.resolve(vi.fn());
  })
}));

import { App } from "./App";

describe("application authentication and history flows", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.deepLinkHandler = undefined;
    mocks.accessibilityStatus.mockResolvedValue(true);
    mocks.currentSyncedPreferences.mockResolvedValue(null);
    mocks.hasSession.mockResolvedValue(false);
    mocks.listHistory.mockResolvedValue([]);
    mocks.listenDispose.mockReturnValue(undefined);
    mocks.loadSettings.mockResolvedValue(defaultSettings);
    mocks.saveSession.mockResolvedValue(undefined);
    mocks.saveSettings.mockResolvedValue(undefined);
    mocks.onAuthStateChange.mockReturnValue({
      data: { subscription: { unsubscribe: vi.fn() } }
    });
  });

  it("exchanges only deep links that contain an authorization code", async () => {
    render(<App />);

    await waitFor(() => expect(mocks.deepLinkHandler).toBeTypeOf("function"));
    await act(() =>
      mocks.deepLinkHandler!([
        "verbalix://auth/callback?code=pkce-code",
        "verbalix://auth/callback?error=denied"
      ])
    );

    expect(mocks.exchangeCodeForSession).toHaveBeenCalledOnce();
    expect(mocks.exchangeCodeForSession).toHaveBeenCalledWith("pkce-code");
  });

  it("persists a refreshed Supabase session at the native Keychain boundary", async () => {
    let authHandler:
      | ((_event: string, session: {
          access_token: string;
          refresh_token: string;
        }) => void)
      | undefined;
    mocks.onAuthStateChange.mockImplementation((handler) => {
      authHandler = handler;
      return { data: { subscription: { unsubscribe: vi.fn() } } };
    });
    render(<App />);

    await waitFor(() => expect(authHandler).toBeTypeOf("function"));
    act(() => {
      authHandler!("TOKEN_REFRESHED", {
        access_token: "access",
        refresh_token: "refresh"
      });
    });

    await waitFor(() =>
      expect(mocks.saveSession).toHaveBeenCalledWith("access", "refresh")
    );
    expect(await screen.findByText("Sessão protegida")).toBeInTheDocument();
  });

  it("loads and mutates synchronized history from its tab", async () => {
    const user = userEvent.setup();
    mocks.hasSession.mockResolvedValue(true);
    mocks.loadSettings.mockResolvedValue({
      ...defaultSettings,
      historyEnabled: true
    });
    mocks.listHistory.mockResolvedValue([
      {
        id: "history-1",
        operation: "translate",
        source_text: "Olá",
        result_text: "Hello",
        source_language: "pt",
        target_language: "en",
        created_at: "2026-07-23T12:00:00.000Z"
      }
    ]);
    render(<App />);

    await user.click(await screen.findByRole("button", { name: /Histórico/ }));
    expect(await screen.findByText("Hello")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Excluir" }));

    await waitFor(() =>
      expect(mocks.deleteHistory).toHaveBeenCalledWith("history-1")
    );
    expect(screen.queryByText("Hello")).not.toBeInTheDocument();
  });

  it("saves changed settings and reports completion", async () => {
    render(<App />);

    await waitFor(() =>
      expect(mocks.loadSettings).toHaveBeenCalledOnce()
    );
    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));

    await waitFor(() => expect(mocks.saveSettings).toHaveBeenCalled());
    expect(await screen.findByText("Preferências salvas")).toBeInTheDocument();
  });

  it("guides recovery when the macOS accessibility entry is stale", async () => {
    mocks.accessibilityStatus.mockResolvedValue(false);

    render(<App />);

    expect(
      await screen.findByText("Se o Verbalix já aparece habilitado:")
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Remova a entrada antiga/)
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Apple Development ou Developer ID/)
    ).toBeInTheDocument();
  });
});
