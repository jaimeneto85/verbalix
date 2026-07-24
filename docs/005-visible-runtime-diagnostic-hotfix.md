# 005 — Runtime visível, toolbar resiliente e readiness de IA

## Resultado

O hotfix foi aprovado para código e comportamento local. O Verbalix permanece visível no Dock/Cmd+Tab durante o MVP, fechar a janela principal apenas a oculta e Dock/tray conseguem reabri-la.

A toolbar não encerra mais o processo ao aparecer. A causa era `WebviewWindow::set_focusable(false)` depois da troca dinâmica da classe nativa para `NSPanel`; o setter Tauri acessava um ivar ausente na classe convertida. A configuração não ativante agora fica integralmente no boundary AppKit.

## Posicionamento

Falhas de `AXBoundsForRange` não viram mais o sentinela `(0,0,1,1)`. A geometria segue:

1. bounds válidos da seleção;
2. frame/posição+tamanho do elemento AX;
3. posição global do cursor via Core Graphics.

O trace registra somente a fonte e valores geométricos sanitizados, nunca o texto selecionado.

## Ações de IA

Traduzir e Aprimorar não falham mais silenciosamente. O runtime distingue login ausente, backend não configurado e provider indisponível, mostra uma mensagem dimensionada e permite abrir a janela principal para autenticação/configuração.

Configuração pública pode ser embutida no build do bundle, com override de runtime para desenvolvimento. Segredos OpenAI e service-role continuam exclusivos ao backend.

Os nomes canônicos são `VITE_SUPABASE_URL` e `VITE_SUPABASE_ANON_KEY`. O build Tauri lê o par completo do `.env` ignorado na raiz e o embute no nativo; variáveis completas do processo prevalecem em desenvolvimento. `VERBALIX_SUPABASE_URL` e `VERBALIX_SUPABASE_ANON_KEY` permanecem apenas como aliases legados e nunca são combinados parcialmente com o par canônico.

No smoke deste artefato, a interação exibiu corretamente a mensagem de backend ausente. A transformação real permanece bloqueada porque não existem URL/anon key configuradas, projeto Supabase/Edge Function implantado nem sessão autenticada. Portanto, este documento não declara tradução ou aprimoramento por IA funcionais.

## Evidências

- QA independente: `APPROVED`.
- Rust: 49/49.
- Clippy com warnings como erro.
- Vitest: 28/28.
- Playwright: 3/3.
- Edge Function: 6/6.
- Build e analyzer aprovados.
- Computer Use: processo vivo, toolbar interativa e erro explícito de backend ausente.

## Status

Branch `hotfix-visible-runtime-debug`, HEAD base de QA `0e4edbf`. Nenhum merge ou push foi realizado.

Próximo gate externo: provisionar Supabase URL/anon key, implantar a Edge Function, criar sessão válida e repetir Traduzir/Aprimorar com resposta real.
