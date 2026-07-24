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
- No macOS 26, uma seleção física pode expor `AXSelectedTextMarkerRange` mesmo quando `AXSelectedText` retorna `no_value`/`attribute_unsupported` e `AXSelectedTextRange` é um CFRange vazio. A rota segura usa o marker opaco com `AXStringForTextMarkerRange`, `AXBoundsForTextMarkerRange`, start/end markers e índices/length parametrizados; nunca lê `AXValue` do documento inteiro.
- Testes assíncronos Rust exigem `macros` e um runtime habilitados no Tokio.
- Erros de setup Tauri precisam ser convertidos para `Box<dyn std::error::Error>`.
- Bundles ad-hoc podem manter uma entrada TCC visualmente habilitada que não corresponde ao requisito designado do build atual; nunca resetar TCC automaticamente.
- Refresh de sessão deve separar autenticação inválida (`400/401/403`) de indisponibilidade transitória (`429/5xx`, transporte ou JSON inválido); somente a primeira rota abre o login.

## Dependências & Integrações
- Transformações passam exclusivamente pela Edge Function autenticada.
- Sessões sensíveis usam Keychain; preferências não sensíveis usam store local.
- O clamp do overlay usa `NSScreen.visibleFrame` capturado no setup da aplicação e mantém fallback pelos monitores do Tauri.
- O dispatcher de overlay cria, configura, posiciona, mostra, oculta e confirma `is_visible` exclusivamente dentro de `run_on_main_thread`.
- Snapshots derivados de text markers são sempre read-only até existir um contrato reversível de mutação; `replace` e `restore` rejeitam `writable=false`, e o caminho clássico revalida e escreve usando o mesmo handle AX.
- Diagnósticos AX usam estágio, origem de extração e categoria tipados, emitindo novamente apenas quando a categoria de um estágio/origem muda.
- Depois de trocar a classe nativa de uma WebView para `NSPanel`, não chamar setters de janela do wrapper Tauri que dependam da classe/ivars originais; o painel não ativante deve ser configurado integralmente no boundary AppKit.
- Durante o MVP diagnosticável, `ActivationPolicy::Regular`, fechamento da janela principal como hide e reabertura centralizada por Dock/tray mantêm o processo observável.
- Configuração pública do backend usa pares completos na ordem processo `VITE_*`, processo legado `VERBALIX_*`, embedded `VITE_*`, embedded legado; o build gera constantes em `OUT_DIR` para não transportar valores por stdout.
- Edge Functions mantêm `Deno.serve` apenas no entrypoint; handler, autenticação, provider factory, secrets e scheduler são injetáveis para testes sem rede ou efeitos colaterais.
- A defesa de autenticação da Edge confirma o bearer no endpoint `/auth/v1/user` e rejeita a anon key e papéis anônimos mesmo com `verify_jwt=true`.
- Limites da transformação são aplicados em três camadas: body HTTP em streaming antes do parse, caracteres Unicode no contrato e tokens/caracteres na saída do provider.
- Timeout total usa `Promise.race` além de `AbortController`, pois um adapter defeituoso pode ignorar o sinal de cancelamento.

## Observações
- A validação manual AX exige um app assinado/em execução com permissão de Acessibilidade e não pode ser substituída por testes unitários.
- O fluxo toolbar → transformação → preview → apply → undo é coberto por integração com adapters; a versão desktop real permanece gate manual por exigir AX, app externo focado e sessão remota.
- `npm test`, `npm run test:coverage`, `npm run build`, `cargo test`, `cargo clippy` e o bundle smoke são os gates mínimos antes do handoff.
- Identidade TCC estável entre builds exige Apple Development ou Developer ID; assinatura ad-hoc e mudança de caminho exigem reautorizar o bundle exato.
