# [013] - M2: Pipeline de interpretação frase a frase (sem microfone virtual)

## Contexto
Segundo marco (M2) da feature de interpretação ao vivo do Verbalix. Fecha o loop
ponta-a-ponta por enunciado: o usuário fala UMA frase no microfone físico, o app segmenta
por silêncio, envia à Edge Function `interpret` (STT ElevenLabs Scribe → tradução OpenAI com
língua-alvo explícita → TTS ElevenLabs com a voz clonada do usuário do M1) e reproduz o áudio
traduzido no alto-falante padrão. SEM microfone virtual (isso é o M3). Constrói sobre o M1
(enrollment de voz, `docs/012`) reusando seus padrões: split de Edge Function, adapter remoto,
thread dedicada cpal, setting não sincronizada e composição do `RuntimePause`.

## Escopo

### Incluído
- Edge Function Deno `interpret` (split `index/handler/stages/contract/provider/service_client`),
  `verify_jwt = true`, secret `ELEVEN_LABS_KEY` + `SUPABASE_SERVICE_ROLE_KEY` só via `Deno.env`.
- Rust: `domain/live_interpretation.rs` (LiveSessionId/SegmentId/LanguageTag/LiveState + `accepts()`
  puro), `domain/endpointing.rs` (VAD por energia/silêncio), `LiveInterpretationCoordinator` +
  `live_queue.rs` (reorder buffer bounded) + `live_worker.rs` (dispatch concorrente), `RemoteVoicePipeline`,
  ports `AudioStreamPort`/`VoicePipelinePort`/`AudioPreviewPort`, adapters cpal de captura streaming e
  playback (cfg macos + stub), extensão on-air do `RuntimePause`, `audio_wav.rs` compartilhável,
  comandos `enter_live`/`leave_live`/`live_status`/`set_target_language`, setting `target_language`.
- Frontend: seção "Ao vivo" no painel Interpretação (seletor de língua-alvo, Entrar/Sair do ar,
  status ouvindo/processando/falando, latência do último enunciado, segmento descartado, erro sanitizado).
- Diagnostics de latência por estágio/contagem, sanitizados.

### Excluído
- Microfone virtual / driver Core Audio HAL (M3).
- Streaming incremental WebSocket, jitter buffer, contexto de tradução (M4).
- Deploy da Edge Function `interpret` e `supabase secrets set` (ação de OPS — gate manual).
- Sync remoto de `target_language` e espelho iOS.

## Solução Implementada

### Arquitetura
Hexagonal, seguindo os padrões do M1:

- **Segredo/voz server-only**: `interpret/service_client.ts` resolve o `provider_voice_id` do perfil
  `ready` do usuário via service role escopada pelo `user_id` do JWT; o cliente nunca envia nem recebe
  o `voice_id`. Só o UUID opaco do perfil e metadados trafegam.
- **Pipeline de 3 estágios explícitos**: `stages.ts` encapsula `transcribe` (ElevenLabs Scribe) →
  `translate` (OpenAI Responses, prompt com língua-alvo EXPLÍCITA e texto delimitado em
  `<untrusted_text>` com invariante de sistema contra prompt-injection — NÃO o prompt PT↔EN do transform)
  → `synthesize` (ElevenLabs TTS). Timeout re-derivado para o encadeamento: abort de 45 s na Edge
  Function, timeout de 55 s no cliente Rust. `ErrorCode` stage-específico (`STT_FAILED`/`TRANSLATION_FAILED`/
  `TTS_FAILED`) preservado até variantes sanitizadas no Rust — único sinal de debug, já que conteúdo nunca
  é logado.
- **Coordinator + reorder ordenado**: dispatch concorrente (segmento N+1 começa o STT sem esperar o TTS
  de N), com playback estritamente ordenado via `live_queue` (N só toca depois de N-1; falha de N-1 libera
  N sem travar a cadeia). `accepts(session_id, segment_id)` (função pura) invalida respostas de sessão
  parada/trocada (fail-closed). Fila bounded com backpressure (descarta o mais antigo ainda-não-enviado).
  Circuit-breaker encerra a sessão após K falhas consecutivas; auto-leave por inatividade.
- **Captura streaming**: modo separado sobre o worker `MacAudioCapture` do M1 via extensão do
  `CaptureCommand`; streaming e enrollment são mutuamente exclusivos. VAD suprimido enquanto `Speaking`
  (mitiga o feedback loop mic→alto-falante sem headphones).
- **Playback**: `MacAudioPlayback` cpal com thread dedicada dona do `Stream` (não-`Send`) e reply-timeout
  no comando de `Play` (device desconectado não trava o worker). Stub não-macOS compilando.
- **`RuntimePause` on-air**: `on_air` é um `AtomicBool` TERCEIRO e INDEPENDENTE, com `OnAirGuard` RAII
  próprio (nunca roteado pelo `ActionGuard`/grace, afinado para ações sub-segundo). Compõe via
  `!is_on_air()` nos 5 entrypoints (polling, AXObserver, mouse-dismiss, shortcut, clipboard). Tray "Pausar"
  durante on-air dispara `leave_live` NÃO-BLOQUEANTE; "Retomar" não religa o mic.
- **Settings**: `target_language` com `#[serde(default)]`, validada por allowlist, preservada em
  `apply_remote` via struct-literal (compile-time), NÃO sincronizada. A sessão captura a língua-alvo no
  `enter_live` (snapshot); mudanças só valem no próximo `enter_live`.
- **Privacidade**: nenhum áudio, transcrição, tradução ou `provider_voice_id` em `diagnostics.rs`, erros,
  eventos `live-state` ou logs. O evento emite só status + `stageMs` (durações).

### Arquivos Modificados
| Arquivo | Tipo |
|---------|------|
| `supabase/functions/interpret/{index,handler,stages,contract,provider,service_client}.ts` | Criado |
| `supabase/functions/interpret/{contract,handler,provider}_test.ts` | Criado |
| `supabase/config.toml` | Modificado (`[functions.interpret] verify_jwt = true`) |
| `src-tauri/src/domain/{live_interpretation,endpointing}.rs` (+ `_tests.rs`) | Criado |
| `src-tauri/src/domain/{error,settings,mod}.rs` | Modificado |
| `src-tauri/src/application/{live_interpretation,live_queue,live_worker,voice_pipeline}.rs` (+ `_tests.rs`) | Criado |
| `src-tauri/src/application/{ports,runtime_pause,mod}.rs` | Modificado |
| `src-tauri/src/platform/{audio_wav,audio_playback}.rs` | Criado |
| `src-tauri/src/platform/{audio_capture,mod}.rs` | Modificado |
| `src-tauri/src/commands_live.rs` | Criado |
| `src-tauri/src/{lib,runtime,diagnostics}.rs` | Modificado |
| `src/components/LivePanel.tsx` (+ `LivePanel.test.tsx`) | Criado |
| `src/{native,types}.ts` (+ `native.test.ts`) | Modificado |
| `src/components/InterpretationPanel.tsx` (+ test) | Modificado |
| `e2e/live-interpretation.e2e.ts` | Criado |

## Testes
| Métrica | Valor |
|---------|-------|
| Deno (interpret + functions existentes) | 112 |
| Rust (`cargo test`) | 296 |
| Vitest (frontend) | 92 |
| Cobertura (native.ts + types.ts, threshold enforced) | 100% |
| Playwright e2e | 10 |

## Verificação de Qualidade
| Critério | Status |
|----------|--------|
| `npm test` / `npm run test:coverage` | OK / 100% |
| `npm run test:e2e` | OK (10) |
| `npm run build` | OK |
| `cargo test` | OK (296) |
| `cargo clippy --all-targets --all-features -- -D warnings` | Limpo |
| `cargo fmt --check` | Limpo |
| `deno test supabase/functions/` | OK (112, sem regressão) |
| `tauri build --debug --bundles app` | OK (assinado ad-hoc) |
| Gate de tamanho `lib.rs` (≤301) | 266 |
| QA (conformidade + análise dual) | APPROVED (após 1 ciclo REJECTED_CODE) |

### Histórico de QA
A auditoria de conformidade (além dos gates verdes) confirmou em código: privacidade (sem conteúdo/voice_id
em diagnostics/erros/eventos), `on_air` como gate separado composto nos 5 entrypoints, `accepts()`/reorder
ordenado/circuit-breaker/idle/fila bounded no coordinator, `verify_jwt=true` e prompt de língua-alvo explícita.
Verdict inicial `REJECTED_CODE` com 1 bloqueador corrigido:
1. `interpret/translate` interpolava o texto transcrito no prompt sem guard de untrusted-text — endurecido
   com mensagem de sistema (invariante) + delimitação `<untrusted_text>`, espelhando o `transform`.
Observação aceita sem bloqueio: `application/live_interpretation.rs` com 308 linhas efetivas (levemente acima
de ~300); a extração foi avaliada e recusada por não haver limite coeso (o `enter_live` captura múltiplos
`Arc` entre closures) — dividir pioraria a navegação. `audio_capture.rs` (299 efetivas) está dentro.

## Gates Manuais Pendentes (NÃO verificados por testes automatizados)
1. Fala real ao microfone → ouvir a tradução com a voz clonada no alto-falante em 2–5 s (aspiracional;
   p95 realista até ~8 s por causa dos 3 round-trips encadeados). Medir p50/p95.
2. `supabase secrets set ELEVEN_LABS_KEY=...` + deploy da Edge Function `interpret` (ação de OPS).
3. Permissão real de microfone / TCC (bundle assinado + concessão do usuário).
4. Recomendação de headphones: sem microfone virtual e sem cancelamento de eco, o alto-falante pode
   realimentar o microfone; a supressão de VAD em `Speaking` mitiga, mas headphones são recomendados.
5. Auditoria `VERBALIX_DIAGNOSTICS=1` confirmando ausência de áudio, transcrição, tradução ou `voice_id`.

---
**Verificado por:** Workflow Orchestrator (gates re-executados empiricamente a cada handoff)
**Data:** 2026-08-18
**Branch/Worktree:** `live-interpretation` / `.worktrees/live-interpretation` (NÃO mergeado)
**Status Final:** APROVADO — pendente de gates manuais, deploy da function e aprovação do usuário para merge
