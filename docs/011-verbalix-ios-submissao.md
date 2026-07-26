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

---

## AÇÃO DO DESENVOLVEDOR — pré-submissão

### 1. Preencher o Team ID

Abra `ios/Local.xcconfig` (gitignored) e descomente a linha:

```
DEVELOPMENT_TEAM = SEU_TEAM_ID_AQUI
```

O Team ID de 10 caracteres está no Apple Developer Portal → Account → Membership.
Sem isso, arquivamento e upload para o App Store Connect falham.

**Nota CA2**: o build de simulador (Debug) passa com Team vazio porque `CODE_SIGNING_ALLOWED = NO`
está no Debug.xcconfig. CA2 verde NÃO valida signing de Release/device — são caminhos distintos.

### 2. Registrar no Apple Developer Portal

- **Bundle IDs** (Identifiers → App IDs):
  - `com.verbalix.ios` (App principal, tipo Application)
  - `com.verbalix.ios.action` (Action Extension, tipo App Extension)
  - `com.verbalix.ios.keyboard` (Keyboard Extension, tipo App Extension)
- **App Group**: `group.com.verbalix.shared`
  - Habilitar em cada Bundle ID acima
- **Keychain Sharing**: habilitar `com.verbalix.shared` em cada Bundle ID
- **Capabilities do Keyboard Extension**: marcar "Increased Memory Limit" se aplicável

### 3. Criar o app no App Store Connect

- Criar novo app com Bundle ID `com.verbalix.ios`
- Preencher metadados: nome, subtítulo, descrição, screenshots (mínimo iPhone 6.7")
- **URL de política de privacidade** (OBRIGATÓRIA — especialmente para o teclado com Acesso Total):
  hospede uma página pública descrevendo o que é coletado e o que não é. Sem ela o app é rejeitado.

### 4. Adicionar redirect URL no Supabase (CRÍTICO — sem isso o login não funciona)

No Supabase Dashboard → Authentication → URL Configuration → Redirect URLs, adicione:

```
verbalix-ios://auth/callback
```

Veja detalhes em `docs/fix-supabase-auth-redirect.md`.

### 5. Aplicar migration `user_preferences` no Supabase

Execute a migration pendente no projeto Supabase de produção (ver `docs/010` para contexto).

### 6. Arquivar e enviar para o App Store Connect

```bash
# Após preencher Local.xcconfig com o Team ID:
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

Use Xcode Organizer ou `xcrun altool` / `xcrun notarytool` para upload.

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
| `xcodebuild` simulador (Debug) | Compilação, linking, recursos | Signing de Release, comportamento em device |
| `swift test --package-path ios/VerbalixKit` | Lógica de VerbalixKit | UI, fluxos de autenticação |
| `cargo test` + `cargo clippy` | Core Rust macOS | Nada do iOS |
| `npm test` + `npm run build` | Frontend macOS | Nada do iOS |
| Nenhum gate automatizado | Fluxo de auth real, teclado em uso, histórico real, AI transform ponta-a-ponta |

A validação manual mínima antes de submeter:
- Instalar em device físico (iPhone 17 ou similar, iOS 17+) via TestFlight ou Ad Hoc
- Testar login via magic link (e-mail → callback `verbalix-ios://auth/callback`)
- Testar Action Extension em app de terceiros
- Testar Keyboard Extension com e sem Acesso Total
- Verificar que `VerbalixSupabaseURL` no app instalado aponta para o host correto
  (bug histórico: xcconfig `//` resolvia para `https:` sem host — ver commit 4198797)
