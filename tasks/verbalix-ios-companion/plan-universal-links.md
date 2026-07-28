# Plano — Verbalix iOS: Universal Links + AASA + Política de Privacidade

> Continuação no worktree `.worktrees/verbalix-ios-companion`. NÃO merge para main.
> Contexto de produção: magic link caiu em `http://localhost:3000/?error=access_denied&error_code=otp_expired`
> porque `verbalix-ios://auth/callback` NÃO está na allow-list do Supabase → o serviço descarta o `redirect_to`
> e usa o Site URL (`http://localhost:3000`). Decisão: migrar para Universal Links em `app.verbali.xyz`,
> MANTENDO o custom scheme como fallback (o domínio ainda não está hospedado — TLS inválido hoje).
> LIÇÃO: build verde não prova comportamento correto (o bug do `//` passou por 12 gates) — verificar o
> VALOR FINAL COMPILADO (entitlements no `.app`, CFBundleURLTypes, AASA parseável).

## 🎯 SCOPE

### Arquivos Afetados
- `ios/project.yml` (Associated Domains no bloco `entitlements.properties` do target `Verbalix`)
- `ios/VerbalixKit/Sources/VerbalixKit/Session/AuthService.swift` (callbackURL https + handleDeepLink aceitando 2 formas)
- NOVO `ios/VerbalixKit/Sources/VerbalixKit/Session/AuthCallback.swift` (classificador PURO e testável de URL)
- `ios/VerbalixKit/Sources/VerbalixKit/Models/VerbalixError.swift` + `Localization/ErrorMessages.swift` (erro de callback expirado/ inválido, pt-BR)
- `ios/Verbalix/AppSession.swift` (SURFACE do erro — hoje há `catch {}` silencioso)
- NOVO `ios/VerbalixKit/Tests/VerbalixKitTests/AuthCallbackTests.swift`
- NOVO `ios/hosting/apple-app-site-association` (sem extensão)
- NOVO `ios/hosting/privacy.html`
- `docs/011-verbalix-ios-submissao.md` (passo a passo de hospedagem/allow-list)

### Fora do Escopo
- Hospedar/servir `app.verbali.xyz` (premissa: hoje é parking Sedo sem TLS — é ação do usuário).
- Alterar allow-list do Supabase (ação do usuário; documentar).
- NÃO commitar `ios/Local.xcconfig` (Team ID lá é gitignored). Team ID literal SÓ no AASA.
- macOS `verbalix://auth/callback` permanece intocado.

### Riscos de Impacto
- R1 (CRÍTICO): trocar o scheme pelo Universal Link deixaria o usuário SEM login enquanto o domínio não é hospedado. Mitigar: aceitar AMBAS as formas; scheme continua registrado.
- R2: `entitlements` do project.yml é GERADO por xcodegen (sobrescreve o `.entitlements` commitado). Associated Domains DEVE ir em `entitlements.properties` do project.yml.
- R3: `AppSession.handleDeepLink` tem `catch {}` — erro silencioso. O caso `otp_expired` precisa virar mensagem pt-BR visível.
- R4: comportamento real de `AuthClient.session(from:)` do supabase-swift com URL de erro/host custom é incerto — VERIFICAR na lib, não assumir; preferir classificar a URL antes e usar `exchangeCodeForSession(authCode:)`.
- R5: Team ID em arquivo versionado — permitido SÓ no AASA (Team IDs são públicos).

## 📋 REQUIREMENTS
- [ ] RF1: Associated Domains `applinks:app.verbali.xyz` no target `Verbalix` (via `entitlements.properties` do project.yml). Verificado no `.app` COMPILADO (`codesign -d --entitlements -`).
- [ ] RF2: `AuthService.callbackURL` = `https://app.verbali.xyz/auth/callback` (redirect enviado ao Supabase).
- [ ] RF3: `handleDeepLink` aceita AMBAS: `https://app.verbali.xyz/auth/callback?...` e `verbalix-ios://auth/callback?...`. `CFBundleURLTypes` com `verbalix-ios` PERMANECE no Info.plist.
- [ ] RF4: Classificador PURO `AuthCallback.parse(_ url:) -> AuthCallbackOutcome` (`.code(String)` | `.failure(VerbalixError)`), testável sem rede: aceita host/path corretos para as 2 formas; rejeita host errado, path errado, ausência de `code`; mapeia `?error=...&error_code=otp_expired` para um `VerbalixError` com mensagem pt-BR clara ("O link de acesso expirou ou é inválido. Solicite um novo link."). `handleDeepLink` usa o outcome (e `exchangeCodeForSession` para o code), e propaga o erro.
- [ ] RF5: `AppSession.handleDeepLink` deixa de engolir o erro — expõe estado de erro (mensagem pt-BR) para a UI; sem crash, sem silêncio.
- [ ] RF6: `ios/hosting/apple-app-site-association` (sem extensão): `applinks.details[].appIDs = ["GNVWRB9T3G.com.verbalix.ios"]`, components/paths restritos a `/auth/callback` (NÃO `*`). JSON parseável por `jq .`.
- [ ] RF7: `ios/hosting/privacy.html` estático autocontido (CSS inline, sem CDN), responsivo, pt-BR, FACTUAL derivado do código (ler antes de escrever): texto enviado só sob ação explícita → Edge Function própria → OpenAI; chave OpenAI nunca no cliente; histórico só se `historyEnabled`, owner-only via RLS, expira 30 dias; texto selecionado NÃO é logado (invariante testado no macOS); justificativa do Acesso Total do teclado (rede); como excluir dados; contato com placeholder ÓBVIO `[SEU-EMAIL-DE-CONTATO]`.
- [ ] RF8: `docs/011` atualizado com a ordem correta de hospedagem/allow-list e validação pós-hospedagem.

### Critérios de Aceitação (Gates — SAÍDA REAL)
- [ ] CA1: `bash ios/scripts/bootstrap.sh` OK.
- [ ] CA2: `xcodebuild build` dos 3 schemes (`iPhone 17 Pro,OS=26.5`) BUILD SUCCEEDED.
- [ ] CA3: `swift test --package-path ios/VerbalixKit` verde (com AuthCallbackTests).
- [ ] CA4: `jq . ios/hosting/apple-app-site-association` OK.
- [ ] CA5: do `.app` COMPILADO: `codesign -d --entitlements -` contém `applinks:app.verbali.xyz`; `CFBundleURLTypes` com `verbalix-ios` presente no Info.plist compilado.
- [ ] CA6: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings` OK.
- [ ] CA7: `npm test`, `npm run build` OK.

### Edge Cases (cobertos por AuthCallbackTests)
- EC1: `https://app.verbali.xyz/auth/callback?code=abc` → `.code("abc")`.
- EC2: `verbalix-ios://auth/callback?code=abc` → `.code("abc")`.
- EC3: host errado (`https://evil.com/auth/callback?code=x`) → `.failure`.
- EC4: path errado (`https://app.verbali.xyz/wrong?code=x`) → `.failure`.
- EC5: sem `code` → `.failure`.
- EC6: `?error=access_denied&error_code=otp_expired` → `.failure` com pt-BR de link expirado (o caso real do usuário).

## 🏗️ DESIGN
- Classificador puro desacopla parsing/erro da rede e do supabase-swift, permitindo os 6 edge cases sem rede. `handleDeepLink`: `switch AuthCallback.parse(url) { case .code(c): try exchangeCodeForSession(c); case .failure(e): throw e }`. VERIFICAR na lib o método real de troca de code (PKCE) antes de fixar.
- Associated Domains via `entitlements.properties` (fonte da verdade; o `.entitlements` é regenerado). O custom scheme continua no `CFBundleURLTypes` (Info.plist), garantindo fallback.
- AASA sem wildcard: `components: [{ "/": "/auth/callback" }]` (ou `paths: ["/auth/callback"]`) — restrito.
- Verificação de artefato compilado obrigatória (lição do `//`): entitlement e URL types lidos do `.app`, não do YAML.
- Erro de callback: novo caso em `VerbalixError` (ex.: `.authLinkExpired`) OU reuso semântico, com mensagem pt-BR dedicada; NÃO reusar mensagem genérica que esconda a causa (link expirado).

## 📝 TASKS
- [ ] T1a: [MEDIUM] `AuthCallback.swift` puro + caso de erro em VerbalixError/ErrorMessages (pt-BR).
- [ ] T1b: [MEDIUM] `AuthService` (callbackURL https + handleDeepLink via classificador + exchangeCodeForSession) e Associated Domains no project.yml/entitlements.
- [ ] T1c: [MEDIUM] `AppSession` expõe erro (remove `catch {}`) + UI mostra a mensagem.
- [ ] T1d: [MEDIUM] `AuthCallbackTests` cobrindo EC1-EC6.
- [ ] T2: [LOW] `ios/hosting/apple-app-site-association` (jq-válido, restrito a /auth/callback) + requisitos de hospedagem no docs.
- [ ] T3: [MEDIUM] `ios/hosting/privacy.html` factual derivado do código.
- [ ] T4: [LOW] `docs/011` passo a passo (hospedar → AASA → privacy → allow-list Supabase mantendo `verbalix://` → Site URL fallback → validação/device).
- [ ] T5: [LOW] Verificação dos artefatos COMPILADOS (CA5) + AASA (CA4).

## CORREÇÃO CIRÚRGICA (pós-entrega) — callback de EMISSÃO configurável
> Motivo: a EMISSÃO virou https-only (`AuthService.callbackURL` hardcoded), então o fallback do custom scheme
> só existia no PARSING. Com o domínio ainda sem TLS, o usuário fica SEM caminho de login e a allow-list do
> custom scheme não adianta (esse valor nunca é emitido). Simplificação de processo justificada: mudança
> cirúrgica e bem-especificada sobre plano já aprovado; sem dual-analysis nova.
- [x] CF1: Nova chave Info.plist `VerbalixAuthCallback` no target `Verbalix`, injetada pelo `project.yml` a partir
  de uma build setting com DEFAULT `verbalix-ios://auth/callback`. IMPORTANTE: definir o default como build
  setting no `settings` do `project.yml` (vai para o `.pbxproj`, NÃO para xcconfig) — em xcconfig o `//`
  iniciaria comentário (o mesmo bug do `//`); documentar que o opt-in https, se feito via xcconfig, precisa
  escapar as barras como o `bootstrap.sh` já faz.
- [x] CF2: `BackendConfig` expõe `authCallbackURL: URL` lido de `VerbalixAuthCallback`, com FALLBACK para
  `verbalix-ios://auth/callback` quando ausente OU inválida (nunca crashar).
- [x] CF3: `AuthService.callbackURL` deixa de ser `static let` hardcoded e passa a vir da config na init.
- [x] CF4: `AuthCallback.parse` INTOCADO (aceita as duas formas).
- [x] CF5: docs/011 — como virar para Universal Links por CONFIG (sem tocar Swift) quando o domínio estiver
  hospedado; deixar explícito que a URL EMITIDA precisa estar na allow-list do Supabase (as duas durante a transição).
- Testes: default = custom scheme quando a chave ausente; chave https válida respeitada; chave INVÁLIDA cai no
  default (sem crash); `parse` continua aceitando as duas formas (regressão).
- Gate que importa: do Info.plist COMPILADO no `.app`, `VerbalixAuthCallback` == custom scheme (default).

## Análise Dual

### 🟢 Oportunidades incorporadas (downsideup)
- O1: Classificador puro segue o padrão de `ai_readiness.rs` (classificação pura antes de I/O); reusar `ErrorMessages.swift` (switch exaustivo) e `Support/VerbalixErrorMatching.swift` nos testes.
- O2: Migração ADITIVA (mantém `verbalix-ios://` no CFBundleURLTypes) evita cutover quebrado — documentar como template.
- O3: T2 (AASA), T3 (privacy.html), T4 (docs) são paralelos a T1; fatos da privacy vêm da Edge Function/migration/`diagnostics.rs` (invariante testado no macOS).
- O4: Verificação de artefato compilado (CA5) reusa `codesign -d --entitlements -` + `jq` — vale um `ios/scripts/verify-universal-links.sh`.

### 🔴 Riscos mitigados (upsidedown) — AMENDAS OBRIGATÓRIAS (verificadas no fonte do supabase-swift 2.x)
- M1 (CRÍTICO — NÃO reimplementar o exchange): `AuthClient.session(from:)` (checkout `.build/.../Auth/AuthClient.swift:927-1015`) com `.pkce` JÁ trata `error`/`error_code`/`error_description`, lança `AuthError.pkceGrantCodeExchange(message:error:code:)` (inclui `code: "otp_expired"`) e no caminho feliz faz `exchangeCodeForSession` internamente (lê o `code_verifier` single-use do próprio storage). REESCOPO: NÃO chamar `exchangeCodeForSession` manualmente. Design correto:
  - `AuthCallback.parse(url) -> AuthCallbackOutcome`: PRÉ-validação PURA e testável — valida host/path das 2 formas; lê query E fragment (espelhando `extractParams` da lib, `Internal/Helpers.swift:4-24`); detecta `error`/`error_code`/`error_description` e mapeia (ex.: `otp_expired`) para `VerbalixError` pt-BR; ausência de `code` → `.failure`. Retorna `.proceed(URL)` | `.failure(VerbalixError)`.
  - `handleDeepLink`: `switch parse(url) { case .failure(e): throw e; case .proceed(u): let s = try await client.session(from: u) }` e mapear qualquer `AuthError.pkceGrantCodeExchange` lançado (2ª rede de segurança, esp. `otp_expired`) para `VerbalixError` pt-BR. Assim os 6 EC são unit-testáveis SEM rede (via o classificador) e o exchange real fica com a lib.
- M2 (CRÍTICO — race de cold start): `ios/Verbalix/VerbalixApp.swift:12-18` anexa `.onOpenURL` DENTRO de `if let appSession`; magic link com app FECHADO (caminho mais comum) chega enquanto `appSession == nil` e é PERDIDO. AMENDA (entra em T1c): anexar `.onOpenURL` no nível do `WindowGroup` (fora do `if let`) e guardar `pendingURL` processada assim que `appSession`/sessão estiver pronta. Cobrir com teste de unidade da lógica de "pending" onde possível (o lifecycle SwiftUI em si é gate manual/device).
- M3 (CRÍTICO — gate de release / emissão https-only): `sendMagicLink` passa a emitir SEMPRE `https://app.verbali.xyz/auth/callback`. Se o build for distribuído ANTES de (a) domínio hospedado com TLS válido, (b) AASA verificado pela Apple, (c) allow-list do Supabase com a URL https — o login fica 100% quebrado, sem fallback de EMISSÃO (o fallback do plano é só de PARSING). AMENDA: adicionar CA de release BLOQUEANTE em `docs/011` (NÃO distribuir build/TestFlight até hospedagem+AASA+allow-list confirmados) e deixar explícito que enquanto isso o desbloqueio imediato do usuário é ter `verbalix-ios://auth/callback` (e depois a URL https) na allow-list. Registrar como decisão para o coordenador no relatório final.
- M4 (AASA em `.well-known/`): documentar que o AASA DEVE ser servido em `https://app.verbali.xyz/.well-known/apple-app-site-association` (recomendação atual da Apple), Content-Type `application/json`, SEM redirect, TLS válido, sem auth. O arquivo em `ios/hosting/apple-app-site-association` é a fonte; docs indicam o path de publicação.
- M5 (chave de entitlement exata): `com.apple.developer.associated-domains` = `["applinks:app.verbali.xyz"]` no `entitlements.properties` do target `Verbalix` no `project.yml` (o `.entitlements` é regenerado). Provar no `.app` via `codesign -d --entitlements -` (CA5) — ler o gerado, NUNCA o `.entitlements` do repo.
- M6 (duplo consumo do code_verifier): clicar o link 2x (single-use, `codeVerifierStorage.set(nil)` após uso) cai em "sem code verifier" → adicionar EC7 e mensagem pt-BR específica (link já utilizado / expirado) para não confundir o usuário.
- M7 (CA1-CA7 não provam produção): deixar explícito em docs/011 que gates verdes NÃO provam que o Universal Link funciona em produção (mesma lição do `//`): depende de hospedagem+AASA+TLS reais e teste em DEVICE (Universal Links não funcionam de forma confiável no simulador).

### Reescopo de tarefas
- T1a: classificador PURO de pré-validação (host/path/error/no-code, query+fragment) + caso de erro pt-BR (otp_expired e link-já-usado).
- T1b divide em T1b1 (entitlements/project.yml + regen + `codesign` check) e T1b2 (AuthService: callbackURL https + handleDeepLink via classificador + `session(from:)` + mapeamento de `AuthError`).
- T1c: remover `catch {}` + surface de erro na UI + FILA de `pendingURL` no `WindowGroup` (M2). MEDIUM-HIGH.
- T1d: `AuthCallbackTests` EC1-EC7.
- T4/docs: incluir o gate de release bloqueante (M3), path `.well-known/` (M4), e o aviso de device (M7).
