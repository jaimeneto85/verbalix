# Plano — Verbalix iOS Companion (Fundação: Fases 1 e 2)

> Execução restrita ao worktree `.worktrees/verbalix-ios-companion` (branch `verbalix-ios-companion`).
> ESCOPO DESTA EXECUÇÃO: apenas Fase 1 (Swift Package `VerbalixKit`) e Fase 2 (sync de preferências)
> mais a parte de testes correspondente (Fase 6 parcial). As Fases 3, 4 e 5 (app SwiftUI,
> Action Extension, Keyboard Extension) NÃO fazem parte desta execução — dependem de decisão de tooling
> pendente com o usuário.

## 🎯 SCOPE

### Arquivos Afetados (Criados)
- [x] `ios/VerbalixKit/Package.swift`
- [x] `ios/VerbalixKit/Sources/VerbalixKit/Models/Transform.swift`
- [x] `ios/VerbalixKit/Sources/VerbalixKit/Models/VerbalixError.swift`
- [x] `ios/VerbalixKit/Sources/VerbalixKit/Localization/ErrorMessages.swift`
- [x] `ios/VerbalixKit/Sources/VerbalixKit/Networking/BackendConfig.swift`
- [x] `ios/VerbalixKit/Sources/VerbalixKit/Networking/TransformClient.swift`
- [x] `ios/VerbalixKit/Sources/VerbalixKit/Networking/HistoryClient.swift`
- [x] `ios/VerbalixKit/Sources/VerbalixKit/Session/SharedSessionStore.swift`
- [x] `ios/VerbalixKit/Sources/VerbalixKit/Preferences/PreferencesStore.swift`
- [x] `ios/VerbalixKit/Sources/VerbalixKit/Preferences/PreferencesSync.swift`
- [x] `ios/VerbalixKit/Tests/VerbalixKitTests/*.swift`
- [x] `supabase/migrations/<timestamp>_user_preferences.sql`
- [x] `src-tauri/src/application/remote_preferences.rs`
- [x] `docs/009-verbalix-ios-fundacao.md`

### Arquivos Afetados (Modificados)
- [x] `src-tauri/src/application/mod.rs` (registrar módulo `remote_preferences`)
- [x] `src-tauri/src/runtime.rs` (novo campo opcional para o adapter de preferências)
- [x] `src-tauri/src/lib.rs` (wiring do adapter)
- [x] `src-tauri/src/commands.rs` (espelhar prefs em load/save quando há sessão, falha não-fatal)

### Fora do Escopo
- App SwiftUI, Action Extension, Keyboard Extension (Fases 3-5).
- Introdução de `supabase-swift` ou qualquer dependência externa no `VerbalixKit` (só na Fase 3).
- Alterações em `src-tauri/src/domain/` e `src-tauri/src/platform/`.
- Alterar campos macOS-only (`shortcut`, `automatic_toolbar`, `confirm_before_replace`) via sync.

### Riscos de Impacto
- R1: O sync remoto quebrar `load_settings`/`save_settings` do desktop. Mitigação: falha de rede SEMPRE
  não-fatal; `settings.json` é fonte de verdade; nenhuma chamada remota bloqueante no caminho crítico.
- R2: Vazamento de token/texto em log ou mensagem. Mitigação: adapter nunca loga corpo/token; erros
  mapeados para `VerbalixError` sem conteúdo.
- R3: Keychain access-group exigir entitlement e quebrar `swift test` no host. Mitigação: access group
  injetável; testes usam store com diretório/estratégia sem entitlement.
- R4: Divergência entre o contrato Swift e `contract.ts`. Mitigação: fixtures derivadas de `contract_test.ts`.

## 📋 REQUIREMENTS

### Requisitos Funcionais
- [ ] RF01: `Transform.swift` espelha exatamente `contract.ts` (enums, request/response, `requestId` lowercase).
- [ ] RF02: Validação local espelha `contract.ts:38-73` + `transform.rs`: vazio→invalidResponse;
  `unicodeScalars.count > 12_000`→textTooLong; body JSON > 64 KiB→textTooLong; improve sem preferences→
  invalidResponse; formality fora de 1...5→invalidResponse.
- [ ] RF03: `VerbalixError` cobre códigos do servidor + casos de cliente, com `init?(serverCode:)`.
- [ ] RF04: `ErrorMessages` fornece pt-BR consistente com `commands_transform.rs` e `ai_readiness.rs`.
- [ ] RF05: `BackendConfig` deriva endpoints; carrega de Info.plist (`VerbalixSupabaseURL`/
  `VerbalixSupabaseAnonKey`); config ausente/incompleta → nil (nunca crash).
- [ ] RF06: `TransformClient` (URLSession, 20s) envia headers corretos; parseia `{"error":{"code"}}` em
  não-2xx e mapeia por code; `.providerRejected` só quando corpo inválido; valida resposta (contract.ts:75-102).
- [ ] RF07: `HistoryClient` fala com `/rest/v1/transform_history` (snake_case, `Prefer: return=minimal`,
  list ordenada, delete por id e delete-all), `user_id` via `/auth/v1/user`.
- [ ] RF08: `SharedSessionStore` guarda `{accessToken, refreshToken}` no Keychain com
  `kSecAttrAccessibleAfterFirstUnlock` e access group injetável (default `com.verbalix.shared`).
- [ ] RF09: `PreferencesStore` com defaults idênticos a `settings.rs:33-45`; JSON atômico em App Group;
  ausente→defaults; corrompido→falha fechada (lança, NÃO reseta).
- [ ] RF10: Migration `user_preferences` com RLS owner-only (SELECT/INSERT/UPDATE) para `authenticated`.
- [ ] RF11: `PreferencesSync` fetch + upsert (`resolution=merge-duplicates,return=minimal`), merge
  last-write-wins por `updated_at`.
- [ ] RF12: `remote_preferences.rs` (application) espelha campos de IA no save/load com sessão; falha
  não-fatal; nunca sobrescreve campos macOS-only; nunca loga token/texto.

### Requisitos Não-Funcionais
- [ ] RNF01: `VerbalixKit` = ZERO dependências externas; só Foundation + Security.
- [ ] RNF02: `Package.swift` platforms iOS 17 + macOS 14 (macOS só para `swift test` no host).
- [ ] RNF03: Sem comentários no código; arquivos < ~300 linhas efetivas.
- [ ] RNF04: Nenhuma credencial/token/texto do usuário em código, logs ou mensagens.
- [ ] RNF05: `TransformClient` testável sem rede (URLProtocol stub ou protocolo injetável).

### Critérios de Aceitação (Gates obrigatórios)
- [x] CA01: `swift build --package-path ios/VerbalixKit` OK.
- [x] CA02: `swift test --package-path ios/VerbalixKit` OK.
- [x] CA03: `cargo test --manifest-path src-tauri/Cargo.toml` OK.
- [x] CA04: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` OK.
- [x] CA05: `cargo fmt --manifest-path src-tauri/Cargo.toml` aplicado.
- [x] CA06: `npm test` e `npm run build` OK (regressão desktop).
- [x] CA07: `deno test supabase/functions/transform/contract_test.ts` OK.

### Edge Cases
- EC01: Texto multibyte que estoura o body de 64 KiB ANTES do limite de 12.000 scalars.
- EC02: Resposta com `requestId` divergente → invalidResponse.
- EC03: improve com `targetLanguage != null` → invalidResponse; translate com `targetLanguage == null` → invalidResponse.
- EC04: Config Info.plist parcial (só URL, sem key) → nil.
- EC05: Arquivo de preferências corrompido → erro, não reset.
- EC06: Upsert com dois `updated_at` → o mais novo vence.
- EC07: Sync remoto offline → load/save do desktop continuam funcionando com `settings.json`.

## 🏗️ DESIGN

### Padrões Utilizados
- Espelhamento de contrato: Swift Codable espelha `contract.ts` byte-a-byte no wire (camelCase para
  transform, snake_case para tabelas REST). Justificativa: o servidor é a fonte da verdade e já é testado.
- Injeção de dependência para testabilidade: `TransformClient`/`HistoryClient`/`PreferencesSync` recebem
  um transporte injetável (protocolo `HTTPTransport` sobre URLSession, com stub em teste). Justificativa:
  RNF05 exige teste sem rede; espelha o padrão de ports/adapters do Rust.
- Falha fechada em persistência corrompida: `PreferencesStore` lança em JSON inválido. Justificativa:
  replica decisão deliberada de `settings_file.rs` (não descartar preferências do usuário silenciosamente).
- Escrita atômica `.tmp`+rename: espelha `settings_file.rs`.
- Adapter remoto opcional no macOS: novo `remote_preferences.rs` na camada application, seguindo o estilo
  de `supabase.rs`/`auth_refresh.rs` (headers `apikey`+`Authorization`, `error_for_status`, mapeamento p/
  `VerbalixError`). Justificativa: mantém a hexagonal — domain/platform intocados.

### Fluxo de Dados
- Transform (iOS): validação local → POST `/functions/v1/transform` → parse erro/resposta → validação → resultado.
- Preferências (macOS save): `save_settings` grava `settings.json` (fonte de verdade) → dispara upsert
  remoto best-effort (não-fatal). load: lê `settings.json` → tenta fetch remoto → merge LWW por `updated_at`
  apenas nos campos de IA → nunca falha por rede.

### Interfaces/Contratos (Swift)
- `enum TransformOperation/LengthPreference/TonePreference: String, Codable`
- `struct TransformPreferences { formality: Int; length; tone }`
- `struct TransformRequest { requestId: UUID; operation; text; preferences: TransformPreferences? }`
- `struct TransformResponse { requestId: UUID; sourceLanguage: String; targetLanguage: String?; result: String }`
- `enum VerbalixError { ...serverCodes...; providerNotConfigured; loginRequired; transport; localFailure }`
  com `init?(serverCode: String)`.
- `struct BackendConfig { supabaseURL; anonKey; transformEndpoint; historyEndpoint; preferencesEndpoint; userEndpoint }`
- `protocol HTTPTransport { func send(_:) async throws -> (Data, HTTPURLResponse) }`
- `struct SyncedPreferences: Codable { formality; length; tone; historyEnabled; updatedAt }`

### Interfaces/Contratos (Rust)
- `remote_preferences.rs`: `struct RemotePreferencesRepository { client, base_url, anonymous_key }` com
  `async fn fetch(access_token) -> Result<Option<RemotePreferences>, VerbalixError>` e
  `async fn upsert(&self, prefs, access_token) -> Result<(), VerbalixError>`. `RemotePreferences` cobre
  só campos de IA + `updated_at`. Merge LWW helper puro e testável isoladamente.

### Componentes Reutilizáveis
- Mock TcpListener de `supabase_history_tests.rs` → base para testes Rust de `remote_preferences`.
- Fixtures derivadas de `contract_test.ts` → testes Swift do contrato.
- Estilo de `20260723000000_transform_history.sql` → nova migration.

## 📝 TASKS

### Fase 1 — VerbalixKit (Swift)
- [x] T1.1: [LOW] `Package.swift` (iOS 17 + macOS 14, target VerbalixKit + testes, zero deps).
- [x] T1.2: [MEDIUM] `Models/Transform.swift` (Codable + validação local, guardas 12k scalars e 64 KiB).
- [x] T1.3: [LOW] `Models/VerbalixError.swift` (+ `init?(serverCode:)`).
- [x] T1.4: [LOW] `Localization/ErrorMessages.swift` (pt-BR consistente com desktop).
- [x] T1.5: [MEDIUM] `Networking/BackendConfig.swift` (Info.plist + init explícito; nil-safe).
- [x] T1.6: [MEDIUM] `Networking/TransformClient.swift` (HTTPTransport, 20s, parse erro por code, validação resposta).
- [x] T1.7: [MEDIUM] `Networking/HistoryClient.swift` (insert/list/delete/delete-all + user_id).
- [x] T1.8: [MEDIUM] `Session/SharedSessionStore.swift` (Keychain, access group injetável, save/load/clear).
- [x] T1.9: [MEDIUM] `Preferences/PreferencesStore.swift` (JSON atômico App Group, defaults, falha-fechada).

### Fase 2 — Sync de preferências
- [x] T2.1: [LOW] Migration `user_preferences` (RLS owner-only SELECT/INSERT/UPDATE).
- [x] T2.2: [MEDIUM] `Preferences/PreferencesSync.swift` (fetch + upsert, merge LWW).
- [x] T2.3: [MEDIUM] `remote_preferences.rs` (adapter application, merge LWW puro).
- [x] T2.4: [MEDIUM] Wiring macOS em `mod.rs`/`runtime.rs`/`lib.rs`/`commands.rs` (não-fatal, macOS-only preservado).

### Fase 6 (parcial) — Testes
- [x] T6.1: [MEDIUM] Testes Swift do contrato (fixtures de `contract_test.ts`; improve/translate; targetLanguage).
- [x] T6.2: [MEDIUM] Testes Swift das guardas (12k scalars, 64 KiB multibyte).
- [x] T6.3: [MEDIUM] Testes Swift mapeamento ErrorCode→VerbalixError→pt-BR.
- [x] T6.4: [MEDIUM] Testes Swift `TransformClient` com stub p/ 200/401/413/429/504/500/corpo inválido.
- [x] T6.5: [MEDIUM] Testes Swift `PreferencesStore` (round-trip, defaults, falha-fechada) + serialização upsert.
- [x] T6.6: [MEDIUM] Testes Rust `remote_preferences` (mock TcpListener): falha não bloqueia; macOS-only preservado; merge LWW.

### Documentação
- [x] T7.1: [LOW] `docs/009-verbalix-ios-fundacao.md` em português.

## Análise Dual

### 🟢 Oportunidades incorporadas (downsideup)
- O1: Reusar o mock HTTP `TcpListener` de `supabase_history_tests.rs` (bind em `127.0.0.1:0`, porta efêmera,
  `read_request`/`respond`) como base dos testes Rust de `remote_preferences` (T6.6). Zero dependência nova.
- O2: Derivar as fixtures Swift do contrato dos MESMOS literais de `contract_test.ts` (T6.1) para evitar drift.
- O3: Copiar a forma exata das políticas RLS de `20260723000000_transform_history.sql`
  (`(select auth.uid()) = user_id`) — evita reintroduzir o problema de performance de `auth.uid()` por linha.
- O4: `remote_preferences.rs` nasce como sibling de `auth_refresh.rs` (~90-120 linhas), reaproveitando o
  padrão de headers `apikey` + `Authorization: Bearer` e `error_for_status()`.
- O5: `HTTPTransport` injetável em Swift é entregável de primeira classe: base testável e sem rede para as
  Fases 3-5 futuras. Elevado a decisão de DESIGN.

### 🔴 Riscos mitigados (upsidedown) — AMENDAS OBRIGATÓRIAS
- M1 (CRÍTICO — async): `load_settings`/`save_settings` hoje são `fn` sync. Verificado: frontend usa
  `invoke()` (Promise, transparente a async); NÃO há chamador Rust direto além do `invoke_handler` em
  `lib.rs`. DIRETRIZ:
  - `save_settings` PERMANECE com semântica não-bloqueante: grava `settings.json` primeiro (fonte da
    verdade), re-registra o shortcut, e SÓ ENTÃO dispara o upsert remoto como tarefa DETACHED
    (`tauri::async_runtime::spawn`). O upsert NUNCA precede/atrasa a re-registração do shortcut.
  - `load_settings` vira `async fn`: lê `settings.json` primeiro; tenta fetch remoto com timeout curto;
    faz merge LWW só dos campos de IA; persiste o merge quando o remoto vence (próximas leituras instantâneas);
    QUALQUER falha/timeout → retorna o valor local. Nunca propaga erro de rede.
- M2 (CRÍTICO — timeout): `remote_preferences.rs` DEVE usar timeout explícito e estrito (`Client::builder().timeout`)
  de 4s (mais curto que os 8s do histórico), pois settings estão no caminho de bootstrap. Nunca herdar o
  `Client::new()` sem timeout de `auth_refresh.rs`.
- M3 (CRÍTICO — entitlement Keychain/App Group no `swift test`): bare `swift test` no host NÃO tem
  entitlement de keychain-access-groups nem de App Group. DIRETRIZ:
  - `SharedSessionStore`: separar via protocolo `SessionPersisting` (real Keychain + double em memória).
    Os testes exercitam APENAS o double em memória e a serialização; o caminho real de Keychain com access
    group NÃO é executado em CI (compila, mas não roda, ou pula com graceful skip em `errSecMissingEntitlement`).
  - `PreferencesStore`: o diretório é injetável (já em RF09); os testes usam `tmp` dir e NUNCA
    `FileManager.containerURL(forSecurityApplicationGroupIdentifier:)`.
- M4 (guardas 64 KiB × 12k scalars): verificado que `index.ts` NÃO tem checagem de tamanho de corpo — o
  guard de 64 KiB é um pré-check DEFENSIVO do cliente (definido pela tarefa), avaliado sobre o corpo JSON
  serializado. As duas guardas são INDEPENDENTES (não if/else): validar vazio → depois avaliar AMBAS
  (12k scalars E 64 KiB do body serializado). T6.2 inclui fixture multibyte (ex.: emoji ZWJ / CJK) que
  estoura 64 KiB ANTES de 12k scalars, provando que a segunda guarda dispara.
- M5 (LWW `updated_at` server-authoritative): a migration define `updated_at timestamptz not null default now()`
  e um trigger `BEFORE INSERT/UPDATE` que força `now()` no servidor, ignorando valor do cliente. O merge LWW
  (Rust e Swift) trata `updated_at` ausente/nulo do remoto como "infinitamente antigo" (local vence) e tem
  tie-break determinístico (empate → local mantém). Testes cobrem empate e `updated_at` nulo.
- M6 (RLS negativo): adicionar critério de aceite — a migration deve ter políticas SELECT/INSERT/UPDATE
  owner-only; documentar no doc de entrega que cross-user é negado por RLS (teste negativo real é gate
  manual de Supabase, fora do CI local, mas a forma da policy espelha a de `transform_history`).
- M7 (primeiro sync / linha remota ausente): fetch sem linha remota ⇒ o local (`settings.json`) é a verdade
  inicial e é enviado via upsert; nunca sobrescreve local com defaults do servidor inexistente.
- M8 (campos de IA enumerados): "campos de IA" = EXATAMENTE `formality`, `length`, `tone`, `history_enabled`.
  `shortcut`, `automatic_toolbar`, `confirm_before_replace` são macOS-only e NUNCA saem/entram pelo sync.
  Um teste Rust garante que um fetch remoto não altera os campos macOS-only.

### Reescopo de tarefas (pós-análise)
- T1.2 elevada para MEDIUM-HIGH (Codable + duas guardas com semântica multibyte).
- T2.1 elevada para MEDIUM (RLS + trigger server-side de `updated_at` + índice).
- T2.4 dividida: T2.4a `load_settings` async + audit de chamadores (frontend/e2e), spawn detached no
  `save_settings`; T2.4b wiring do adapter em `mod.rs`/`runtime.rs`/`lib.rs` (endpoint via `backend_config`).
- T6.4 inclui explicitamente a construção do harness `HTTPTransport` stub antes dos 7 cenários.
