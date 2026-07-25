# 010 — Verbalix iOS: App SwiftUI e Extensões (Fases 3–5)

## Escopo

Este documento descreve a arquitetura das Fases 3–5 do projeto Verbalix iOS:
- **Fase 3** — Autenticação e perfil do usuário (AuthView, AppSession)
- **Fase 4** — App SwiftUI funcional (SettingsView, HistoryView, EditorView, OnboardingView)
- **Fase 5** — Extensões de sistema (VerbalixAction, VerbalixKeyboard)

As correções da Fase 2 (LWW, sidecar, evento, migration) estão documentadas em `docs/009-verbalix-ios-fundacao.md`.

---

## Projeto Xcode (XcodeGen + Bootstrap)

O arquivo `ios/project.yml` é a fonte de verdade do projeto Xcode.
O `ios/Verbalix.xcodeproj` é derivado e **gitignored** — não o edite diretamente.

### Bootstrap

```bash
bash ios/scripts/bootstrap.sh
```

O script realiza duas tarefas:
1. Lê `VITE_SUPABASE_URL` e `VITE_SUPABASE_ANON_KEY` do `.env` na raiz do checkout (não do worktree),
   e gera `ios/Config/Supabase.xcconfig`.
2. Executa `xcodegen generate --spec ios/project.yml` para regenerar o projeto Xcode.

### Resolução do `.env` em worktrees git

A versão anterior usava `IOS_DIR/../..` que resolvia errado dentro de um worktree
(retornava `.worktrees/` em vez da raiz do checkout).

A correção usa `git rev-parse --git-common-dir`:

```bash
COMMON_GIT="$(git -C "$IOS_DIR" rev-parse --git-common-dir 2>/dev/null || true)"
if [[ "$COMMON_GIT" = /* ]]; then
  REPO_ROOT="$(dirname "$COMMON_GIT")"
elif GIT_TOPLEVEL="$(git -C "$IOS_DIR" rev-parse --show-toplevel 2>/dev/null)"; then
  REPO_ROOT="$GIT_TOPLEVEL"
else
  REPO_ROOT="$(cd "$IOS_DIR/../.." && pwd)"
fi
```

O bootstrap falha loudly se o `.env` estiver ausente ou incompleto, e nunca ecoa URL ou anon key.

---

## Compartilhamento de Sessão

App + extensões compartilham a sessão autenticada via:

| Mecanismo | Propósito |
|---|---|
| **Keychain** (accessGroup `$(AppIdentifierPrefix)com.verbalix.shared`) | Armazenamento permanente dos tokens |
| **App Group** `group.com.verbalix.shared` | Container compartilhado para arquivos (lock de refresh) |
| **`SharedSessionStore`** | Abstração sobre `KeychainSessionStore` com accessGroup |
| **`VerbalixAuthStorage`** | Implementa `AuthLocalStorage` do supabase-swift; sincroniza com `SharedSessionStore` ao salvar |
| **`SessionRefresher`** | Obtém token válido (extensões); usa `RefreshLock` para serializar refresh cross-processo |
| **`RefreshLock`** | Lock POSIX (`fcntl F_SETLK`) no container do App Group; timeout 12 s; detecta processo morto |

### Fluxo de autenticação (app principal)

1. `VerbalixApp` cria `AppSession` com `BackendConfig` lido do Info.plist.
2. `AppSession.checkSession()` chama `AuthService.currentSession()`, que consulta `AuthClient.session`.
   O `AuthClient` tenta renovar automaticamente um token expirado via `VerbalixAuthStorage`.
3. Se autenticado, `AppSession.accessToken` é preenchido e `RootView` mostra `MainTabView`.
4. Caso contrário, `RootView` mostra `AuthView`.

### Fluxo de magic link (PKCE)

1. `AuthView` chama `AuthService.sendMagicLink(email:)`.
   Internamente: `AuthClient.signInWithOTP(email:redirectTo:)` → envia e-mail com link para
   `verbalix-ios://auth/callback`.
2. O usuário toca no link → iOS entrega a URL ao app via `onOpenURL`.
3. `AppSession.handleDeepLink(_:)` chama `AuthService.handleDeepLink(_:)`.
   Internamente: `AuthClient.session(from: url)` troca o code PKCE por tokens.
4. `VerbalixAuthStorage.store(key:value:)` persiste no Keychain e chama `syncToSessionStore(_:)`,
   atualizando `SharedSessionStore`.
5. As extensões já podem usar `SessionRefresher.validAccessToken()` para acessar a API.

### Fluxo de extensões (sem AuthClient)

As extensões usam `SessionRefresher` diretamente:

```swift
let token = try await refresher.validAccessToken()
```

Se o token estiver expirado (resposta 401), a extensão pode chamar `refreshAndGetToken()`.
O `RefreshLock` serializa o refresh entre app e extensões para evitar invalidade do refresh token.

---

## Arquitetura do App SwiftUI

### `VerbalixApp`

Ponto de entrada. Cria `AppSession` com `BackendConfig` do `Bundle.main.infoDictionary`,
injeta via `.environment(session)`, e registra `onOpenURL` para o callback de autenticação.

### `AppSession` (`@Observable`, `@MainActor`)

Estado de sessão global. Expõe:
- `accessToken: String?` — token atual (ou nil se não autenticado)
- `isLoading: Bool` — enquanto verifica sessão na inicialização
- `authService: AuthService` — para operações de auth direto das views

### `RootView`

Decide entre `ProgressView`, `AuthView` e `MainTabView` com base no estado de `AppSession`.

### `MainTabView`

Quatro abas: Editor, Histórico, Ajustes, Teclado.

### `AuthView`

Estados: `idle`, `sending`, `sent`, `error(String)`.
Chama `session.authService.sendMagicLink(email:)`.

### `SettingsView`

- Slider de formalidade (1–5), pickers de tom e comprimento, toggle de histórico.
- A cada mudança, debounce de 600 ms para evitar N upserts ao arrastar o slider (M7).
- Chama `PreferencesStore.save()` localmente com `updatedAt = Date()`, depois
  `PreferencesSync.upsert()` para sincronizar com o servidor.
- Botão "Sair" chama `AppSession.signOut()`.

### `HistoryView`

- Lista itens via `HistoryClient.list(accessToken:)`.
- Swipe para copiar (coloca o resultado em `UIPasteboard`).
- Swipe para apagar item individual; botão "Apagar tudo" com confirmação.
- Estados: loading, vazio, erro, lista.

### `EditorView`

`UIViewRepresentable` sobre `UITextView` com `UIEditMenuInteraction`.
- Ao selecionar texto, o menu de edição exibe "Traduzir" e "Aprimorar".
- A ação chama `TransformClient.transform(_:accessToken:)` e substitui a seleção in-place
  via `textView.replace(selectedTextRange, withText:)`.

### `OnboardingView`

Instruções passo a passo para:
1. Habilitar o teclado Verbalix em Ajustes → Geral → Teclado.
2. Conceder Acesso Total ao teclado.
3. Usar a extensão de Ação.

Inclui botão de atalho para `UIApplication.openSettingsURLString`.

---

## Extensão de Ação (`VerbalixAction`)

- Tipo: `com.apple.ui-services` com `NSExtensionActivationSupportsText = true`.
- Lê `public.plain-text` do `NSExtensionItem.attachments`.

### Estados

| Estado | Causa | UI |
|---|---|---|
| `loading` | Transformação em andamento | `UIActivityIndicatorView` |
| `result(text)` | Sucesso | Texto + botões Copiar / Compartilhar |
| `error(.unauthenticated)` | Sem sessão | Botão "Abrir Verbalix" |
| `error(.noText)` | Sem texto selecionado | Mensagem explicativa |
| `error(.textTooLong)` | > 12.000 caracteres | Mensagem explicativa |
| `error(.timeout)` | `VerbalixError.providerTimeout` | Mensagem |
| `error(.rateLimited)` | `VerbalixError.rateLimited` | Mensagem |
| `error(.general)` | Outros erros | Mensagem genérica |

O resultado **não é inserido automaticamente** — a extensão exibe um aviso explícito e oferece
"Copiar" para que o usuário cole manualmente.

Timeout de cliente: **15 s** (abaixo do limite de 20 s da Edge Function, conforme M12).

---

## Extensão de Teclado (`VerbalixKeyboard`)

- Tipo: `com.apple.keyboard-service` com `RequestsOpenAccess = true`.

### Comportamento

- `hasFullAccess == false`: exibe banner explicativo + botão "Ajustes". Nunca falha em silêncio.
- Sem seleção (`textDocumentProxy.selectedText == nil`): exibe dica na barra.
- Com seleção: exibe botões "Traduzir" e "Aprimorar" na barra superior.
- Durante transformação: botões desativados + `UIActivityIndicatorView`.
- Ao receber resultado: apaga os caracteres selecionados e insere o texto transformado via
  `textDocumentProxy.insertText(_:)`.
- Nenhum estado é retido entre invocações (memória apertada em extensões de teclado).

Timeout de cliente: **15 s** (M12).

---

## Configuração de Supabase via xcconfig

`ios/Config/Supabase.xcconfig` é gerado pelo bootstrap a partir do `.env` raiz:

```
VerbalixSupabaseURL = https://...
VerbalixSupabaseAnonKey = ...
```

As chaves são injetadas no `Info.plist` de cada target via `$(VerbalixSupabaseURL)` e
`$(VerbalixSupabaseAnonKey)`, e lidas em runtime por `BackendConfig(infoPlist:)`.

O arquivo é **gitignored**. Nunca versione credenciais.

---

## Gates Manuais (obrigatórios antes de distribuição)

### 1. Team ID (build em device)

Copie `ios/Local.xcconfig.example` para `ios/Local.xcconfig` (gitignored) e preencha:

```
DEVELOPMENT_TEAM = XXXXXXXXXX
```

Builds de simulador funcionam sem Team ID graças a `CODE_SIGNING_ALLOWED=NO` no `Debug.xcconfig`.

### 2. Redirect URL no Supabase (magic link)

No dashboard do Supabase → Authentication → URL Configuration, adicione:

```
verbalix-ios://auth/callback
```

Sem esse passo, o magic link não redirecionará para o app. Veja também `docs/fix-supabase-auth-redirect.md`.

### 3. Habilitar o teclado no dispositivo

Em Ajustes → Geral → Teclado → Teclados → Adicionar novo teclado → Verbalix.

### 4. Conceder Acesso Total

Em Ajustes → Geral → Teclado → Teclados → Verbalix → Permitir Acesso Total.

Sem isso, `KeyboardViewController.hasFullAccess` retorna `false` e a barra exibe o banner
de aviso — o teclado nunca falhará em silêncio.

### 5. Teste em device real

O simulador iOS não executa extensões de teclado e de ação em apps de terceiros.
Para validação completa é necessário:
- Bundle assinado com Development Certificate
- Device registrado no Provisioning Portal
- App Group e Keychain Access Group configurados no portal com o Team ID correto

As funcionalidades testáveis em simulador incluem apenas a compilação e o app principal.
