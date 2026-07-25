# [009] - Nota some ao acionar ação da toolbar (gate de ação em voo)

## Contexto
No macOS, ao selecionar texto no TextEdit e clicar em "Traduzir" na toolbar flutuante, a janela de nota abria e fechava no mesmo segundo — sem traduzir e sem exibir conteúdo.

Causa raiz (validada por leitura de código; a confirmação AX final permanece gate manual): nada suspendia o pipeline de detecção de seleção enquanto uma ação da toolbar estava em voo. Durante `transform_selection` (que leva segundos: refresh de sessão + chamada ao provider, com abort de 20s no Edge Function), os entrypoints automáticos continuavam ativos. Como a captura lê o *system-wide focused element*, o deslocamento de foco durante o clique provocava falha de captura no polling/AXObserver, que despachava `SelectionEvent::Invalidated → overlay.hide_all() → Idle`, fechando a nota e tornando `current_snapshot()` `None` (falha subsequente com `SelectionUnavailable`/`StaleSelection`).

Defeito secundário independente: `show_readiness`/`show_provider_unavailable` só exibiam a nota de erro quando havia snapshot; com estado já invalidado, o erro era silenciosamente engolido.

## Escopo
### Incluído
- Gate de "ação em voo" que suspende os entrypoints automáticos de detecção (polling, AXObserver, monitor global de mouse) enquanto `transform_selection` executa.
- Período de graça curto pós-ação para cobrir o intervalo IPC + render do frontend até a nota aparecer.
- Fallback de geometria escopado para a nota de erro (última geometria conhecida), aplicado somente durante uma ação em voo.

### Excluído
- Máquina de estados `SelectionState`, `same_target`/latest-wins core.
- `macos_geometry.rs`, captura AXUIElement, dispatcher de overlay, frontend React, auth, Edge Function, histórico.
- Confirmação do comportamento AX real (gate manual — exige bundle assinado + Acessibilidade).

## Solução Implementada

### Arquitetura
Estendeu-se `RuntimePause` (o "single gate" de runtime) com um contador atômico de ações em voo (`in_flight: AtomicUsize`), em vez de criar um mecanismo paralelo — preservando e reforçando a invariante documentada.

- **`begin_action() -> ActionGuard` (RAII):** aberto no topo de `transform_selection`, antes da checagem de readiness/snapshot. O `Drop` decrementa o contador e, ao zerar, arma um curto período de graça (relógio injetável para testes determinísticos), cobrindo o gap IPC+render (evita que uma falha de captura pós-retorno reproduza o bug). Reentrância via contador: ações sobrepostas só reabilitam a detecção quando a última terminar.
- **Composição do gate:** `run_polling`, `run_ax_observer` e o novo `run_mouse_dismiss` passam a checar `!is_action_in_flight()`. O re-check pós-debounce (150 ms) do AXObserver foi alinhado à thread de polling (`!is_paused() && !is_action_in_flight()`), fechando a janela do sleep. Mouse-dismiss é gated somente por `!is_action_in_flight()` (não por `is_paused()`), mantendo a semântica de pausa inalterada.
- **Dismiss legítimo preservado:** `dismiss_overlays` (Escape/botão da nota), tray "Pausar" e `undo` despacham `Invalidated` diretamente no coordinator, sem passar pelos entrypoints automáticos — logo, o gate não os afeta.
- **Fallback de geometria escopado:** `SelectionCoordinator` guarda `last_bounds`, atualizado ao aceitar candidate/mostrar toolbar/resultado e **limpo em `Invalidated`**. O helper `error_bounds` usa `current_snapshot().bounds` ou, somente quando `is_action_in_flight()`, `last_known_bounds()`. Assim, `ai_readiness` standalone permanece no-op sem seleção ativa (sem nota fantasma) e um dismiss legítimo durante a ação não reabre nota "zumbi" em geometria obsoleta.

### Arquivos Modificados
| Arquivo | Tipo de Mudança |
|---------|-----------------|
| `src-tauri/src/application/runtime_pause.rs` | Modificado (gate in-flight, graça, ActionGuard, run_mouse_dismiss) |
| `src-tauri/src/application/coordinator.rs` | Modificado (last_bounds, last_known_bounds, clear em Invalidated) |
| `src-tauri/src/commands.rs` | Modificado (helper error_bounds escopado) |
| `src-tauri/src/commands_transform.rs` | Modificado (begin_action em transform_selection) |
| `src-tauri/src/lib.rs` | Modificado (mouse-dismiss e AXObserver via gate; inline de trigger_shortcut) |
| `src-tauri/src/application/mod.rs` | Modificado (exports) |
| `src-tauri/src/application/coordinator_bounds_tests.rs` | Criado (testes de last_known_bounds) |
| `src-tauri/src/commands_tests.rs` | Modificado (testes de error_bounds nos ramos reais) |

## Testes
| Métrica | Valor |
|---------|-------|
| Testes Rust (cargo test) | 242 passed / 0 failed |
| Testes frontend (vitest) | 55 passed / 0 failed |
| Cobertura (native.ts + types.ts) | 100% stmts/branch/funcs/lines |
| E2E (Playwright) | 6 passed |
| Contract (deno) | 14 passed |

Cobertura específica da correção (unit Rust puros, sem AX): supressão in-flight de polling/observer/mouse-dismiss; reentrância por contador; período de graça pós-Drop via relógio injetável; `last_known_bounds` presente enquanto ativo e `None` após `Invalidated`; `error_bounds` exercitando os ramos reais (snapshot presente, fallback in-flight, e no-op sem ação); preservação de `dismiss_overlays`/pause/undo.

## Verificação de Qualidade
| Critério | Status |
|----------|--------|
| `cargo fmt --check` | OK |
| `cargo clippy -D warnings` | OK |
| `cargo test` | OK (242) |
| `npm test` | OK (55) |
| `npm run build` | OK |
| `npm run test:coverage` | OK (100%) |
| `npm run test:e2e` | OK (6) |
| `deno test contract_test.ts` | OK (14) |
| `tauri build --debug --bundles app` | OK (bundle assinado ad-hoc) |
| QA (dupla análise) | APPROVED |
| Trivy | 0 CRITICAL / 0 HIGH (3 MEDIUM = chave anônima pública, falso positivo por design) |

## Gate manual pendente (não automatizável — CLAUDE.md "Known manual gates")
O comportamento AX real exige bundle assinado + permissão de Acessibilidade e não pode ser afirmado a partir de unit/Playwright. Verificação recomendada:
1. Conceder Acessibilidade ao bundle atual (atenção a TCC stale — `docs/004`; nunca resetar TCC).
2. `export VERBALIX_DIAGNOSTICS=1` e abrir o app pelo terminal para coletar o trace.
3. No TextEdit, selecionar texto → clicar "Traduzir".
4. Correção confirmada se: a nota permanece aberta e exibe a tradução; e no trace NÃO aparece `coordinator invalidated` entre o clique e `coordinator result_ready`.
5. Não regredir o dismiss: com a nota aberta, Escape / botão fechar → nota some (`coordinator invalidated`); clique fora após a ação → some.
6. Repetir em Chrome e VS Code além do TextEdit.

---
**Verificado por:** Workflow Orchestrator
**Data:** 2026-07-25
**Status Final:** APROVADO (pendente apenas o gate manual AX descrito acima) — merge aguardando aprovação explícita do usuário.
