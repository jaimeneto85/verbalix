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
- `supabase/functions/voice-status/{index,handler,contract}.ts` (+ testes) — service-role, protege `provider_voice_id`
- reuso direto de `../transform/auth.ts` (`SupabaseUserAuthenticator`) e `../transform/handler.ts` (`readBoundedBody`/`TimeoutScheduler`)
- `supabase/config.toml` (MOD — `[functions.voice-enroll]`, `[functions.voice-delete]`, `[functions.voice-status]` com `verify_jwt = true`)
- Secrets Edge (OPS, não commitar): `ELEVEN_LABS_KEY`, e `SUPABASE_SERVICE_ROLE_KEY` (já disponível no runtime Supabase) via `Deno.env`
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
- [ ] RF11: Enroll idempotente por `request_id` (retry pós-sucesso não cria segunda voz); órfão na ElevenLabs após falha de DB é limpo best-effort.
- [ ] RF12: Re-enrollment com perfil existente faz replace (exclui o anterior antes de finalizar o novo).
- [ ] RF13: `voice-status` (3ª Edge Function, service-role) fornece status sem expor `provider_voice_id`; captura formato mono 16 kHz 16-bit com cap de duração client-side.
- [ ] RF14: `request_microphone_permission` é async e emite evento `microphone-permission` (não bloqueia a thread do command handler).

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
- EC09: JWT expira entre `begin` e `finish` (gravação longa) → `finish` tenta refresh de sessão (reusar `runtime.auth.refresh` + `session.save`, padrão do transform) antes do upload; se falhar, `Unauthenticated` com UX clara (distinta de "sem sessão"). Buffer preservado para reenviar após relogin quando possível.
- EC10: ElevenLabs cria a voz mas o INSERT/UPDATE no DB falha → best-effort `DELETE` da voz recém-criada (sem órfão billado); erro retornado ao cliente.
- EC11: Retry client-side de `finish` após sucesso (blip de rede) → dedup por `request_id` retorna a view existente, sem segunda voz.
- EC12: Novo enroll com perfil anterior preso em `deleting` → reconciliar/forçar delete do anterior antes de prosseguir.
- EC13: App dorme/vai a background por longo tempo entre `begin` e `finish` (menu-bar, sem garantia de lifecycle) → estado in-memory pode ser perdido; UI sinaliza claramente "amostra não salva" até o enroll concluir; `finish` sem buffer válido → erro amigável, não crash.

## 🏗️ DESIGN

### Padrões Utilizados
- **Hexagonal (portas + adapters)**: `AudioCapturePort`/`VoiceEnrollmentPort` no `application`, adapters em `platform`/`application`. Domínio puro em `domain/voice.rs`.
- **Split de Edge Function idêntico ao transform**: `index` (composição/`Deno.serve`), `handler` (deps injetáveis, cap de corpo, timeout, mapeamento de erro→status), `contract` (parse/validate + `ErrorCode`), `provider` (ElevenLabs atrás de interface com `fetcher` injetável). **Reuso**: importar `SupabaseUserAuthenticator`/`UserAuthenticator` de `../transform/auth.ts` (DRY) — decisão: importar direto do transform para evitar duplicação; se acoplamento incomodar, mover para `_shared/auth.ts` e reapontar o transform (avaliar custo; preferir import direto para não mexer no transform em M1).
- **Adapter remoto espelhando `RemoteHistoryRepository`**: `reqwest::Client` com timeout, `bearer_auth` + header `apikey`, erros mapeados a `VerbalixError` sem conteúdo.
- **Publish-then-read para status** (análogo a note-result): estado de enrollment/metering guardado no backend; frontend escuta eventos.

### Escolha de crate de áudio + formato + thread-ownership (decisões revisadas 🔴)
- **`cpal`** para captura: enumeração de dispositivos, stream de input, multiplataforma. Metering (RMS/peak) calculado no callback.
- **⚠️ `cpal::Stream` NÃO é `Send` no backend CoreAudio.** O trait `AudioCapturePort: Send + Sync` NÃO pode guardar um `cpal::Stream` num `Mutex` compartilhado — não compila (ou exige `unsafe impl Send`, footgun proibido, ver `docs/003`). **DECISÃO**: o `cpal::Stream` é possuído por uma **thread de captura dedicada** (peer conceitual do `MainThreadOverlayDispatcher`); o resto do app fala com ela por um surface `Send`-safe: comandos via `mpsc` (`Start`/`Stop`/`Cancel`) e nível via `Arc<AtomicU32>` (bits de f32). O buffer PCM acumula na thread de captura; `stop()` devolve o `EnrollmentSample` por um canal. Documentar como padrão nomeado em `domain`/`platform` (mini-ADR no topo do arquivo NÃO — sem comentários; documentar no doc de entrega e MEMORY).
- **Formato de áudio FIXADO** (evita `SAMPLE_TOO_LARGE` no happy-path): capturar no formato nativo do device e **reamostrar/encodar para mono 16 kHz 16-bit PCM WAV** antes de gerar `sampleBase64`. 120 s → ~3,84 MB raw / ~5,1 MB base64, folgado sob o cap de 10 MB. Teste de tamanho: sample sintético de 120 s < cap com margem.
- **Cap de duração client-side**: limitar a gravação (ex. 120 s) na captura, cortando/rejeitando excesso, para o cap de bytes ser sempre respeitado (EC02/EC03 viram requisito, não afterthought).
- **Permissão de microfone (assíncrona)**: `AVCaptureDevice.requestAccess(for:.audio, completionHandler:)` é **assíncrona** (bloco em fila arbitrária). NÃO modelar como `fn request_permission(&self) -> MicrophonePermission` síncrono bloqueante (trava a thread do command handler se o usuário ignorar o diálogo). **DECISÃO**: `microphone_permission_status` (síncrono, lê `authorizationStatus`) + `request_microphone_permission` como **command async** que dispara o diálogo e **emite um evento** (`microphone-permission` via publish-then-emit, padrão note-result) com o resultado; retorna imediatamente `pending`/estado atual. Port: `permission_status()` síncrono; `request_permission(callback)` ou versão que aceita um emissor.
- Dep de permissão: `objc2-av-foundation` (`cfg(macos)`) reusando o padrão msg-send já presente; fallback msg-send manual via `objc2` se necessário.
- Registrar no MEMORY do software-engineer o crate/formato efetivamente escolhidos.

### Modelo de dados — `voice_profiles`
```
id                uuid primary key default gen_random_uuid()
user_id           uuid not null references auth.users(id) on delete cascade
provider          text not null default 'elevenlabs'
provider_voice_id text            -- SERVER-ONLY, nunca exposto ao cliente
display_name      text not null
status            text not null check (status in ('enrolling','ready','failed','deleting'))
request_id        uuid            -- idempotência: dedup de finish_voice_enrollment
created_at        timestamptz not null default now()
updated_at        timestamptz not null default now()  -- trigger set_updated_at (REUSA a função já existente do user_preferences; só criar o trigger)
```
- RLS owner-only (select/insert/update/delete) como em `user_preferences`.
- **Proteção do `provider_voice_id` — DECISÃO REVISADA (Opção A) após análise de risco 🔴**:
  - A Opção B (view `security_invoker=on` + `revoke select` na base para `authenticated`) foi **REJEITADA**: com `security_invoker=on` os checks de GRANT são avaliados como o papel `authenticated`; sem SELECT na base, a consulta à view falha com `permission denied for table voice_profiles`. Desligar `security_invoker` reabre o vazamento de linhas de outros usuários se o dono da view tiver BYPASSRLS. É um modelo quebrado.
  - **DECISÃO FINAL — Opção A: o cliente NUNCA toca no PostgREST desta tabela.** Todas as três operações (enroll, delete, status) passam por Edge Functions que escrevem/leem com **service role key** (`SUPABASE_SERVICE_ROLE_KEY` via `Deno.env`, bypass RLS), sempre escopando por `user_id = <sub do JWT autenticado>`. Cada função retorna somente a `VoiceProfileView` segura (`{ voiceProfileId, status, displayName }`), nunca `provider_voice_id`/`user_id`.
  - Consequência: adicionar uma **terceira Edge Function `voice-status`** (mesmo split), simétrica a enroll/delete, backed por service role. Mantém um único modelo de acesso a dados (sem introduzir o primeiro padrão view-com-grants-restritos do codebase).
  - RLS owner-only permanece na base como defesa em profundidade (mesmo que o cliente nunca a consulte diretamente).
  - **Validação obrigatória**: teste que prova que um SELECT direto via PostgREST à `voice_profiles` como `authenticated` NÃO retorna `provider_voice_id` de terceiros (idealmente teste de grant/RLS com `set role authenticated` + claim jwt).

### Contratos Edge Function
`voice-enroll` request (JSON, camelCase):
```
{ requestId: uuid, displayName: string, sampleBase64: string, mimeType: "audio/wav" }
```
- `contract.ts`: valida uuid, displayName não-vazio (limite de chars), mimeType em allowlist, tamanho de `sampleBase64` ≤ `MAX_SAMPLE_BYTES` (10 MB). `ErrorCode`: `UNAUTHENTICATED | SAMPLE_TOO_LARGE | INVALID_REQUEST | PROVIDER_TIMEOUT | PROVIDER_REJECTED | INTERNAL_ERROR`.
- `provider.ts`: decodifica base64 → `Blob`, monta `multipart/form-data` (`name`, `files`) e faz `POST https://api.elevenlabs.io/v1/voices/add` com header `xi-api-key`. Extrai `voice_id`. `fetcher` injetável p/ teste.
- `handler.ts`: cap próprio de corpo, timeout 60 s (abort), autentica (reuso), insere/atualiza `voice_profiles` (service role), retorna view segura.
- `voice-enroll` response: `{ voiceProfileId: uuid, status, displayName }` (SEM provider_voice_id).

**Idempotência do enroll (🔴)**: `handler` de `voice-enroll` deve deduplicar por `request_id` — se já existe linha do mesmo `user_id`+`request_id`, retornar a view existente sem criar segunda voz na ElevenLabs (evita voz órfã billada em retry de rede pós-sucesso). Fluxo: (1) INSERT linha `status='enrolling'` com `request_id` (unique por user+request_id), tratando conflito como "já processado"; (2) chamar ElevenLabs; (3) em sucesso, UPDATE `provider_voice_id`+`status='ready'`; (4) **se o UPDATE/DB falhar após a ElevenLabs criar a voz → best-effort `DELETE /v1/voices/{id}` para não orfanar** (EC-orphan), e retornar erro.
**Re-enrollment (🔴)**: se o usuário já tem perfil `ready`/`enrolling`, o novo enroll é um **replace**: excluir o perfil anterior (ElevenLabs + linha) antes de finalizar o novo, ou bloquear com erro tipado `EnrollmentFailed`/estado explícito. DECISÃO: replace — deletar o anterior no início do finish; se houver perfil preso em `deleting`, tentar reconciliar antes.

`voice-status` request: `{ requestId: uuid, voiceProfileId: uuid }` → retorna `VoiceProfileView | null` (service role, escopado por user_id do JWT). Nunca expõe `provider_voice_id`.

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
- [x] T1.1: [MEDIUM] Migration `voice_profiles` (com `request_id`) + RLS owner-only (defesa em profundidade) + trigger `set_updated_at` (REUSA função existente, só `create trigger`) + índice unique `(user_id, request_id)`. SEM view/revoke (Opção A). Teste de grant/RLS provando não-vazamento de `provider_voice_id` a terceiros.
- [x] T1.2: [HIGH] `voice-enroll`: `contract.ts` (parse/validate, cap 10 MB, ErrorCodes; REUSA `isUuid`) + `provider.ts` (ElevenLabs IVC multipart, fetcher injetável) + `handler.ts` (auth REUSA `../transform/auth.ts`; `readBoundedBody`/`TimeoutScheduler` REUSADOS/parametrizados; service-role; idempotência por request_id; cleanup de órfão; replace de perfil existente; timeout 60 s) + `index.ts`.
- [x] T1.3: [MEDIUM] `voice-delete`: contract + provider (DELETE ElevenLabs, idempotente) + handler (status `deleting`, reconciliação, service-role) + index.
- [x] T1.4: [MEDIUM] `voice-status`: contract + handler (service-role, escopado por user_id, retorna `VoiceProfileView | null`) + index.
- [x] T1.5: [LOW] `supabase/config.toml`: entradas das TRÊS functions com `verify_jwt = true`.

### Fase 2: Domínio + Ports + Erros (Rust)
- [x] T2.1: [LOW] `domain/voice.rs`: `VoiceProfileStatus`, `VoiceProfileView`, `EnrollmentSample`, `MicrophonePermission` (serde camelCase) + export em `domain/mod.rs`.
- [x] T2.2: [LOW] `domain/error.rs`: novas variantes sanitizadas + mensagens pt-BR.
- [x] T2.3: [LOW] `domain/settings.rs`: `voice_profile_id: Option<Uuid>` com `#[serde(default)]`; `remote_preferences::apply_remote` preserva o campo.
- [x] T2.4: [MEDIUM] `application/ports.rs`: `AudioCapturePort` + `VoiceEnrollmentPort` (+ exports em `application/mod.rs`).

### Fase 3: Adapters (Rust)
- [x] T3.1: [MEDIUM] `application/voice_enrollment.rs`: `RemoteVoiceEnrollment` (enroll/delete/status) via `reqwest`, base64 do sample, mapeamento de erro sem conteúdo.
- [x] T3.2: [MEDIUM] `application/enrollment_session.rs`: estado in-memory (Mutex) da gravação — begin/finish/cancel, guarda `EnrollmentSample`, expõe metering (padrão `NoteResultState` publish-then-emit). Guard contra `begin` duplo (EC06).
- [x] T3.3: [HIGH] `platform/audio_capture.rs` (cfg macos, `cpal`): **thread de captura dedicada** possuindo o `cpal::Stream` (não-Send); surface Send-safe (`mpsc` + `Arc<AtomicU32>` de nível); reamostragem/encode mono 16 kHz 16-bit WAV; cap de duração. `platform/audio_permission.rs` (objc2-av-foundation): `authorizationStatus` síncrono + `requestAccess` async emitindo evento.
- [x] T3.4: [LOW] `platform/mod.rs`: stub não-macOS retornando `UnsupportedPlatform`/estado neutro (mantém compilação).
- [x] T3.5: [LOW] `Cargo.toml`: adicionar `cpal` + `objc2-av-foundation` (cfg macos) — validar clippy.

### Fase 4: Comandos + Wiring (Rust)
- [x] T4.1: [MEDIUM] `commands_voice.rs`: os 7 comandos, IPC camelCase, erros tipados, sem conteúdo em logs. `finish` refaz refresh de token (EC09).
- [x] T4.2: [MEDIUM] `runtime.rs`: novos campos em `AppRuntime` + **helper de construção do wiring de voz** (mantém `lib.rs` enxuto).
- [x] T4.3: [MEDIUM] `lib.rs`: `mod commands_voice;`, registro dos 7 comandos no `generate_handler!`, 1 chamada ao helper de wiring. **⚠️ GATE DE LINHAS: `lib.rs` está em 278/301** — orçamento apertado. Gerir ativamente: mover wiring para `runtime.rs`; se necessário, extrair código existente de `lib.rs`. Rodar `bundle-smoke.test.ts` para confirmar ≤301.
- [x] T4.4: [LOW] `tauri.conf.json` (infoPlist mic) + `Entitlements.plist` (audio-input).

### Fase 5: Frontend
- [x] T5.1: [LOW] `types.ts`: tipos de voz + `voiceProfileId?` em `AppSettings`.
- [x] T5.2: [LOW] `native.ts`: 7 wrappers IPC.
- [x] T5.3: [MEDIUM] `components/InterpretationPanel.tsx`: consentimento, status/solicitação de permissão, medidor de nível (via `listen`), gravar/parar/re-gravar/enviar/excluir. Áudio nunca no React.
- [x] T5.4: [LOW] `App.tsx`: aba "Interpretação" + `styles/panels.css`.

### Fase 6: Testes (test-engineer)
- [x] T6.1: Deno contract/provider/handler tests das duas functions (cap, auth, parse, sucesso, reconciliação de delete).
- [x] T6.2: Rust: `enrollment_session`, stub não-macOS, `RemoteVoiceEnrollment` (fake HTTP), `apply_remote` preserva voice_profile_id, settings round-trip serde default.
- [x] T6.3: Vitest: `native.ts` (7 wrappers) + `InterpretationPanel` (consentimento/permissão/sem áudio).
- [x] T6.4: Playwright e2e: roteamento da aba + sequência de invoke (declara não-cobertura de mic/TCC/real).

### Fase 7: Gates + Entrega
- [ ] T7.1: Rodar suíte completa: `npm test`, `npm run test:coverage`, `npm run test:e2e`, `npm run build`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `deno test` das functions novas, `npm run tauri -- build --debug --bundles app`.
- [ ] T7.2: Verificar `Info.plist` compilado tem `NSMicrophoneUsageDescription`.
- [ ] T7.3: Doc de entrega `docs/NNN-*.md` (pt-BR) com gates manuais listados.

## Correções QA — Iteração 1 (veredito REJECTED_CODE)
Bloqueadores a corrigir (software-engineer):
- [ ] C1 [CRITICAL]: `voice-enroll/handler.ts:~81` — idempotência compara `previous.voiceProfileId` (coluna `id`) com `enrollReq.requestId` (coluna `request_id`) — espaços de UUID distintos, condição sempre verdadeira → dedup nunca dispara e todo retry recria voz. `getPreviousProfile` deve retornar `request_id`; comparar `previous.requestId !== enrollReq.requestId`. Corrigir também `handler_test.ts:101-111` para usar `previousProfile.requestId = requestId` (cenário atual é impossível em produção).
- [ ] C2 [MAJOR]: `voice-enroll/handler.ts:142-154` — se `setReady` falha após a ElevenLabs criar a voz, voz órfã billada + perfil preso em `enrolling`. Envolver em try-catch: best-effort `provider.deleteVoice(providerVoiceId)` + `setFailed(voiceProfileId)` e então erro (EC10).
- [ ] C3 [FILE_SIZE]: `voice-enroll/handler.ts` tem 339 linhas efetivas (>300). Extrair `createSupabaseServiceClient` para `voice-enroll/service_client.ts` (SRP).
- [ ] C4 [SECURITY]: migration `20260818000000_voice_profiles.sql:23-26` — policy `"owners can read voice profiles"` concede SELECT de todas as colunas a `authenticated`, expondo `provider_voice_id` via PostgREST. Remover a policy SELECT (as 3 functions usam service role); manter INSERT/UPDATE/DELETE como defesa em profundidade. Adicionar teste/validação de que `authenticated` não faz SELECT na base.
- [ ] C5 [HIGH]: race concorrente cria dois perfis por usuário. Adicionar partial unique index `UNIQUE (user_id) WHERE status NOT IN ('deleting','failed')` na migration.

Não-bloqueadores (endereçar nesta reentrada):
- [ ] C6 [MED]: `voice-delete/handler.ts` — `markDeleting`/`resolveProviderVoiceId` ignoram `_userId`; usar `userId` nos filtros (defesa contra escalada).
- [ ] C7 [MED]: `audio_capture.rs:44-116` — `start()` retorna `Ok(())` mesmo sem device/config/formato; erro só aparece em `stop()`. Adicionar canal de reply síncrono confirmando abertura do stream.
- [ ] C8 [LOW]: `voice-delete/contract.ts` — `requestId` validado mas não usado; remover ou implementar dedup.
- [ ] C9 [LOW]: inconsistência de caps — `contract.ts:50` usa `MAX_SAMPLE_BYTES*1.5` (15MB), `handler.ts:9` usa 14MB; alinhar ao cap real de 10MB binário documentado.
- [ ] C10 [LOW]: `domain/voice.rs` — `EnrollmentSample.duration_secs` com `#[allow(dead_code)]`, nunca validado; validar duração (server e/ou client) ou justificar.

Pós-correção: re-rodar TODOS os gates (incl. `deno test` e `tauri build --debug` com verificação de Info.plist), test-engineer ajusta/adiciona testes, re-submeter ao qa-reviewer.

## Gates Manuais (NÃO alegar como verificados por testes automatizados)
- Permissão real de microfone / TCC (bundle assinado + concessão do usuário).
- Enrollment real na ElevenLabs (chave setada via `supabase secrets set ELEVEN_LABS_KEY=...` + `SUPABASE_SERVICE_ROLE_KEY` disponível às functions; voz aparece/some no dashboard).
- Deploy das Edge Functions `voice-enroll`/`voice-delete`.
- Diagnostics sem amostras/tokens/voice_id (auditoria manual do output com `VERBALIX_DIAGNOSTICS=1`).

## Análise Dual

### 🔴 Riscos (upsidedown) — incorporados ao plano
1. **CRÍTICO — Proteção do `provider_voice_id`**: `security_invoker=on` + `revoke select` quebra a leitura (permission denied) e a variante insegura vaza linhas de terceiros. → **Corrigido**: Opção A (cliente nunca toca PostgREST; 3 Edge Functions service-role; teste de grant/RLS obrigatório).
2. **CRÍTICO — Cap de áudio vs. duração**: 120 s @44.1k/16-bit ≈ 14 MB base64 > cap de 10 MB. Formato de captura não estava fixado. → **Corrigido**: mono 16 kHz 16-bit WAV (~5 MB base64) + cap de duração client-side + teste de tamanho.
3. **`cpal::Stream` não-Send** vs. `AudioCapturePort: Send+Sync`. → **Corrigido**: thread de captura dedicada + surface Send-safe (mpsc + AtomicU32). T3.3 → HIGH.
4. **Permissão AVFoundation assíncrona** vs. port síncrono. → **Corrigido**: command async + evento `microphone-permission`.
5. **Gate de linhas de `lib.rs` (278/301)**. → **Corrigido**: T4.3 → MEDIUM, wiring em `runtime.rs`, gerir orçamento.
6. **Requisitos faltando**: idempotência de enroll, cleanup de órfão, replace de perfil, expiração de token mid-recording, sono/background. → **Adicionados** (RF11-14, EC09-13).
7. Timeout de 60 s da ElevenLabs IVC é hipótese; medir no gate manual, não tratar como verdade fixa em teste.

### 🟢 Oportunidades (downsideup) — incorporadas ao plano
1. `handler.ts`/`contract.ts`/`auth.ts`/`readBoundedBody`/`TimeoutScheduler`/`isUuid` do transform são reutilizáveis quase 1:1 (não só `auth.ts`) → refletido em T1.2.
2. `set_updated_at()` já existe (migration user_preferences) → só criar o trigger (T1.1).
3. `RemoteHistoryRepository` é molde direto de `RemoteVoiceEnrollment` (mesmo padrão bearer+apikey+error mapping).
4. `NoteResultState`/`PublicationGuard` já testam a corrida "listener anexa tarde" que o metering enfrenta → molde de `enrollment_session` (T3.2).
5. Padrão de aba do `App.tsx` estende com ~10 linhas; gate de auth via prop `authenticated` (EC08).
6. Struct-literal em `apply_remote` protege `voice_profile_id` de clobber em compile-time (gratuito).
7. **Paralelização**: 3 workstreams disjuntos (Deno / Rust / Frontend) contra assinaturas de contrato/IPC já fixadas no DESIGN. Nota: `voice-status` como Edge Function (não REST-view) foi a divergência entre os analistas — resolvida a favor da SEGURANÇA (upsidedown), custando uma função a mais mas eliminando o modelo de grants quebrado.
