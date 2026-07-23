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
        {!granted && (
          <div className="permission-recovery">
            <strong>Se o Verbalix já aparece habilitado:</strong>
            <ol>
              <li>Remova a entrada antiga em Privacidade e Segurança → Acessibilidade.</li>
              <li>Adicione o bundle Verbalix.app que você está abrindo agora.</li>
              <li>Habilite a nova entrada, encerre o app e abra novamente.</li>
            </ol>
            <small>
              Builds ad-hoc podem mudar de identidade. Apple Development ou Developer ID mantém
              uma identidade estável entre builds.
            </small>
          </div>
        )}
      </div>
      {!granted && (
        <button className="primary" disabled={checking} onClick={request}>
          {checking ? "Verificando…" : "Abrir permissão"}
        </button>
      )}
    </section>
  );
}
