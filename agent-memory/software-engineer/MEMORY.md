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
- `RuntimePause` é o gate único para polling, AXObserver, atalho e fallback de clipboard; callbacks revalidam a pausa após o debounce.
- Resultados da nota usam evento mais state pull: o backend publica o estado antes de emitir e o frontend registra o listener antes de consultar `current_note_result`.
- Recapturas AX equivalentes devem devolver o snapshot ativo do coordinator; retornar o UUID recém-capturado quebra `DebounceElapsed`.
- Diagnóstico do pipeline usa `VERBALIX_DIAGNOSTICS=1` e metadados estruturados sem texto, tokens ou credenciais.

## Erros Recorrentes & Soluções
- A Accessibility API é incompleta em alguns aplicativos; ausência de atributos deve produzir degradação segura.
- Testes assíncronos Rust exigem `macros` e um runtime habilitados no Tokio.
- Erros de setup Tauri precisam ser convertidos para `Box<dyn std::error::Error>`.
- Bundles ad-hoc podem manter uma entrada TCC visualmente habilitada que não corresponde ao requisito designado do build atual; nunca resetar TCC automaticamente.

## Dependências & Integrações
- Transformações passam exclusivamente pela Edge Function autenticada.
- Sessões sensíveis usam Keychain; preferências não sensíveis usam store local.
- O clamp do overlay usa `NSScreen.visibleFrame` capturado no setup da aplicação e mantém fallback pelos monitores do Tauri.
- O dispatcher de overlay cria, configura, posiciona, mostra, oculta e confirma `is_visible` exclusivamente dentro de `run_on_main_thread`.

## Observações
- A validação manual AX exige um app assinado/em execução com permissão de Acessibilidade e não pode ser substituída por testes unitários.
- O fluxo toolbar → transformação → preview → apply → undo é coberto por integração com adapters; a versão desktop real permanece gate manual por exigir AX, app externo focado e sessão remota.
- `npm test`, `npm run test:coverage`, `npm run build`, `cargo test`, `cargo clippy` e o bundle smoke são os gates mínimos antes do handoff.
- Identidade TCC estável entre builds exige Apple Development ou Developer ID; assinatura ad-hoc e mudança de caminho exigem reautorizar o bundle exato.
