# [012] - M1: Enrollment de voz (clonagem ElevenLabs IVC)

## Contexto
Primeiro marco (M1) da feature de interpretação ao vivo do Verbalix: permitir que o
usuário grave uma amostra da própria voz (~1-2 min) dentro do app e crie um perfil de
voz clonada via ElevenLabs Instant Voice Cloning, proxiado por Edge Function (a chave
nunca chega ao cliente). É a fundação para o M2 (pipeline de tradução frase-a-frase) e
o M3 (microfone virtual), que NÃO fazem parte desta entrega. Plano aprovado do produto:
microfone virtual + tradução ao vivo com voz clonada.

## Escopo

### Incluído
- Edge Functions Deno `voice-enroll`, `voice-delete` e `voice-status` (split
  index/handler/contract/provider idêntico ao `transform`, reusando o
  `SupabaseUserAuthenticator`), com secret `ELEVEN_LABS_KEY` e `SUPABASE_SERVICE_ROLE_KEY`
  só via `Deno.env`. `verify_jwt = true` para as três em `supabase/config.toml`.
- Migration `voice_profiles` com RLS owner-only (defesa em profundidade), `provider_voice_id`
  server-only, e unicidade de perfil ativo por usuário.
- Rust: `AudioCapturePort`/`VoiceEnrollmentPort`, adapter de captura Core Audio (cpal) atrás
  de `cfg(target_os="macos")` com stub não-macOS, permissão de microfone via AVFoundation,
  7 novos comandos Tauri, wiring em `AppRuntime`/`lib.rs`, variantes sanitizadas em
  `VerbalixError`, campo `voice_profile_id` em settings.
- Permissão de microfone: `NSMicrophoneUsageDescription` (Info.plist) + entitlement
  `com.apple.security.device.audio-input`.
- Frontend: aba "Interpretação" com fluxo de enrollment (consentimento, status/solicitação
  de permissão, medidor de nível, gravar/re-gravar/enviar/excluir). Áudio bruto nunca cruza
  o React.
- Exclusão de voz na ElevenLabs E no banco, com reconciliação de falha parcial (status
  `deleting`).

### Excluído
- Pipeline de tradução ao vivo, VAD, `LiveInterpretationCoordinator`, Edge Function
  `interpret` (M2).
- Driver de microfone virtual / Core Audio HAL (M3).
- Settings `target_language`, `input_device_uid`, `monitor_translated_audio` (M2/M3).
- Sync remoto de `voice_profile_id` e espelho iOS (fora do M1).
- Deploy das Edge Functions e `supabase secrets set` (ação de OPS — gate manual).

## Solução Implementada

### Arquitetura
Hexagonal, seguindo os padrões existentes do projeto:

- **Segredo e voice_id server-only (Opção A)**: o cliente NUNCA consulta `voice_profiles`
  via PostgREST. As três Edge Functions escrevem/leem com service role, escopando por
  `user_id` do JWT, e retornam apenas `VoiceProfileView` (`voiceProfileId`, `status`,
  `displayName`) — jamais `provider_voice_id`/`user_id`. A migration não concede SELECT à
  role `authenticated` (só INSERT/UPDATE/DELETE como defesa em profundidade).
- **Idempotência/consistência do enroll**: dedup por `request_id` (early return antes do
  upsert quando o mesmo `requestId` reaparece); cleanup best-effort da voz na ElevenLabs se
  a persistência falhar após a criação (sem voz órfã billada); replace do perfil anterior;
  partial unique index `(user_id) WHERE status NOT IN ('deleting','failed')` para impedir
  dois perfis ativos por usuário em corrida concorrente.
- **Captura de áudio**: o `cpal::Stream` (não-`Send`) é possuído por uma thread de captura
  dedicada; o restante do app fala com ela por um surface `Send`-safe (canal `mpsc` +
  `Arc<AtomicU32>` de nível). `start()` confirma a abertura do stream por canal de reply
  síncrono antes de retornar. Amostra reamostrada/encodada para mono 16 kHz 16-bit WAV com
  cap de duração; base64 gerado no Rust e enviado à Edge Function.
- **Permissão de microfone**: `microphone_permission_status` síncrono (lê
  `authorizationStatus`) e `request_microphone_permission` assíncrono que dispara o diálogo
  AVFoundation e emite evento, sem bloquear a thread do command handler.
- **Settings**: `voice_profile_id: Option<Uuid>` com `#[serde(default)]`, preservado em
  `apply_remote` (struct-literal força o tratamento em compile-time) e NÃO sincronizado
  remotamente.
- **Privacidade**: nenhum áudio, transcrição, token ou `provider_voice_id` em
  `diagnostics.rs`, erros ou eventos; novas variantes de `VerbalixError` são genéricas.

### Arquivos Modificados
| Arquivo | Tipo |
|---------|------|
| `supabase/migrations/20260818000000_voice_profiles.sql` | Criado |
| `supabase/functions/voice-enroll/{index,handler,contract,provider,service_client}.ts` | Criado |
| `supabase/functions/voice-delete/{index,handler,contract,provider}.ts` | Criado |
| `supabase/functions/voice-status/{index,handler,contract}.ts` | Criado |
| `supabase/config.toml` | Modificado |
| `src-tauri/src/domain/voice.rs` | Criado |
| `src-tauri/src/application/{voice_enrollment,enrollment_session}.rs` | Criado |
| `src-tauri/src/platform/{audio_capture,audio_permission}.rs` | Criado |
| `src-tauri/src/commands_voice.rs` | Criado |
| `src-tauri/src/application/{ports,mod,remote_preferences,settings_file}.rs` | Modificado |
| `src-tauri/src/domain/{error,mod,settings}.rs` | Modificado |
| `src-tauri/src/{lib,runtime,diagnostics,commands_transform}.rs` | Modificado |
| `src-tauri/src/platform/mod.rs` | Modificado |
| `src-tauri/{Cargo.toml,Entitlements.plist,Info.plist,tauri.conf.json}` | Criado/Modificado |
| `src/components/InterpretationPanel.tsx` | Criado |
| `src/{App.tsx,native.ts,types.ts,styles/panels.css}` | Modificado |
| Testes: Deno (`*_test.ts`), Rust (inline), Vitest (`native.test.ts`, `InterpretationPanel.test.tsx`), Playwright (`e2e/interpretation-tab.e2e.ts`) | Criado/Modificado |

## Testes
| Métrica | Valor |
|---------|-------|
| Deno (voice-enroll/delete/status) | 49 |
| Rust (`cargo test`) | 268 |
| Vitest (frontend) | 72 |
| Cobertura (native.ts + types.ts, threshold enforced) | 100% |
| Playwright e2e | 8 |

## Verificação de Qualidade
| Critério | Status |
|----------|--------|
| Build frontend (`npm run build`) | OK |
| Debug bundle (`tauri build --debug`) | OK (assinado, `codesign --verify --deep --strict` OK) |
| `NSMicrophoneUsageDescription` no Info.plist compilado | Presente |
| Entitlement `com.apple.security.device.audio-input` no binário assinado | Presente |
| `cargo clippy -D warnings` | Limpo |
| `cargo fmt --check` | Limpo |
| Gate de tamanho `lib.rs` (≤301) | 295 |
| `deno test transform` (reuso de auth) | 38, sem regressão |
| QA (com dual analysis) | APPROVED (após 1 ciclo REJECTED_CODE corrigido) |

### Histórico de QA
Primeiro veredito foi `REJECTED_CODE` com 5 bloqueadores, todos corrigidos e reverificados:
1. Idempotência comparava a coluna `id` (gerada pelo DB) com `request_id` (do cliente) —
   dedup nunca disparava; corrigido para comparar `request_id` e teste ajustado.
2. Voz órfã na ElevenLabs se a persistência falhasse após a criação — adicionado cleanup
   best-effort (`deleteVoice` + `setFailed`).
3. `voice-enroll/handler.ts` com 339 linhas (>300) — `service_client.ts` extraído (217).
4. RLS concedia SELECT de todas as colunas a `authenticated`, expondo `provider_voice_id` —
   policy SELECT removida.
5. Corrida concorrente criava dois perfis por usuário — partial unique index adicionado.

## Gates Manuais Pendentes (NÃO verificados por testes automatizados)
1. Permissão real de microfone / TCC (bundle assinado + concessão do usuário no macOS).
2. Enrollment real na ElevenLabs: `supabase secrets set ELEVEN_LABS_KEY=...` (e
   `SUPABASE_SERVICE_ROLE_KEY` disponível às functions), deploy das 3 Edge Functions, e
   verificação de que a voz aparece/some no dashboard ElevenLabs.
3. Deploy das Edge Functions `voice-enroll`/`voice-delete`/`voice-status`.
4. Auditoria com `VERBALIX_DIAGNOSTICS=1` confirmando ausência de áudio, `provider_voice_id`,
   token ou `voice_id` nos logs.
5. Validação cruzada de RLS: confirmar que `authenticated` não consegue SELECT em
   `voice_profiles` após o deploy da migration.

---
**Verificado por:** Workflow Orchestrator (gates re-executados empiricamente)
**Data:** 2026-08-18
**Branch/Worktree:** `voice-enrollment` / `.worktrees/voice-enrollment` (NÃO mergeado)
**Status Final:** APROVADO — pendente de gates manuais e aprovação do usuário para merge
