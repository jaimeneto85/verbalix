# Plano — Verbalix iOS: Correções Fase 2 + Fases 3-5 (App SwiftUI + Extensões)

> Continuação no mesmo worktree `.worktrees/verbalix-ios-companion` (branch `verbalix-ios-companion`).
> PRIMEIRO as correções (Parte A, prioridade máxima), DEPOIS as Fases 3-5 (Parte B).
> Defeitos A1-A4 CONFIRMADOS por leitura direta do código pelo orquestrador.

## 🎯 SCOPE

### Parte A — Correções (arquivos existentes)
- `src-tauri/src/application/remote_preferences.rs` — LWW real (A1)
- `src-tauri/src/application/settings_file.rs` ou novo sidecar store — timestamp local (A1)
- `src-tauri/src/commands.rs` — `load_settings` não-bloqueante + evento (A2); `save_settings` grava timestamp antes do upsert (A1)
- `src-tauri/src/lib.rs` — remover aliases `RP`/`su`/`ak` (A3); wiring do sidecar/evento
- `src/App.tsx` + `src/native.ts` — ouvir evento `preferences-synced` (A2)
- `ios/VerbalixKit/Sources/.../Preferences/PreferencesSync.swift` + `PreferencesStore.swift` — mesmo LWW (A1)
- `supabase/migrations/20260725000000_user_preferences.sql` — `search_path=''` + remover índice redundante (A4)

### Parte B — Fases 3-5 (arquivos novos)
- `ios/project.yml` (XcodeGen, versionado) → gera `ios/Verbalix.xcodeproj` (gitignored)
- `ios/Local.xcconfig.example` (versionado) + `ios/Local.xcconfig` (gitignored)
- `ios/Config/Supabase.xcconfig` gerado (gitignored)
- `ios/scripts/bootstrap.sh` (gera xcconfig do `.env` + `xcodegen generate`)
- `ios/Verbalix/` (app SwiftUI: `AuthView`, `SettingsView`, `HistoryView`, `EditorView`, `OnboardingView`, entry, `Info.plist`, `.entitlements`, `PrivacyInfo.xcprivacy`)
- `ios/VerbalixAction/` (Action Extension)
- `ios/VerbalixKeyboard/` (Keyboard Extension)
- `ios/VerbalixKit` ganha produto/dependência `supabase-swift` (só `Auth`) + `AuthLocalStorage` sobre `SharedSessionStore` + lock de refresh por arquivo no App Group
- `docs/010-verbalix-ios-app-extensoes.md` (novo) e atualização de `docs/009`

### Fora do Escopo
- Publicação/assinatura de distribuição, App Store Connect, provisioning real além de simulador.
- Alterar o contrato `AppSettings` que cruza o IPC (`src/types.ts`, VerbalixKit) — o timestamp de sync fica em SIDECAR.
- Alterar `domain/` e `platform/` do Rust.

### Riscos de Impacto
- R1: Regressão do desktop pelo evento/timestamp. Mitigação: sidecar isolado; falha não-fatal; `settings.json` fonte de verdade.
- R2: `xcodebuild` exigir Team ID para simulador. Mitigação: `Local.xcconfig` vazio permite build de simulador; assinatura só para device (gate manual).
- R3: Rotação de refresh token entre app+extensões. Mitigação: lock por arquivo no container do App Group serializa o refresh.
- R4: Arquivos gerados (`.xcodeproj`, xcconfig de segredo) vazarem no git. Mitigação: `.gitignore` explícito; fonte da verdade é o YAML/`.env`.
- R5: `.env` ausente no worktree (é gitignored). Verificado: fica em `../../.env`. O bootstrap deve resolver o `.env` do checkout raiz sem logar valores.

## 📋 REQUIREMENTS

### Parte A
- [ ] RA1: `merge_preferences` só deixa o remoto vencer se `remote.updated_at > local.updated_at`. Empate → local vence. Remoto ausente/sem `updated_at` → local vence. Quando o LOCAL vence, dispara re-push (upsert).
- [ ] RA2: timestamp local persistido em SIDECAR `preferences_sync.json` (`{updatedAt, syncedAt}`) no app config dir; NÃO em `AppSettings`. `save_settings` grava o timestamp ANTES de tentar o upsert.
- [ ] RA3: Sidecar ausente/corrompido → "local desconhecido" → remoto vence (comportamento antigo, seguro p/ primeira instalação).
- [ ] RA4: `load_settings` retorna o local IMEDIATAMENTE; sync ocorre fora do caminho crítico e emite evento Tauri `preferences-synced` com as prefs mescladas ao concluir. `save_settings` continua não-bloqueante.
- [ ] RA5: `src/App.tsx` ouve `preferences-synced` e atualiza o estado; coberto por vitest no estilo de `app-flows.test.tsx`/`overlay-flows.test.tsx` (mock de `@tauri-apps/api/event`).
- [ ] RA6: Swift (`PreferencesSync`/`PreferencesStore`) aplica a MESMA lógica LWW usando `SyncedPreferences.updatedAt`, para iPhone e macOS convergirem igual.
- [ ] RA7: `lib.rs` sem aliases de 1-2 letras; nomes completos, quebrando linha normalmente.
- [ ] RA8: migration com `set_updated_at ... set search_path = ''`; remover índice `user_preferences_owner_idx` (a `unique(user_id)` já indexa). Editar o arquivo existente.

### Parte B
- [ ] RB1: `ios/project.yml` gera 3 targets (`Verbalix` `com.verbalix.ios`, `VerbalixAction` `com.verbalix.ios.action`, `VerbalixKeyboard` `com.verbalix.ios.keyboard`); extensões embedadas; todos dependem de `ios/VerbalixKit`; deployment iOS 17.0.
- [ ] RB2: App Group `group.com.verbalix.shared` + Keychain Access Group `$(AppIdentifierPrefix)com.verbalix.shared` nos 3 targets via `.entitlements`.
- [ ] RB3: `DEVELOPMENT_TEAM` lido de `ios/Local.xcconfig` (gitignored) com `.example` versionado; vazio permite build de simulador.
- [ ] RB4: `Supabase.xcconfig` gerado do `.env` raiz (VITE_SUPABASE_URL/ANON_KEY) → Info.plist `VerbalixSupabaseURL`/`VerbalixSupabaseAnonKey` (lido por `BackendConfig.swift`); gitignored; `ios/scripts/bootstrap.sh` gera + roda `xcodegen generate`.
- [ ] RB5: `supabase-swift` (produto `Auth` apenas) adicionado; `AuthLocalStorage` custom sobre `SharedSessionStore`; refresh serializado por lock de arquivo no container do App Group.
- [ ] RB6: App SwiftUI — `AuthView` (magic link, redirect `verbalix-ios://auth/callback` em `CFBundleURLTypes`), `SettingsView` (formalidade/tom/comprimento/histórico, SEM campos macOS-only), `HistoryView` (lista 30 dias, copiar, apagar item, apagar tudo), `EditorView` (`UIViewRepresentable` sobre `UITextView` + `UIEditMenuInteraction` com "Traduzir"/"Aprimorar" e substituição in-place), `OnboardingView` (habilitar teclado + Acesso Total).
- [ ] RB7: Action Extension — `com.apple.ui-services`, `NSExtensionActivationSupportsText=YES`, lê `public.plain-text`, transforma, resultado com Copiar/Compartilhar, `completeRequest`; UI explicita que o resultado não volta sozinho (colar). Estados: não-auth (deep link), sem texto, acima do limite, timeout, rate limit.
- [ ] RB8: Keyboard Extension — `com.apple.keyboard-service`, `RequestsOpenAccess=YES`, barra Traduzir/Aprimorar/progresso; lê `textDocumentProxy.selectedText`, `insertText(result)`; `hasFullAccess==false` → banner + link Ajustes, nunca falha em silêncio; sem estado retido entre invocações.
- [ ] RB9: `PrivacyInfo.xcprivacy` nos targets com `NSPrivacyAccessedAPITypes` necessários.
- [ ] RB10: `.gitignore` cobre `ios/Verbalix.xcodeproj`, `ios/Local.xcconfig`, `ios/Config/Supabase.xcconfig`, `ios/**/*.generated.*`, DerivedData.

### Critérios de Aceitação (Gates)
- [ ] CA01: `swift build`/`swift test --package-path ios/VerbalixKit` OK.
- [ ] CA02: `xcodegen generate --spec ios/project.yml` OK.
- [ ] CA03: `xcodebuild build` do scheme `Verbalix` no simulador (`iPhone 17`; fallback `iPhone 16e` booted) `-allowProvisioningUpdates`.
- [ ] CA04: build dos schemes `VerbalixAction` e `VerbalixKeyboard`.
- [ ] CA05: `cargo test`, `cargo clippy -D warnings`, `cargo fmt` OK.
- [ ] CA06: `npm test`, `npm run build`, `npm run test:e2e` OK.
- [ ] CA07: `deno test supabase/functions/transform/contract_test.ts` OK.

### Edge Cases
- EC01: Editou offline, upsert falhou; próximo `load_settings` NÃO reverte (regressão A1 coberta por teste).
- EC02: Sidecar corrompido → remoto vence, sem crash.
- EC03: Empate exato de `updated_at` → local mantém.
- EC04: `Local.xcconfig` vazio → build de simulador funciona (sem assinatura de device).
- EC05: Extensão sem Acesso Total (`hasFullAccess==false`) → banner, nunca silêncio.
- EC06: App+extensão refresh simultâneo → lock serializa, sem invalidar token.

## 🏗️ DESIGN

### Decisões
- LWW com sidecar: o timestamp local vive em `preferences_sync.json` (fora do `AppSettings`) para não alterar o contrato IPC. `merge_preferences` passa a receber `local_updated_at: Option<OffsetDateTime/String>`. Justificativa explícita do coordenador.
- Sync fora do caminho crítico: `load_settings` retorna local; um `tauri::async_runtime::spawn` faz fetch→merge→persist→`app.emit("preferences-synced", merged)`. Reusa o padrão `listen`/`emit` já existente em `Overlay.tsx`/`note-result`.
- Re-push quando local vence: após o merge, se o local for mais novo, o mesmo fluxo dispara `upsert` (best-effort).
- XcodeGen como fonte de verdade: `.xcodeproj` é derivado e gitignored, espelhando a política de artefatos gerados do repo (`build.rs`/`OUT_DIR`).
- Config Supabase via xcconfig gerado do `.env`: espelha o papel do `build.rs` no desktop; segredo nunca versionado.
- `AuthLocalStorage` sobre `SharedSessionStore`: sessão única para app+extensões; refresh serializado por `flock`/lock de arquivo no App Group para lidar com rotação de refresh token do Supabase.

### Interfaces
- Rust: `PreferencesSyncStore` (sidecar) com `load() -> Option<LocalSyncMeta>` e `record_change(now)`. `merge_preferences(local, local_ts, remote) -> MergeOutcome { settings, needs_push }`.
- Evento: `preferences-synced` payload = `AppSettings` (camelCase, igual ao retorno de `load_settings`).
- Swift: `mergeSyncedPreferences(local:remote:) -> (SyncedPreferences, needsPush: Bool)` puro.

### Reuso
- `listen`/`emit` de `note-result` (Overlay.tsx + overlay-flows.test.tsx mock) → base para `preferences-synced`.
- Estilo atômico `.tmp`+rename de `settings_file.rs` → sidecar.
- Componentes web `AuthPanel.tsx`/`SettingsPanel.tsx`/`HistoryPanel.tsx` → referência de UX para as views SwiftUI.
- `BackendConfig.swift` já lê `VerbalixSupabaseURL`/`VerbalixSupabaseAnonKey` → xcconfig só precisa preencher.
- `docs/fix-supabase-auth-redirect.md` → precedente do gate manual de redirect URL.

## 📝 TASKS

### Parte A — Correções (PRIMEIRO)
- [ ] TA1: [LOW] Migration: `search_path=''` no `set_updated_at` + remover índice redundante (A4).
- [ ] TA2: [LOW] `lib.rs`: nomes completos, remover `RP`/`su`/`ak` (A3).
- [ ] TA3: [MEDIUM] Sidecar `preferences_sync.json` (store atômico) + `record_change` em `save_settings` ANTES do upsert (A1/RA2).
- [ ] TA4: [MEDIUM] `merge_preferences` vira LWW real com `local_updated_at`; retorna `needs_push` (A1/RA1).
- [ ] TA5: [MEDIUM] `load_settings` não-bloqueante + `spawn` de fetch/merge/persist/`emit("preferences-synced")` + re-push quando local vence (A2/RA4).
- [ ] TA6: [MEDIUM] `src/App.tsx`+`native.ts`: listener `preferences-synced` atualizando estado (A2/RA5).
- [ ] TA7: [MEDIUM] Swift `PreferencesSync`/`PreferencesStore`: mesma LWW via `SyncedPreferences.updatedAt` (RA6).

### Parte B — Projeto e config
- [ ] TB1: [MEDIUM] `ios/project.yml` + `.entitlements` (3 targets, App Group, Keychain group, embed extensões, dep VerbalixKit).
- [ ] TB2: [MEDIUM] `Local.xcconfig.example` + `scripts/bootstrap.sh` (gera `Supabase.xcconfig` do `.env` raiz, roda xcodegen) + `.gitignore`.
- [ ] TB3: [MEDIUM] `supabase-swift` (Auth) no package + `AuthLocalStorage` sobre `SharedSessionStore` + lock de refresh por arquivo no App Group.

### Parte B — App e extensões
- [ ] TB4: [MEDIUM] App shell + `OnboardingView` + `AuthView` (magic link, deep link `verbalix-ios://auth/callback`).
- [ ] TB5: [MEDIUM] `SettingsView` (sem campos macOS-only) + `HistoryView` (lista/copiar/apagar/apagar tudo).
- [ ] TB6: [MEDIUM] `EditorView` (`UIViewRepresentable` + `UIEditMenuInteraction` Traduzir/Aprimorar, substituição in-place).
- [ ] TB7: [MEDIUM] Action Extension (RB7) com todos os estados.
- [ ] TB8: [MEDIUM] Keyboard Extension (RB8) com gate de Acesso Total.
- [ ] TB9: [LOW] `PrivacyInfo.xcprivacy` nos targets.

### Documentação
- [ ] TD1: [LOW] `docs/010-verbalix-ios-app-extensoes.md` (Fases 3-5 + seção de gates manuais: Team ID, redirect URL no Supabase, habilitar teclado, Acesso Total, teste em device real) e atualizar `docs/009` (correções A1-A4).

## Análise Dual

### 🟢 Oportunidades incorporadas (downsideup)
- O1: Extrair `atomic_write_json` compartilhada com `settings_file.rs` para o sidecar (TA3), evitando duplicar `.tmp`+rename.
- O2: Reusar o padrão `listen`/`emit` de `note-result` (`Overlay.tsx` + mock em `overlay-flows.test.tsx`) para `preferences-synced`; considerar hook `useTauriEvent`.
- O3: Parte A (Rust/web) e Parte B (Xcode) são independentes em arquivos — TB1/TB2/TB9/TD1 paralelizáveis com TA1-TA7.
- O4: TA1 e TA2 são LOW e desacopladas — commitar cedo e isoladas, reduzindo o diff de risco.
- O5: `AuthLocalStorage` sobre `SharedSessionStore` e um `TransformErrorPresentation` comum servem app + 2 extensões (reduz TB7/TB8).
- O6: `build.rs`/`OUT_DIR` é o precedente exato de "config pública gerada, segredo não versionado" para o xcconfig (TB2).

### 🔴 Riscos mitigados (upsidedown) — AMENDAS OBRIGATÓRIAS
- M1 (CRÍTICO — race cross-writer): o spawn de `load_settings` captura `local` ANTES da rede; um `save_settings` concorrente pode ser sobrescrito com dados stale (o merge copia `shortcut`/`automatic_toolbar`/`confirm_before_replace` do `local` capturado). MITIGAÇÃO: o sidecar guarda uma SEQUENCE/version counter além do timestamp; o spawn, ANTES de `settings.save(merged)`, RE-LÊ `settings.json`+sidecar e ABORTA a escrita se a sequence mudou desde a captura. Teste obrigatório que intercala save durante um fetch in-flight e prova que o edit local NÃO é revertido (superset do EC01).
- M2 (CRÍTICO — iOS não bumpa updatedAt): sem setar `updatedAt = Date()` em toda edição local no `SettingsView`/`PreferencesStore`, o bug A1 REAPARECE no iOS. AMENDA: adicionar RB5a/TB5a — `PreferencesStore` ganha `recordLocalChange()`/`touch()` que seta `updatedAt` antes de persistir; `SettingsView` chama isso em cada mudança; teste unitário análogo ao EC01 no lado Swift.
- M3 (CRÍTICO — TA7 não é greenfield): VERIFICADO — `mergePreferences(local:remote:) -> SyncedPreferences` JÁ existe (`PreferencesSync.swift:127`) e é testado (`MergePreferencesTests`). REESCOPO de TA7: ESTENDER/renomear para retornar `needsPush` e ATUALIZAR `PreferencesStoreTests.swift`; NÃO reimplementar do zero. Manter os casos remote-wins/tie-local/nil já cobertos.
- M4 (CRÍTICO — evento perdido + Overlay): `note-result` reusa `listen` + PULL de fallback (`current_note_result`) porque o backend pode emitir antes do listener registrar (invariante publish-then-emit). AMENDA: RA5/TA6 incluem (a) registrar `listen("preferences-synced")` ANTES de chamar `loadSettings()` no `App.tsx`; (b) um comando de PULL de fallback (ex.: `current_synced_preferences`) que o front consulta após registrar o listener. VERIFICADO que `src/Overlay.tsx:26` também chama `loadSettings()` — ADICIONAR `src/Overlay.tsx` ao escopo (ao menos não regredir; idealmente também ouvir o evento).
- M5 (CRÍTICO — assinatura de simulador): App Group/Keychain entitlements nos 3 targets costumam quebrar build de simulador sem Team mesmo com `-allowProvisioningUpdates`. AMENDA: TB1/TB2 devem definir `CODE_SIGNING_ALLOWED=NO`, `CODE_SIGNING_REQUIRED=NO`, `CODE_SIGN_IDENTITY=""` para a config de simulador no `project.yml`/`Local.xcconfig.example`; assinatura real só para device (gate manual).
- M6 (bootstrap fail-loud + sem log de segredo): `bootstrap.sh` DEVE falhar com mensagem clara se `.env` (em `../../.env` a partir do worktree) estiver ausente/incompleto, NUNCA emitir xcconfig vazio silencioso, e NUNCA ecoar URL/anon key em stdout/CI. Vira critério de aceite.
- M7 (debounce/coalescing): rápidas mudanças (arrastar slider de formalidade) NÃO podem disparar N upserts. AMENDA: `save_settings` re-push e o front devem coalescer (debounce) a escrita/upsert; documentar o intervalo.
- M8 (stale lock da extensão): Keyboard/Action Extension podem ser mortas pelo iOS segurando o lock de refresh (App Group). O lock por arquivo DEVE ter expiração por timeout / recuperação de lock órfão, nunca bloquear o app principal indefinidamente. Vira requisito de TB3.
- M9 (needs_push idempotência): upsert pode ter sucesso no servidor e o cliente perder a resposta (timeout). Evitar re-push infinito: após um `load_settings` com sucesso de fetch, o sidecar registra `syncedAt`; só re-empurra quando `local.updatedAt > syncedAt`.
- M10 (atomicidade cross-file): `settings.json` e `preferences_sync.json` são dois arquivos; ordenar a escrita (conteúdo primeiro, depois sidecar) e tratar sidecar ausente como "local desconhecido" (remoto vence, RA3), evitando merge na direção errada após crash.
- M11 (fixture de merge compartilhada): manter Rust `merge_preferences` e Swift `mergePreferences` em lockstep via um arquivo de fixtures JSON (local/remote/expected/needsPush) consumido por `remote_preferences_tests.rs` e `PreferencesStoreTests.swift`, ou cross-reference documentada em docs/010.
- M12 (timeout de execução da extensão vs Edge 20s): o abort de 20s da Edge Function pode exceder o orçamento de runtime da Action/Keyboard Extension. Definir timeout de cliente mais curto nas extensões e mapear para o estado "timeout" gracioso, nunca deixar o OS matar o processo silenciosamente.

### Reescopo de tarefas (pós-análise)
- TA5 elevada para HIGH (não-bloqueante + race guard M1 + atualização de assertions em `app-flows.test.tsx`/`overlay-flows.test.tsx`).
- TA7 reescopada: estender `mergePreferences` para `needsPush` + `touch()` no `PreferencesStore` + atualizar testes (M2, M3).
- TB3 isolada como tarefa própria de maior risco (SPM dep + AuthLocalStorage + lock cross-processo com expiração, M8).
- Nova TA8: comando de pull `current_synced_preferences` + registro de listener antes de `loadSettings` no `App.tsx` e no `Overlay.tsx` (M4).
- TB1/TB2 incluem chaves de assinatura de simulador (M5) e fail-loud do bootstrap (M6).
