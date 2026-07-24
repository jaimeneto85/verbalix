import { useEffect, useState } from "react";
import { onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { AuthPanel } from "./components/AuthPanel";
import { HistoryPanel } from "./components/HistoryPanel";
import { PermissionCard } from "./components/PermissionCard";
import { SettingsPanel } from "./components/SettingsPanel";
import { native } from "./native";
import { getSupabase } from "./supabase";
import type { AppSettings, HistoryItem } from "./types";
import { defaultSettings } from "./types";
import brandMark from "../branding/verbalix-mark.svg";

type Tab = "settings" | "history";

export function App() {
  const [settings, setSettings] = useState(defaultSettings);
  const [permission, setPermission] = useState(false);
  const [authenticated, setAuthenticated] = useState(false);
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [tab, setTab] = useState<Tab>("settings");
  const [saving, setSaving] = useState(false);
  const [toast, setToast] = useState("");

  useEffect(() => {
    let authSubscription: { unsubscribe: () => void } | undefined;
    let active = true;
    Promise.all([
      native.loadSettings(),
      native.accessibilityStatus(),
      native.hasSession()
    ]).then(([loadedSettings, granted, hasSession]) => {
      setSettings(loadedSettings);
      setPermission(granted);
      setAuthenticated(hasSession);
    });

    getSupabase()
      .then((supabase) => {
        if (!active || !supabase) return;
        const {
          data: { subscription }
        } = supabase.auth.onAuthStateChange((_event, session) => {
          if (!session) return;
          native
            .saveSession(session.access_token, session.refresh_token)
            .then(() => setAuthenticated(true));
        });
        authSubscription = subscription;
      });
    const deepLink = onOpenUrl(async (urls) => {
      const supabase = await getSupabase().catch(() => null);
      if (!supabase) return;
      for (const value of urls) {
        const code = new URL(value).searchParams.get("code");
        if (code) {
          await supabase.auth.exchangeCodeForSession(code);
        }
      }
    });
    return () => {
      active = false;
      authSubscription?.unsubscribe();
      deepLink.then((dispose) => dispose());
    };
  }, []);

  useEffect(() => {
    if (!authenticated || !settings.historyEnabled) return;
    native.listHistory().then(setHistory).catch(() => setHistory([]));
  }, [authenticated, settings.historyEnabled]);

  const save = async () => {
    setSaving(true);
    try {
      await native.saveSettings(settings);
      setToast("Preferências salvas");
      window.setTimeout(() => setToast(""), 2400);
    } finally {
      setSaving(false);
    }
  };

  const deleteItem = async (id: string) => {
    await native.deleteHistory(id);
    setHistory((items) => items.filter((item) => item.id !== id));
  };

  const deleteAll = async () => {
    await native.deleteHistory();
    setHistory([]);
  };

  return (
    <main className="app-shell">
      <aside>
        <div className="brand">
          <img className="brand-mark" src={brandMark} alt="" />
          <div>
            <strong>Verbalix</strong>
            <span>Technical writing copilot</span>
          </div>
        </div>
        <nav>
          <button
            className={tab === "settings" ? "active" : ""}
            onClick={() => setTab("settings")}
          >
            <span>⌁</span> Configurações
          </button>
          <button
            className={tab === "history" ? "active" : ""}
            onClick={() => setTab("history")}
          >
            <span>↺</span> Histórico
          </button>
        </nav>
        <div className="shortcut-card">
          <span>Atalho rápido</span>
          <kbd>⌥</kbd><kbd>⇧</kbd><kbd>Space</kbd>
        </div>
        <div className="privacy-note">
          <span>●</span>
          Texto enviado somente após seu clique.
        </div>
      </aside>
      <div className="workspace">
        <header>
          <div>
            <span className="eyebrow">macOS MVP</span>
            <h1>Escreva com precisão, em qualquer app.</h1>
          </div>
          <div className={`status-pill ${permission ? "online" : ""}`}>
            <i />
            {permission ? "Ativo" : "Permissão pendente"}
          </div>
        </header>

        <PermissionCard granted={permission} onChange={setPermission} />

        {tab === "settings" ? (
          <div className="content-grid">
            <SettingsPanel
              settings={settings}
              onChange={setSettings}
              onSave={save}
              saving={saving}
            />
            <div className="side-column">
              <AuthPanel authenticated={authenticated} />
              <section className="panel demo-card">
                <span className="eyebrow">Como funciona</span>
                <ol>
                  <li><b>1</b> Selecione um texto</li>
                  <li><b>2</b> Escolha uma ação</li>
                  <li><b>3</b> Continue escrevendo</li>
                </ol>
              </section>
            </div>
          </div>
        ) : (
          <HistoryPanel
            items={history}
            enabled={settings.historyEnabled}
            onDelete={deleteItem}
            onDeleteAll={deleteAll}
          />
        )}
      </div>
      {toast && <div className="toast">{toast}</div>}
    </main>
  );
}
