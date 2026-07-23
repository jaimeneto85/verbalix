# Agent Memory — software-engineer

## Padrões do Projeto
- O domínio é isolado de Tauri, macOS e Supabase por ports pequenos.
- Snapshots de seleção são imutáveis e toda escrita exige revalidação.

## Stack & Configuração
- Tauri 2, React, TypeScript, Vite e Rust.
- O MVP requer macOS 14+; integrações específicas usam `cfg(target_os = "macos")`.

## Padrões de Código
- Tipos compartilhados Rust são serializáveis em camelCase para consumo da WebView.
- Falhas externas são convertidas em erros de domínio e nunca incluem o texto selecionado.
- O coordinator encerra transformações por uma única rotina que recupera o estado da toolbar em qualquer falha pós-`Processing`, preservando latest-wins.
- Implementações macOS extensas ficam separadas por responsabilidade: acessibilidade, observer, restauração e overlay.

## Erros Recorrentes & Soluções
- A Accessibility API é incompleta em alguns aplicativos; ausência de atributos deve produzir degradação segura.
- Testes assíncronos Rust exigem `macros` e um runtime habilitados no Tokio.
- Erros de setup Tauri precisam ser convertidos para `Box<dyn std::error::Error>`.

## Dependências & Integrações
- Transformações passam exclusivamente pela Edge Function autenticada.
- Sessões sensíveis usam Keychain; preferências não sensíveis usam store local.
- O clamp do overlay usa `NSScreen.visibleFrame` capturado no setup da aplicação e mantém fallback pelos monitores do Tauri.

## Observações
- A validação manual AX exige um app assinado/em execução com permissão de Acessibilidade e não pode ser substituída por testes unitários.
- `npm test`, `npm run test:coverage`, `npm run build`, `cargo test`, `cargo clippy` e o bundle smoke são os gates mínimos antes do handoff.
