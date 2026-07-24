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

## Critérios de Rejeição
- Possibilidade de replace/restore em elemento diferente, mesmo dentro do PID esperado.
- Fallback que continue após `ApiDisabled`, `CannotComplete`, tipo inesperado ou estado estrutural inválido.
- Valor CF passado a `AXValueGetValue` sem type-check estrito.
- Diagnostics com texto selecionado, PID, range ou bounds.
- Arquivo modificado acima de 300 linhas efetivas.
- Trivy com vulnerabilidade HIGH/CRITICAL ou scan obrigatório não concluído.

## Stack & Ferramentas
- Desktop Tauri 2, Rust, React/Vite e Vitest.
- Playwright cobre fluxos E2E simulados; Accessibility real permanece um gate macOS separado.
- Trivy é executado por Docker para vulnerabilidades e misconfigurações.
- `cargo-llvm-cov` não está instalado; cobertura frontend instrumentada é reportada pelo Vitest.

## Observações
- Em `aca5f44`, analyzer passou para 19 arquivos; Rust 68/68, Vitest 30/30, Playwright 3/3, cobertura frontend 100%, fmt/check/clippy/build/diff-check passaram.
- Trivy contemporâneo na mesma worktree passou com zero HIGH/CRITICAL em `package-lock` e `Cargo.lock` e zero misconfigurações.
- SonarQube não estava configurado.
