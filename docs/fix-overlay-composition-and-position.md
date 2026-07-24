# Entrega: composição e posição do overlay

## Resultado

O código está aprovado para o gate manual CA6. A toolbar e a nota usam uma superfície transparente restrita às rotas de overlay, um `NSPanel` clear/non-opaque e posicionamento em pontos AppKit a partir da zero screen.

## Correções

- Fundo global e dimensões mínimas da janela principal não atingem mais `html`, `body` e `#root` dos overlays.
- O macOS não usa `LogicalPosition` nem `scale_factor` para posicionar a toolbar.
- Seleção de tela, conversão AX → Cocoa, centralização, fallback vertical e clamp são determinísticos.
- A WebView permanece escondida até o commit React e o ACK aplicado na main thread.
- UUID por documento, identidade da `NSView` chamadora e compare-and-invalidate impedem ACK, reload ou rollback stale.
- Reload destrói a WebView antiga e a solicitação seguinte cria URL e geração novas.
- Falhas de build/configuração fazem rollback transacional e não deixam janela opaca ou ativante reutilizável.

## Evidências automatizadas

- Rust: 93/93.
- Vitest: 47/47.
- Playwright: 6/6.
- Cobertura frontend configurada: 100%.
- Build, fmt, check, Clippy estrito e diff-check: aprovados.
- Arquivos de produção: no máximo 300 linhas.
- Trivy: zero HIGH/CRITICAL e zero misconfigurações.
- QA dual final: `APPROVED`.

## Gate restante

Antes de merge e release, o agente raiz deve executar Computer Use no bundle reconstruído e comprovar:

- cantos realmente transparentes;
- toolbar centralizada sobre seleção real na tela correta;
- foco permanece no aplicativo alvo;
- reload/seleção subsequente recupera a toolbar;
- processo permanece estável.
