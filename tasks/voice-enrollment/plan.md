# Plano — M1: Enrollment de voz (clonagem ElevenLabs IVC)

> Worktree: `.worktrees/voice-enrollment` · Branch: `voice-enrollment`
> Escopo derivado do plano aprovado `no-computador-a-aplica-o-concurrent-shamir.md` (SOMENTE M1).

## 🎯 SCOPE

### Objetivo
Permitir que o usuário grave uma amostra de voz (~1-2 min) dentro do Verbalix, envie-a para clonagem (ElevenLabs Instant Voice Cloning) via Edge Function proxy, veja o status do perfil de voz e possa excluí-lo. Nenhum áudio bruto nem `voice_id` da ElevenLabs chega ao cliente/React/logs. É a fundação para M2 (pipeline de tradução ao vivo), que NÃO faz parte deste escopo.

### Arquivos/Módulos Afetados (criados salvo indicação)
Edge Functions (Deno):
- `supabase/functions/voice-enroll/{index,handler,contract,provider}.ts` (+ testes `*_test.ts`)
- `supabase/functions/voice-delete/{index,handler,contract,provider}.ts` (+ testes `*_test.ts`)
- `supabase/functions/_shared/auth.ts` OU reuso direto de `../transform/auth.ts` (ver DESIGN — reutilizar `SupabaseUserAuthenticator`)
- `supabase/config.toml` (MOD — adicionar `[functions.voice-enroll]` e `[functions.voice-delete]` com `verify_jwt = true`)
- `supabase/migrations/2026NNNN_voice_profiles.sql`

Rust (`src-tauri/src/`):
- `application/ports.rs` (MOD — novo trait `AudioCapturePort` e `VoiceEnrollmentPort`)
- `application/mod.rs` (MOD — exports)
- `application/voice_enrollment.rs` (novo — `RemoteVoiceEnrollment` adapter HTTP → Edge Functions; espelha padrão `RemoteHistoryRepository`)
- `application/enrollment_session.rs` (novo — estado in-memory da gravação em curso: buffer + metering + lifecycle)
- `domain/voice.rs` (novo — tipos `VoiceProfileStatus`, `VoiceProfileView`, `EnrollmentSample`, `MicrophonePermission`)
- `domain/error.rs` (MOD — novas variantes sanitizadas)
- `domain/mod.rs`, `domain/settings.rs` (MOD — campo `voice_profile_id`)
- `platform/audio_capture.rs` (novo — adapter Core Audio/cpal atrás de `cfg(target_os="macos")`)
- `platform/audio_permission.rs` (novo — status/solicitação de permissão de microfone via objc2/AVFoundation, `cfg(macos)`)
- `platform/mod.rs` (MOD — stub não-macOS de captura+permissão)
- `commands_voice.rs` (novo — os 7 comandos)
- `lib.rs` (MOD — `mod commands_voice;`, registro no `generate_handler!`, construção do adapter/estado no setup)
- `runtime.rs` (MOD — campos novos em `AppRuntime`: `voice_enrollment`, `audio_capture`, `enrollment_session`)
- `application/remote_preferences.rs` (MOD — `apply_remote` preserva `voice_profile_id` local)
- `Cargo.toml` (MOD — dependência de captura de áudio; ver DESIGN)
- `tauri.conf.json` (MOD — `bundle.macOS.infoPlist.NSMicrophoneUsageDescription`)
- `Entitlements.plist` (MOD — `com.apple.security.device.audio-input`)

Frontend (`src/`):
- `types.ts` (MOD — tipos de voz + `voiceProfileId` em `AppSettings`)
- `native.ts` (MOD — 7 novos wrappers IPC)
- `components/InterpretationPanel.tsx` (novo — aba/painel de enrollment)
- `App.tsx` (MOD — nova aba "Interpretação")
- `styles/panels.css` (MOD — estilos do painel/medidor)

### Fora do Escopo (NÃO tocar)
- Pipeline de tradução ao vivo, VAD, `LiveInterpretationCoordinator`, Edge Function `interpret` (M2).
- Driver de microfone virtual / Core Audio HAL / `VirtualMicOutputPort` (M3).
- `target_language`, `input_device_uid`, `monitor_translated_audio` (settings de M2/M3).
- Sync remoto de `voice_profile_id` (fora de M1; iOS fora de escopo).
- `SelectionCoordinator` e qualquer fluxo de seleção/transform existente.
- Deploy real das Edge Functions e `supabase secrets set` (ação de OPS do usuário; documentar como gate manual).

### Riscos de Impacto Lateral
- R1: `AppSettings` cruza IPC, sync remoto e `preferences_sync`. Adicionar campo exige `#[serde(default)]` e revisão de `apply_remote`/`merge_preferences`/`PartialEq` (equality afeta a decisão de sync). Mitigação: struct-literal em `apply_remote` força tratamento em compile-time.
- R2: `lib.rs` tem gate de tamanho (`bundle-smoke.test.ts`, ≤301 linhas). Adicionar wiring pode estourar. Mitigação: construir adapter/estado numa função helper em `runtime.rs`, não inline no `setup`.
- R3: `readBoundedBody`/`MAX_BODY_BYTES` do transform é 64KB — insuficiente p/ áudio. Contract novo precisa de cap próprio.
- R4: CSP e rede — chamadas à ElevenLabs são SEMPRE server-side (Edge Function); cliente só fala com `*.supabase.co` (já permitido no CSP e no entitlement `network.client`).

## 📋 REQUIREMENTS

### Requisitos Funcionais
- [ ] RF01: Edge Function `voice-enroll` recebe amostra de áudio autenticada (JWT obrigatório), chama ElevenLabs IVC (`POST /v1/voices/add`), persiste `voice_profiles` (provider_voice_id server-only) e retorna ao cliente APENAS `{ voiceProfileId (UUID opaco), status, displayName }`.
- [ ] RF02: Edge Function `voice-delete` recebe `voiceProfileId`, resolve o `provider_voice_id` do dono, chama ElevenLabs (`DELETE /v1/voices/{id}`) e remove a linha; em falha parcial mantém status `deleting` para reconciliação.
- [ ] RF03: Migration `voice_profiles` com RLS owner-only; `provider_voice_id` nunca legível pelo cliente (base table sem SELECT para `authenticated`; acesso do cliente só via Edge Function ou view sem a coluna).
- [ ] RF04: `AudioCapturePort` (Rust) captura do microfone físico para um buffer em memória, expondo nível (RMS/peak) para metering; adapter macOS real + stub não-macOS.
- [ ] RF05: Comandos Tauri: `microphone_permission_status`, `request_microphone_permission`, `begin_voice_enrollment`, `finish_voice_enrollment`, `cancel_voice_enrollment`, `delete_voice_profile`, `voice_profile_status`.
- [ ] RF06: `finish_voice_enrollment` codifica o buffer (WAV/PCM), envia à `voice-enroll`, grava `voice_profile_id` opaco em `settings.json` e retorna a `VoiceProfileView`.
- [ ] RF07: UI "Interpretação": consentimento explícito antes de gravar, exibição do status de permissão do mic, botão para solicitar permissão, medidor de nível durante a gravação, ações gravar/parar/re-gravar/enviar/excluir.
- [ ] RF08: Áudio bruto NUNCA cruza para o React — apenas estado e metering via eventos Tauri (`listen`).
- [ ] RF09: Settings ganha `voice_profile_id: Option<UUID opaco>` com `#[serde(default)]`; espelho `voiceProfileId?: string` em `types.ts`. NÃO sincronizado remotamente.
- [ ] RF10: Excluir voz limpa `voice_profile_id` do settings ao concluir; UI reflete estado "sem voz".

### Requisitos Não-Funcionais
- [ ] RNF01: `ELEVEN_LABS_KEY` só via `Deno.env` na Edge Function; nunca no bundle, `build.rs`, cliente ou logs. `.env` local só serve para `supabase secrets set` (não commitá-la, não lê-la no Rust).
- [ ] RNF02: Nenhum áudio, transcrição, nome de voz sensível ou `provider_voice_id` em `diagnostics.rs`/logs/erros (mesma invariante do projeto). Novas variantes de `VerbalixError` são mensagens genéricas.
- [ ] RNF03: Arquivos ≤ ~300 linhas efetivas; sem comentários no código; SOLID/DRY (reusar `SupabaseUserAuthenticator`, padrão de handler/contract do transform).
- [ ] RNF04: Contract de `voice-enroll` com cap próprio de corpo (proposto 10 MB) e timeout adequado (proposto 60 s) — validado por teste.
- [ ] RNF05: Tipos que cruzam IPC serializam camelCase.
- [ ] RNF06: `lib.rs` permanece ≤301 linhas (gate `bundle-smoke.test.ts`).

### Critérios de Aceitação
- [ ] CA01: `deno test` das duas functions novas passa (contract: cap de tamanho, JWT ausente/anon → `UNAUTHENTICATED`, parse inválido, sucesso; provider com fetcher fake; reconciliação de delete).
- [ ] CA02: `cargo test` cobre `enrollment_session` (begin/finish/cancel, buffer, metering), stub não-macOS, `RemoteVoiceEnrollment` com fake HTTP, preservação de `voice_profile_id` em `apply_remote`, `settings` round-trip com `#[serde(default)]`.
- [ ] CA03: `npm test` cobre `native.ts` (7 wrappers) e o `InterpretationPanel` (consentimento, gate de permissão, sem áudio no React).
- [ ] CA04: `npm run test:e2e` prova roteamento da aba e sequência de `invoke`; declara explicitamente que NÃO comprova mic/TCC/enrollment real.
- [ ] CA05: `npm run build`, `cargo clippy --all-targets --all-features -D warnings`, `cargo fmt --check`, `npm run test:coverage` e `npm run tauri -- build --debug --bundles app` verdes; `Info.plist` compilado contém `NSMicrophoneUsageDescription`.

### Edge Cases
- EC01: Permissão de mic negada/não determinada → comando retorna estado tipado; UI orienta a abrir Preferências do Sistema; nunca crash.
- EC02: Amostra muito curta/silenciosa → validação local (duração/energia mínima) antes do upload; erro amigável.
- EC03: Amostra excede o cap → `contract` rejeita (`SAMPLE_TOO_LARGE`) antes de chamar ElevenLabs.
- EC04: ElevenLabs 429/5xx/timeout → mapeado a variante genérica; linha não fica órfã (não inserir provider_voice_id fantasma).
- EC05: Delete com sucesso na ElevenLabs mas falha no DB (ou vice-versa) → status `deleting`; `voice_profile_status` reporta pendência para nova tentativa.
- EC06: `begin` chamado com gravação já em curso → rejeitado (`OperationInProgress`/estado explícito) sem corromper buffer.
- EC07: App fechado durante gravação → buffer em memória descartado (nada persistido); nenhum vazamento.
- EC08: Usuário sem sessão autenticada → comandos de rede retornam `Unauthenticated`; UI roteia ao login (padrão existente).

## 🏗️ DESIGN

### Padrões Utilizados
- **Hexagonal (portas + adapters)**: `AudioCapturePort`/`VoiceEnrollmentPort` no `application`, adapters em `platform`/`application`. Domínio puro em `domain/voice.rs`.
- **Split de Edge Function idêntico ao transform**: `index` (composição/`Deno.serve`), `handler` (deps injetáveis, cap de corpo, timeout, mapeamento de erro→status), `contract` (parse/validate + `ErrorCode`), `provider` (ElevenLabs atrás de interface com `fetcher` injetável). **Reuso**: importar `SupabaseUserAuthenticator`/`UserAuthenticator` de `../transform/auth.ts` (DRY) — decisão: importar direto do transform para evitar duplicação; se acoplamento incomodar, mover para `_shared/auth.ts` e reapontar o transform (avaliar custo; preferir import direto para não mexer no transform em M1).
- **Adapter remoto espelhando `RemoteHistoryRepository`**: `reqwest::Client` com timeout, `bearer_auth` + header `apikey`, erros mapeados a `VerbalixError` sem conteúdo.
- **Publish-then-read para status** (análogo a note-result): estado de enrollment/metering guardado no backend; frontend escuta eventos.

### Escolha de crate de áudio (decisão)
- **`cpal`** (recomendado) para captura: enumeração de dispositivos, stream de input, multiplataforma (facilita o stub/compilação não-macOS via feature nativa), amplamente mantido. Metering (RMS/peak) calculado no callback de captura.
- **Permissão de microfone**: NÃO é coberta por `cpal`. Implementar via `objc2`/AVFoundation (`AVCaptureDevice.authorizationStatus(for: .audio)` / `requestAccess`), já que o projeto usa `objc2`/`objc2-app-kit`. Avaliar `objc2-av-foundation` como dependência `cfg(macos)`; se indisponível/pesada, chamar as APIs via `objc2` msg-send manual. Decisão final delegada ao `@software-engineer` após dual analysis própria, mas o **default recomendado é `cpal` + `objc2-av-foundation`**.
- Registrar no MEMORY do software-engineer o crate efetivamente escolhido.

### Modelo de dados — `voice_profiles`
```
id                uuid primary key default gen_random_uuid()
user_id           uuid not null references auth.users(id) on delete cascade
provider          text not null default 'elevenlabs'
provider_voice_id text            -- SERVER-ONLY, nunca exposto ao cliente
display_name      text not null
status            text not null check (status in ('enrolling','ready','failed','deleting'))
created_at        timestamptz not null default now()
updated_at        timestamptz not null default now()  -- trigger set_updated_at (reusar padrão)
```
- RLS owner-only (select/insert/update/delete) como em `user_preferences`.
- **Proteção do `provider_voice_id`**: a Edge Function escreve com **service role key** (`SUPABASE_SERVICE_ROLE_KEY` via `Deno.env`, bypass RLS). Para o cliente:
  - Opção A (preferida): cliente NUNCA faz SELECT direto; todo status vem via comando Rust → Edge Function (`voice-enroll` retorna a view segura; um endpoint leve de status pode ser derivado ou o Rust guarda o último `VoiceProfileView`).
  - Opção B: `create view public.voice_profiles_public with (security_invoker=on) as select id, provider, display_name, status, created_at, updated_at from public.voice_profiles;` + `revoke select on public.voice_profiles from authenticated;` + grant na view. Cliente lê a view (sem `provider_voice_id`/`user_id`).
  - Decisão: implementar **Opção B** (view segura) — permite `voice_profile_status` ler via REST autenticado sem expor a coluna, mantendo o backend Rust simples. Confirmar na dual analysis.

### Contratos Edge Function
`voice-enroll` request (JSON, camelCase):
```
{ requestId: uuid, displayName: string, sampleBase64: string, mimeType: "audio/wav" }
```
- `contract.ts`: valida uuid, displayName não-vazio (limite de chars), mimeType em allowlist, tamanho de `sampleBase64` ≤ `MAX_SAMPLE_BYTES` (10 MB). `ErrorCode`: `UNAUTHENTICATED | SAMPLE_TOO_LARGE | INVALID_REQUEST | PROVIDER_TIMEOUT | PROVIDER_REJECTED | INTERNAL_ERROR`.
- `provider.ts`: decodifica base64 → `Blob`, monta `multipart/form-data` (`name`, `files`) e faz `POST https://api.elevenlabs.io/v1/voices/add` com header `xi-api-key`. Extrai `voice_id`. `fetcher` injetável p/ teste.
- `handler.ts`: cap próprio de corpo, timeout 60 s (abort), autentica (reuso), insere/atualiza `voice_profiles` (service role), retorna view segura.
- `voice-enroll` response: `{ voiceProfileId: uuid, status, displayName }` (SEM provider_voice_id).

`voice-delete` request: `{ requestId: uuid, voiceProfileId: uuid }`.
- Marca `status='deleting'`, resolve `provider_voice_id`, chama `DELETE /v1/voices/{id}`, depois `DELETE` da linha. Falha parcial → permanece `deleting`. Idempotente (voice_id ausente na ElevenLabs = sucesso lógico).

### Fluxo de dados (enrollment)
```
UI consentimento → request_microphone_permission (cmd) → status
UI "Gravar" → begin_voice_enrollment (cmd) → AudioCapturePort.start() → buffer + eventos de metering (Tauri emit)
UI medidor consome eventos (listen) [SEM áudio, só nível/estado]
UI "Parar/Enviar" → finish_voice_enrollment (cmd):
    AudioCapturePort.stop() → encode WAV → RemoteVoiceEnrollment.enroll(sample, displayName, token)
    → grava voice_profile_id no settings → retorna VoiceProfileView
UI "Excluir" → delete_voice_profile (cmd) → RemoteVoiceEnrollment.delete(id, token) → limpa settings
```

### Interfaces/Contratos (Rust)
```rust
// application/ports.rs
pub trait AudioCapturePort: Send + Sync {
    fn start(&self) -> Result<(), VerbalixError>;
    fn stop(&self) -> Result<EnrollmentSample, VerbalixError>; // PCM/WAV bytes + duração
    fn cancel(&self);
    fn level(&self) -> f32; // metering atual (0.0..1.0), sem conteúdo
    fn permission_status(&self) -> MicrophonePermission;
    fn request_permission(&self) -> MicrophonePermission;
}
pub trait VoiceEnrollmentPort: Send + Sync {
    async fn enroll(&self, sample: &EnrollmentSample, display_name: &str, token: &str)
        -> Result<VoiceProfileView, VerbalixError>;
    async fn delete(&self, id: uuid::Uuid, token: &str) -> Result<(), VerbalixError>;
    async fn status(&self, id: uuid::Uuid, token: &str) -> Result<Option<VoiceProfileView>, VerbalixError>;
}
```
`VoiceProfileView { id: Uuid, status: VoiceProfileStatus, display_name: String }` — NUNCA inclui provider_voice_id.

### Novas variantes `VerbalixError` (sanitizadas)
- `MicrophonePermissionDenied`, `AudioCaptureFailed`, `EnrollmentFailed` (genérica p/ falhas de provider de voz). Reusar `Unauthenticated`, `ProviderTimeout`, `ProviderRejected`, `LocalFailure` onde aplicável. Mensagens em pt-BR, sem conteúdo.

### Settings
- `AppSettings.voice_profile_id: Option<Uuid>` com `#[serde(default)]`. `validate()` inalterado (campo opcional).
- `apply_remote` (remote_preferences.rs): adicionar `voice_profile_id: local.voice_profile_id` (struct literal força isso — protege contra clobber por remoto). `RemotePreferences`/upload NÃO inclui o campo.
- `types.ts`: `voiceProfileId?: string` em `AppSettings` e `defaultSettings` sem o campo (ou `undefined`).

### Permissão macOS
- `tauri.conf.json`: `bundle.macOS.infoPlist = { "NSMicrophoneUsageDescription": "O Verbalix usa o microfone para gravar uma amostra e criar sua voz de interpretação." }`.
- `Entitlements.plist`: adicionar `com.apple.security.device.audio-input`.
- Gate: `NSMicrophoneUsageDescription` presente no `Info.plist` COMPILADO do `.app`.

### Componentes Reutilizáveis
- `SupabaseUserAuthenticator` (Deno) — reuso direto.
- Padrão `handler.ts`/`contract.ts`/`timeout scheduler` do transform.
- `RemoteHistoryRepository` como molde para `RemoteVoiceEnrollment`.
- Trigger `set_updated_at` e políticas RLS de `user_preferences`.
- Padrão de aba/nav e componentes de `SettingsPanel.tsx`.
- Padrão publish-then-emit (`note-result`) para metering/estado.

## 📝 TASKS

### Fase 1: Backend Supabase (Deno + SQL)
- [ ] T1.1: [MEDIUM] Migration `voice_profiles` + RLS owner-only + trigger `set_updated_at` + view segura `voice_profiles_public` (sem `provider_voice_id`) e revoke de SELECT na base para `authenticated`.
- [ ] T1.2: [MEDIUM] `voice-enroll`: `contract.ts` (parse/validate, cap 10 MB, ErrorCodes) + `provider.ts` (ElevenLabs IVC multipart, fetcher injetável) + `handler.ts` (auth reuso, service-role upsert, timeout 60 s) + `index.ts`.
- [ ] T1.3: [MEDIUM] `voice-delete`: contract + provider (DELETE ElevenLabs, idempotente) + handler (status `deleting`, reconciliação) + index.
- [ ] T1.4: [LOW] `supabase/config.toml`: entradas das duas functions com `verify_jwt = true`.

### Fase 2: Domínio + Ports + Erros (Rust)
- [ ] T2.1: [LOW] `domain/voice.rs`: `VoiceProfileStatus`, `VoiceProfileView`, `EnrollmentSample`, `MicrophonePermission` (serde camelCase) + export em `domain/mod.rs`.
- [ ] T2.2: [LOW] `domain/error.rs`: novas variantes sanitizadas + mensagens pt-BR.
- [ ] T2.3: [LOW] `domain/settings.rs`: `voice_profile_id: Option<Uuid>` com `#[serde(default)]`; `remote_preferences::apply_remote` preserva o campo.
- [ ] T2.4: [MEDIUM] `application/ports.rs`: `AudioCapturePort` + `VoiceEnrollmentPort` (+ exports em `application/mod.rs`).

### Fase 3: Adapters (Rust)
- [ ] T3.1: [MEDIUM] `application/voice_enrollment.rs`: `RemoteVoiceEnrollment` (enroll/delete/status) via `reqwest`, base64 do sample, mapeamento de erro sem conteúdo.
- [ ] T3.2: [MEDIUM] `application/enrollment_session.rs`: estado in-memory (Mutex) da gravação — begin/finish/cancel, guarda buffer, expõe metering.
- [ ] T3.3: [MEDIUM] `platform/audio_capture.rs` (cfg macos, `cpal`): captura → buffer + nível; `platform/audio_permission.rs` (objc2/AVFoundation): status/request.
- [ ] T3.4: [LOW] `platform/mod.rs`: stub não-macOS retornando `UnsupportedPlatform`/estado neutro (mantém compilação).
- [ ] T3.5: [LOW] `Cargo.toml`: adicionar `cpal` (e dep de permissão) — validar que compila com clippy.

### Fase 4: Comandos + Wiring (Rust)
- [ ] T4.1: [MEDIUM] `commands_voice.rs`: os 7 comandos, IPC camelCase, erros tipados, sem conteúdo em logs.
- [ ] T4.2: [MEDIUM] `runtime.rs`: novos campos em `AppRuntime` + helper de construção (evita inflar `lib.rs`).
- [ ] T4.3: [LOW] `lib.rs`: `mod commands_voice;`, registro no `generate_handler!`, chamada do helper de wiring. MANTER ≤301 linhas.
- [ ] T4.4: [LOW] `tauri.conf.json` (infoPlist mic) + `Entitlements.plist` (audio-input).

### Fase 5: Frontend
- [ ] T5.1: [LOW] `types.ts`: tipos de voz + `voiceProfileId?` em `AppSettings`.
- [ ] T5.2: [LOW] `native.ts`: 7 wrappers IPC.
- [ ] T5.3: [MEDIUM] `components/InterpretationPanel.tsx`: consentimento, status/solicitação de permissão, medidor de nível (via `listen`), gravar/parar/re-gravar/enviar/excluir. Áudio nunca no React.
- [ ] T5.4: [LOW] `App.tsx`: aba "Interpretação" + `styles/panels.css`.

### Fase 6: Testes (test-engineer)
- [ ] T6.1: Deno contract/provider/handler tests das duas functions (cap, auth, parse, sucesso, reconciliação de delete).
- [ ] T6.2: Rust: `enrollment_session`, stub não-macOS, `RemoteVoiceEnrollment` (fake HTTP), `apply_remote` preserva voice_profile_id, settings round-trip serde default.
- [ ] T6.3: Vitest: `native.ts` (7 wrappers) + `InterpretationPanel` (consentimento/permissão/sem áudio).
- [ ] T6.4: Playwright e2e: roteamento da aba + sequência de invoke (declara não-cobertura de mic/TCC/real).

### Fase 7: Gates + Entrega
- [ ] T7.1: Rodar suíte completa: `npm test`, `npm run test:coverage`, `npm run test:e2e`, `npm run build`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `deno test` das functions novas, `npm run tauri -- build --debug --bundles app`.
- [ ] T7.2: Verificar `Info.plist` compilado tem `NSMicrophoneUsageDescription`.
- [ ] T7.3: Doc de entrega `docs/NNN-*.md` (pt-BR) com gates manuais listados.

## Gates Manuais (NÃO alegar como verificados por testes automatizados)
- Permissão real de microfone / TCC (bundle assinado + concessão do usuário).
- Enrollment real na ElevenLabs (chave setada via `supabase secrets set ELEVEN_LABS_KEY=...` + `SUPABASE_SERVICE_ROLE_KEY` disponível às functions; voz aparece/some no dashboard).
- Deploy das Edge Functions `voice-enroll`/`voice-delete`.
- Diagnostics sem amostras/tokens/voice_id (auditoria manual do output com `VERBALIX_DIAGNOSTICS=1`).

## Análise Dual
(preenchido após 1b — relatórios upsidedown 🔴 e downsideup 🟢)
