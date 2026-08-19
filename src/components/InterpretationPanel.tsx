import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { native } from "../native";
import type { MicrophonePermission, VoiceProfileStatus, VoiceProfileView } from "../types";
import { LivePanel } from "./LivePanel";
import { VirtualMicSection } from "./VirtualMicSection";

type RecordingState = "idle" | "recording" | "recorded" | "uploading" | "done";

type Props = {
  authenticated: boolean;
  voiceProfileId?: string;
  onVoiceProfileChange: (id: string | undefined) => void;
  outputToVirtualMic?: boolean;
  onOutputToVirtualMicChange?: (value: boolean) => void;
};

export function InterpretationPanel({
  authenticated,
  voiceProfileId,
  onVoiceProfileChange,
  outputToVirtualMic = false,
  onOutputToVirtualMicChange = () => {}
}: Props) {
  const [consent, setConsent] = useState(false);
  const [permission, setPermission] = useState<MicrophonePermission>("notDetermined");
  const [recordingState, setRecordingState] = useState<RecordingState>("idle");
  const [level, setLevel] = useState(0);
  const [displayName, setDisplayName] = useState("");
  const [profile, setProfile] = useState<VoiceProfileView | null>(null);
  const [error, setError] = useState("");
  const [deleting, setDeleting] = useState(false);
  const [liveOnAir, setLiveOnAir] = useState(false);
  const [virtualMicFallback, setVirtualMicFallback] = useState(false);
  const levelUnlistenRef = useRef<(() => void) | null>(null);
  const permUnlistenRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    native.microphonePermissionStatus().then(setPermission).catch(() => {});

    listen<MicrophonePermission>("microphone-permission", (e) => {
      setPermission(e.payload);
    }).then((unlisten) => { permUnlistenRef.current = unlisten; });

    if (voiceProfileId) {
      native.voiceProfileStatus(voiceProfileId)
        .then((p) => { if (p) setProfile(p); })
        .catch(() => {});
    }

    return () => {
      permUnlistenRef.current?.();
    };
  }, [voiceProfileId]);

  const requestPermission = async () => {
    setError("");
    try {
      await native.requestMicrophonePermission();
    } catch {
      setError("Não foi possível solicitar permissão de microfone.");
    }
  };

  const startRecording = async () => {
    setError("");
    setLevel(0);
    try {
      await native.beginVoiceEnrollment();
      setRecordingState("recording");
      listen<number>("enrollment-level", (e) => {
        setLevel(e.payload);
      }).then((unlisten) => { levelUnlistenRef.current = unlisten; });
    } catch {
      setError("Falha ao iniciar gravação.");
    }
  };

  const stopRecording = () => {
    levelUnlistenRef.current?.();
    levelUnlistenRef.current = null;
    setLevel(0);
    setRecordingState("recorded");
  };

  const cancelRecording = async () => {
    levelUnlistenRef.current?.();
    levelUnlistenRef.current = null;
    await native.cancelVoiceEnrollment().catch(() => {});
    setRecordingState("idle");
    setLevel(0);
  };

  const upload = async () => {
    if (!displayName.trim()) {
      setError("Informe um nome para o perfil de voz.");
      return;
    }
    setError("");
    setRecordingState("uploading");
    try {
      const view = await native.finishVoiceEnrollment(displayName.trim());
      setProfile(view);
      onVoiceProfileChange(view.voiceProfileId);
      setRecordingState("done");
    } catch {
      setError("Falha ao enviar amostra de voz. Tente novamente.");
      setRecordingState("recorded");
    }
  };

  const deleteProfile = async () => {
    if (!voiceProfileId) return;
    setDeleting(true);
    setError("");
    try {
      await native.deleteVoiceProfile(voiceProfileId);
      setProfile(null);
      onVoiceProfileChange(undefined);
      setRecordingState("idle");
    } catch {
      setError("Falha ao excluir perfil de voz.");
    } finally {
      setDeleting(false);
    }
  };

  if (!authenticated) {
    return (
      <section className="panel interpretation-panel">
        <div className="section-heading">Interpretação de Voz</div>
        <p className="panel-notice">Faça login para usar o enrollment de voz.</p>
      </section>
    );
  }

  return (
    <section className="panel interpretation-panel">
      <div className="section-heading">Interpretação de Voz</div>

      {!consent && (
        <div className="consent-card">
          <p>O Verbalix vai gravar uma amostra de voz para criar um perfil de clonagem local. O áudio é enviado à ElevenLabs via servidor — nunca ao cliente. Ao continuar, você autoriza essa coleta.</p>
          <button className="btn-primary" onClick={() => setConsent(true)}>
            Entendi e concordo
          </button>
        </div>
      )}

      {consent && (
        <>
          <div className="permission-row">
            <span>Microfone: <strong>{permissionLabel(permission)}</strong></span>
            {permission === "notDetermined" && (
              <button className="btn-secondary" onClick={requestPermission}>
                Solicitar permissão
              </button>
            )}
            {permission === "denied" && (
              <span className="hint">Abra Preferências do Sistema → Privacidade → Microfone</span>
            )}
          </div>

          {permission === "authorized" && (
            <div className="enrollment-workspace">
              {(recordingState === "idle" || recordingState === "done") && !profile && (
                <>
                  <input
                    className="voice-name-input"
                    placeholder="Nome do perfil de voz"
                    value={displayName}
                    onChange={(e) => setDisplayName(e.target.value)}
                    maxLength={64}
                  />
                  <button className="btn-primary" onClick={startRecording}>
                    Gravar amostra (~1-2 min)
                  </button>
                </>
              )}

              {recordingState === "recording" && (
                <div className="recording-controls">
                  <div className="level-meter">
                    <div className="level-bar" style={{ width: `${Math.round(level * 100)}%` }} />
                  </div>
                  <span className="recording-hint">Leia um texto em voz alta por 1-2 minutos</span>
                  <div className="button-row">
                    <button className="btn-secondary" onClick={stopRecording}>Parar</button>
                    <button className="btn-danger" onClick={cancelRecording}>Cancelar</button>
                  </div>
                </div>
              )}

              {recordingState === "recorded" && (
                <div className="recording-controls">
                  <p>Amostra gravada. Pronto para enviar.</p>
                  <input
                    className="voice-name-input"
                    placeholder="Nome do perfil de voz"
                    value={displayName}
                    onChange={(e) => setDisplayName(e.target.value)}
                    maxLength={64}
                  />
                  <div className="button-row">
                    <button className="btn-primary" onClick={upload}>Enviar</button>
                    <button className="btn-secondary" onClick={() => setRecordingState("idle")}>Re-gravar</button>
                  </div>
                </div>
              )}

              {recordingState === "uploading" && (
                <p className="status-hint">Enviando amostra...</p>
              )}

              {profile && (
                <div className="profile-card">
                  <div className="profile-info">
                    <strong>{profile.displayName}</strong>
                    <span className={`status-badge status-${profile.status}`}>
                      {statusLabel(profile.status)}
                    </span>
                  </div>
                  <div className="button-row">
                    <button className="btn-secondary" onClick={() => { setProfile(null); setRecordingState("idle"); }}>
                      Novo enrollment
                    </button>
                    <button className="btn-danger" onClick={deleteProfile} disabled={deleting}>
                      {deleting ? "Excluindo..." : "Excluir voz"}
                    </button>
                  </div>
                </div>
              )}
            </div>
          )}

          {error && <p className="error-hint">{error}</p>}

          {profile && profile.status === "ready" && (
            <>
              <div className="section-heading">Ao vivo</div>
              <LivePanel
                voiceProfileId={profile.voiceProfileId}
                onAirChange={(next) => {
                  setLiveOnAir(next);
                  if (next) setVirtualMicFallback(false);
                }}
                onVirtualMicFallback={() => setVirtualMicFallback(true)}
              />
              <VirtualMicSection
                outputToVirtualMic={outputToVirtualMic}
                onOutputToVirtualMicChange={onOutputToVirtualMicChange}
                onAir={liveOnAir}
                fallbackWarning={virtualMicFallback}
              />
            </>
          )}
        </>
      )}
    </section>
  );
}

function permissionLabel(p: MicrophonePermission): string {
  switch (p) {
    case "authorized": return "autorizado";
    case "denied": return "negado";
    case "restricted": return "restrito";
    default: return "não determinado";
  }
}

function statusLabel(s: VoiceProfileStatus): string {
  switch (s) {
    case "ready": return "pronto";
    case "enrolling": return "processando";
    case "failed": return "falha";
    case "deleting": return "excluindo";
  }
}
