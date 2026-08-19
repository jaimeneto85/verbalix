import { useState, useEffect, useRef } from "react";
import type { LiveStateEvent, LiveStatus } from "../types";
import { native } from "../native";

const ALLOWED_LANGUAGES = ["pt", "en", "es", "fr", "de", "it", "ja", "ko", "zh"];
const LANGUAGE_LABELS: Record<string, string> = {
  pt: "Português",
  en: "English",
  es: "Español",
  fr: "Français",
  de: "Deutsch",
  it: "Italiano",
  ja: "日本語",
  ko: "한국어",
  zh: "中文",
};

const STATUS_LABELS: Record<LiveStatus, string> = {
  idle: "Inativo",
  listening: "Escutando",
  processing: "Processando",
  speaking: "Reproduzindo",
  error: "Erro",
};

interface LivePanelProps {
  voiceProfileId?: string;
  onAirChange?: (onAir: boolean) => void;
  onVirtualMicFallback?: () => void;
}

export function LivePanel({ voiceProfileId, onAirChange, onVirtualMicFallback }: LivePanelProps) {
  const [targetLanguage, setTargetLanguage] = useState("en");
  const [liveState, setLiveState] = useState<LiveStateEvent>({ status: "idle" });
  const [onAir, setOnAir] = useState(false);
  const errorResetTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const setOnAirState = (next: boolean) => {
    setOnAir(next);
    onAirChange?.(next);
  };

  useEffect(() => {
    const unlisten = native.onLiveStateChange((event) => {
      if ((event.status as string) === "virtual-mic-fallback") {
        onVirtualMicFallback?.();
        return;
      }
      setLiveState(event);
      if (event.status === "idle" || event.status === "error") {
        setOnAirState(false);
      }
      if (event.status === "error") {
        if (errorResetTimer.current) clearTimeout(errorResetTimer.current);
        errorResetTimer.current = setTimeout(() => {
          setLiveState((prev) => ({ ...prev, status: "idle", errorMessage: undefined }));
        }, 5000);
      }
    });

    return () => {
      unlisten();
      if (errorResetTimer.current) clearTimeout(errorResetTimer.current);
    };
  }, [onVirtualMicFallback]);

  const handleEnter = async () => {
    if (!voiceProfileId) return;
    try {
      await native.enterLive({ targetLanguage, voiceProfileId });
      setOnAirState(true);
    } catch {
      setLiveState({ status: "error", errorMessage: "Falha ao iniciar interpretação ao vivo" });
    }
  };

  const handleLeave = async () => {
    try {
      await native.leaveLive();
    } catch {
    } finally {
      setOnAirState(false);
      setLiveState({ status: "idle" });
    }
  };

  const isTransitioning =
    liveState.status === "processing" || liveState.status === "speaking";

  if (!voiceProfileId) {
    return (
      <div className="live-panel">
        <p className="panel-notice">
          Configure seu perfil de voz para usar a interpretação ao vivo
        </p>
      </div>
    );
  }

  return (
    <div className="live-panel">
      <div className="live-language-select">
        <label htmlFor="live-target-lang">Idioma destino</label>
        <select
          id="live-target-lang"
          value={targetLanguage}
          disabled={onAir}
          onChange={(e) => setTargetLanguage(e.target.value)}
        >
          {ALLOWED_LANGUAGES.map((lang) => (
            <option key={lang} value={lang}>
              {LANGUAGE_LABELS[lang]}
            </option>
          ))}
        </select>
      </div>

      <div className={`live-status live-status--${liveState.status}`}>
        {STATUS_LABELS[liveState.status]}
      </div>

      {liveState.firstAudioMs !== undefined && (
        <div className="live-latency live-latency--first-audio">
          <span className="live-latency__label">fala→primeiro áudio</span>
          <span className="live-latency__value">{liveState.firstAudioMs} ms</span>
        </div>
      )}

      {liveState.lastLatencyMs !== undefined && (
        <div className="live-latency">{liveState.lastLatencyMs} ms</div>
      )}

      {liveState.lastDropped && (
        <div className="live-dropped">Segmento descartado</div>
      )}

      {liveState.status === "error" && liveState.errorMessage && (
        <div className="error-hint">{liveState.errorMessage}</div>
      )}

      <div className="button-row">
        {onAir ? (
          <button
            className="live-button live-button--leave btn-danger"
            disabled={isTransitioning}
            onClick={handleLeave}
          >
            Sair do ar
          </button>
        ) : (
          <button
            className="live-button live-button--enter btn-primary"
            disabled={isTransitioning}
            onClick={handleEnter}
          >
            Entrar no ar
          </button>
        )}
      </div>
    </div>
  );
}
