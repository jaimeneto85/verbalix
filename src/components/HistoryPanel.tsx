import type { HistoryItem } from "../types";

type Props = {
  items: HistoryItem[];
  enabled: boolean;
  onDelete: (id: string) => void;
  onDeleteAll: () => void;
};

export function HistoryPanel({
  items,
  enabled,
  onDelete,
  onDeleteAll
}: Props) {
  return (
    <section className="panel history-panel">
      <div className="section-heading">
        <div>
          <span className="eyebrow">Últimos 30 dias</span>
          <h2>Histórico</h2>
        </div>
        {items.length > 0 && (
          <button className="ghost danger" onClick={onDeleteAll}>
            Excluir tudo
          </button>
        )}
      </div>
      {!enabled ? (
        <div className="empty-state">Ative o histórico nas configurações.</div>
      ) : items.length === 0 ? (
        <div className="empty-state">Suas próximas transformações aparecerão aqui.</div>
      ) : (
        <div className="history-list">
          {items.map((item) => (
            <article key={item.id}>
              <div className="history-meta">
                <span>{item.operation === "translate" ? "Tradução" : "Aprimoramento"}</span>
                <time>{new Date(item.created_at).toLocaleString("pt-BR")}</time>
              </div>
              <p>{item.source_text}</p>
              <p className="result">{item.result_text}</p>
              <div className="history-actions">
                <button onClick={() => navigator.clipboard.writeText(item.result_text)}>
                  Copiar
                </button>
                <button onClick={() => onDelete(item.id)}>Excluir</button>
              </div>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
