# Plano — M2: Pipeline de interpretação frase a frase (sem microfone virtual)

> Worktree: `.worktrees/live-interpretation` · Branch: `live-interpretation` (a partir de `voice-enrollment`)
> Milestone M2 do plano aprovado. M1 (enrollment) entregue em `docs/012`.

## 🎯 SCOPE

### Objetivo
Fechar o loop ponta-a-ponta de interpretação por enunciado: o usuário fala UMA frase no microfone
físico → o app segmenta por silêncio → envia à Edge Function `interpret` (STT ElevenLabs Scribe →
tradução OpenAI com língua-alvo explícita → TTS ElevenLabs com a voz clonada do usuário) → reproduz
o áudio traduzido no alto-falante padrão. SEM microfone virtual (isso é M3).

### Arquivos Afetados (criar)
- [ ] `supabase/functions/interpret/{index,handler,contract,provider,service_client}.ts`
- [ ] `supabase/functions/interpret/{contract_test,handler_test,provider_test}.ts` (+ helpers)
- [ ] `src-tauri/src/domain/live_interpretation.rs` (estado + LanguageTag + SegmentId + staleness puros)
- [ ] `src-tauri/src/domain/live_interpretation_tests.rs`
- [ ] `src-tauri/src/domain/endpointing.rs` (VAD/endpointer puro) + `endpointing_tests.rs`
- [ ] `src-tauri/src/application/live_interpretation.rs` (`LiveInterpretationCoordinator`)
- [ ] `src-tauri/src/application/live_interpretation_tests.rs`
- [ ] `src-tauri/src/application/voice_pipeline.rs` (`RemoteVoicePipeline` — adapter da Edge Function)
- [ ] `src-tauri/src/platform/audio_playback.rs` (`MacAudioPlayback` cpal, cfg macos)
- [ ] `src-tauri/src/commands_live.rs` (comandos Tauri)
- [ ] `src/components/LivePanel.tsx` (ou seção "Ao vivo" dentro de `InterpretationPanel.tsx`)

### Arquivos Afetados (modificar)
- [ ] `supabase/config.toml` (bloco `[functions.interpret] verify_jwt = true`)
- [ ] `src-tauri/src/domain/settings.rs` (`target_language` com `#[serde(default)]`)
- [ ] `src-tauri/src/domain/error.rs` (novas variantes sanitizadas)
- [ ] `src-tauri/src/domain/mod.rs` / `application/mod.rs` (re-exports)
- [ ] `src-tauri/src/application/ports.rs` (`AudioStreamPort`/streaming, `VoicePipelinePort`, `AudioPreviewPort`)
- [ ] `src-tauri/src/application/runtime_pause.rs` (gate on-air de longa duração)
- [ ] `src-tauri/src/application/remote_preferences.rs` (preservar `target_language` no `apply_remote`)
- [ ] `src-tauri/src/platform/audio_capture.rs` (modo streaming de frames p/ VAD, sem quebrar enrollment)
- [ ] `src-tauri/src/platform/mod.rs` (re-export + stub não-macOS de playback/streaming)
- [ ] `src-tauri/src/{lib,runtime}.rs` (wiring dos novos adapters/coordinator + registro dos comandos)
- [ ] `src-tauri/src/diagnostics.rs` (buckets de latência por estágio, contagem de segmentos)
- [ ] `src/{native.ts,types.ts}` (comandos + tipos camelCase)
- [ ] `src/styles/panels.css` (estilos da seção Ao vivo)
- [ ] `e2e/` (novo spec de roteamento/sequência de comandos da seção Ao vivo)

### Fora do Escopo (rejeitar se surgir)
- Microfone virtual / driver Core Audio HAL (M3).
- Streaming incremental WebSocket, jitter buffer, contexto de tradução (M4).
- Deploy da function `interpret` e `supabase secrets` (ação de OPS / sessão principal — gate manual).
- Sync remoto de `target_language` e espelho iOS.
- Merge em `main`.
- Cancelamento de eco / mic virtual (não há mic virtual neste milestone).

### Riscos de Impacto
- R1: Extensão da captura para modo streaming NÃO pode regredir o enrollment (M1) — o caminho
  `start()/stop()/cancel()/level()` deve continuar idêntico. Streaming é um caminho SEPARADO.
- R2: `RuntimePause` é O gate único; adicionar o conceito on-air sem alterar a semântica de `is_paused`
  nem do `in_flight`/grace existentes (regressão do "abre e fecha" da toolbar — memória).
- R3: cpal `Stream` (captura e playback) NÃO é `Send` — repetir o padrão de thread dedicada + surface
  `Send`-safe do M1 para ambos os lados.
- R4: gate de tamanho `lib.rs` ≤ 301 linhas (`bundle-smoke.test.ts`) — extrair fiação para `runtime.rs`.
- R5: arquivos ≤ ~300 linhas — `handler.ts` do interpret tende a estourar (3 estágios); dividir cedo.

## 📋 REQUIREMENTS

### Requisitos Funcionais
- [ ] RF01: Edge Function `interpret` recebe áudio (base64 WAV mono 16 kHz) de UM enunciado +
      `targetLanguage` validada + `requestId`; retorna áudio TTS (base64) + metadados
      (`detectedLanguage`, `targetLanguage`, durações por estágio em ms).
- [ ] RF02: `interpret` resolve o `provider_voice_id` server-side a partir do `voice_profile` do
      usuário (JWT → service role), NUNCA aceita voice_id do cliente e NUNCA o retorna.
- [ ] RF03: STT via ElevenLabs Scribe; tradução via OpenAI (Responses API) com prompt de língua-alvo
      EXPLÍCITA; TTS via ElevenLabs para a voz clonada.
- [ ] RF04: `verify_jwt = true`; requisições sem sessão/token anônimo são rejeitadas; erros no
      formato `{"error":{"code":...}}`.
- [ ] RF05: Segmentação por enunciado: silêncio contínuo de ~700 ms–1 s fecha o enunciado; duração
      mínima (~400 ms de fala) descarta ruído; máxima (~15 s) força fechamento.
- [ ] RF06: `LiveInterpretationCoordinator` orquestra captura→VAD→pipeline→playback com `session_id`
      + `segment_id` monotônico; segmento N só reproduz depois de N-1 (ordering preservado).
- [ ] RF07: `enter_live` valida língua-alvo e presença de voz clonada, adquire guard on-air, inicia
      streaming de captura. `leave_live` para captura, cancela sessão, invalida tudo pendente.
- [ ] RF08: Áudio traduzido reproduzido no dispositivo de saída padrão via cpal.
- [ ] RF09: Setting `target_language` (default sensato) persistida em `settings.json`, NÃO sincronizada.
- [ ] RF10: UI "Ao vivo": seletor de língua-alvo, botão Entrar no ar/Sair do ar, status
      (ouvindo/processando/falando), latência do último enunciado, erro sanitizado.
- [ ] RF11: Tray "Pausar" durante on-air para a captura e cancela a sessão; "Retomar" NÃO religa o mic.
      A transição tray→`leave_live` é NÃO-BLOQUEANTE (dispatch fire-and-forget), NUNCA chamada síncrona
      da callback do menu que bloqueie a main thread esperando reply de thread worker.
- [ ] RF12: Enquanto o estado é `Speaking` (playback ativo), os frames capturados NÃO abrem novos
      enunciados no `Endpointer` (mitigação de feedback mic→alto-falante sem headphones).
- [ ] RF13: Captura em modo streaming (on-air) e captura de enrollment (M1) são MUTUAMENTE EXCLUSIVAS
      sobre o mesmo worker `MacAudioCapture`: `enter_live` falha se enrollment ativo e vice-versa.
- [ ] RF14: Revogação de permissão de microfone DURANTE a sessão (erro no callback do stream cpal,
      hoje `|_| {}`) é detectada → `leave_live` fail-closed com erro sanitizado (não silencioso).
- [ ] RF15: Circuit-breaker — após K falhas consecutivas de segmento (default K=3), a sessão surface
      "interpretação indisponível" e faz `leave_live` em vez de martelar o provedor indefinidamente.
- [ ] RF16: Auto-leave por inatividade — após ~2 min sem enunciado válido, a sessão encerra sozinha.

### Requisitos Não-Funcionais
- [ ] RNF01: Privacidade — NUNCA logar/retornar/emitir áudio, transcrição, tradução ou `provider_voice_id`.
      Diagnostics só metadados (durações em buckets, contagens, códigos de erro).
- [ ] RNF02: Fail-closed — qualquer falha (rede, STT, tradução, TTS, playback, sessão obsoleta) leva a
      estado consistente sem reproduzir áudio obsoleto; silêncio em vez de áudio errado.
- [ ] RNF03: Filas bounded — backpressure quando enunciados chegam mais rápido que o processamento
      (descartar/coalescer com política definida, nunca crescer sem limite).
- [ ] RNF04: Arquivos ≤ ~300 linhas efetivas; `lib.rs` ≤ 301 (gate vitest).
- [ ] RNF05: Sem comentários no código; IPC camelCase; segredos só no backend.
- [ ] RNF06: Adapters de plataforma atrás de `cfg(target_os="macos")` com stub compilando fora do macOS.
- [ ] RNF07: Latência p95 alvo SOFT ≤ 8 s por enunciado (aspiracional 2–5 s); NÃO é gate automático de
      teste (depende de rede/provedor), mas documentada e medida no gate manual.
- [ ] RNF08: Concorrência limitada — no máximo 2 chamadas `interpret` em voo simultâneas (evita burst
      de quota OpenAI/ElevenLabs). Dispatch concorrente para latência, playback ainda ordenado.
- [ ] RNF09: Erro de conexão (sem status HTTP) e timeout são mapeados a erro RECUPERÁVEL de segmento
      (`InterpretationFailed`/stage-específico), NUNCA encerram a sessão por si só (só o circuit-breaker).

### Critérios de Aceitação
- [ ] CA01: `deno test supabase/functions/interpret/*` verde; contract rejeita língua fora da allowlist,
      áudio ausente/grande, request malformado; handler 401 sem JWT e quando o usuário não tem voz.
- [ ] CA02: `cargo test` cobre coordinator com fake ports: cancelamento, ordering N→N+1, sessão trocada
      invalida pendentes, fail-closed, backpressure de fila bounded, VAD (endpointer) por energia/silêncio.
- [ ] CA03: `RuntimePause` on-air suprime polling/AXObserver/shortcut/clipboard sem regredir os testes
      existentes de pause/in-flight.
- [ ] CA04: Diagnostics — teste garante ausência de conteúdo (áudio/transcrição/tradução/voice_id).
- [ ] CA05: Frontend — Vitest do LivePanel (estados, seletor, botão, latência, erro) + e2e de sequência.
- [ ] CA06: Todos os gates do repo verdes (lista em Verificação).

### Edge Cases
- EC01: Usuário sem voz clonada (`voice_profile_id == None`) → `enter_live` falha claro antes de capturar.
- EC02: `target_language` == língua detectada do enunciado → ainda transcreve+sintetiza (ou política de
      no-op definida); nunca crash.
- EC03: Enunciado só com silêncio/ruído (< duração mínima) → descartado, sem chamada de rede.
- EC04: Resposta chega depois de `leave_live` ou de troca de sessão → descartada (staleness).
- EC05: Segmento N-1 falha mas N sucede → política de ordering (N não fura a fila; falha de N-1 libera N
      ou aborta a cadeia — definir determinístico e testar).
- EC06: Device de saída ausente/troca durante on-air → falha recuperável, sem panic.
- EC07: Edge Function timeout (> 20 s) → `ProviderTimeout` mapeado, sessão continua no ar para o próximo.
- EC08: Múltiplos enunciados em rápida sucessão → fila bounded aplica backpressure.
- EC09: Permissão de mic revogada / não concedida → `enter_live` falha claro (reusa MicrophonePermission).
- EC10: Frames capturados durante `Speaking` (playback do TTS) NÃO abrem novo enunciado (feedback loop).
- EC11: `voice_profile` deletado (fluxo M1) no meio da sessão → segmento falha com `NO_VOICE_PROFILE`;
      circuit-breaker (K falhas) encerra a sessão em vez de falhar todos os segmentos silenciosamente.
- EC12: Queda total de rede no meio do enunciado → erro de conexão recuperável, sessão continua p/ o próximo.
- EC13: `target_language` alterada no painel DURANTE on-air → a sessão usa o valor capturado no
      `enter_live` (snapshot); a mudança só vale no próximo `enter_live`.
- EC14: Segmento descartado por backpressure de fila bounded → UI mostra indicador "descartado".

## 🏗️ DESIGN

### Padrões reutilizados do M1/projeto
- Split Edge Function `index/handler/contract/provider/service_client` idêntico a `voice-enroll`;
  reuso do `SupabaseUserAuthenticator` (auth.ts do transform) e do `service_client.ts` (resolução por
  service role escopada por `user_id` do JWT). `verify_jwt` em `config.toml`.
- Adapter Rust `RemoteVoicePipeline` espelha `RemoteVoiceEnrollment` (reqwest, bearer + apikey,
  `Box::pin` async, timeout > 20 s, erros mapeados sem conteúdo).
- Captura cpal com thread dedicada + surface `Send`-safe (canal `mpsc` + atômico) — igual `audio_capture.rs`.
- Playback cpal com a MESMA disciplina (thread dedicada dona do `Stream`, comandos por canal).
- Setting novo não sincronizado: `#[serde(default)]` + struct-literal em `apply_remote` força preservação
  em compile-time (padrão `voice_profile_id`).

### Edge Function `interpret`
- `contract.ts`: `InterpretRequest { requestId, targetLanguage, audioBase64, mimeType:"audio/wav" }`;
  allowlist BCP-47 (ex.: `en`, `pt`, `es`, `fr`, `de`, `it`, `ja`… — subset multilíngue suportado pelo
  TTS); `MAX_AUDIO_BYTES` próprio (cap ~2 MB → ~15 s de WAV 16 kHz mono; lembrar overhead base64 33%);
  `ErrorCode` (`UNAUTHENTICATED|INVALID_REQUEST|NO_VOICE_PROFILE|LANGUAGE_UNSUPPORTED|AUDIO_TOO_LARGE|
  STT_FAILED|TRANSLATION_FAILED|TTS_FAILED|PROVIDER_TIMEOUT|INTERNAL_ERROR`).
- `provider.ts`: três funções puras testáveis — `transcribe` (ElevenLabs `/v1/speech-to-text`,
  modelo Scribe, retorna texto + língua detectada), `translate` (OpenAI `/v1/responses`, prompt com
  língua-alvo EXPLÍCITA — NÃO reusar o prompt PT↔EN do transform), `synthesize` (ElevenLabs
  `/v1/text-to-speech/{voice_id}`, retorna áudio). `fetcher` injetável p/ testes.
- `service_client.ts`: resolve `provider_voice_id` do `voice_profile` ativo do usuário (service role,
  por `user_id` do JWT); rejeita se ausente/não `ready` → `NO_VOICE_PROFILE`.
- `handler.ts` + `stages.ts`: orquestração fica em `handler.ts`; a lógica dos 3 estágios encadeados
  (STT→tradução→TTS, medição de durações) vai para `stages.ts` para manter AMBOS ≤ 300 linhas (o
  handler do voice-enroll estourou 300 no M1 e teve que extrair `service_client.ts` — mesma lição).
- **Timeout re-derivado (NÃO copiar os 20 s do transform)**: 3 round-trips encadeados exigem orçamento
  maior. Abort interno da Edge Function ~45 s (soma STT+tradução+TTS de um clipe ≤ 15 s); cliente Rust
  usa timeout ~50 s (levemente acima do edge). EC07 revisado: > 20 s é esperado, não é erro.
- **Erro stage-específico preservado**: manter `ErrorCode` distinto (`STT_FAILED|TRANSLATION_FAILED|
  TTS_FAILED`) até uma variante Rust sanitizada — stage-of-failure NÃO é conteúdo sensível e é o único
  sinal de debug disponível (diagnostics não loga texto). NÃO colapsar tudo em um genérico.
- `index.ts`: JWT required + confirma bearer no Auth endpoint (rejeita anônimo, padrão do transform).
- Resposta: `{ requestId, detectedLanguage, targetLanguage, audioBase64, mimeType, stageMs:{stt,translate,tts} }`.

### Domain `live_interpretation.rs` (puro)
- `LiveSessionId(Uuid)`; `SegmentId(u64)` monotônico por sessão.
- `LanguageTag` validada por allowlist (mesma do contract; fonte única conceitual, replicada no Rust).
- `LiveState`: `Idle → Preparing → OnAir → {Recovering|Stopping} → {Idle|Failed}`; transições puras.
- Regras de staleness puras: `accepts(session_id, segment_id)` — resposta só é válida se a sessão é a
  corrente e o segmento ainda não foi superado/descartado; sessão parada/trocada invalida tudo.
- `endpointing.rs`: `Endpointer` puro — recebe energia/RMS por frame + timestamps, emite
  `SegmentBoundary` quando silêncio ≥ limiar após fala ≥ mínimo; respeita duração máxima. Sem IO.

### Application `LiveInterpretationCoordinator` — DECOMPOSIÇÃO EXPLÍCITA (obrigatória)
Para não estourar o orçamento de ~300 linhas nem concentrar 5 responsabilidades num arquivo, T3.3 é
dividido em sub-módulos ANTES da implementação:
- `application/live_interpretation.rs` — API pública do coordinator + máquina de estado + fiação;
  detém o `Endpointer` e os handles dos sub-módulos.
- `application/live_queue.rs` — **reorder buffer** bounded + política de backpressure, PURO/testável
  (sem IO). Recebe `(segment_id, outcome)` possivelmente fora de ordem e emite em ordem N-1→N para
  playback. Capacidade fixa (default 8 segmentos); política: descartar o MAIS ANTIGO ainda-não-enviado
  quando cheio (preserva ordem dos já em voo), sinaliza `dropped(segment_id)` p/ diagnostics+UI (EC14).
- `application/live_worker.rs` — loop async de dispatch: consome enunciados fechados, aplica o cap de
  concorrência (RNF08, máx 2 em voo), chama `VoicePipelinePort`, injeta resultados no `live_queue`.
- Cada sub-módulo com seu `_tests.rs`.

**Modelo de dispatch/ordering (EC05 resolvido no papel):** dispatch é CONCORRENTE (segmento N+1 começa
o STT sem esperar o TTS de N — reduz latência), mas o PLAYBACK é estritamente ordenado via `live_queue`:
o reorder buffer só libera N depois que N-1 foi liberado (tocado, descartado ou falho). Se N chega antes
de N-1, N espera no buffer. Se N-1 FALHA, ele é marcado resolvido e libera N (a cadeia não trava numa
falha). Tudo determinístico e testável sem rede.

- Ports novos em `ports.rs`:
  - `AudioStreamPort` (SEPARADO do `AudioCapturePort` de enrollment): `start_stream(sink) / stop_stream()`
    entrega chunks de frames (f32 + sample_rate + channels). Compartilha o worker `MacAudioCapture` via
    extensão do enum `CaptureCommand`; enrollment e streaming são MUTUAMENTE EXCLUSIVOS (RF13) — o worker
    rejeita `StartStream` se um enrollment está ativo e vice-versa. API de enrollment (`start/stop/cancel/
    level/permission_status`) permanece IDÊNTICA (sem regressão M1).
  - `VoicePipelinePort::interpret(session_id, segment_id, wav_bytes, target_language, token) -> Future<InterpretOutcome>`.
  - `AudioPreviewPort::play(pcm/bytes) -> Result<()>` (com reply-timeout, ver playback) + `stop()`; sinaliza
    fim de reprodução p/ o worker liberar o próximo.
- Fluxo (worker em thread/task dedicada, disparado por `enter_live`):
  1. valida permissão de mic + `target_language` (snapshot, EC13) + `voice_profile_id`; rejeita se
     enrollment ativo (RF13); adquire guard on-air; inicia streaming.
  2. frames → `Endpointer` (SUPRIMIDO enquanto `Speaking`, RF12/EC10); ao fechar enunciado, encoda WAV
     (reusa `audio_wav.rs`, T2.4), atribui `segment_id`, envia ao `live_worker`.
  3. `InterpretOutcome` → valida `accepts(session,segment)` → `live_queue` → playback ordenado.
  4. `leave_live`/troca/pause/circuit-breaker/idle → invalida sessão, para captura+playback, drena fila.
- Detecção de revogação de permissão (RF14): o callback de erro do stream cpal (hoje `|_| {}`) passa a
  sinalizar falha ao worker → `leave_live` fail-closed.
- Circuit-breaker (RF15): contador de falhas consecutivas de segmento; K=3 → `leave_live` + estado `error`.
- Auto-leave idle (RF16): sem enunciado válido por ~2 min → `leave_live`.
- Emissão de estado por evento Tauri (`live-state`: listening|processing|speaking|error + lastLatencyMs
  + lastDropped); áudio NUNCA cruza o React.

### `RuntimePause` on-air — GATE SEPARADO (não reutilizar ActionGuard/grace)
- **DECISÃO**: `on_air` é um `AtomicBool` TERCEIRO e INDEPENDENTE, NUNCA roteado por `begin_action()`/
  `ActionGuard`/`grace_deadline` (esses são afinados para ações sub-segundo; segurar por uma sessão de
  minutos suprimiria a toolbar/nota o tempo todo — regressão silenciosa do M1). Novo `OnAirGuard` RAII
  irmão do `ActionGuard` (mesmo idioma `Drop`+`Ordering::Release`), set true em `enter_live`, false no Drop.
- `is_on_air()` compõe via `&& !self.is_on_air()` nos 5 entrypoints (`run_polling`, `run_ax_observer`,
  `run_mouse_dismiss`, `run_global_shortcut`, `run_clipboard_fallback`). NÃO tocar em `is_paused`/`in_flight`.
- Teste dedicado: supressão dura a SESSÃO INTEIRA (não só 400 ms) e recupera IMEDIATAMENTE no `leave_live`,
  desacoplada do clock de grace do `in_flight`. Testes existentes de pause/in-flight/grace continuam verdes.
- Tray "Pausar" (toggle de `is_paused`) durante on-air → dispatch NÃO-BLOQUEANTE (spawn) de `leave_live`;
  a callback do menu NUNCA bloqueia a main thread esperando reply de worker (RF11). "Retomar" só limpa
  `is_paused`; NÃO chama `enter_live`.
- Nota de dívida técnica (aceita): `RuntimePause` passa a ter 3 gates (`paused`/`in_flight`/`on_air`).
  Como todos gateiam os MESMOS entrypoints de detecção, mantê-los aqui é coerente com o propósito de
  "gate único". Se M3/M4 adicionarem um 4º, avaliar extrair um `LiveGate` composto — fora deste milestone.

### Settings
- `target_language: String` (ou `Option<LanguageTag>`), default `"en"`, `#[serde(default)]`,
  validado em `AppSettings::validate` contra a allowlist; preservado em `remote_preferences::apply_remote`.
- Espelho `src/types.ts` (`targetLanguage`) + controle no LivePanel.

### Playback `MacAudioPlayback` (platform, cfg macos)
- Thread dedicada dona do `cpal::Stream` de saída; comandos `Play(pcm, reply)` / `Stop` por canal.
- Decodifica WAV base64 → PCM → fila interna do stream; sinaliza fim (p/ o coordinator liberar N+1).
- **Reply-timeout** no canal de `Play` (mirror de `MacAudioCapture::start/stop`), para device
  desconectado/troca não travar o worker esperando reply para sempre (EC06).
- Stub não-macOS em `platform/mod.rs` retornando `UnsupportedPlatform`.

### Diagnostics
- Novos eventos sanitizados: `segment_started`, `segment_completed{stt_ms,translate_ms,tts_ms,total_ms}`
  em buckets, `segment_dropped{reason}`, contagem por sessão. NUNCA texto/áudio/voice_id. Teste enforce.

### Erros novos (`VerbalixError`, sanitizados)
- `LiveSessionInactive`, `TargetLanguageUnsupported`, `VoiceProfileMissing`, `AudioPlaybackFailed`,
  `InterpretationFailed` (genérico p/ falhas de pipeline). Mensagens sem conteúdo.

## 📝 TASKS

> **Pré-requisito duro:** T2.4 (`audio_wav.rs`) DEVE ser concluída ANTES de T3.2/T4.1/T4.2 para não
> criar duas implementações paralelas de `encode_wav`/`resample_to_16k` (dívida técnica evitável).

### Fase 1: Backend Edge Function
- [x] T1.1: [MEDIUM] `interpret/contract.ts` — request/response, allowlist BCP-47, caps próprios, ErrorCodes stage-específicos + `contract_test.ts`.
- [x] T1.2: [MEDIUM] `interpret/provider.ts` — `transcribe`/`translate`/`synthesize` (fetcher injetável) + `provider_test.ts`; prompt de tradução com língua-alvo EXPLÍCITA (não reusar prompt PT↔EN). Confirmar endpoints ElevenLabs: STT `/v1/speech-to-text` (Scribe), TTS `/v1/text-to-speech/{voice_id}`.
- [x] T1.3: [LOW/copy-adapt de `voice-enroll/service_client.ts`] `interpret/service_client.ts` — resolve `provider_voice_id` do perfil `ready` por JWT; rejeita ausente → `NO_VOICE_PROFILE`.
- [x] T1.4: [MEDIUM] `interpret/handler.ts` + `interpret/stages.ts` + `index.ts` — orquestração 3 estágios (stages.ts p/ ≤300 linhas), durações, timeout ~45s, JWT/anon reject + `handler_test.ts`.
- [x] T1.5: [LOW] `supabase/config.toml` — `[functions.interpret] verify_jwt = true`.

### Fase 2: Domain puro
- [x] T2.4: [LOW — FAZER PRIMEIRO] Extrair `encode_wav`/`resample_to_16k` p/ `audio_wav.rs` compartilhável (sem regredir enrollment).
- [x] T2.1: [MEDIUM] `domain/live_interpretation.rs` — LiveSessionId/SegmentId/LanguageTag/LiveState + `accepts()` staleness + `_tests.rs`.
- [x] T2.2: [MEDIUM] `domain/endpointing.rs` — `Endpointer` (VAD por energia/silêncio, min/max, supressão em Speaking) + `_tests.rs`.
- [x] T2.3: [LOW] `domain/error.rs` novas variantes stage-específicas + `domain/settings.rs` `target_language` (`#[serde(default)]`, validação allowlist) + preservação em `apply_remote` + testes.

### Fase 3: Application + Ports
- [x] T3.1: [MEDIUM] `ports.rs` — `AudioStreamPort` (separado, mutex c/ enrollment), `VoicePipelinePort`, `AudioPreviewPort`.
- [x] T3.2: [LOW/copy-adapt de `voice_enrollment.rs`] `application/voice_pipeline.rs` — `RemoteVoicePipeline` (reusa `map_status_error`, timeout ~50s, erro de conexão → recuperável).
- [x] T3.3a: [MEDIUM] `application/live_queue.rs` — reorder buffer bounded + backpressure (puro) + `_tests.rs`.
- [x] T3.3b: [MEDIUM] `application/live_worker.rs` — loop de dispatch concorrente (cap 2 em voo) + `_tests.rs` com fake ports.
- [x] T3.3c: [MEDIUM] `application/live_interpretation.rs` — coordinator (estado, staleness, fail-closed, circuit-breaker, idle, permissão revogada) + `_tests.rs` com fake ports.
- [x] T3.4: [LOW/copy-adapt de `ActionGuard`] `runtime_pause.rs` — `on_air: AtomicBool` SEPARADO + `OnAirGuard` compondo nos 5 entrypoints + teste de supressão por sessão inteira (sem regressão).

### Fase 4: Plataforma (macOS + stub)
- [x] T4.1: [HARD] `platform/audio_capture.rs` — modo streaming de frames p/ VAD via extensão do `CaptureCommand` (mutex c/ enrollment, sem regredir M1).
- [x] T4.2: [MEDIUM] `platform/audio_playback.rs` — `MacAudioPlayback` cpal (thread dedicada, reply-timeout) + stub não-macOS em `mod.rs`.

### Fase 5: Wiring + Commands
- [x] T5.1: [MEDIUM] `commands_live.rs` — `enter_live`, `leave_live`, `live_status`, `set_target_language` (ou via save_settings) + registro.
- [x] T5.2: [MEDIUM] `runtime.rs`/`lib.rs` — construir adapters, agregar no `AppRuntime`, registrar comandos, wire tray-pause→leave_live NÃO-BLOQUEANTE. Verificar headroom de `lib.rs` (hoje 295/301) ANTES; se estourar, mover fiação p/ `runtime.rs` (89 linhas, tem folga).
- [x] T5.3: [LOW] `diagnostics.rs` — eventos de latência por estágio/contagem + teste de sanitização.

### Fase 6: Frontend
- [x] T6.1: [MEDIUM] `src/types.ts` + `src/native.ts` — tipos camelCase + wrappers dos comandos e listener de `live-state`.
- [x] T6.2: [MEDIUM] `LivePanel` (seção "Ao vivo") — seletor de língua-alvo, Entrar/Sair do ar, status, latência, erro + `styles/panels.css`.
- [x] T6.3: [LOW] Vitest do LivePanel + e2e de sequência de comandos (Tauri stubbed).

### Fase 7: Gates + entrega
- [x] T7.1: Rodar toda a suíte de gates (ver Verificação) DENTRO do worktree; corrigir até verde.
- [x] T7.2: QA (dual analysis) → verdict.
- [x] T7.3: `docs/013-*.md` (português) com escopo, solução, testes, qualidade, gates manuais pendentes.

## Verificação (gates antes do handoff, DENTRO do worktree)
`npm test` · `npm run test:coverage` · `npm run test:e2e` · `npm run build` · `cargo test` ·
`cargo clippy --all-targets --all-features -- -D warnings` · `cargo fmt --check` ·
`deno test` (interpret novo + functions existentes) · `npm run tauri -- build --debug --bundles app`.

### Gates manuais (NÃO alegar como verificados)
- Fala real ao mic → ouvir tradução com voz clonada no alto-falante em 2–5 s; medir p50/p95.
- `supabase secrets` + deploy da function `interpret` (OPS — NÃO fazer aqui).
- TCC/permissão de microfone real em bundle assinado.
- Auditoria `VERBALIX_DIAGNOSTICS=1` confirmando ausência de conteúdo/voice_id nos logs.

## Análise Dual

### 🔴 Riscos críticos incorporados (upsidedown)
1. **RuntimePause on-air NÃO pode reusar ActionGuard/grace** — grace é sub-segundo; segurar por uma
   sessão suprimiria toolbar/nota o tempo todo (regressão M1 silenciosa). → `on_air` é flag SEPARADO
   (DESIGN §RuntimePause on-air; T3.4).
2. **Tray-pause→leave_live deve ser NÃO-BLOQUEANTE** — chamada síncrona da callback do menu bloqueando
   reply de worker (padrão `recv_timeout(70s)`) congela a UI. → dispatch fire-and-forget (RF11).
3. **Feedback loop mic→alto-falante** — TTS tocando é recapturado; VAD dispara segmentos espúrios. →
   suprimir `Endpointer` enquanto `Speaking` (RF12/EC10); recomendar headphones (gate manual).
4. **T3.3 subespecificado** — decomposto em `live_queue.rs`/`live_worker.rs`/`live_interpretation.rs`
   (T3.3a/b/c), cada um testável isolado; modelo de dispatch concorrente + reorder ordenado resolvido
   no papel (DESIGN, EC05).
5. **Timeout de 20 s do transform é curto p/ 3 estágios** — re-derivado: edge ~45 s, cliente ~50 s.
6. **Captura streaming vs enrollment sobre um worker** — mutuamente exclusivas via `CaptureCommand`
   estendido (RF13); T4.1 re-sizeada HARD (reescreve modelo de concorrência de componente shippado).
7. **Falhas não cobertas** — permissão revogada mid-sessão (RF14), voz deletada mid-sessão + circuit-
   breaker (RF15/EC11), queda de rede sem status HTTP → recuperável (RNF09/EC12), device de saída
   desconecta → reply-timeout no playback (EC06).
8. **Erro genérico é buraco de debug** — preservar `STT_FAILED|TRANSLATION_FAILED|TTS_FAILED` até o Rust.
9. **Segmento descartado invisível** — indicador de UI + diagnostics (EC14/RF10).
10. **Sem NFR de latência** — p95 soft ≤ 8 s documentada (RNF07), medida no gate manual.

### 🟢 Oportunidades de reuso incorporadas (downsideup)
- **Copy-adapt de arquivos nomeados** (effort reduzido p/ LOW): T1.3 ← `voice-enroll/service_client.ts`;
  T3.2 ← `voice_enrollment.rs` (`RemoteVoiceEnrollment` + `map_status_error`); T3.4 `OnAirGuard` ←
  `ActionGuard`. Delegar com instrução "comece a partir deste arquivo".
- **`encode_wav`/`resample_to_16k`** já puros → extração mecânica p/ `audio_wav.rs` (T2.4 FEITA PRIMEIRO).
- **Padrão thread-dedicada cpal** (`MacAudioCapture`) é o molde espelhado de `MacAudioPlayback` (T4.2).
- **Struct-literal em `apply_remote`** força preservar `target_language` em compile-time (T2.3).
- **Harness de fake ports** dos `coordinator_tests.rs` reusado nos testes de T3.3a/b/c.
- **`SupabaseUserAuthenticator`** (transform/auth.ts) reusado no `interpret/index.ts`.
- **Diagnostics sanitizado-por-construção** + técnica de teste "grep-for-absence" estendida p/ áudio/voice_id.

### Paralelização (4 grupos independentes, após T2.4)
1. Edge Function `interpret/*` (Fase 1) — zero dep de Rust.
2. Domain puro (T2.1/T2.2/T2.3) — zero IO.
3. Plataforma áudio (T4.1/T4.2) — depende só de `audio_wav.rs` (T2.4).
4. `RemoteVoicePipeline` (T3.2) — só precisa da forma do contrato (T1.1).
O coordinator (T3.3a/b/c) integra e pode ser desenvolvido test-first com fake ports antes dos adapters reais.
