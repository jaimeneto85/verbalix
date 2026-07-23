import type {
  AppSettings,
  LengthPreference,
  TonePreference
} from "../types";

type Props = {
  settings: AppSettings;
  onChange: (settings: AppSettings) => void;
  onSave: () => Promise<void>;
  saving: boolean;
};

const tones: Array<{ value: TonePreference; label: string }> = [
  { value: "neutral", label: "Neutro" },
  { value: "friendly", label: "Amigável" },
  { value: "assertive", label: "Assertivo" },
  { value: "technical", label: "Técnico" }
];

const lengths: Array<{ value: LengthPreference; label: string }> = [
  { value: "concise", label: "Conciso" },
  { value: "balanced", label: "Equilibrado" },
  { value: "detailed", label: "Detalhado" }
];

export function SettingsPanel({
  settings,
  onChange,
  onSave,
  saving
}: Props) {
  const patch = <K extends keyof AppSettings>(
    key: K,
    value: AppSettings[K]
  ) => onChange({ ...settings, [key]: value });

  return (
    <section className="panel settings-panel">
      <div className="section-heading">
        <div>
          <span className="eyebrow">Comportamento da IA</span>
          <h2>Seu texto, no seu tom</h2>
        </div>
        <button className="primary compact" disabled={saving} onClick={onSave}>
          {saving ? "Salvando…" : "Salvar"}
        </button>
      </div>

      <label className="setting-block">
        <span>
          Formalidade <strong>{settings.formality}</strong>
        </span>
        <input
          type="range"
          min="1"
          max="5"
          value={settings.formality}
          onChange={(event) => patch("formality", Number(event.target.value))}
        />
        <small>
          <span>Conversacional</span>
          <span>Formal</span>
        </small>
      </label>

      <div className="setting-block">
        <span>Tom</span>
        <div className="segmented four">
          {tones.map((tone) => (
            <button
              className={settings.tone === tone.value ? "selected" : ""}
              key={tone.value}
              onClick={() => patch("tone", tone.value)}
            >
              {tone.label}
            </button>
          ))}
        </div>
      </div>

      <div className="setting-block">
        <span>Extensão</span>
        <div className="segmented">
          {lengths.map((length) => (
            <button
              className={settings.length === length.value ? "selected" : ""}
              key={length.value}
              onClick={() => patch("length", length.value)}
            >
              {length.label}
            </button>
          ))}
        </div>
      </div>

      <label className="setting-block shortcut-input">
        <span>Atalho global</span>
        <input
          value={settings.shortcut}
          placeholder="Option+Shift+Space"
          onChange={(event) => patch("shortcut", event.target.value)}
        />
        <small>Use combinações como Option+Shift+Space.</small>
      </label>

      <div className="switches">
        <Toggle
          checked={settings.confirmBeforeReplace}
          label="Confirmar antes de substituir"
          description="Mostra uma prévia quando o conteúdo é editável."
          onChange={(value) => patch("confirmBeforeReplace", value)}
        />
        <Toggle
          checked={settings.automaticToolbar}
          label="Menu automático"
          description="Exibe as ações depois que a seleção estabiliza."
          onChange={(value) => patch("automaticToolbar", value)}
        />
        <Toggle
          checked={settings.historyEnabled}
          label="Histórico sincronizado"
          description="Guarda resultados por até 30 dias."
          onChange={(value) => patch("historyEnabled", value)}
        />
      </div>
    </section>
  );
}

function Toggle({
  checked,
  label,
  description,
  onChange
}: {
  checked: boolean;
  label: string;
  description: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="toggle-row">
      <span>
        <strong>{label}</strong>
        <small>{description}</small>
      </span>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <i />
    </label>
  );
}
