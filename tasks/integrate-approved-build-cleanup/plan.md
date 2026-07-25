# Plano — integrar build aprovado e limpar worktrees

## 0. Escopo

- Auditar main, worktrees, branches, dirty state e ancestralidade.
- Integrar somente `fix-transform-action-regression` no estado aprovado `ae77b2a`.
- Excluir da integração `fix-transform-action-smoke-failure`, que contém diagnóstico incompleto.
- Preservar integralmente mudanças locais do usuário em `package.json`, `vitest.config.ts`, `CLAUDE.md` e `README.md`.
- Rodar gates proporcionais, gerar bundle macOS, assinar com o mesmo Team ID e instalar em `/Applications/Verbalix.app`.
- Remover worktrees e branches locais já integradas ou descartadas com segurança.
- Não fazer push, release, deploy remoto ou retomar o fix interrompido.

## 1. Requisitos e aceite

- [ ] RF1: Nenhuma mudança local do usuário é perdida, alterada ou incluída acidentalmente em commit.
- [ ] RF2: Main contém o HEAD aprovado `ae77b2a` e não contém commits posteriores da branch smoke.
- [ ] RF3: Gates Rust/frontend/Deno/Playwright e checks de qualidade permanecem verdes.
- [ ] RF4: Bundle instalado corresponde ao source integrado, possui `com.verbalix.desktop` e Team ID `WQ44ZM274W`.
- [ ] RF5: `/Applications/Verbalix.app` passa launch smoke sem crash.
- [ ] RF6: Todas as worktrees secundárias e branches locais associadas são removidas após preservação/auditoria.
- [ ] CA1: `git status` final exibe somente as mudanças locais originais do usuário.
- [ ] CA2: `git worktree list` final contém apenas a raiz do repositório.
- [ ] CA3: `git log` confirma aprovado incluído e smoke diagnóstico excluído.

## 2. Design

Esta tarefa simplifica o SDD completo porque não cria lógica nova: integra um commit já aprovado por QA dual e executa build/instalação. A auditoria substitui análises paralelas de implementação.

O merge ocorre na main preservando o dirty state porque os arquivos locais não são tocados pela branch aprovada. Antes da remoção, cada worktree deve estar limpa e seus commits devem estar classificados como integrados, redundantes ou diagnóstico incompleto descartável.

O bundle será reconstruído a partir da main integrada usando o `.env` local sem imprimir valores. A assinatura reutiliza a identidade de distribuição do Team ID instalado. O app atual só será substituído após build e verificação; um backup recuperável será mantido em `/private/tmp`.

## 3. Tarefas

- [x] T1 Auditar worktrees, branches, dirty state, commits e ancestralidade.
- [ ] T2 Integrar `fix-transform-action-regression` na main sem incluir a branch smoke.
- [ ] T3 Rodar gates completos e verificar arquivos/segredos/diff.
- [ ] T4 Gerar e assinar bundle macOS da main integrada.
- [ ] T5 Preservar instalação anterior, instalar e executar launch smoke.
- [ ] T6 Remover worktrees e branches locais auditadas.
- [ ] T7 Confirmar estado final e documentar a entrega.
