# 011 — Verbalix iOS: Checklist de Submissão à App Store

> Referência: docs/010 (fundação + extensões) e docs/fix-supabase-auth-redirect.md (redirect URL).
> Este documento separa o que é **código já entregue** do que é **ação manual do desenvolvedor**.

## Aviso Importante

O app nunca rodou em device físico nem foi exercitado manualmente em nenhum fluxo de ponta a ponta.
Os gates de CI (simulador) provam build e testes unitários — não provam comportamento correto em device,
signing de Release/device, nem integração real com Supabase. Veja "Gates e o que eles NÃO cobrem" abaixo.

---

## CÓDIGO — já implementado neste escopo

| Item | Status | Arquivo |
|------|--------|---------|
| AppIcon 1024x1024 sem alpha (achatado sobre branco) | Entregue | `ios/Verbalix/Assets.xcassets/AppIcon.appiconset/` |
| `ASSETCATALOG_COMPILER_APPICON_NAME = AppIcon` no target Verbalix | Entregue | `ios/project.yml` |
| `UILaunchScreen` (dicionário vazio) no Info.plist do app | Entregue | `ios/project.yml` → `ios/Verbalix/Info.plist` |
| `CODE_SIGN_STYLE = Automatic` no Release.xcconfig | Entregue | `ios/Config/Release.xcconfig` |
| `DEVELOPMENT_TEAM` lido de `ios/Local.xcconfig` via `#include?` | Entregue | `ios/Config/Release.xcconfig` |
| `MARKETING_VERSION = 1.0` e `CURRENT_PROJECT_VERSION = 1` centralizados | Entregue | `ios/project.yml` (`settings.base`) |
| 3 Info.plist usando `$(MARKETING_VERSION)` e `$(CURRENT_PROJECT_VERSION)` | Entregue | `ios/Verbalix/Info.plist`, `ios/VerbalixAction/Info.plist`, `ios/VerbalixKeyboard/Info.plist` |
| Script de regeneração do ícone (ImageMagick) | Entregue | `ios/scripts/regen-appicon.sh` |
| Universal Links (`applinks:app.verbali.xyz`) no entitlement do target Verbalix | Entregue | `ios/project.yml` → `ios/Verbalix/Verbalix.entitlements` |
| Custom scheme `verbalix-ios://` mantido como fallback de parsing | Entregue | `ios/project.yml` (`CFBundleURLTypes`) |
| `AuthCallback.swift` — classificador puro (host/path/erro/code, query+fragment) | Entregue | `ios/VerbalixKit/Sources/VerbalixKit/Session/AuthCallback.swift` |
| `AuthService.callbackURL` lida de `BackendConfig.authCallbackURL` (configurável, default custom scheme) | Entregue | `ios/VerbalixKit/Sources/VerbalixKit/Session/AuthService.swift` |
| `BackendConfig.authCallbackURL` lê `VerbalixAuthCallback` do Info.plist (fallback: `verbalix-ios://auth/callback`) | Entregue | `ios/VerbalixKit/Sources/VerbalixKit/Networking/BackendConfig.swift` |
| `VerbalixAuthCallback` injetado no Info.plist via build setting `VERBALIX_AUTH_CALLBACK` (default no `.pbxproj`) | Entregue | `ios/project.yml` |
| `AppSession` expõe `callbackError` (remove `catch {}` silencioso) | Entregue | `ios/Verbalix/AppSession.swift` |
| `VerbalixApp` — `.onOpenURL` fora do `if let` (corrige race de cold start) | Entregue | `ios/Verbalix/VerbalixApp.swift` |
| `AuthCallbackTests` (EC1-EC7) | Entregue | `ios/VerbalixKit/Tests/VerbalixKitTests/AuthCallbackTests.swift` |
| AASA file (restrito a `/auth/callback`) | Entregue | `ios/hosting/apple-app-site-association` |
| Política de privacidade estática (pt-BR, factual) | Entregue | `ios/hosting/privacy.html` |

---

## Callback de Emissão Configurável (sem recompilar Swift)

O `AuthService` emite a callback URL lida de `BackendConfig.authCallbackURL`, que por sua vez
lê `VerbalixAuthCallback` do Info.plist. O valor padrão é `verbalix-ios://auth/callback`
(build setting `VERBALIX_AUTH_CALLBACK` no `.pbxproj`).

### Trocar para Universal Links quando o domínio estiver hospedado

**Opção A — via build setting no Xcode** (recomendado para CI/Release):

No scheme ou no xcconfig de Release, defina:

```
VERBALIX_AUTH_CALLBACK = https://app.verbali.xyz/auth/callback
```

> Atenção: se fizer via xcconfig, o `//` não é comentário porque o valor é uma URL (o `//`
> inicia comentário apenas quando aparece no início da linha ou como separador de chave/valor).
> Verifique no Info.plist compilado que o valor está correto (como o bootstrap já faz com
> `VerbalixSupabaseURL`).

**Opção B — via `ios/Local.xcconfig`** (desenvolvimento local):

Adicione ao `Local.xcconfig` (gitignored):

```
VERBALIX_AUTH_CALLBACK = https://app.verbali.xyz/auth/callback
```

### Gate obrigatório após trocar

Após alterar o build setting, verifique no Info.plist do `.app` compilado:

```bash
plutil -p "<Verbalix.app>/Info.plist" | grep VerbalixAuthCallback
# Deve mostrar: "VerbalixAuthCallback" => "https://app.verbali.xyz/auth/callback"
```

### URL Emitida deve estar na allow-list do Supabase

Durante a transição, **ambas** as URLs devem estar na allow-list:

- `verbalix-ios://auth/callback` (custom scheme, desenvolvimento / fallback)
- `https://app.verbali.xyz/auth/callback` (Universal Link, produção)

A URL emitida pelo `sendMagicLink` é a URL do build instalado. Se o build em uso emite
`verbalix-ios://auth/callback` mas ela não está na allow-list, o login falha. Se emite
`https://app.verbali.xyz/auth/callback` mas o domínio não está hospedado com TLS e AASA
válidos, o link abre no Safari em vez de no app.

---

## BLOQUEIO DE RELEASE — não distribua antes de completar esta lista

> O build de Debug/simulador usa o default `verbalix-ios://auth/callback`.
> O build de Release para TestFlight/App Store **deve** usar `https://app.verbali.xyz/auth/callback`
> (trocar via build setting conforme seção acima). Se o domínio não estiver hospedado com TLS
> válido, AASA válido e a URL na allow-list do Supabase, **o login fica 100% quebrado** em
> produção. NÃO distribua via TestFlight ou App Store até os itens 1–4 abaixo estarem confirmados.

**Desbloqueio imediato para desenvolvimento local / testes:** adicione
`verbalix-ios://auth/callback` na allow-list do Supabase. Com o default (custom scheme),
o fluxo funciona sem hospedar o domínio.

---

## AÇÃO DO DESENVOLVEDOR — ordem obrigatória pré-release

### 1. Hospedar `app.verbali.xyz` com HTTPS válido

O domínio precisa de:
- Certificado TLS válido (não auto-assinado, não expirado)
- Sem redirect de HTTP para HTTPS na rota `/.well-known/apple-app-site-association`
- Sem autenticação básica ou qualquer requisito de credencial na rota acima

### 2. Publicar o AASA em `/.well-known/apple-app-site-association`

O arquivo fonte está em `ios/hosting/apple-app-site-association`.

Servir em:
```
https://app.verbali.xyz/.well-known/apple-app-site-association
```

Requisitos obrigatórios (M4):
- `Content-Type: application/json`
- Status HTTP 200 (sem redirect — inclusive sem redirect de HTTP para HTTPS nesta rota)
- TLS válido
- Sem autenticação

Validação após publicação:
```bash
curl -sS -I https://app.verbali.xyz/.well-known/apple-app-site-association
curl -sS https://app.verbali.xyz/.well-known/apple-app-site-association | jq .
```

O `Content-Type` deve ser `application/json` e o JSON deve ser parseável pelo `jq`.

### 3. Publicar `privacy.html` e preencher o e-mail de contato

O arquivo fonte está em `ios/hosting/privacy.html`.

Antes de publicar, substitua o placeholder:
```
[SEU-EMAIL-DE-CONTATO]
```
pelo e-mail real de suporte/privacidade.

Hospede em uma URL pública estável (ex.: `https://app.verbali.xyz/privacy`).
Essa URL é obrigatória no App Store Connect (especialmente com o Keyboard Extension).

### 4. Atualizar Redirect URLs no Supabase

**Projeto Supabase:** `liuqrsuwvvaycyxeecdq`

No Supabase Dashboard → Authentication → URL Configuration → Redirect URLs, adicione:

```
https://app.verbali.xyz/auth/callback
```

**Mantenha** o `verbalix://auth/callback` (macOS) e, durante o desenvolvimento antes da
hospedagem, `verbalix-ios://auth/callback` na allow-list.

#### Sobre o Site URL

O Site URL (atualmente `http://localhost:3000`) é o **fallback** usado pelo Supabase quando
o `redirect_to` não está na allow-list. Foi ele que causou o link caindo em `localhost` no
e-mail do usuário. Após adicionar a URL https à allow-list, o Site URL deixa de ser relevante
para este fluxo — mas não precisa ser alterado agora.

### 5. Preencher o Team ID

Abra `ios/Local.xcconfig` (gitignored) e descomente a linha:

```
DEVELOPMENT_TEAM = GNVWRB9T3G
```

Sem isso, arquivamento e upload para o App Store Connect falham.

**Nota:** o build de simulador (Debug) passa com Team vazio porque `CODE_SIGNING_ALLOWED = NO`
está no Debug.xcconfig.

### 6. Registrar no Apple Developer Portal

- **Bundle IDs** (Identifiers → App IDs):
  - `com.verbalix.ios` (App principal, tipo Application)
    - Habilitar capability **Associated Domains**
  - `com.verbalix.ios.action` (Action Extension, tipo App Extension)
  - `com.verbalix.ios.keyboard` (Keyboard Extension, tipo App Extension)
- **App Group**: `group.com.verbalix.shared` — habilitar em cada Bundle ID
- **Keychain Sharing**: habilitar `com.verbalix.shared` em cada Bundle ID

### 7. Validação pós-hospedagem (antes de submeter)

```bash
# Verificar AASA
curl -sS -I "https://app.verbali.xyz/.well-known/apple-app-site-association"
curl -sS "https://app.verbali.xyz/.well-known/apple-app-site-association" | jq .

# Verificar que o Content-Type é application/json e não há redirect
```

**Teste em DEVICE físico (obrigatório — M7):**

Universal Links não funcionam de forma confiável no Simulador. A validação real exige:
1. Instalar o build assinado em um iPhone físico (iOS 17+) via TestFlight ou Ad Hoc
2. O domínio hospedado com AASA válido já resolvido pelo CDN da Apple
3. Clicar em um link `https://app.verbali.xyz/auth/callback?...` em outro app (ex.: Safari, Notes)
4. Confirmar que o iOS abre o Verbalix diretamente (sem passar pelo Safari)

Gates CI verdes (`xcodebuild`, `swift test`) **não provam** que o Universal Link funciona em
produção — provam compilação e lógica de parsing. Este é o mesmo princípio do bug do `//` que
passou por 12 gates: build verde ≠ comportamento correto em produção.

### 8. Aplicar migration `user_preferences` no Supabase

Execute a migration pendente no projeto Supabase de produção (ver `docs/010` para contexto).

### 9. Arquivar e enviar para o App Store Connect

```bash
xcodebuild archive \
  -project ios/Verbalix.xcodeproj \
  -scheme Verbalix \
  -destination generic/platform=iOS \
  -archivePath build/Verbalix.xcarchive

xcodebuild -exportArchive \
  -archivePath build/Verbalix.xcarchive \
  -exportOptionsPlist ios/Config/ExportOptions.plist \
  -exportPath build/Verbalix.ipa
```

---

## App Review — pontos de atenção do teclado com Acesso Total

O Keyboard Extension solicita `RequestsOpenAccess: true`. A App Review escrutina teclados com
Acesso Total por risco de captura de senha/dados sensíveis. Prepare-se para:

- **Justificar** o acesso à rede no campo "App Review Notes": o teclado faz chamadas ao
  Supabase Edge Function para tradução/melhoria de texto; não coleta nem armazena o texto digitado
  fora do fluxo explicitamente iniciado pelo usuário.
- **Declarar** o que não é coletado: histórico de digitação, senhas, dados de campos sensíveis.
- **Garantir** que o teclado não acessa campos marcados como `UITextContentTypePassword` ou similares.
- Ter a URL de política de privacidade aprovada e acessível antes de submeter.

---

## Gates e o que eles NÃO cobrem

| Gate | Cobre | NÃO cobre |
|------|-------|-----------|
| `xcodebuild` simulador (Debug) | Compilação, linking, recursos, entitlements no `.app` | Signing de Release, comportamento em device |
| `swift test --package-path ios/VerbalixKit` | Lógica de VerbalixKit, parsing de AuthCallback (EC1-EC7) | UI, fluxos de autenticação com rede, PKCE real |
| `codesign -d --entitlements -` no `.app` compilado | Entitlement `applinks:app.verbali.xyz` presente no binário | Validação do AASA pelo servidor Apple, roteamento real |
| `jq . ios/hosting/apple-app-site-association` | JSON válido e parseável | Hospedagem, Content-Type, TLS, ausência de redirect |
| `cargo test` + `cargo clippy` | Core Rust macOS | Nada do iOS |
| `npm test` + `npm run build` | Frontend macOS | Nada do iOS |
| Nenhum gate automatizado | Universal Link em produção, fluxo de auth real, teclado em uso |

A validação manual mínima antes de submeter:
- Instalar em device físico (iPhone 17 ou similar, iOS 17+) via TestFlight ou Ad Hoc
- Hospedar AASA antes de testar Universal Links — o iOS valida o AASA no momento da instalação
- Testar login via magic link (e-mail → callback `https://app.verbali.xyz/auth/callback`)
- Testar login com link expirado — deve exibir mensagem pt-BR ("O link de acesso expirou...")
- Testar Action Extension em app de terceiros
- Testar Keyboard Extension com e sem Acesso Total
- Verificar que `VerbalixSupabaseURL` no app instalado aponta para o host correto
  (bug histórico: xcconfig `//` resolvia para `https:` sem host — ver commit 4198797)
