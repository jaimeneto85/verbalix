import { useState } from "react";
import { native } from "../native";

type Props = {
  granted: boolean;
  onChange: (granted: boolean) => void;
};

export function PermissionCard({ granted, onChange }: Props) {
  const [checking, setChecking] = useState(false);

  const request = async () => {
    setChecking(true);
    try {
      onChange(await native.accessibilityStatus(true));
    } finally {
      setChecking(false);
    }
  };

  return (
    <section className={`permission-card ${granted ? "granted" : ""}`}>
      <div className="permission-icon">{granted ? "✓" : "⌘"}</div>
      <div>
        <span className="eyebrow">Acessibilidade</span>
        <h2>{granted ? "Verbalix está pronto" : "Permita acesso às seleções"}</h2>
        <p>
          {granted
            ? "A seleção é processada localmente até você escolher uma ação."
            : "O macOS exige sua autorização para ler e substituir o texto selecionado."}
        </p>
      </div>
      {!granted && (
        <button className="primary" disabled={checking} onClick={request}>
          {checking ? "Verificando…" : "Abrir permissão"}
        </button>
      )}
    </section>
  );
}
