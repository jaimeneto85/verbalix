import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { native } from "./native";
import type { AppSettings } from "./types";
import { defaultSettings } from "./types";

export function Overlay({ kind }: { kind: "toolbar" | "note" }) {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [result, setResult] = useState("");
  const [busy, setBusy] = useState<"translate" | "improve" | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    native.loadSettings().then(setSettings).catch(() => undefined);
    if (kind !== "note") return;
    const unlisten = listen<{ text: string }>("note-result", (event) =>
      setResult(event.payload.text)
    );
    return () => {
      unlisten.then((dispose) => dispose());
    };
  }, [kind]);

  useEffect(() => {
    const close = (event: KeyboardEvent) => {
      if (event.key === "Escape") native.dismissOverlays();
    };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, []);

  const transform = async (operation: "translate" | "improve") => {
    setBusy(operation);
    setError("");
    try {
      await native.transformSelection(operation, settings);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(null);
    }
  };

  if (kind === "note") {
    return (
      <main className="note">
        <div className="note-heading">
          <span>Resultado</span>
          <button onClick={() => native.dismissOverlays()}>×</button>
        </div>
        <p>{result || "Processando…"}</p>
        <button
          className="note-copy"
          disabled={!result}
          onClick={() => navigator.clipboard.writeText(result)}
        >
          Copiar
        </button>
      </main>
    );
  }

  return (
    <main className="toolbar">
      <button disabled={busy !== null} onClick={() => transform("translate")}>
        <span>文</span>
        {busy === "translate" ? "Traduzindo…" : "Traduzir"}
      </button>
      <i />
      <button disabled={busy !== null} onClick={() => transform("improve")}>
        <span>✦</span>
        {busy === "improve" ? "Aprimorando…" : "Aprimorar"}
      </button>
      {error && <div className="toolbar-error">{error}</div>}
    </main>
  );
}
