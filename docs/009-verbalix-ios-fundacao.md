# 009 — Verbalix iOS: Fundação (Fases 1 e 2)

## Escopo entregue

Este documento descreve a fundação técnica do companion iOS do Verbalix, implementada nas Fases 1 e 2. As Fases 3, 4 e 5 (app SwiftUI, Action Extension, Keyboard Extension) dependem de decisão de tooling pendente.

## Fase 1 — Swift Package `VerbalixKit`

Localização: `ios/VerbalixKit/`

Plataformas suportadas: iOS 17+ e macOS 14+ (macOS somente para `swift test` no host de CI). Zero dependências externas — apenas Foundation e Security.

### Contratos espelhados

`Models/Transform.swift` espelha `supabase/functions/transform/contract.ts` byte-a-byte no wire:

- Enums camelCase: `TransformOperation`, `LengthPreference`, `TonePreference`
- `TransformRequest.validated()`: duas guardas independentes (não if/else): escalares Unicode > 12.000 → `textTooLong`; corpo JSON serializado > 65.536 bytes → `textTooLong`; `improve` sem preferências → `invalidResponse`; formalidade fora de 1...5 → `invalidResponse`
- `TransformResponse.validated(for:)`: verifica `requestId`, `sourceLanguage`, `result` não-vazio; `translate` exige `targetLanguage` não-nulo/não-vazio; `improve` exige `targetLanguage == nil`

### Erros e localização

`Models/VerbalixError.swift` cobre todos os `ErrorCode` do servidor (`init?(serverCode:)`) e casos de cliente. `Localization/ErrorMessages.swift` fornece mensagens em pt-BR consistentes com as mensagens Rust em `commands_transform.rs` e `ai_readiness.rs`.

### Rede com transporte injetável

Protocolo `HTTPTransport` injetado em `TransformClient`, `HistoryClient` e `PreferencesSync`, permitindo testes sem rede via stub. `URLSessionTransport` é a implementação padrão com timeout de 20s.

`BackendConfig` lê `VerbalixSupabaseURL` e `VerbalixSupabaseAnonKey` do Info.plist; config ausente ou incompleta retorna `nil` (nunca crash).

### Sessão e keychain

`SessionPersisting` separa o caminho real de Keychain (`KeychainSessionStore`, com `kSecAttrAccessibleAfterFirstUnlock` e access group injetável) do double em memória (`InMemorySessionStore`) usado em testes. Testes exercitam apenas o double — o caminho real com access group não roda em CI (sem entitlement).

### Preferências locais

`PreferencesStore` usa diretório injetável (em testes: `FileManager.default.temporaryDirectory`, nunca `containerURL(forSecurityApplicationGroupIdentifier:)`). Escrita atômica via `.tmp` + rename, espelhando `settings_file.rs`. JSON corrompido → lança `VerbalixError.localFailure` (falha-fechada, sem reset silencioso). Defaults espelham `AppSettings::default()` em Rust: `formality: 3, length: .balanced, tone: .technical, historyEnabled: false`.

## Fase 2 — Sync de preferências

### Migration SQL

`supabase/migrations/20260725000000_user_preferences.sql` cria a tabela `user_preferences` com:

- `user_id uuid not null unique` — chave de upsert
- `updated_at timestamptz not null default now()` + trigger `BEFORE INSERT OR UPDATE` que força `now()` no servidor (M5: cliente nunca controla o timestamp)
- RLS owner-only SELECT/INSERT/UPDATE usando `(select auth.uid()) = user_id` (mesmo padrão de `transform_history`)
- Política UPDATE ausente via `delete` — cross-user é negado por RLS

### `PreferencesSync` (Swift)

`Preferences/PreferencesSync.swift` implementa `fetch` (GET `/rest/v1/user_preferences?select=*&limit=1`) e `upsert` (POST com `Prefer: resolution=merge-duplicates,return=minimal`). O upsert omite `updated_at` — o servidor define via trigger.

`mergePreferences(local:remote:)` é uma função pura (testável isoladamente):
- `remote == nil` → local vence
- `remote.updatedAt == nil` → local vence (remoto "infinitamente antigo")
- Empate (datas iguais) → local mantém
- `remote.updatedAt > local.updatedAt` → campos de IA do remoto aplicados

### `remote_preferences.rs` (Rust)

`src-tauri/src/application/remote_preferences.rs` é o adapter na camada application, sibling de `auth_refresh.rs`. Usa `Client::builder().timeout(Duration::from_secs(4))` — mais curto que os 8s do histórico, pois settings estão no caminho de bootstrap (M2).

`merge_preferences(local, remote)` pura e testável:
- `remote == None` → local
- `remote.updated_at == None` → local (M5)
- `remote.updated_at == Some(_)` → campos de IA do remoto; campos macOS-only (`shortcut`, `automatic_toolbar`, `confirm_before_replace`) NUNCA sobrescritos (M8)

### Wiring macOS

`load_settings` foi convertido para `async fn`: lê `settings.json` → tenta fetch remoto → merge LWW → persiste se remoto vencer → retorna local em qualquer falha de rede (nunca propaga erro de rede).

`save_settings` permanece `fn` síncrona: grava `settings.json` → re-registra shortcut → **depois** dispara upsert remoto como tarefa detached (`tauri::async_runtime::spawn`). O upsert nunca atrasa a re-registração do shortcut (M1).

`AppRuntime` recebe `remote_preferences: Option<Arc<RemotePreferencesRepository>>` — `None` quando backend não configurado, mantendo compatibilidade total com o fluxo sem sessão.

## Invariantes preservados

- Falha de rede em qualquer ponto do sync → `settings.json` continua sendo a fonte de verdade
- Campos macOS-only (`shortcut`, `automatic_toolbar`, `confirm_before_replace`) nunca entram ou saem pelo sync
- Nenhum token, texto do usuário ou credencial é logado ou exposto em mensagens de erro
- `swift test` sem entitlement de Keychain/App Group: testes usam `InMemorySessionStore` e diretório temporário

## Gates verificados

| Gate | Resultado |
|------|-----------|
| `swift build --package-path ios/VerbalixKit` | Build complete! |
| `swift test --package-path ios/VerbalixKit` | 49 passed, 0 failed |
| `cargo test` | 241 passed, 0 failed |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 warnings |
| `cargo fmt --check` | limpo |
| `npm test` | 55 passed, 0 failed |
| `npm run build` | ✓ built |
| `deno test contract_test.ts` | 14 passed, 0 failed |
