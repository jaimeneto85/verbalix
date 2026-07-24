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
- O coordinator encerra transformações por uma única rotina que recupera a toolbar apenas se a mesma request ainda possui o lease.
- Transformações fixam `snapshot.id + request_id` no coordinator antes do primeiro `await`; o contexto local não integra o payload remoto e uma segunda ação em `Processing` é rejeitada.
- Polling, AXObserver e mouse dismiss usam invalidação transitória: ela fecha estados ociosos, mas não apaga `Processing`; a revalidação AX final continua impedindo escrita após mudança real.
- Durante `Processing`, uma captura bem-sucedida equivalente preserva snapshot/request originais; um candidato diferente substitui atomicamente o lease por `Candidate`, oculta o overlay anterior e torna a conclusão antiga inerte. Isso é cancelamento lógico, não cancelamento físico do provider.
- Implementações macOS extensas ficam separadas por responsabilidade: acessibilidade, observer, restauração e overlay.
- `RuntimePause` é o gate único para polling, AXObserver, atalho e fallback de clipboard; callbacks revalidam a pausa após o debounce.
- Resultados da nota usam evento mais state pull: o backend publica o estado antes de emitir e o frontend registra o listener antes de consultar `current_note_result`.
- Recapturas AX equivalentes devem devolver o snapshot ativo do coordinator; retornar o UUID recém-capturado quebra `DebounceElapsed`.
- Diagnóstico do pipeline usa `VERBALIX_DIAGNOSTICS=1` e metadados estruturados sem texto, tokens ou credenciais.
- Boundaries macOS e seus testes ficam em módulos separados quando necessário para manter responsabilidade única e o hard gate de 300 linhas por arquivo.

## Erros Recorrentes & Soluções
- A Accessibility API é incompleta em alguns aplicativos; ausência de atributos deve produzir degradação segura.
- No macOS 26, uma seleção física pode expor `AXSelectedTextMarkerRange` mesmo quando `AXSelectedText` retorna `no_value`/`attribute_unsupported` e `AXSelectedTextRange` é um CFRange vazio. A rota segura usa o marker opaco com `AXStringForTextMarkerRange`, `AXBoundsForTextMarkerRange`, start/end markers e índices/length parametrizados; nunca lê `AXValue` do documento inteiro.
- Fallback de text marker deve preservar `AxFailure` completo e autorizar somente combinações explícitas de estágio/categoria; falhas estruturais, de geometria ou de tipo encerram a captura.
- `EmptyRange` é um estado temporal terminal da rota clássica, não evidência de capacidade marker; nunca deve autorizar fallback entre leituras.
- Replace e restore clássicos precisam exigir `AXIdentifier` não vazio antes do primeiro lookup AX; restore também revalida identidade completa, PID, role, writability, texto atual, location e length UTF-16 no mesmo handle antes de qualquer escrita.
- Testes assíncronos Rust exigem `macros` e um runtime habilitados no Tokio.
- Erros de setup Tauri precisam ser convertidos para `Box<dyn std::error::Error>`.
- Bundles ad-hoc podem manter uma entrada TCC visualmente habilitada que não corresponde ao requisito designado do build atual; nunca resetar TCC automaticamente.
- Refresh de sessão deve separar autenticação inválida (`400/401/403`) de indisponibilidade transitória (`429/5xx`, transporte ou JSON inválido); somente a primeira rota abre o login.
- Readiness de IA deve ter uma única autoridade no command Tauri; uma pré-checagem async no overlay cria uma janela de invalidação antes de `Processing`.
- Depois que `SelectionPort::replace` retorna sucesso, `Applied` é o commit point. Feedback de undo é best-effort e não pode reclassificar a mutação.
- Autorização final, write AX e transição para `Applied` são linearizados pelo mutex do state. Feedback de erro só pertence à request enquanto o snapshot ID atual ainda coincide; conclusão superseded não pode mostrar erro sobre o novo alvo.

## Dependências & Integrações
- Transformações passam exclusivamente pela Edge Function autenticada.
- Sessões sensíveis usam Keychain; preferências não sensíveis usam store local.
- O clamp do overlay usa `NSScreen.visibleFrame` capturado no setup da aplicação e mantém fallback pelos monitores do Tauri.
- O dispatcher de overlay cria, configura, posiciona, mostra, oculta e confirma `is_visible` exclusivamente dentro de `run_on_main_thread`.
- Snapshots derivados de text markers são sempre read-only até existir um contrato reversível de mutação; `replace` e `restore` rejeitam `writable=false`, e o caminho clássico revalida e escreve usando o mesmo handle AX.
- Replace macOS resolve o elemento focado no PID original com `AXUIElementCreateApplication` e revalida identidade forte, texto, range UTF-16 e writability no mesmo handle antes do setter; ponteiros AX não atravessam awaits.
- Diagnósticos AX usam estágio, origem de extração e categoria tipados, emitindo novamente apenas quando a categoria de um estágio/origem muda.
- Depois de trocar a classe nativa de uma WebView para `NSPanel`, não chamar setters de janela do wrapper Tauri que dependam da classe/ivars originais; o painel não ativante deve ser configurado integralmente no boundary AppKit.
- Overlays macOS usam uma única conversão de coordenadas AX top-left para Cocoa bottom-left baseada no `frame.maxY` da zero screen, `NSScreen.screens.first`; `mainScreen` representa a tela da key window e nunca pode ser a referência global. Seleção de tela usa `frame`, clamp usa `visibleFrame`, e `setFrameOrigin:` recebe diretamente pontos Cocoa sem `scale_factor`.
- O fallback geométrico puro prioriza `SelectedRange`; cursor só é aceito quando finito e inclusivamente contido em um `FocusedElement` válido, com limites aritméticos finitos; o frame permanece o último fallback e cursor órfão falha fechado.
- A superfície transparente precisa ser composta em três camadas: classe de documento aplicada antes do render para `html/body/#root`, WebView criada com `transparent(true)` e `NSPanel` não opaco com `NSColor.clearColor`.
- Janelas de overlay novas permanecem hidden até o handshake frontend `overlay_surface_ready`; readiness e visibilidade desejada são estados separados, e `HideAll` cancela o desejo antes de esconder para que readiness atrasada não ressuscite uma seleção inválida.
- O handshake de readiness nasce em `useLayoutEffect`, depois do commit React, e só aceita ACK após a main thread aplicar o estado nativo. Tentativas são idempotentes, limitadas por quantidade, param no primeiro ACK e registram a exaustão.
- Readiness de overlay é vinculada a uma geração UUID emitida pelo Rust e inserida na URL. O comando valida geração, label e a `NSView` da WebView chamadora atual; ACK antigo retorna `false`. No segundo page load, o runtime invalida a geração e destrói a WebView; a próxima solicitação cria UUID/URL frescos. Falhas de invalidação, destruição e fallback de ocultação são diagnosticadas.
- Retries frontend são estritamente sequenciais após `false`/erro, limitados a três e não usam timeout por `Promise.race`.
- Rotas `?overlay=` sem um UUID v4 Rust válido preservam a superfície transparente, mas renderizam root vazio e nunca a aplicação principal.
- Criação de overlay é transacional: a geração aberta por `build` só permanece válida após `configure`; qualquer falha invalida o documento e executa rollback `destroy → hide`, com diagnóstico de todas as etapas. Uma janela parcialmente configurada nunca pode ser reutilizada.
- Toda invalidação de documento usa compare-and-invalidate com a geração esperada. Callbacks e rollbacks stale retornam `false`, são diagnosticados e nunca removem a geração atual.
- `AppRuntime` vive em `runtime.rs`; o registro de comandos em `lib.rs` não deve voltar a ultrapassar o hard gate de 300 linhas.
- Durante o MVP diagnosticável, `ActivationPolicy::Regular`, fechamento da janela principal como hide e reabertura centralizada por Dock/tray mantêm o processo observável.
- Configuração pública do backend usa pares completos na ordem processo `VITE_*`, processo legado `VERBALIX_*`, embedded `VITE_*`, embedded legado; o build gera constantes em `OUT_DIR` para não transportar valores por stdout.
- Edge Functions mantêm `Deno.serve` apenas no entrypoint; handler, autenticação, provider factory, secrets e scheduler são injetáveis para testes sem rede ou efeitos colaterais.
- A defesa de autenticação da Edge confirma o bearer no endpoint `/auth/v1/user` e rejeita a anon key e papéis anônimos mesmo com `verify_jwt=true`.
- Limites da transformação são aplicados em três camadas: body HTTP em streaming antes do parse, caracteres Unicode no contrato e tokens/caracteres na saída do provider.
- Timeout total usa `Promise.race` além de `AbortController`, pois um adapter defeituoso pode ignorar o sinal de cancelamento.
- O provider da Responses API usa `reasoning.effort: none` e orçamento de output calculado por caracteres Unicode: `ceil(chars * 2/3 + 128)`, limitado entre 500 e 8.000 tokens. Isso evita pagar o teto em seleções curtas sem quebrar o contrato de 12.000 caracteres.
- Envelopes OpenAI só são aceitos com `status === "completed"` e `incomplete_details` ausente ou nulo. Status ausente, desconhecido ou incompleto, inclusive `max_output_tokens`, falha como `INVALID_RESPONSE` antes do parse e nunca chega a replace/history.

## Observações
- A validação manual AX exige um app assinado/em execução com permissão de Acessibilidade e não pode ser substituída por testes unitários.
- O fluxo toolbar → transformação → preview → apply → undo é coberto por integração com adapters; a versão desktop real permanece gate manual por exigir AX, app externo focado e sessão remota.
- `npm test`, `npm run test:coverage`, `npm run build`, `cargo test`, `cargo clippy` e o bundle smoke são os gates mínimos antes do handoff.
- Identidade TCC estável entre builds exige Apple Development ou Developer ID; assinatura ad-hoc e mudança de caminho exigem reautorizar o bundle exato.
