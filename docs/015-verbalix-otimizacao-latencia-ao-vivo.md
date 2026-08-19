# [015] - M4: Otimização de latência da interpretação ao vivo (streaming)

## Contexto
Quarto marco (M4) da interpretação ao vivo do Verbalix. O pipeline do M2/M3 era frase a frase e
**buffered**: o enunciado completo era enviado à Edge Function `interpret` (STT ElevenLabs Scribe →
tradução OpenAI → TTS ElevenLabs), que só respondia depois de receber o corpo INTEIRO do TTS (MP3 em
base64, envelope JSON); só então o Rust decodificava e tocava no alto-falante ou no microfone virtual.
Isso adicionava a latência de gerar todo o áudio antes de qualquer som sair. M4 torna o TTS **streaming**
de ponta a ponta, adiciona **contexto curto de tradução**, **métricas p50/p95 por estágio** e **tuning de
endpointing**, reduzindo a latência percebida "fala → primeiro áudio".

## Escopo

### Incluído
- **TTS streaming na Edge `interpret`**: resposta com corpo streaming (passthrough do PCM da ElevenLabs)
  atrás de flag `stream` no request; metadados não-conteúdo via headers `X-Verbalix-*`; watchdog de
  inatividade dedicado ao corpo. Modo JSON base64 legado preservado byte-idêntico ao M2 (rollback).
- **Playback progressivo no Rust**: consumo do corpo em chunks PCM, início do playback do segmento assim
  que o pré-buffer enche, com lock explícito de ordenação N-1→N e fail-closed.
- **Contexto de tradução** (source-only, janela curta) para coerência de pronomes/terminologia.
- **Métricas** p50/p95 por estágio + latência "fala→primeiro áudio" exibida no painel.
- **Tuning de endpointing** com histerese e constantes nomeadas testáveis.

### Excluído (trilha futura documentada)
- **Relay WebSocket dedicado** e **STT incremental/streaming de transcrição** (Scribe consome o arquivo
  completo — não é incremental neste modo).
- **Overlap captura→upload em chunked transfer-encoding** — avaliado e REJEITADO nesta rodada (Decisão D3):
  o STT precisa do corpo fechado, então o ganho seria só o tempo de upload do áudio final (dezenas de ms)
  contra alto custo/risco; só faz sentido junto de STT incremental.
- Deploy da Edge `interpret` e `supabase secrets set` (ação de OPS — gate manual).
- Merge em `main`.

## Solução Implementada

### Decisões de design
- **Formato de áudio = PCM streaming (não MP3)**: no modo `stream`, a Edge pede à ElevenLabs
  `output_format=pcm_24000` (PCM 16-bit LE mono 24 kHz, sem container). O Rust converte i16 LE → f32 e faz
  **resample incremental com estado de fase** (24 kHz → taxa do device ou 48 kHz para o mic virtual,
  reusando `resample_f32`), evitando um decodificador de MP3 e o parsing de fronteira de frame. O modo JSON
  legado mantém `mp3_44100_128`. Isso ainda resolve uma dívida antiga: o caminho MP3 nunca foi validado
  automaticamente (era gate manual).
- **Streaming na Edge = STT+translate buffered, TTS passthrough**: STT e tradução permanecem `await`
  (seus erros DEVEM materializar `{"error":{"code":...}}` ANTES de o corpo fluir); só a síntese vira
  passthrough. O fetch do TTS checa `response.ok` ANTES de pipar (TTS não-ok ainda vira JSON de erro).
- **Frame de metadados no CORPO (não em header) para conteúdo**: o corpo streaming começa com
  `[4B magic "VLBX" = 0x56,0x4C,0x42,0x58][4B u32 BIG-ENDIAN: tamanho do JSON][JSON UTF-8 {"sourceText"}]`
  seguido do PCM. O `sourceText` (transcrição, para o contexto) é CONTEÚDO e fica no corpo (bytes opacos à
  infraestrutura), NUNCA em header — headers de acesso do Supabase/Deno/proxies logam headers, e pôr texto
  de voz em header seria regressão do invariante "conteúdo nunca em logs". Metadados NÃO-conteúdo
  (línguas, durações de estágio, formato) vão nos headers `X-Verbalix-*`.
- **Lock explícito de playback (D8)**: o ordering N-1→N do M2 era garantido por ACIDENTE (chamada síncrona
  bloqueante num runtime `current_thread`). O playback progressivo assíncrono quebraria isso. Adicionado um
  `tokio::sync::Mutex<()>` de capacidade 1 (`playback_lock`) mantido pelo worker, adquirido antes de tocar
  o segmento N e liberado ao terminar — garantia INDEPENDENTE do runtime. Teste de burst-drain (um
  `insert()` liberando N-1 atrasado + N + N+1 juntos) prova a ordem.
- **Drenagem desacoplada da vez de tocar (D9)**: cada task de dispatch concorrente (`MAX_IN_FLIGHT=2`)
  drena o corpo para um buffer em memória (`Arc<Mutex<VecDeque<f32>>>` + flags `complete`/`cancel`) assim
  que os bytes chegam, liberando o socket cedo (evita o timeout de 55 s do client cobrir a espera na fila e
  evita socket órfão em segmento descartado). A vez de tocar apenas CONSOME o buffer. `underrun` no meio do
  segmento (jitter) → silêncio local + contador nas métricas.
- **Watchdog do corpo streaming na Edge (D10)**: o `clearTimeout` do setup neutraliza o abort quando os
  headers saem; adicionado um `TransformStream` que aborta o corpo se nenhum chunk chegar em
  `BODY_INACTIVITY_MS = 30 s`.
- **Contexto (D4)**: `TranslationContext` por-sessão (janela cap 2 itens, source-only, cap de scalars),
  enviado no request; **promovido só APÓS playback bem-sucedido** (re-checando `accepts` — sem contexto
  fantasma, reset vence a corrida com `leave_live`); envolto em `<untrusted_context>` próprio no prompt
  (defesa contra prompt-injection via contexto, separado do `<untrusted_text>` do segmento). O texto vive
  só em memória do Rust: NUNCA logado, em diagnostics, em evento `live-state`, persistido ou no React.
  Defasagem causal reconhecida: com `MAX_IN_FLIGHT=2` o contexto é best-effort dos segmentos já concluídos.
- **Endpointing (D7)**: histerese no fechamento por silêncio (só fecha cedo se o enunciado teve duração
  mínima de voz), constantes nomeadas em `EndpointerConfig` (injetável/testável).

### Arquivos Modificados
| Arquivo | Tipo |
|---------|------|
| `supabase/functions/interpret/{contract,provider,stages,handler,index}.ts` | Modificado (stream + context) |
| `supabase/functions/interpret/streaming.ts` | Criado (frame VLBX, watchdog, prepend) |
| `supabase/functions/interpret/{contract,provider,handler,stages}_test.ts`, `{handler_edge_cases,provider_stream,streaming}_test.ts` | Criado/Modificado |
| `src-tauri/Cargo.toml` (+ `Cargo.lock`) | Modificado (feature `stream` do reqwest) |
| `src-tauri/src/platform/audio_wav.rs` | Modificado (`pcm_i16le_to_f32`, split) |
| `src-tauri/src/platform/audio_resample.rs` | Criado (`IncrementalResampler` com fase) |
| `src-tauri/src/platform/audio_playback.rs` + `audio_playback_stream.rs` | Modificado/Criado (playback progressivo) |
| `src-tauri/src/domain/endpointing.rs` | Modificado (histerese) |
| `src-tauri/src/domain/live_interpretation.rs` | Modificado (`TranslationContext`, `SegmentResult` puro) |
| `src-tauri/src/application/streaming_audio.rs` | Criado (`StreamSegmentHandle`) |
| `src-tauri/src/application/voice_pipeline.rs` + `voice_pipeline_stream.rs` | Modificado/Criado (drenagem, frame) |
| `src-tauri/src/application/live_worker.rs` + `live_worker/playback.rs` | Modificado/Criado (lock, wiring, métricas) |
| `src-tauri/src/application/{live_interpretation,live_session_setup,ports}.rs` | Modificado |
| `src-tauri/src/diagnostics.rs` + `diagnostics/latency.rs` | Modificado/Criado (buckets p50/p95, underruns) |
| `src-tauri/src/commands_live.rs` | Modificado (`firstAudioMs` no evento) |
| `src/{types,native}.ts` (+ `native.test.ts`), `src/components/LivePanel.tsx` (+ test) | Modificado (firstAudioMs) |
| Vários `*_tests.rs` (worker/streaming/context/frame) | Criado/Modificado |

## Testes
| Métrica | Valor |
|---------|-------|
| Rust (`cargo test`) | 382 |
| Deno (interpret + functions existentes) | 162 |
| Vitest (frontend) | 110 |
| Cobertura (native.ts + types.ts, threshold enforced) | 100% |
| Playwright e2e | 14 |

## Verificação de Qualidade
| Critério | Status |
|----------|--------|
| `npm test` / `npm run test:coverage` | OK / 100% |
| `npm run test:e2e` | OK (14) |
| `npm run build` | OK |
| `cargo test` | OK (382) |
| `cargo clippy --all-targets --all-features -- -D warnings` | Limpo (0) |
| `cargo fmt --check` | Limpo |
| `deno test supabase/functions/` | OK (162, sem regressão) |
| `tauri build --debug --bundles app` | OK (assinado ad-hoc) |
| Gate de tamanho (`lib.rs` ≤301; arquivos tocados < ~300 efetivas) | 270; todos < 300 |
| Invariantes (ordering N-1→N com lock, fail-closed, `accepts()`, filas bounded, sem conteúdo em logs/eventos, contrato de erro no caminho não-stream) | Verificados em código |

### Nota de conformidade
Conforme o padrão dos marcos anteriores, gates verdes são necessários mas não suficientes: a auditoria de
conformidade (privacidade do `sourceText`/contexto, promoção só pós-playback, lock explícito de ordenação,
frame VLBX casando byte-a-byte com o parser Rust `from_be_bytes`, watchdog do corpo, tamanhos de arquivo)
foi verificada além dos gates. Dois arquivos de produção (`live_worker.rs` 308, `diagnostics.rs` 302) e dois
de teste (`live_worker_tests.rs` 420, `voice_pipeline_stream_tests.rs` 308) estouraram o gate de ~300 e
foram extraídos por responsabilidade (`live_worker/playback.rs`, `diagnostics/latency.rs`,
`live_worker_test_helpers.rs`, `live_worker_streaming_tests.rs`, `voice_pipeline_stream_frame_tests.rs`).

## Gates Manuais Pendentes (NÃO verificados por testes automatizados)
1. **Latência real medida com fala**: comparar "fala→primeiro áudio" ANTES (M2/M3 buffered) vs DEPOIS
   (streaming PCM), registrando p50/p95. Requer Edge deployada + sessão autenticada + permissão de mic.
2. `supabase secrets set ELEVEN_LABS_KEY=...` + deploy da Edge `interpret` com o novo contrato (`stream`,
   `context`, headers `X-Verbalix-*`, `output_format=pcm_24000`) — ação de OPS.
3. Permissão real de microfone / TCC (bundle assinado + concessão do usuário).
4. Auditoria `VERBALIX_DIAGNOSTICS=1` confirmando ausência de áudio/transcrição/tradução/`voice_id` no
   novo caminho de contexto e nos sumários de latência.
5. Headphones recomendados (sem cancelamento de eco; VAD suprimido em `Speaking` mitiga).

## Trilha Futura (M5)
- Relay WebSocket dedicado + STT incremental (streaming de transcrição), habilitando o overlap
  captura→upload (D3) rejeitado aqui.
- Critério de remoção do caminho JSON/MP3 legado (mantido nesta rodada como rollback seguro).

---
**Verificado por:** Workflow Orchestrator (gates re-executados empiricamente a cada handoff)
**Data:** 2026-08-19
**Branch/Worktree:** `live-latency` / `.worktrees/live-latency` (NÃO mergeado)
**Status Final:** APROVADO — pendente de gates manuais, deploy da function e aprovação do usuário para merge
