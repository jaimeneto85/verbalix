import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { native } from "./native";
import { OverlayReadyGate } from "./overlayReady";
import type { AppSettings, NoteResult } from "./types";
import { defaultSettings } from "./types";

export function Overlay({
  kind,
  generation
}: {
  kind: "toolbar" | "note";
  generation: string;
}) {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [result, setResult] = useState("");
  const [noteMode, setNoteMode] =
    useState<"error" | "result" | "preview" | "undo">("result");
  const [requestId, setRequestId] = useState("");
  const [busy, setBusy] = useState<"translate" | "improve" | null>(null);
  const [noteBusy, setNoteBusy] = useState<"apply" | "undo" | null>(null);
  const [settingsReady, setSettingsReady] = useState(false);

  useEffect(() => {
    let active = true;

    const syncUnlisten = listen<AppSettings>("preferences-synced", (event) => {
      if (active) setSettings(event.payload);
    }).then(async (dispose) => {
      if (!active) {
        dispose();
        return () => undefined;
      }
      const synced = await native.currentSyncedPreferences().catch(() => null);
      if (synced && active) setSettings(synced);
      return dispose;
    });

    native
      .loadSettings()
      .then((s) => { if (active) setSettings(s); })
      .catch(() => undefined)
      .finally(() => { if (active) setSettingsReady(true); });

    let noteUnlisten: Promise<() => void> | undefined;

    if (kind === "note") {
      const showResult = (payload: NoteResult) => {
        if (!active) return;
        setResult(payload.text);
        setNoteMode(payload.mode);
        setRequestId(payload.requestId ?? "");
      };
      noteUnlisten = listen<NoteResult>("note-result", (event) => {
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
    }

    return () => {
      active = false;
      syncUnlisten.then((dispose) => dispose());
      noteUnlisten?.then((dispose) => dispose());
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
    try {
      await native.transformSelection(operation, settings);
    } catch {
      return;
    } finally {
      setBusy(null);
    }
  };

  if (kind === "note") {
    const apply = async () => {
      setNoteBusy("apply");
      try {
        await native.applyPreview(requestId);
        setNoteMode("undo");
      } catch {
        return;
      } finally {
        setNoteBusy(null);
      }
    };
    const undo = async () => {
      setNoteBusy("undo");
      try {
        await native.undoReplacement(result);
      } catch {
        return;
      } finally {
        setNoteBusy(null);
      }
    };
    return (
      <OverlayReadyGate kind={kind} generation={generation}>
        <main className="note">
          <div className="note-heading">
            <span>{noteMode === "error" ? "Ação necessária" : "Resultado"}</span>
            <button onClick={() => native.dismissOverlays()}>×</button>
          </div>
          <p>{result || "Processando…"}</p>
          <div className="note-actions">
            {noteMode === "error" ? (
              <button className="note-copy" onClick={() => native.openMainWindow()}>
                Abrir Verbalix
              </button>
            ) : (
              <button
                className="note-copy"
                disabled={!result}
                onClick={() => navigator.clipboard.writeText(result)}
              >
                Copiar
              </button>
            )}
            {noteMode === "preview" && (
              <button
                className="note-copy"
                disabled={!requestId || noteBusy !== null}
                onClick={apply}
              >
                Aplicar
              </button>
            )}
            {noteMode === "undo" && (
              <button
                className="note-copy"
                disabled={noteBusy !== null}
                onClick={undo}
              >
                Desfazer
              </button>
            )}
          </div>
        </main>
      </OverlayReadyGate>
    );
  }

  return (
    <OverlayReadyGate kind={kind} generation={generation}>
      <main className="toolbar">
        <button
          disabled={!settingsReady || busy !== null}
          onClick={() => transform("translate")}
        >
          <span>文</span>
          {busy === "translate" ? "Traduzindo…" : "Traduzir"}
        </button>
        <i />
        <button
          disabled={!settingsReady || busy !== null}
          onClick={() => transform("improve")}
        >
          <span>✦</span>
          {busy === "improve" ? "Aprimorando…" : "Aprimorar"}
        </button>
      </main>
    </OverlayReadyGate>
  );
}
