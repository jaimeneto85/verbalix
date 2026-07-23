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

## Erros Recorrentes & Soluções
- A Accessibility API é incompleta em alguns aplicativos; ausência de atributos deve produzir degradação segura.
- Testes assíncronos Rust exigem `macros` e um runtime habilitados no Tokio.
- Erros de setup Tauri precisam ser convertidos para `Box<dyn std::error::Error>`.

## Dependências & Integrações
- Transformações passam exclusivamente pela Edge Function autenticada.
- Sessões sensíveis usam Keychain; preferências não sensíveis usam store local.

## Observações
- A validação manual AX exige um app assinado/em execução com permissão de Acessibilidade e não pode ser substituída por testes unitários.
- `npm test`, `npm run build` e `cargo test` são os gates mínimos antes do handoff.
