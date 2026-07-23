import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { native } from "./native";
import type { AppSettings, NoteResult } from "./types";
import { defaultSettings } from "./types";

export function Overlay({ kind }: { kind: "toolbar" | "note" }) {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [result, setResult] = useState("");
  const [noteMode, setNoteMode] = useState<"result" | "preview" | "undo">("result");
  const [requestId, setRequestId] = useState("");
  const [busy, setBusy] = useState<"translate" | "improve" | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    native.loadSettings().then(setSettings).catch(() => undefined);
    if (kind !== "note") return;
    let active = true;
    const showResult = (payload: NoteResult) => {
      if (!active) return;
      setResult(payload.text);
      setNoteMode(payload.mode);
      setRequestId(payload.requestId ?? "");
    };
    const unlisten = listen<NoteResult>("note-result", (event) => {
      showResult(event.payload);
    }).then(async (dispose) => {
      if (!active) {
        dispose();
        return () => undefined;
      }
      const current = await native.currentNoteResult().catch(() => null);
      if (current) showResult(current);
      return dispose;
    });
    return () => {
      active = false;
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
    const apply = async () => {
      await native.applyPreview(requestId);
      setNoteMode("undo");
    };
    const undo = async () => {
      await native.undoReplacement(result);
      await native.dismissOverlays();
    };
    return (
      <main className="note">
        <div className="note-heading">
          <span>Resultado</span>
          <button onClick={() => native.dismissOverlays()}>×</button>
        </div>
        <p>{result || "Processando…"}</p>
        <div className="note-actions">
          <button
            className="note-copy"
            disabled={!result}
            onClick={() => navigator.clipboard.writeText(result)}
          >
            Copiar
          </button>
          {noteMode === "preview" && (
            <button className="note-copy" disabled={!requestId} onClick={apply}>
              Aplicar
            </button>
          )}
          {noteMode === "undo" && (
            <button className="note-copy" onClick={undo}>
              Desfazer
            </button>
          )}
        </div>
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
