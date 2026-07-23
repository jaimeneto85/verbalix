import { useState } from "react";
import { getSupabase } from "../supabase";

export function AuthPanel({ authenticated }: { authenticated: boolean }) {
  const [email, setEmail] = useState("");
  const [message, setMessage] = useState("");

  const sendLink = async () => {
    const supabase = await getSupabase().catch(() => null);
    if (!supabase) {
      setMessage("Este build não contém a configuração pública do backend.");
      return;
    }
    const { error } = await supabase.auth.signInWithOtp({
      email,
      options: { emailRedirectTo: "verbalix://auth/callback" }
    });
    setMessage(error ? error.message : "Link enviado. Confira seu e-mail.");
  };

  return (
    <section className="panel auth-panel">
      <div>
        <span className="eyebrow">Conta</span>
        <h2>{authenticated ? "Sessão protegida" : "Entre por magic link"}</h2>
        <p>
          {authenticated
            ? "Seu token está salvo no Keychain do macOS."
            : "A conta habilita IA e histórico sincronizado. Não usamos senha."}
        </p>
      </div>
      {!authenticated && (
        <div className="email-form">
          <input
            type="email"
            placeholder="voce@empresa.com"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
          />
          <button className="primary" onClick={sendLink}>
            Enviar link
          </button>
        </div>
      )}
      {message && <p className="inline-message">{message}</p>}
    </section>
  );
}
