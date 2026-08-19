# Agent Memory — workflow-orchestrator

## Padrões do Projeto
- O Verbalix usa Tauri 2 como shell desktop e Rust para a lógica central e integrações nativas do macOS.
- Funcionalidades dependentes de seleção global são isoladas atrás de contratos testáveis e adapters de plataforma.
- Toda tarefa é executada em worktree dedicado e só é integrada à branch de origem após aprovação explícita.

## Decisões Arquiteturais
- Interpretação ao vivo (M2, `docs/013`, branch `live-interpretation`): pipeline frase-a-frase SEM mic
  virtual. Edge Function `interpret` (split `index/handler/stages/contract/provider/service_client`,
  `verify_jwt=true`) faz STT (ElevenLabs Scribe `/v1/speech-to-text`) → tradução (OpenAI Responses,
  prompt com língua-alvo EXPLÍCITA — NÃO o PT↔EN do transform — e texto delimitado em `<untrusted_text>`
  com invariante de sistema contra prompt-injection) → TTS (`/v1/text-to-speech/{voice_id}`). `provider_voice_id`
  resolvido server-side pelo `service_client.ts` (perfil `ready` por `user_id` do JWT), nunca no cliente.
  Timeout re-derivado para 3 round-trips encadeados: 45 s abort na Edge Function, 55 s no cliente Rust
  (NÃO copiar os 20 s do transform). `ErrorCode` stage-específico (`STT_FAILED`/`TRANSLATION_FAILED`/
  `TTS_FAILED`) preservado até o Rust — único sinal de debug (conteúdo nunca logado).
- M2 coordinator: `LiveInterpretationCoordinator` decomposto em `live_interpretation.rs` (estado + fiação),
  `live_queue.rs` (reorder buffer bounded + backpressure, PURO) e `live_worker.rs` (dispatch concorrente,
  cap 2 em voo). Dispatch é CONCORRENTE (N+1 começa STT sem esperar TTS de N) mas playback ORDENADO via
  reorder buffer (N só toca após N-1; falha de N-1 libera N sem travar). `accepts(session_id, segment_id)`
  puro invalida sessão parada/trocada (fail-closed). Circuit-breaker (K falhas → leave_live), auto-leave
  idle, permissão revogada mid-sessão → fail-closed. VAD suprimido enquanto `Speaking` (feedback loop
  mic→alto-falante; recomendar headphones — gate manual). Captura streaming e enrollment MUTUAMENTE
  EXCLUSIVAS sobre o mesmo worker `MacAudioCapture` via extensão do `CaptureCommand`.
- M2 `RuntimePause` on-air: `on_air` é um `AtomicBool` TERCEIRO e INDEPENDENTE com `OnAirGuard` RAII próprio
  — NUNCA reusar `ActionGuard`/`grace_deadline`/`in_flight` (afinados p/ ações sub-segundo; segurar por uma
  sessão de minutos suprimiria a toolbar/nota o tempo todo — regressão M1 silenciosa que nenhum teste
  existente pegaria). Compõe via `!is_on_air()` nos 5 entrypoints. Tray "Pausar" durante on-air → `leave_live`
  NÃO-BLOQUEANTE (spawn), nunca síncrono da callback do menu esperando reply de worker (deadlock de UI).
  Teste dedicado: supressão dura a SESSÃO INTEIRA, não os 400 ms do grace.
- M2 playback: `MacAudioPlayback` cpal espelha o padrão thread-dedicada de `MacAudioCapture` (Stream não-Send
  possuído pela thread; comandos por canal) COM reply-timeout no `Play` (device desconectado não trava o
  worker). `encode_wav`/`resample_to_16k` extraídos p/ `platform/audio_wav.rs` compartilhável (puros, sem cpal).
- M2 setting `target_language`: `#[serde(default)]`, allowlist, preservada em `apply_remote` via struct-literal,
  NÃO sincronizada. A sessão captura a língua-alvo no `enter_live` (snapshot); mudança só vale no próximo enter.
- Voz (M1 enrollment, `docs/012`): segredo/`provider_voice_id` server-only via OPÇÃO A —
  cliente NUNCA consulta a tabela via PostgREST; 3 Edge Functions (`voice-enroll`/`voice-delete`/
  `voice-status`) escrevem/leem com `SUPABASE_SERVICE_ROLE_KEY` escopando por `user_id` do JWT e
  retornam só `VoiceProfileView` (voiceProfileId/status/displayName). A migration NÃO concede SELECT
  a `authenticated` (só INSERT/UPDATE/DELETE como defesa em profundidade). A tentativa de view
  `security_invoker=on` + `revoke select` é um MODELO QUEBRADO (permission denied com invoker=on;
  vazamento de linhas de terceiros com invoker=off) — rejeitada na análise dual.
- Áudio de enrollment: `cpal::Stream` NÃO é `Send` no backend CoreAudio. Padrão adotado: thread de
  captura DEDICADA possui o Stream; surface `Send`-safe = canal `mpsc` (start/stop/cancel) +
  `Arc<AtomicU32>` (nível). `start()` confirma abertura por canal de reply síncrono antes de retornar
  (senão device ausente só falha em `stop()`). Formato FIXADO mono 16 kHz 16-bit WAV (~5 MB base64
  p/ 120 s) para caber no cap; base64 gerado no Rust, áudio bruto nunca cruza o React. Permissão de mic
  AVFoundation é ASSÍNCRONA → `request_microphone_permission` é command async + evento (publish-then-emit),
  nunca síncrono bloqueante. Crates: `cpal` (captura) + `objc2-av-foundation` (permissão), cfg macos.
- Payload de áudio excede o cap de 64KB do transform: contract novo precisa de cap próprio (~10 MB
  binário; lembrar overhead ~33% do base64 ao dimensionar o cap de corpo) e timeout maior (60 s p/ IVC).
- Idempotência de enroll: dedup por `request_id` do CLIENTE (NUNCA comparar com a coluna `id` gerada
  pelo DB — espaços de UUID distintos, bug clássico que passou nos testes com fixture impossível).
  Consistência: cleanup best-effort da voz na ElevenLabs se a persistência falhar pós-criação (sem
  órfão billado); partial unique index `(user_id) WHERE status NOT IN ('deleting','failed')` contra
  corrida concorrente; replace do perfil anterior no re-enroll.
- Campo novo em `AppSettings` que não deve sincronizar (`voice_profile_id`): `#[serde(default)]` +
  o struct-literal em `remote_preferences::apply_remote` FORÇA (compile-time) preservar o valor local,
  protegendo contra clobber pelo remoto de graça.
- O companion iOS começa como Swift Package puro `ios/VerbalixKit` (só Foundation + Security, zero deps externas),
  com `platforms: [.iOS(.v17), .macOS(.v14)]` — macOS existe só para `swift test` rodar no host sem simulador.
  supabase-swift só entra na Fase 3 (app SwiftUI/extensões), ainda pendente de decisão de tooling.
- O contrato iOS (`Transform.swift`) espelha `supabase/functions/transform/contract.ts` no wire (camelCase),
  com `requestId` serializado em lowercase via `CodingKeys`+`encode(to:)`. Guardas 12.000 unicode scalars e
  64 KiB do corpo JSON são INDEPENDENTES; o guard de 64 KiB é pré-check defensivo do cliente (index.ts não
  impõe tamanho de corpo). Fixtures Swift derivam dos mesmos literais de `contract_test.ts` para evitar drift.
- Sync de preferências: só campos de IA (`formality`, `length`, `tone`, `history_enabled`) trafegam;
  `shortcut`/`automatic_toolbar`/`confirm_before_replace` são macOS-only e nunca entram/saem pelo sync.
  `settings.json` continua a fonte de verdade; qualquer falha de rede é não-fatal e nunca propaga erro.
  `load_settings` virou `async fn` (transparente ao frontend, que usa `invoke()` retornando Promise);
  `save_settings` segue não-bloqueante — grava local, re-registra shortcut e só então dispara upsert detached
  via `tauri::async_runtime::spawn`. Adapter `remote_preferences.rs` usa timeout estrito de 4s (bootstrap path).
- LWW de preferências: `updated_at` é server-authoritative (default now() + trigger BEFORE INSERT/UPDATE que
  força now()); merge trata remoto ausente/nulo como "infinitamente antigo" (local vence) e empate mantém local.
- O MVP tem macOS 14 como versão mínima e distribuição direta, assinada e notarizada fora da Mac App Store.
- Settings e onboarding usam a WebView do Tauri; observação de seleção, geometria e overlays usam APIs nativas do macOS.
- Resultados atrasados nunca podem alterar uma seleção nova: toda transformação referencia e revalida um snapshot.
- O caminho primário usa Accessibility API. O fallback copy-only preserva/restaura o clipboard, só ocorre pelo atalho Option+Shift+Space e nunca simula colagem.
- A transformação usa Supabase Edge Function com OpenAI atrás de `AiProvider`; o modelo é definido por ambiente e a chave não chega ao cliente.
- Supabase Auth usa magic link, a sessão fica no Keychain e o histórico opcional tem RLS owner-only e retenção de 30 dias.
- Magic links do app desktop exigem a entrada exata `verbalix://auth/callback` na allow-list remota do Supabase Auth; ausência da entrada faz o serviço retornar ao `site_url`, mesmo quando `emailRedirectTo` está correto no cliente.

## Erros Recorrentes & Soluções
- Repositórios sem commit não permitem o worktree convencional: criar primeiro um commit-base vazio em `main`.
- Nem todo aplicativo implementa todos os atributos AX: tratar ausência, timeout e elemento invalidado como falhas recuperáveis.
- O ícone carregado pelo Tauri no startup precisa ser PNG 8-bit RGBA. Um PNG 16-bit causou panic em `did_finish_launching`, antes da UI abrir.
- Ao validar correções de bundle, reconstruir o `.app` e conferir `Contents/Resources`, Info.plist, `codesign --verify --deep --strict` e launch smoke; artefatos antigos podem mascarar a correção.
- Callbacks do AXObserver não executam na main thread. Toda criação, configuração, posicionamento, emissão e show/hide de NSWindow/NSPanel deve passar pelo dispatcher `run_on_main_thread`; AppKit fora desse boundary encerra o processo com `Must only be used from the main thread`.
- Recapturas AX equivalentes criam novos UUIDs; `refresh_selection` deve retornar o snapshot ativo quando `same_target` para que polling/AXObserver debouncem o ID armazenado no coordenador.
- Uma ação de toolbar (`transform_selection`) leva segundos (refresh de sessão + provider, abort 20s). Durante ela, os entrypoints automáticos de detecção (polling, AXObserver, monitor global de mouse) precisam ser SUSPENSOS, senão uma falha de captura AX no meio despacha `Invalidated → hide_all → Idle` e fecha a nota recém-aberta ("abre e fecha"). Solução: estender `RuntimePause` (o single gate) com contador atômico `in_flight` + `ActionGuard` RAII aberto no topo de `transform_selection`; compor `!is_action_in_flight()` em `run_polling`/`run_ax_observer`/`run_mouse_dismiss`.
- O dismiss legítimo (`dismiss_overlays` de Escape/botão da nota, tray "Pausar", `undo`) despacha `Invalidated` DIRETO no coordinator, sem passar pelos entrypoints automáticos — por isso um gate aplicado só a polling/observer/mouse-dismiss preserva o dismiss do usuário integralmente.
- ATENÇÃO ao re-check pós-debounce (após o `thread::sleep(150ms)`): tanto a thread de polling quanto o callback do AXObserver têm um segundo dispatch de `DebounceElapsed` DEPOIS do sleep; ambos precisam checar `!is_paused() && !is_action_in_flight()`. QA pegou o AXObserver com o check interno faltando (só polling tinha) — sempre alinhar os dois pontos.
- O gate in-flight precisa de um curto período de graça pós-`Drop` da guarda (relógio injetável para teste determinístico, NÃO sleep) para cobrir o gap IPC+render entre `transform_selection` retornar e o frontend exibir a nota; sem isso uma falha de captura pós-retorno ainda reproduz o bug.
- Fallback de "última geometria conhecida" para nota de erro (`last_known_bounds`) DEVE ser escopado por `is_action_in_flight()` e o cache limpo em `Invalidated`. Caso contrário vaza para `ai_readiness` standalone (nota fantasma em posição obsoleta sem seleção) e reabre nota "zumbi" após dismiss legítimo durante a ação. Mouse-dismiss gate SÓ por `!is_action_in_flight()`, nunca por `is_paused()`, para não mudar a semântica de pausa.
- Um bundle ad-hoc sem `TeamIdentifier` usa requisito designado por `cdhash`; uma entrada antiga habilitada em Acessibilidade pode estar stale para o build atual. A recuperação é remover a entrada antiga, adicionar o bundle exato, habilitar e reabrir — nunca resetar TCC automaticamente.
- Depois de trocar dinamicamente uma `WebviewWindow` para `NSPanel`, setters do wrapper Tauri que dependem dos ivars da classe original podem causar panic; configure o painel inteiramente no boundary AppKit e não chame `set_focusable` após o swizzle.
- `AXBoundsForRange` pode falhar em apps como Slack. Nunca materializar a falha como retângulo sentinela: validar o range, tentar frame/posição+tamanho AX e por último o cursor global via Core Graphics.
- Superfícies Tauri transparentes também precisam neutralizar o fundo e as dimensões mínimas de `html/body/#root`; `transparent(true)` sozinho não remove o canvas CSS opaco.
- Coordenadas AX globais não devem passar por `LogicalPosition` no macOS. Converter uma vez para Cocoa usando `NSScreen.screens.firstObject()` como zero screen e aplicar `setFrameOrigin:` em pontos evita dupla escala Retina e a tela da key window.
- Quando `AXBoundsForRange` não existe, a geometria segue `SelectedRange → Cursor contido no frame focado → FocusedElement → None`; cursor global sem frame válido nunca é aceito e não recebe margem implícita.
- Contenção cursor-frame é uma heurística espacial, não temporal. Em editores grandes, validar por Computer Use seleção por mouse, teclado e cursor movido antes de merge/release; staleness exige um sinal causal separado.
- Readiness de overlay precisa de UUID por documento, caller `NSView`, ACK após a main thread e compare-and-invalidate. Reload e rollback devem destruir/inutilizar apenas a própria geração, nunca o documento atual.
- A criação de overlay é transacional: falha depois do build invalida a geração e destrói a janela, com hide diagnosticado como fallback.
- Publicação visual guardada precisa separar a lifetime cancelável da ação de um permit single-use por comando. Um claim único na lifetime bloqueia feedbacks sequenciais legítimos como Preview → erro de Apply.
- O boundary visual correto é `prepare → claim do permit → emit/show`: cancelamento durante preparação vence com zero efeito; cancelamento depois do claim lineariza `publish → hide` e termina oculto.
- `AXSecureTextField` é subrole. Gates de privacidade precisam classificar `AXRole + AXSubrole` antes de identifier, bounds, settable, token ou qualquer leitura de conteúdo, inclusive no último boundary do setter e em reconcile.
- Eventos de foco/destruição precisam revogar a geração antes de qualquer leitura AX auxiliar. Eventos de seleção própria exigem correlação one-shot forte; ausência ou mismatch deve falhar como evento externo imediatamente.
- AXIdentifier é identidade causal interna e não pertence a DTO/serde/IPC/Debug. Redigir somente o token não basta se o snapshot ainda serializa a mesma informação.
- Mutation ledgers devem expor outcomes tipados por operação; uma API genérica de terminalização permite transições cruzadas inválidas mesmo quando os callers atuais parecem corretos.

## Erros Recorrentes & Soluções (iOS/Swift)
- `swift test` bare no host NÃO tem entitlement de keychain-access-groups nem de App Group: `SecItem*` com
  `kSecAttrAccessGroup` e `FileManager.containerURL(forSecurityApplicationGroupIdentifier:)` falham. Solução:
  separar via protocolo (`SessionPersisting`) com double em memória e diretório tmp injetável; o caminho real
  de Keychain/App Group compila mas não roda em CI.
- `XCTAssertEqual(_:_:accuracy:)` exige `FloatingPoint` NÃO-opcional; comparar `TimeInterval?` quebra a
  compilação. Desembrulhar com `try XCTUnwrap` antes de comparar com tolerância.
- `NSLock.unlock()` em contexto assíncrono é warning no Swift 5 e ERRO no Swift 6 language mode. Usar locking
  com escopo (`withLock`) numa seção síncrona, sem `lock()/unlock()` cruzando `await`, em stubs de transporte.

## Aprendizados de QA
- M1 voz: verdes em TODOS os gates automáticos (clippy, cargo/deno/vitest/e2e, coverage 100%, bundle)
  NÃO garantiram correção. O qa-reviewer com dual analysis pegou 5 defeitos reais que os gates não
  viam: (1) idempotência comparando `id` (DB) vs `request_id` (cliente) com TESTE mascarando via
  fixture impossível; (2) voz órfã sem cleanup; (3) handler.ts 339 linhas > gate 300; (4) RLS SELECT
  expondo `provider_voice_id`; (5) corrida criando 2 perfis. LIÇÃO: rodar os gates é necessário mas
  insuficiente — a revisão de QA de conformidade (escopo/design/segurança) é o que fecha o buraco;
  desconfiar de teste que "passa" quando o cenário testado é impossível em produção.
- Relatórios finais de sub-agentes podem chegar truncados/otimistas; o orquestrador DEVE re-rodar os gates
  empiricamente (swift test, cargo test/clippy/fmt, npm test/build, deno, e os 3 xcodebuild) antes de aceitar
  qualquer verdict. Ao longo desta entrega os sub-agentes repetidamente: deixaram `Tests/` vazio; entregaram
  teste que não compilava (accuracy sobre Optional); pararam com código quebrado NÃO-commitado (RefreshLock
  com `flock` ambíguo; SessionRefresher lançando `.localFailure`); e terminaram sem commitar apesar de "tudo
  verde". Padrão de mitigação que funcionou: (1) escopo por rodada com "pouse SEMPRE verde+commitado"; (2)
  o orquestrador verifica git log/status + roda os gates a cada retorno; (3) preservar trabalho commitando
  quando o sub-agente esquece.
- M2 (interpretação ao vivo): tarefa GRANDE (Edge Function + domain + application concorrente + 2 adapters
  cpal + frontend) fez o `@software-engineer` retornar relatório TRUNCADO em 4 rodadas seguidas — SEMPRE
  parando mid-edit sem commitar e sem delegar. Padrão de mitigação que funcionou (repetível): a cada
  notificação de conclusão, (1) NÃO confiar no texto; inspecionar `git log`/`status` + rodar TODOS os gates
  empiricamente; (2) fazer um commit-checkpoint do trabalho em progresso que COMPILA (preserva contra reset
  de worktree); (3) re-delegar uma continuação BOUNDED com o inventário EXATO de falhas (nomes de teste,
  contagem de erros de clippy, arquivos). Convergiu de ~70% → verde em 3 continuações. No fim, o orquestrador
  teve que DIRIGIR cada handoff da cadeia (engineer→test-engineer→qa-reviewer) porque cada sub-agente
  truncou antes de delegar. O qa-reviewer também truncou sem verdict → o orquestrador conduziu a auditoria
  de conformidade ele mesmo (read-only) e emitiu o verdict.
- M2 clippy `-D warnings` "never used/constructed" (métodos/variantes/campos como `accepts`, `emit_live_state`,
  `stage_ms`) foi o MELHOR detector de FIAÇÃO INCOMPLETA: código implementado mas não ligado aos commands/
  eventos aparece como dead-code. LIÇÃO: exigir do engenheiro COMPLETAR a funcionalidade (ligar de verdade),
  NUNCA silenciar com `#[allow(dead_code)]`. `active_stream` "assigned but never read" x6 era bug real de
  atribuição morta no caminho de streaming/playback.
- M2 QA pegou 1 defeito de segurança que os gates verdes não viam: `interpret/translate` interpolava o texto
  transcrito direto no prompt sem o guard `<untrusted_text>`+invariante de sistema que o `transform` usa
  (prompt-injection). Reforça a lição do M1: auditar CONFORMIDADE (segurança/design), não só rodar gates.
  Quando um novo provider de LLM recebe texto do usuário/transcrição, SEMPRE espelhar o hardening do transform.
- Fase 2 (sync de prefs) tinha DEFEITO real de "remoto sempre vence": LWW exige timestamp local. Solução
  aprovada: SIDECAR `preferences_sync.json` (`{updatedAt, syncedAt, sequence}`) fora do `AppSettings` (que
  cruza IPC), com sequence-guard contra race entre `save_settings` síncrono e o spawn de `load_settings`;
  `load_settings` retorna local na hora e emite evento `preferences-synced` (padrão listen/emit de
  `note-result`, com comando de pull de fallback). No iOS, TODA edição local deve chamar `touch()` senão o
  bug reaparece.

## iOS — Auth deep link / Universal Links
- Bug de produção: magic link caiu em `http://localhost:3000/?error=access_denied&error_code=otp_expired`
  porque o `redirect_to` (`verbalix-ios://auth/callback`) NÃO estava na allow-list do Supabase → o serviço
  descarta o redirect e usa o Site URL. Allow-list é ação de OPS (dashboard), não de código.
- supabase-swift 2.x `AuthClient.session(from:)` (.pkce) JÁ trata `error`/`error_code`/`error_description`,
  lança `AuthError.pkceGrantCodeExchange(message:error:code:)` (inclui `otp_expired`) e faz o
  `exchangeCodeForSession` internamente com o `code_verifier` single-use do próprio storage. NÃO reimplementar
  o exchange manualmente. Padrão adotado: classificador PURO `AuthCallback.parse(url) -> .proceed(URL)|.failure`
  (valida host/path das 2 formas, lê query E fragment, mapeia error→VerbalixError pt-BR) e, no `.proceed`,
  delegar a `session(from:)`, mapeando `AuthError` lançado como 2ª rede. Isso torna os edge cases testáveis
  SEM rede e não duplica a lógica PKCE da lib.
- Race de cold start: em SwiftUI, `.onOpenURL` DENTRO de `if let appSession` PERDE o link quando o app é
  aberto fechado pelo e-mail (caminho mais comum). Anexar `.onOpenURL` no nível do `WindowGroup` e guardar
  `pendingURL` processada quando a sessão estiver pronta.
- `catch {}` em handlers de deep link é falha silenciosa: o caso `otp_expired` não mostrava nada. Sempre
  superficializar erro tipado numa `@Published`/observável (`callbackError`) exibida na tela de login.
- Migração para Universal Links é ADITIVA: manter `verbalix-ios://` no `CFBundleURLTypes` como fallback de
  PARSING. A EMISSÃO (`sendMagicLink redirectTo`) NÃO pode ser https-only hardcoded — isso deixou o usuário
  sem NENHUM caminho de login (domínio sem TLS + allow-list do custom scheme inútil porque nunca é emitido).
  CORRIGIDO: callback de emissão CONFIGURÁVEL via chave Info.plist `VerbalixAuthCallback`, injetada de uma
  build setting `VERBALIX_AUTH_CALLBACK` (default `verbalix-ios://auth/callback` no `settings` do project.yml →
  pbxproj, NÃO xcconfig por causa do `//`). `BackendConfig.authCallbackURL` lê a chave com FALLBACK seguro ao
  custom scheme quando ausente OU inválida (nunca crashar); `AuthService.callbackURL` vem da config (não é mais
  `static let`). Virar para Universal Links quando o domínio subir = mudança de CONFIG, sem tocar Swift. Durante
  a transição, AMBAS as URLs emitidas devem estar na allow-list do Supabase. Gate que importa: valor de
  `VerbalixAuthCallback` no Info.plist COMPILADO do `.app`.
- Associated Domains: chave `com.apple.developer.associated-domains: ["applinks:app.verbali.xyz"]` no
  `entitlements.properties` do target no `project.yml` (o `.entitlements` é REGENERADO por xcodegen — fonte da
  verdade é o project.yml). Em build de SIMULADOR (unsigned, `CODE_SIGNING_ALLOWED=NO`), `codesign -d
  --entitlements` NÃO mostra nada (entitlements embutem no signing de device); verificar o `.entitlements`
  GERADO. Universal Links não funcionam de forma confiável no simulador — teste é gate de DEVICE.
- AASA: arquivo `apple-app-site-association` SEM extensão, servido em `/.well-known/`, `Content-Type:
  application/json`, SEM redirect, TLS válido, sem auth; `appIDs=["<TeamID>.com.verbalix.ios"]` (Team ID é
  público — pode ir versionado no AASA), `components` restritos a `/auth/callback` (nunca `*`).

## iOS — Prontidão para App Store
- Em `.xcconfig`, `//` inicia COMENTÁRIO: gravar `VerbalixSupabaseURL = https://x.supabase.co` faz o valor
  resolver para `https:` (sem host). O build passa; só o Info.plist COMPILADO revela. `bootstrap.sh` escapa
  a barra (VERBALIX_SLASH). LIÇÃO GERAL: onde valores atravessam camadas (xcconfig→Info.plist→BackendConfig),
  verificar o VALOR FINAL COMPILADO com `plutil -p "<Verbalix.app>/Info.plist"`, não só que compila.
- Versões: `MARKETING_VERSION`/`CURRENT_PROJECT_VERSION` no `settings.base` do project.yml SÓ têm efeito se os
  Info.plist referenciam `$(MARKETING_VERSION)`/`$(CURRENT_PROJECT_VERSION)` — literais `1.0`/`1` ignoram a
  build setting. Centralizar em settings.base garante paridade app↔extensões (divergência = rejeição no upload).
- AppIcon: bloqueio duro. Asset catalog single-size (Xcode 14+): um PNG 1024x1024 com
  `Contents.json {idiom: universal, platform: ios, size: 1024x1024}` + `ASSETCATALOG_COMPILER_APPICON_NAME`.
  Ícone da App Store NÃO pode ter alpha: achatar com `magick "<src>" -background white -alpha remove -alpha off`
  (ImageMagick em /opt/homebrew; `sips` só zera alpha via round-trip JPEG lossy; PIL/pngcrush ausentes) e
  validar `sips -g hasAlpha` == no. COMMITAR o PNG achatado no appiconset para o build não depender de
  ImageMagick. Provar no `.app`: `AppIcon60x60@2x.png` presente e `Assets.car` gerado.
- `UILaunchScreen` (dict vazio basta) é bloqueio duro (SDK iOS 14+); provar no Info.plist compilado.
- Release signing: `CODE_SIGN_STYLE = Automatic` no `Release.xcconfig` (aplica aos 3 targets, pois `configFiles`
  é por-projeto) + `DEVELOPMENT_TEAM` via `Local.xcconfig` gitignored. NÃO mexer no `CODE_SIGNING_ALLOWED = NO`
  do Debug (é o que permite build de simulador sem Team). Build de simulador verde NÃO valida signing de
  Release/device — isso é gate manual.
- Simulador: há `iPhone 17 Pro` duplicados e runtimes 26.3/26.4/26.5 (SDK 26.5); fixar `OS=26.5` no
  `-destination` evita casar com runtime indisponível.

## iOS App + Extensões (Fases 3-5)
- Tooling: XcodeGen (`ios/project.yml` versionado → `ios/Verbalix.xcodeproj` gitignored). Targets: app
  `Verbalix`, `VerbalixAction` (com.apple.ui-services), `VerbalixKeyboard` (com.apple.keyboard-service),
  extensões embedadas, todos dependem do package local `ios/VerbalixKit`. Deployment iOS 17.0.
- Build de simulador SEM Team: entitlements de App Group/Keychain quebram assinatura, então a config de
  simulador usa `CODE_SIGNING_ALLOWED=NO`/`CODE_SIGNING_REQUIRED=NO`/`CODE_SIGN_IDENTITY=""` (em
  `Config/Debug.xcconfig` e/ou na linha de comando do xcodebuild). `DEVELOPMENT_TEAM` via `ios/Local.xcconfig`
  gitignored (`.example` versionado); vazio compila no simulador. Os 3 schemes buildam verdes assim.
- Config Supabase: `ios/scripts/bootstrap.sh` gera `ios/Config/Supabase.xcconfig` (gitignored) do `.env` da
  raiz e roda `xcodegen`. Em WORKTREE o `.env` (gitignored) não existe localmente; resolver a raiz do checkout
  via `git rev-parse --git-common-dir` (pai do `.git` comum), NUNCA `ios/../..` fixo. O script deve falhar
  loud se ausente e NUNCA ecoar URL/anon key.
- supabase-swift entra só aqui (produto `Auth`, resolvido via SPM 2.53.0). `AuthLocalStorage`/`AuthService`
  sobre `SharedSessionStore`; refresh serializado por `RefreshLock` (fcntl `F_SETLK`, NÃO `flock` — `flock`
  colide com o `struct flock` do Darwin) com expiração de lock órfão. Para testar no host, o lock precisa ser
  INJETÁVEL (init com `lockPath` em tmp); a versão de produção usa `containerURL(App Group)`, que no host
  retorna URL inacessível (não-nil) e faz `open(O_CREAT)` falhar com `.localFailure` — mesmo trap do M3.
- A matriz de compatibilidade precisa cobrir seleção por mouse e teclado, campos editáveis e somente leitura, múltiplos monitores e conteúdo Unicode.
- Testar separadamente detecção, leitura, bounds e escrita evita mascarar incompatibilidades específicas dos aplicativos.
- Pausar precisa bloquear todos os entrypoints: polling, AXObserver, atalho global e fallback de clipboard.
- Eventos de overlay não são enfileirados para listeners futuros; resultados de nota precisam de estado persistido e state pull após registrar o listener.
- Aprovação automatizada de código não substitui o spike manual AX/AppKit na matriz antes da distribuição.
- Mudança frontend, mesmo restrita a UX de permissão, precisa do gate E2E além de Vitest; o E2E simulado deve declarar explicitamente que não comprova o estado real do TCC.
- Smoke de botão sem backend deve provar erro visível/acionável, não IA funcional. Tradução real exige configuração pública do Supabase embutida no bundle, Edge implantada e sessão válida.
- Corridas de overlay devem ser testadas com sincronização determinística, incluindo primeira e segunda publicação da mesma ação, ACK tardio e visibilidade final; sleeps não provam a ordem.

## Dependências & Integrações
- Accessibility e AppKit exigem permissão de Acessibilidade concedida pelo usuário.
- Conteúdo selecionado só pode sair da máquina depois de ação explícita.
- Segredos nunca pertencem à WebView, ao repositório ou aos logs.
- Finder não herda variáveis do shell; configuração pública necessária ao cliente deve ser embutida no build do bundle ou carregada de fonte persistida, enquanto segredos permanecem no backend.
- `VITE_SUPABASE_URL` e `VITE_SUPABASE_ANON_KEY` são o par canônico compartilhado. O nativo aceita `VERBALIX_*` apenas como par legado completo; nunca misturar URL de uma fonte com key de outra.
- Para embutir configuração pública sem expô-la no output do build script, gerar fonte Rust em `OUT_DIR` e incluí-la no binário; não transportar valores por `cargo:rustc-env`.
- Worktrees não recebem arquivos ignorados como `.env`; o smoke pré-merge precisa provisionar o arquivo localmente sem logar valores, enquanto o checkout principal resolve `../.env` normalmente.
- Deploy de Edge Function com provider externo é bloqueado antes da publicação quando qualquer secret obrigatório está ausente; nunca publicar deliberadamente um endpoint que só responderá 500.
- Chave pública Supabase no formato JWT legado não prova sessão de usuário. Além de `verify_jwt`, confirmar o bearer no Auth endpoint e rejeitar papel/token anônimo antes de chamar o provider.
- A Edge Function `transform` foi implantada com `verify_jwt=true`; endpoint non-404 e rejeições de request sem autenticação/token anônimo foram comprovados. O smoke autenticado de IA permanece gate operacional dependente de sessão de usuário.
- A matriz de MVP é Chrome, Safari, VS Code, Slack, Notes e TextEdit.
- Para mutações pontuais da configuração Auth, usar GET → revalidação → PATCH contendo somente o campo necessário → GET, com evidências por booleanos/contagens, token do CLI consumido do Keychain sem output e rerun idempotente.
- Evidência de configuração remota precisa ser contemporânea e persistida para QA: separar relato histórico não auditável de um novo ciclo GET/no-op/GET, registrar categorias HTTP, hashes canônicos, contagens, decisão de payload e cleanup sem persistir respostas ou identificadores.

## Observações
- A promessa do produto é mensurável na matriz de aplicativos suportados e best-effort nos demais; não existe evento universal de seleção no macOS.
- O código do MVP recebeu verdict final `APPROVED`; T5.4 e T5.5 permanecem gates manuais de pré-release.
- RF42 foi aprovado: todos os sinais causais cancelam `Armed|Authorizing` antes do bump; writer-wins preserva `InSetter|Committed`. Gates: Rust 229/229, Vitest 55/55, Playwright 6/6, Deno 38/38 e limite de 300 linhas.
- Ao limpar worktrees, auditar novamente imediatamente antes da remoção: arquivos não rastreados podem surgir depois da auditoria inicial. Preservar trabalho incompleto em bundle/cópia fora do repositório antes de excluir branch ou usar `--force`.
- Os masters da marca vivem em `branding/`; ícones derivados para targets Tauri vivem em `src-tauri/icons/`.
- Existe um gate de tamanho de arquivo real e enforced: `src/bundle-smoke.test.ts` assert `lib.rs` (runtime composition root) com `split("\n").length <= 301` (~300 linhas). Mudanças em `lib.rs` que adicionem linhas quebram esse teste vitest — reduzir extraindo responsabilidade (ex.: inline de função single-use como `trigger_shortcut`, ou mover fiação), nunca comprimir artificialmente nem comentar.
- O ambiente pode limpar/resetar worktrees e avançar `main` ENTRE chamadas de ferramenta (aconteceu neste projeto: worktree recriado sumiu e `main` andou de f9fd6d1 para a7febf8). Verificar `git worktree list` ao retomar; se o worktree sumiu, recriá-lo a partir do `main` atual e revalidar que o código-alvo do plano ainda bate antes de delegar.
- Sub-agentes de implementação podem retornar mensagem final TRUNCADA (status intermediário) em tarefas longas com muitos gates. Não confiar só no texto retornado: inspecionar o worktree (`git log`, `git status`, checkboxes do plan.md) para o estado real e, se preciso, re-delegar uma continuação bounded. Os gates pesados (`test:coverage`, `e2e`, `tauri build --debug`) podem ser rodados pelo próprio orquestrador (verificação read-only) para evitar timeouts do sub-agente.
