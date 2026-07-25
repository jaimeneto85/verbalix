# Agent Memory — qa-reviewer

## Padrões de Qualidade do Projeto
- Gates desktop: `cargo fmt --check`, `cargo check --all-targets --all-features`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `npm test`, `npm run test:coverage`, `npm run test:e2e` e `npm run build`.
- Gates iOS: `swift test --package-path ios/VerbalixKit`, `xcodegen generate --spec ios/project.yml`, `xcodebuild build -scheme <Scheme> -destination 'platform=iOS Simulator,name=iPhone 17' CODE_SIGNING_ALLOWED=NO`.
- O analyzer QA impõe no máximo 300 linhas efetivas por arquivo modificado.
- Boundaries Core Foundation seguem RAII para retornos `Create`/`Copy`; decode AX valida primeiro CF type ID e AXValue subtype.
- Mutação de seleção deve falhar antes de qualquer lookup AX para snapshots read-only ou sem identidade confiável.

## Problemas Recorrentes
- Identidade baseada somente em PID, role, subrole, frame e `AXIdentifier` opcional pode colidir entre campos ou documentos sobrepostos no mesmo processo.
- Fallback AX deve preservar estágio e categoria; falhas estruturais, TCC, tipo inesperado e range vazio não autorizam migração de representação.
- Testes puros e contratos textuais não substituem o gate real de focus, apply e undo em apps macOS.
- `NSScreen.mainScreen` é a tela da key window, não a zero screen. Conversões entre coordenadas AX globais e Cocoa devem obter a referência vertical de `NSScreen.screens.first`.
- Um teste Retina que repete entradas idênticas prova apenas determinismo, não o boundary 1x/2x nem a escolha correta da referência global.
- Aplicar transparência antes do React não garante primeira pintura transparente se a janela puder ser mostrada antes do bootstrap da WebView.
- Timeout por `Promise.race` não cancela invokes nativos; ACKs de readiness precisam ser correlacionados à geração do documento, não apenas ao label da superfície.
- Limite de retries deve ser uma invariável da implementação, com teto explícito, e não somente o valor default.
- Guardas de feedback em duas etapas (`owns(snapshot_id)` seguido de publicação que relê `current_snapshot`) têm TOCTOU: um alvo novo pode entrar entre a autorização e o efeito. Ownership e publicação precisam ser correlacionados em uma única operação linearizável.
- Manter o mutex da state machine durante I/O AX ou overlay serializa Candidate/Invalidated atrás do write e exige uma política explícita para a disputa entre target supersede e commit.
- `SessionRefresher.init(config:store:appGroupID:)` chama `RefreshLock(appGroupID:)` que internamente chama `FileManager.default.containerURL(forSecurityApplicationGroupIdentifier:)`; no host de testes sem entitlement de App Group, retorna URL não-nil mas inacessível, e `Darwin.open()` com `O_CREAT` falha — o fallback `?? temporaryDirectory` nunca dispara porque nil nunca é retornado. Solução: injetar `RefreshLock` via initializer interno.

## Critérios de Rejeição
- Possibilidade de replace/restore em elemento diferente, mesmo dentro do PID esperado.
- Fallback que continue após `ApiDisabled`, `CannotComplete`, tipo inesperado ou estado estrutural inválido.
- Valor CF passado a `AXValueGetValue` sem type-check estrito.
- Diagnostics com texto selecionado, PID, range ou bounds.
- Arquivo modificado acima de 300 linhas efetivas.
- Trivy com vulnerabilidade HIGH/CRITICAL sem justificativa de falso positivo documentada, ou scan obrigatório não concluído.
- Conversão AX → Cocoa baseada na tela da key window em vez da zero screen.
- Overlay mostrado antes de existir garantia de que a superfície WebView transparente está pronta.
- ACK de documento antigo capaz de marcar como pronta uma WebView recriada com o mesmo label.
- Falha de configuração nativa após `WebviewWindowBuilder::build()` capaz de deixar janela e geração registradas para reuso sem rollback.
- Cursor global usado como geometria sem frame focado válido e associação espacial comprovada.
- Erro de uma transformação antiga capaz de ser publicado nas bounds de um snapshot novo entre check e efeito.

## Stack & Ferramentas
- Desktop: Tauri 2, Rust, React/Vite e Vitest.
- iOS: SwiftPM (VerbalixKit), XcodeGen (`ios/project.yml`), XCTest + Swift Testing; `xcodebuild` com `CODE_SIGNING_ALLOWED=NO` para CI.
- Playwright cobre fluxos E2E simulados; Accessibility real permanece um gate macOS separado.
- Trivy é executado por Docker para vulnerabilidades e misconfigurações.
- `cargo-llvm-cov` não está instalado; cobertura frontend instrumentada é reportada pelo Vitest.
- Padrão de injeção iOS: init interno (package-internal) aceita dependências pré-construídas; init público preserva a assinatura de produção — espelha o padrão `PreferencesStore`/`SessionPersisting`.
- `StubURLProtocol` intercept via `URLProtocol.registerClass` + `URLSession.shared`; deve ser registrado antes de criar URLSession e usar `@Suite(.serialized)` para evitar estado global concorrente.
- `ExternalLockHolder` usa Python3 (`/usr/bin/env python3`) para testes cross-process com `fcntl.LOCK_EX`; usa arquivo marcador para confirmar aquisição antes de retornar.

## Observações
- Em `aca5f44`, analyzer passou para 19 arquivos; Rust 68/68, Vitest 30/30, Playwright 3/3, cobertura frontend 100%, fmt/check/clippy/build/diff-check passaram.
- Trivy contemporâneo na mesma worktree passou com zero HIGH/CRITICAL em `package-lock` e `Cargo.lock` e zero misconfigurações.
- SonarQube não está configurado.
- Na revisão de `a085121`, o SDK local confirmou `screens.first` como zero screen e `mainScreen` como tela da key window; a primeira revisão do overlay foi `REJECTED_CODE` apesar dos gates automatizados verdes.
- Na segunda revisão do overlay, zero screen, posicionamento AppKit em pontos, transparência e readiness foram considerados corretos, mas `src-tauri/src/lib.rs` foi modificado e ficou com 309 linhas; o veredito permaneceu `REJECTED_CODE` pelo gate objetivo de 300 linhas.
- Na terceira revisão do overlay, os gates passaram com Rust 86/86, Vitest 44/44, Playwright 5/5 e cobertura frontend configurada em 100%. O veredito permaneceu `REJECTED_CODE`: retries não tinham teto rígido e ACKs atrasados não carregavam geração do documento.
- Na quinta revisão do overlay, Rust 88/88, Vitest 47/47, Playwright 6/6, cobertura configurada 100%, fmt/check/clippy/build/diff-check e limite de linhas passaram. Reload, UUID v4, identidade `label + NSView + generation`, retries e fail-closed foram aprovados. O veredito permaneceu `REJECTED_CODE` porque falha de `macos_overlay_panel::configure` após o build não faz rollback da janela nem da geração.
- Na sétima revisão do overlay, o compare-and-invalidate geracional foi aprovado. `invalidate_if_current` é atômico sob o mutex; reload e rollbacks carregam a geração esperada e preservam G2/B quando G1/A fica stale. Rust 93/93, Vitest 47/47, Playwright 6/6, cobertura configurada 100%, fmt/check/clippy/build/diff, limite de linhas e Trivy sem HIGH/CRITICAL passaram. Veredito `APPROVED`.
- Na revisão do fallback geométrico, a política pura `SelectedRange → Cursor contido → FocusedElement → None`, com margem zero e overflow rejeitado, foi aprovada. Rust 101/101, Vitest 47/47, Playwright 6/6, cobertura frontend configurada 100%, fmt/check/clippy/build/diff, limite de linhas e Trivy sem HIGH/CRITICAL passaram.
- Na revisão do supersede de transformação, state-first, equivalência por identidade, provider fora de ordem inerte e AX fail-closed foram aprovados; Rust 119/119 passou. O verdict foi `REJECTED_CODE`: `request_owns_feedback` autoriza em um instante, mas as funções de publicação releem snapshot global depois, permitindo publicar erro antigo no alvo novo.
- RF42 recebeu `APPROVED`: actor cancela/remove somente `Armed|Authorizing` antes do bump; Rust 229/229 e todos os gates passaram.
- `4a05e26` (RefreshLock injectable): Trivy HIGH para CVE-2026-28980 e CVE-2026-43671 (swift-nio 2.68.0) são **falsos positivos confirmados** — aparecem somente em `.build/checkouts/xctest-dynamic-overlay/IssueReporting.xcworkspace/xcshareddata/swiftpm/Package.resolved`, que é o Package.resolved do workspace interno da dependência, nunca resolúvel pelo SPM da app. `ios/VerbalixKit/Package.resolved` não contém swift-nio. 70 testes (52 XCTest + 18 Swift Testing), todos passando. Veredito `APPROVED`.
