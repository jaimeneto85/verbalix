# 001 — Verbalix macOS MVP

## Contexto

O Verbalix é um aplicativo macOS para engenheiros que transforma texto selecionado em qualquer aplicativo compatível. O MVP traduz português e inglês, aprimora texto técnico no idioma original e usa recursos nativos do macOS para preservar foco, seleção e contexto.

## Escopo entregue

- Tauri 2 com React, TypeScript, Vite e núcleo Rust para macOS 14+.
- Aplicativo menu-bar/accessory com onboarding de Acessibilidade e configurações.
- Captura híbrida via AXUIElement/AXObserver e atalho Option+Shift+Space.
- Toolbar e nota por `NSPanel` não ativante, com `NSScreen.visibleFrame`.
- Tradução PT→EN, EN→PT e outros idiomas→PT; aprimoramento com formalidade, extensão e tom.
- Substituição revalidada, preview opcional, undo temporário e fallback read-only.
- Fallback copy-only acionado pelo atalho, preservando/restaurando o pasteboard sem colar.
- Supabase Auth magic link, sessão no Keychain, Edge Function com `AiProvider` e histórico opcional com RLS e retenção de 30 dias.
- Pausa completa de polling, AXObserver, atalho e clipboard.
- Entrega de resultado da nota resiliente à inicialização do listener.

## Arquitetura

O domínio e o coordinator dependem de ports para seleção, overlays, provider e persistência. Adapters Rust implementam Accessibility, AppKit, clipboard, Keychain e Supabase. A Edge Function mantém a chave OpenAI fora do cliente e usa modelo configurável por ambiente. Snapshots imutáveis, request IDs e recovery de estado evitam aplicar respostas antigas ou incompletas.

## Qualidade

| Gate | Resultado |
|---|---:|
| Rust | 24 testes aprovados |
| Frontend | 21 testes aprovados |
| Edge Function | 6 testes aprovados |
| Total | 51 testes, 0 falhas |
| TypeScript/Vite | Build aprovado |
| Cargo fmt | Aprovado |
| Clippy `-D warnings` | Aprovado |
| Bundle debug `.app` | Gerado |
| Arquivos acima de 300 linhas | 0 |
| QA final de código | APPROVED |

## Segurança e privacidade

- Conteúdo somente é enviado depois de ação explícita.
- Texto, respostas e segredos não são registrados em logs.
- OpenAI é acessada exclusivamente pela Edge Function.
- Histórico é opt-in, owner-only por RLS, excluível e expira após 30 dias.
- Campos seguros são ignorados e qualquer falha antes da revalidação é não destrutiva.

## Gates manuais antes da distribuição

T5.4 e T5.5 permanecem abertos porque exigem bundle assinado, permissão de Acessibilidade, aplicativos externos focados e sessão remota válida:

- Validar Chrome, Safari, VS Code, Slack, Notes e TextEdit.
- Exercitar seleção por mouse/teclado, toolbar, transformação, preview/aplicar e undo.
- Verificar conteúdo editável e read-only, AXObserver, `NSPanel`, múltiplos monitores e fullscreen.
- Confirmar restauração integral do clipboard em processo real.

## Status

Código aprovado pelo QA e preservado na branch/worktree `verbalix-macos-mvp`. Não foi feito merge em `main`. A distribuição permanece bloqueada até concluir os gates manuais acima e obter aprovação explícita do usuário para integração.
