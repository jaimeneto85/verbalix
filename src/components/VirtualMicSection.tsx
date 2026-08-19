import { useEffect, useState } from "react";
import { native } from "../native";
import type { VirtualMicStatus } from "../types";

const STATUS_LABELS: Record<VirtualMicStatus, string> = {
  notInstalled: "Não instalado",
  installed: "Instalado",
  incompatibleVersion: "Versão incompatível"
};

type Props = {
  outputToVirtualMic: boolean;
  onOutputToVirtualMicChange: (value: boolean) => void;
  onAir: boolean;
  fallbackWarning: boolean;
};

export function VirtualMicSection({
  outputToVirtualMic,
  onOutputToVirtualMicChange,
  onAir,
  fallbackWarning
}: Props) {
  const [status, setStatus] = useState<VirtualMicStatus>("notInstalled");

  useEffect(() => {
    native
      .virtualMicStatus()
      .then((response) => setStatus(response.status))
      .catch(() => {});

    const unlisten = native.onVirtualMicStatusChange((event) => {
      setStatus(event.status);
    });

    return () => {
      unlisten();
    };
  }, []);

  const routedToVirtualMic =
    onAir && outputToVirtualMic && status === "installed" && !fallbackWarning;
  const destination = routedToVirtualMic ? "Verbalix Microphone" : "Alto-falante (monitor)";

  return (
    <section className="virtual-mic-section">
      <div className="section-heading">Microfone virtual</div>

      <div className={`virtual-mic-status virtual-mic-status--${status}`}>
        {STATUS_LABELS[status]}
      </div>

      {status !== "installed" && (
        <p className="virtual-mic-install-hint">
          {status === "incompatibleVersion"
            ? "Versão incompatível do driver instalado. Reinstale executando "
            : "Instale o driver executando "}
          <code>scripts/install-virtual-mic.sh</code>
          {" "}no terminal (será solicitada sua senha de administrador).
        </p>
      )}

      <label className="toggle-row">
        <span>
          <strong>Enviar áudio traduzido para o microfone virtual</strong>
          <small>Vale a partir da próxima sessão ao vivo.</small>
        </span>
        <input
          type="checkbox"
          checked={outputToVirtualMic}
          onChange={(event) => onOutputToVirtualMicChange(event.target.checked)}
        />
        <i />
      </label>

      <div className="virtual-mic-destination">
        Saída atual: <strong>{destination}</strong>
      </div>

      {fallbackWarning && (
        <div className="virtual-mic-fallback-warning">
          Áudio saindo pelo alto-falante — não foi possível rotear para o microfone virtual.
        </div>
      )}
    </section>
  );
}
