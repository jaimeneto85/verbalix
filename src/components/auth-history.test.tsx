import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  signInWithOtp: vi.fn()
}));

vi.mock("../supabase", () => ({
  supabaseConfigured: true,
  supabase: { auth: { signInWithOtp: mocks.signInWithOtp } }
}));

import { AuthPanel } from "./AuthPanel";
import { HistoryPanel } from "./HistoryPanel";

describe("account and history panels", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("requests a passwordless link with the Verbalix callback", async () => {
    mocks.signInWithOtp.mockResolvedValue({ error: null });
    const user = userEvent.setup();
    render(<AuthPanel authenticated={false} />);

    await user.type(
      screen.getByPlaceholderText("voce@empresa.com"),
      "dev@example.com"
    );
    await user.click(screen.getByRole("button", { name: "Enviar link" }));

    expect(mocks.signInWithOtp).toHaveBeenCalledWith({
      email: "dev@example.com",
      options: { emailRedirectTo: "verbalix://auth/callback" }
    });
    expect(
      await screen.findByText("Link enviado. Confira seu e-mail.")
    ).toBeInTheDocument();
  });

  it("offers bulk deletion and copy for populated history", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText }
    });
    const deleteAll = vi.fn();
    render(
      <HistoryPanel
        enabled
        items={[
          {
            id: "1",
            operation: "improve",
            source_text: "Before",
            result_text: "After",
            source_language: "en",
            target_language: null,
            created_at: "2026-07-23T12:00:00.000Z"
          }
        ]}
        onDelete={vi.fn()}
        onDeleteAll={deleteAll}
      />
    );

    screen.getByRole("button", { name: "Copiar" }).click();
    screen.getByRole("button", { name: "Excluir tudo" }).click();

    expect(writeText).toHaveBeenCalledWith("After");
    expect(deleteAll).toHaveBeenCalledOnce();
  });
});
