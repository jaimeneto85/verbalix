# Agent Memory — qa-reviewer

## Padrões de Qualidade do Projeto
- Gates desktop: `cargo fmt --check`, `cargo check --all-targets --all-features`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `npm test`, `npm run test:coverage`, `npm run test:e2e` e `npm run build`.
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

## Critérios de Rejeição
- Possibilidade de replace/restore em elemento diferente, mesmo dentro do PID esperado.
- Fallback que continue após `ApiDisabled`, `CannotComplete`, tipo inesperado ou estado estrutural inválido.
- Valor CF passado a `AXValueGetValue` sem type-check estrito.
- Diagnostics com texto selecionado, PID, range ou bounds.
- Arquivo modificado acima de 300 linhas efetivas.
- Trivy com vulnerabilidade HIGH/CRITICAL ou scan obrigatório não concluído.
- Conversão AX → Cocoa baseada na tela da key window em vez da zero screen.
- Overlay mostrado antes de existir garantia de que a superfície WebView transparente está pronta.
- ACK de documento antigo capaz de marcar como pronta uma WebView recriada com o mesmo label.

## Stack & Ferramentas
- Desktop Tauri 2, Rust, React/Vite e Vitest.
- Playwright cobre fluxos E2E simulados; Accessibility real permanece um gate macOS separado.
- Trivy é executado por Docker para vulnerabilidades e misconfigurações.
- `cargo-llvm-cov` não está instalado; cobertura frontend instrumentada é reportada pelo Vitest.

## Observações
- Em `aca5f44`, analyzer passou para 19 arquivos; Rust 68/68, Vitest 30/30, Playwright 3/3, cobertura frontend 100%, fmt/check/clippy/build/diff-check passaram.
- Trivy contemporâneo na mesma worktree passou com zero HIGH/CRITICAL em `package-lock` e `Cargo.lock` e zero misconfigurações.
- SonarQube não estava configurado.
- Na revisão de `a085121`, o SDK local confirmou `screens.first` como zero screen e `mainScreen` como tela da key window; a primeira revisão do overlay foi `REJECTED_CODE` apesar dos gates automatizados verdes.
- Na segunda revisão do overlay, zero screen, posicionamento AppKit em pontos, transparência e readiness foram considerados corretos, mas `src-tauri/src/lib.rs` foi modificado e ficou com 309 linhas; o veredito permaneceu `REJECTED_CODE` pelo gate objetivo de 300 linhas.
- Na terceira revisão do overlay, os gates passaram com Rust 86/86, Vitest 44/44, Playwright 5/5 e cobertura frontend configurada em 100%. O veredito permaneceu `REJECTED_CODE`: retries não tinham teto rígido e ACKs atrasados não carregavam geração do documento.
