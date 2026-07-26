# Plano — Verbalix iOS: Bloqueios de Submissão à App Store

> Continuação no worktree `.worktrees/verbalix-ios-companion` (branch `verbalix-ios-companion`). NÃO fazer merge para main.
> Contexto: o coordenador já corrigiu (commit 4198797) o bug do `//` em .xcconfig que fazia a URL do Supabase
> resolver para `https:` sem host. LIÇÃO TRANSVERSAL: build verde não prova comportamento correto — onde
> valores atravessam camadas (xcconfig → Info.plist → BackendConfig), verificar o valor FINAL COMPILADO.

## 🎯 SCOPE

### Arquivos Afetados
- `ios/Verbalix/Assets.xcassets/AppIcon.appiconset/` (NOVO: Contents.json + PNG 1024 sem alpha)
- `ios/project.yml` (asset catalog, ASSETCATALOG_COMPILER_APPICON_NAME, UILaunchScreen, versões, Release signing)
- `ios/Config/Release.xcconfig` (assinatura automática no Release)
- `ios/scripts/bootstrap.sh` (só se precisar gerar/copiar o ícone da raiz; ver T1)
- `docs/011-verbalix-ios-submissao.md` (NOVO)

### Fora do Escopo
- NÃO alterar `CODE_SIGNING_ALLOWED = NO` do Debug (é o que permite build de simulador sem Team ID).
- NÃO inventar arte nova para a Action Extension (herda o AppIcon do app).
- Keyboard Extension NÃO usa AppIcon.
- NÃO fazer merge para main. NÃO versionar segredos (`Supabase.xcconfig`/`Local.xcconfig` seguem gitignored).

### Riscos de Impacto
- R1: Ícone com canal alpha → rejeição. Verificado: master tem `hasAlpha: yes`. Mitigar achatando sobre fundo opaco.
- R2: Regredir o build de simulador com Team ID vazio ao mexer no Release. Mitigar: signing automático só na config Release; Debug intocado; validar simulador com Team vazio.
- R3: Versões divergentes entre app e extensões → rejeição no upload. Mitigar: `MARKETING_VERSION`/`CURRENT_PROJECT_VERSION` iguais nos 3 targets.
- R4: Caminho da arte (`branding/`) não existe no worktree. Mitigar: resolver a raiz via `git rev-parse --git-common-dir` (mesmo padrão do bootstrap).
- R5: Confiar no project.yml em vez do artefato compilado. Mitigar: todos os aceites leem o Info.plist/asset COMPILADO dentro de `Verbalix.app`.

## 📋 REQUIREMENTS

- [ ] RF1 (T1): `ios/Verbalix/Assets.xcassets/AppIcon.appiconset` no formato single-size do Xcode 14+ (`platform: ios`, `size: 1024x1024`, `idiom: universal`), com PNG 1024x1024 SEM alpha (achatado sobre fundo opaco). Arte de origem: `<repo-root>/branding/verbalix-app-icon-master.png` (raiz via git-common-dir). Registrar no `project.yml` e `ASSETCATALOG_COMPILER_APPICON_NAME = AppIcon` no target `Verbalix`.
- [ ] RF2 (T2): `UILaunchScreen` no Info.plist do target `Verbalix` (via `project.yml`); dicionário vazio satisfaz. Confirmar no Info.plist COMPILADO.
- [ ] RF3 (T3): `ios/Config/Release.xcconfig` com `CODE_SIGN_STYLE = Automatic` e `DEVELOPMENT_TEAM` vindo do `ios/Local.xcconfig` (`#include?` já existe). Com Team vazio o build de SIMULADOR continua funcionando (Debug intocado).
- [ ] RF4 (T3): os 3 targets com `PRODUCT_BUNDLE_IDENTIFIER`, `MARKETING_VERSION` e `CURRENT_PROJECT_VERSION` coerentes e IGUAIS entre app e extensões (app resolve 1.0/1; extensões devem bater).
- [ ] RF5 (T4): `docs/011-verbalix-ios-submissao.md` (pt-BR) com checklist honesto, separando código × ação-do-usuário; incluir todos os itens (Team ID, registro de bundle IDs/App Group/Keychain no portal, App Store Connect, URL de política de privacidade, redirect URL no Supabase, aplicar migration `user_preferences`, e o aviso explícito de que o app nunca rodou em device e o escrutínio extra do teclado com Acesso Total na App Review).

### Critérios de Aceitação (Gates — SAÍDA REAL obrigatória)
- [ ] CA1: `bash ios/scripts/bootstrap.sh` (regenera do zero) OK, sem ecoar segredo.
- [ ] CA2: `xcodebuild build -scheme Verbalix -destination 'platform=iOS Simulator,name=iPhone 17 Pro'` PASSA com `DEVELOPMENT_TEAM` vazio.
- [ ] CA3: build dos schemes `VerbalixAction` e `VerbalixKeyboard` OK.
- [ ] CA4: do Info.plist COMPILADO em `Verbalix.app`: `VerbalixSupabaseURL` (com host completo), `UILaunchScreen`, `CFBundleShortVersionString`, `CFBundleVersion` — todos presentes e corretos.
- [ ] CA5: o `.app` compilado contém `AppIcon60x60@2x.png` (ou equivalente) e `Assets.car` gerado.
- [ ] CA6: `swift test --package-path ios/VerbalixKit` OK.
- [ ] CA7: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings` OK.
- [ ] CA8: `npm test`, `npm run build` OK.

## 🏗️ DESIGN

### Decisões
- Ícone single-size (Xcode 14+ "iOS App icon — single size"): um único 1024 marcado como `platform: ios`, gerando todas as variantes no `Assets.car`. Evita manter 15+ tamanhos manualmente.
- Remoção de alpha: `sips -s format png --deleteColor`? Não — usar recomposição sobre fundo opaco (ex.: `sips` não achata alpha diretamente; usar `sips -g hasAlpha` para checar e, se necessário, achatar via `sips`/`Image I/O`/`pngcrush -rem alpha` ou compositar sobre branco). O implementador escolhe o mecanismo que comprovadamente zera `hasAlpha`, validando com `sips -g hasAlpha`.
- Versões centralizadas: definir `MARKETING_VERSION`/`CURRENT_PROJECT_VERSION` no bloco de settings comum do `project.yml` (ou repetidas idênticas nos 3 targets) para garantir paridade app↔extensões.
- Release signing isolado: `CODE_SIGN_STYLE = Automatic` só no Release.xcconfig; Debug mantém `CODE_SIGNING_ALLOWED = NO`. Com `DEVELOPMENT_TEAM` vazio, o simulador (Debug) ignora assinatura.

### Verificação de valor final (lição do coordenador)
Todo aceite que envolve config lê o ARTEFATO COMPILADO:
`plutil -p "$(find DerivedData -name 'Verbalix.app' -type d | head -1)/Info.plist"` e `find .../Verbalix.app -name 'AppIcon*'` + `ls Assets.car`.

## 📝 TASKS
- [ ] T1: [MEDIUM] AppIcon asset catalog (arte da raiz via git-common-dir, achatar alpha, Contents.json single-size, registrar no project.yml).
- [ ] T2: [LOW] `UILaunchScreen` no Info.plist do app via project.yml.
- [ ] T3: [MEDIUM] Release.xcconfig signing automático + `MARKETING_VERSION`/`CURRENT_PROJECT_VERSION` coerentes nos 3 targets; simulador segue verde com Team vazio.
- [ ] T4: [LOW] `docs/011-verbalix-ios-submissao.md` (checklist honesto código × usuário).
- [ ] T5: [LOW] Verificação dos artefatos COMPILADOS (CA4/CA5) e regeneração limpa (CA1).

## Análise Dual

### 🟢 Oportunidades incorporadas (downsideup)
- O1: Reusar `git rev-parse --git-common-dir` do `bootstrap.sh` para achar `branding/` no worktree.
- O2: Mudanças no `project.yml` são declarativas/aditivas; hospedar versões no `settings.base` (herança) garante paridade estrutural entre os 3 targets.
- O3: Single-size AppIcon (Xcode 14+) gera todas as variantes no `Assets.car` automaticamente.
- O4: `docs/011` referencia `docs/010` e `docs/fix-supabase-auth-redirect.md` em vez de duplicar gates manuais.
- O5: T2 e T4 são paralelizáveis e quase risco-zero.

### 🔴 Riscos mitigados (upsidedown) — AMENDAS OBRIGATÓRIAS
- M1 (CRÍTICO — versões são no-op): VERIFICADO que os 3 `Info.plist` gravam LITERAIS `1.0`/`1`, não `$(MARKETING_VERSION)`. Definir a build setting sozinho NÃO altera o plist compilado. AMENDA: T3 DEVE (a) editar `ios/Verbalix/Info.plist`, `ios/VerbalixAction/Info.plist`, `ios/VerbalixKeyboard/Info.plist` para usar `$(MARKETING_VERSION)` e `$(CURRENT_PROJECT_VERSION)`; (b) definir esses valores UMA vez no `settings.base` (nível projeto, independente de config) do `project.yml`. Adicionar os 3 Info.plist ao escopo de T3. CA4 deve checar o VALOR final (1.0 / 1), não só presença da chave.
- M2 (CRÍTICO — flatten de alpha): `sips` só zera alpha via round-trip JPEG (lossy — degrada o ícone permanentemente). VERIFICADO que `magick`/`convert` (ImageMagick) existem em `/opt/homebrew/bin`; `PIL` e `pngcrush` NÃO existem. AMENDA: achatar com `magick "<master>" -background white -alpha remove -alpha off "<out>.png"` (sem recompressão lossy). Validar: `sips -g hasAlpha` == `no` E `pixelWidth`/`pixelHeight` == 1024 E tamanho de arquivo sane (não minúsculo). COMMITAR o PNG achatado 1024 dentro de `ios/Verbalix/Assets.xcassets/AppIcon.appiconset/` para que o BUILD não dependa de ImageMagick (reprodutível); um script/documentação declara o ImageMagick como ferramenta de REGENERAÇÃO, não de build.
- M3 (CRÍTICO — destino de simulador ambíguo): há múltiplos `iPhone 17 Pro` e runtimes duplicados (iOS 26.3/26.4/26.5; SDK instalado 26.5). AMENDA: rodar o comando exato do coordenador (`name=iPhone 17 Pro`); se o xcodebuild reclamar de destino ambíguo/indisponível, FIXAR `OS=26.5` no `-destination` (ou usar `generic/platform=iOS Simulator` só para build). Reportar o comando efetivo usado.
- M4 (blast radius do signing): `configFiles` no `project.yml` é por-PROJETO, então `Release.xcconfig` aplica a TODOS os 3 targets no Release. Isso é desejado para archive/submissão, mas há interação com `codeSign: false` no embed das extensões. AMENDA: como os gates só buildam SIMULADOR (Debug), CA2 prova apenas que o caminho Debug/simulador continua verde — NÃO valida signing de Release/device. Documentar isso explicitamente em `docs/011` (não confundir CA2 verde com "Release signing validado"). Não tentar validar Release sem Team no CI.
- M5 (numeração de docs): re-checar `ls docs/` IMEDIATAMENTE antes de criar `docs/011` (evitar colisão com trabalho concorrente).
- M6 (Contents.json single-size): schema exato para Xcode 14+ (`idiom: universal`, `platform: ios`, `size: 1024x1024`, `filename` do PNG). Provar no build: `Assets.car` gerado e `AppIcon60x60@2x.png` (ou equivalente) presente dentro do `.app` (CA5). Um `Contents.json` malformado pode fazer o compilador PULAR o ícone sem erro — por isso o gate lê o `.app` COMPILADO.
- M7 (soft-include tolera ausência): confirmar que `#include? "../Local.xcconfig"` no Release não quebra `xcodegen generate` nem o build quando `Local.xcconfig` está com a linha do Team comentada/ausente.

### Reescopo (pós-análise)
- T3 amplia escopo: além de `project.yml`/`Release.xcconfig`, editar os 3 `Info.plist` (M1).
- T1 é o long pole (fonte da arte + flatten não-lossy + Contents.json + registro + prova no Assets.car); orçar 2-3 iterações.
- CA4 vira verificação de VALOR (diff de string), não de presença.
