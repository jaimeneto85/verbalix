# Plano — Correção: janela de nota abre e fecha ao acionar ação da toolbar

Branch/worktree: `.worktrees/fix-toolbar-action-inflight-pause` (branch `fix-toolbar-action-inflight-pause`, base `a7febf8`).

## Bug reportado
No macOS, ao selecionar texto no TextEdit e clicar em "Traduzir" na toolbar flutuante, a janela de nota abre e fecha no mesmo segundo — não traduz e não exibe conteúdo.

## Hipótese de causa raiz (validada por leitura de código; falta CONFIRMAR por trace real `VERBALIX_DIAGNOSTICS=1`)
Nada suspende o pipeline de detecção de seleção enquanto uma ação da toolbar está em voo. Durante `transform_selection` (que leva segundos: refresh de sessão + chamada ao provider, com abort de 20s no Edge Function), os entrypoints automáticos continuam ativos e qualquer falha de captura AX (comum durante o clique, pois a captura lê o *system-wide focused element*) derruba o estado para `Idle` via `SelectionEvent::Invalidated → overlay.hide_all()`. Isso torna `current_snapshot()` `None`, fazendo a nota já exibida ser fechada e/ou o `transform_selection` falhar com `SelectionUnavailable`/`StaleSelection`.

> IMPORTANTE (processo): o trace real ainda NÃO foi coletado. Trate a causa raiz como hipótese principal a validar por trace, não como fato. A correção mira os 3 entrypoints automáticos porque cobri-los é barato e seguro, mas o passo de verificação manual (abaixo) é o que confirma qual efetivamente dispara.

### Enumeração COMPLETA das origens de `SelectionEvent::Invalidated`
| # | Origem | Arquivo | Automático? | Suprimir durante ação em voo? |
|---|--------|---------|-------------|-------------------------------|
| 1 | Falha de captura no polling (`SelectionUnavailable`/`ProtectedField`/`PermissionDenied`) | `lib.rs` (thread de polling) | Sim | **Sim** |
| 2 | Falha de captura no AXObserver (qualquer erro) | `lib.rs` (callback observer) | Sim | **Sim** |
| 3 | Monitor global de mouse (clique fora) | `lib.rs` + `platform/overlay.rs::install_mouse_dismiss_monitor` | Sim | **Sim** |
| 4 | `refresh_selection` com `TextTooLong` | `coordinator.rs` | Só em refresh explícito | Indireto — polling/observer já suprimidos |
| 5 | Tray "Pausar" | `lib.rs` (tray handler) | Não (usuário) | **Não** — dismiss legítimo |
| 6 | `dismiss_overlays` (Escape / botão da nota no frontend) | `overlay_commands.rs` | Não (usuário) | **Não** — dismiss legítimo |
| 7 | Fim do fluxo `undo` | `coordinator.rs::undo` | Não (usuário) | **Não** — legítimo |

**Observação-chave (confirmada pelas duas análises):** as origens legítimas (5, 6, 7) despacham `Invalidated` **diretamente** no coordinator, sem passar pelos entrypoints automáticos (1, 2, 3). Portanto, um gate aplicado somente a 1/2/3 preserva integralmente o dismiss do usuário. **Nota Apple:** `NSEvent.addGlobalMonitorForEventsMatchingMask` só recebe eventos de *outros* apps — o clique na própria toolbar (NSPanel do Verbalix) provavelmente NÃO dispara #3; #3 é gate de seguro barato, mas o gatilho real é quase certamente #1/#2.

### Defeito secundário INDEPENDENTE (confirmado por leitura)
`show_readiness` e `show_provider_unavailable` (`commands.rs:24-44`) só exibem a nota de erro `if let Some(snapshot) = current_snapshot()`. Se o estado já foi invalidado, o snapshot é `None` e o erro é silenciosamente engolido — sem feedback. Precisa de fallback de geometria, **mas com escopo restrito** (ver CRÍTICO-A abaixo).

## Síntese da Análise Dual (riscos CRÍTICOS incorporados)

**CRÍTICO-A — vazamento do fallback de geometria (do upsidedown).** `show_readiness`/`show_provider_unavailable` também são chamados por `ai_readiness` (comando invocável a qualquer momento pelo frontend, sem seleção ativa) e pelos ramos de erro de `transform_selection`. Um cache global de `last_bounds` sem escopo faria:
- `ai_readiness` standalone reabrir nota de erro "do nada" em geometria obsoleta (regressão);
- um dismiss legítimo (Escape) durante o `await` reabrir uma nota "zumbi" na seleção que o usuário acabou de fechar.
Mitigação adotada: (i) o fallback só se aplica quando `is_action_in_flight()` é verdadeiro; (ii) `last_bounds` é LIMPO ao despachar `Invalidated` — então um dismiss legítimo durante a ação zera a geometria e nenhuma nota zumbi reaparece.

**CRÍTICO-B — janela residual pós-retorno (do upsidedown).** A guarda RAII libera exatamente quando `transform_selection` retorna, mas o frontend ainda precisa receber o resultado via IPC e renderizar a nota. Uma falha de captura AX nesse intervalo ainda fecharia a nota, reproduzindo o bug. Mitigação adotada: a supressão in-flight mantém um curto período de graça após o `Drop` da guarda, com relógio injetável para testes determinísticos.

**Decisões de design resolvidas (fechando ambiguidades apontadas):**
- `ActionGuard` referencia `&runtime.pause` e vive pelo corpo `async` de `transform_selection`. Justificativa: o padrão de manter `State<'_, Arc<AppRuntime>>` vivo através de `.await` já existe hoje (`runtime.auth.refresh(&stored).await` na mesma função) e `RuntimePause` é `Sync`. Não há problema de borrow/`Send`. (Fallback, se o borrow-checker resistir: clonar `Arc<AppRuntime>` para dentro da guarda — `AppRuntime` já circula como `Arc`.)
- Mouse-dismiss é gated SOMENTE por `!is_action_in_flight()`, NÃO por `is_paused()`. Mantém a semântica atual de pausa inalterada (mouse-dismiss não está na lista de entrypoints de detecção que a pausa bloqueia).
- Trade-off latest-wins aceito: durante a ação (até ~20s no pior caso), a detecção fica congelada globalmente. É uma política "ação em andamento vence" deliberada e temporária, coberta por RF/CA de pior caso.
- Race residual check-then-act (janela nanoscópica entre o polling checar `!is_action_in_flight()` e a guarda incrementar) é aceita como risco residual conhecido dado o timing (ms vs. segundos) — documentada, não "resolvida".

**Oportunidades incorporadas (do downsideup):**
- Estender `RuntimePause` (o "single gate") em vez de criar mecanismo paralelo — menor atrito, reforça a invariante documentada.
- RAII-on-`Drop` já é idioma no repo (`OwnedAxElement`/`OwnedCfValue` em `macos_ax.rs`) — precedente in-repo.
- Fallback de geometria como helper único `fn error_bounds(runtime) -> Option<Rect>` compartilhado pelas duas funções.
- Correção 100% comprovável por unit test Rust puro (sem AX) → roda em qualquer CI, não só macOS com Acessibilidade.

## 🎯 SCOPE

### Arquivos Afetados
- [ ] `src-tauri/src/application/runtime_pause.rs` — `in_flight: AtomicUsize` + período de graça (relógio injetável), `is_action_in_flight()`, `begin_action() -> ActionGuard` + `Drop`, `run_mouse_dismiss()`; compor `!is_action_in_flight()` em `run_polling`/`run_ax_observer`.
- [ ] `src-tauri/src/lib.rs` — rotear polling, AXObserver e mouse-dismiss pelo gate.
- [ ] `src-tauri/src/commands.rs` — abrir a guarda no início de `transform_selection`; helper `error_bounds` com fallback escopado por `is_action_in_flight()` em `show_readiness`/`show_provider_unavailable`.
- [ ] `src-tauri/src/application/coordinator.rs` — `last_bounds: Mutex<Option<Rect>>`, atualizar ao aceitar candidate/toolbar/resultado, LIMPAR em `Invalidated`, expor `last_known_bounds()`.
- [ ] Testes: `runtime_pause.rs`, `coordinator_tests.rs`, `commands_tests.rs`.

### Fora do Escopo
- Não alterar `SelectionState`/`same_target`/latest-wins core.
- Não tocar `macos_geometry.rs`, AXUIElement capture, dispatcher de overlay, frontend React.
- Não alterar auth, Edge Function ou histórico.

### Riscos de Impacto
- Supressão excessiva → toolbar não some ao trocar de seleção durante a ação. Aceito: janela de segundos, RAII + graça curta.
- Invariante "overlay work on main thread": o gate só lê `AtomicUsize`/deadline antes de despachar `Invalidated`; NÃO toca AppKit. Sem risco.
- Invariante latest-wins: coordinator continua revalidando via `same_target`; gate apenas congela detecção temporariamente. Sem risco de resposta stale aplicada.

## 📋 REQUIREMENTS

### Requisitos Funcionais
- [ ] RF01: Enquanto `transform_selection` está em voo, polling, AXObserver e mouse-dismiss NÃO despacham `Invalidated`.
- [ ] RF02: A guarda é aberta antes da captura do snapshot em `transform_selection` e fechada quando o comando retorna (Ok OU Err), via RAII.
- [ ] RF03: Dismiss legítimo permanece funcional durante e após a ação: `dismiss_overlays` (Escape/botão da nota), tray "Pausar" e `undo`.
- [ ] RF04: O fallback de geometria da nota de erro é aplicado SOMENTE quando `is_action_in_flight()`. `ai_readiness` standalone (sem ação) mantém no-op quando `current_snapshot()` é `None` — SEM nota fantasma. [CRÍTICO-A]
- [ ] RF05: Ações reentrantes de `transform_selection` só reabilitam a detecção quando a ÚLTIMA terminar (contador atômico, não flag).
- [ ] RF06: `last_bounds` é LIMPO ao despachar `Invalidated` — dismiss legítimo durante a ação não reabre nota em geometria obsoleta. [CRÍTICO-A]
- [ ] RF07: A supressão in-flight mantém um curto período de graça após o `Drop` da guarda (cobre o gap IPC+render), com relógio injetável para testes determinísticos. [CRÍTICO-B]
- [ ] RF08: Mouse-dismiss é gated SOMENTE por `!is_action_in_flight()` — NÃO por `is_paused()`. Semântica de pausa inalterada. [decisão de escopo]

### Requisitos Não-Funcionais
- [ ] RNF01: Nenhuma operação AppKit/overlay fora do dispatcher main-thread.
- [ ] RNF02: Sem regressão de latest-wins/revalidação.
- [ ] RNF03: Sem log/retorno de texto selecionado ou segredos.
- [ ] RNF04: Arquivos < ~300 linhas; sem comentários no código (nomes/testes documentam a distinção `is_paused` vs `is_action_in_flight`).
- [ ] RNF05: Todos os gates verdes.

### Critérios de Aceitação
- [ ] CA01: Unit prova que a guarda in-flight suprime a ação de detecção e reabilita ao liberá-la (após a graça).
- [ ] CA02: Unit prova reentrância: duas guardas ativas; liberar uma mantém supressão; liberar a segunda + expirar graça reabilita.
- [ ] CA03: Coordinator: `last_known_bounds()` retorna a última geometria enquanto ativo, e retorna `None` após `Invalidated` (limpeza — CRÍTICO-A/RF06).
- [ ] CA04: Commands: fallback de erro usa geometria SOMENTE com `is_action_in_flight()`; com snapshot `None` e SEM ação em voo, nenhuma nota é exibida (RF04). Com ação em voo e `last_bounds` presente, a nota de erro é exibida.
- [ ] CA05: `dismiss_overlays`, tray pause e `undo` continuam produzindo `Invalidated` (não regridem).
- [ ] CA06: Período de graça: imediatamente após o `Drop` da última guarda, `is_action_in_flight()` ainda é `true`; com relógio avançado além da graça, é `false` (RF07, determinístico via clock injetável).

### Edge Cases
- EC01: Falha de captura AX no instante do clique → suprimida.
- EC02: Escape durante a ação → dismiss funciona (não passa pelo gate) e limpa `last_bounds` → sem nota zumbi.
- EC03: Duas ações rápidas → contador impede reabilitação prematura.
- EC04: `transform_selection` retorna erro cedo (readiness/sessão) → guarda liberada; nota de erro só aparece se ainda em voo e com geometria válida.
- EC05: App pausado (tray) durante ação → pause tem precedência na detecção; mouse-dismiss segue só o in-flight.
- EC06: Rede lenta (~20s) → detecção congelada durante a ação (trade-off aceito, RF documentado).
- EC07: Janela pós-retorno (IPC+render) → coberta pela graça (RF07).

## 🏗️ DESIGN

### Padrões
- **RAII Guard** (`ActionGuard`): incrementa contador no `new`, decrementa no `Drop`; ao chegar a 0, arma o deadline de graça. Precedente in-repo: `OwnedAxElement`/`OwnedCfValue`.
- **Single gate invariant**: `RuntimePause` segue sendo o único gate de runtime, agora com `is_paused` (pausa do usuário) + `is_action_in_flight` (ação + graça). Distinção comunicada por nomes e testes (regra "sem comentários").
- **AtomicUsize** (reentrância) + deadline de graça com relógio injetável.

### Interfaces/Contratos
```rust
// runtime_pause.rs
pub struct RuntimePause {
    paused: AtomicBool,
    in_flight: AtomicUsize,
    grace_deadline: <deadline em millis atômico OU Mutex<Option<Instant>>>,
    clock: <fn injetável para "agora"; default = tempo monotônico real>,
}
impl RuntimePause {
    pub fn is_action_in_flight(&self) -> bool; // in_flight > 0 || now < grace_deadline
    pub fn begin_action(&self) -> ActionGuard<'_>;
    pub fn run_mouse_dismiss<T>(&self, action: impl FnOnce() -> T) -> Option<T>; // (!is_action_in_flight()).then(action)
    // run_polling / run_ax_observer: compor && !self.is_action_in_flight()
}
pub struct ActionGuard<'a> { pause: &'a RuntimePause }
impl Drop for ActionGuard<'_> { /* in_flight-- ; se 0, arma grace_deadline = now + GRACE */ }
```
GRACE: pequeno (sugestão ~300–500 ms) para cobrir IPC+render sem congelar a detecção perceptivelmente. Valor exato à escolha do engenheiro; o teste usa clock injetável (não `sleep`) para determinismo.

```rust
// coordinator.rs
last_bounds: Mutex<Option<Rect>>,
pub fn last_known_bounds(&self) -> Option<Rect>;
// dispatch: em Candidate/DebounceElapsed(toolbar)/ResultReady → set(bounds); em Invalidated → clear.
```

```rust
// commands.rs
fn error_bounds(runtime: &AppRuntime) -> Option<Rect> {
    runtime.coordinator.current_snapshot().map(|s| s.bounds)
        .or_else(|| runtime.pause.is_action_in_flight()
            .then(|| runtime.coordinator.last_known_bounds()).flatten())
}
// show_readiness / show_provider_unavailable usam error_bounds; se None, no-op (comportamento atual).
// transform_selection: let _guard = runtime.pause.begin_action(); no topo.
```

### Componentes Reutilizáveis
- `RuntimePause` (estende), `ActionGuard` (novo), helper `error_bounds` (um, dois callers).
- NÃO reimplementar a cadeia de cursor de `macos_geometry.rs`; `last_known_bounds` é fallback de nível de aplicação, testável.

## 📝 TASKS

### Fase 1: Gate de ação em voo (core)
- [x] T1.1: [MEDIUM] `runtime_pause.rs`: `in_flight`, graça com clock injetável, `is_action_in_flight()`, `begin_action`/`ActionGuard`/`Drop`, `run_mouse_dismiss`; compor in-flight em `run_polling`/`run_ax_observer`.
- [x] T1.2: [LOW] `lib.rs`: rotear closure do mouse-dismiss por `run_mouse_dismiss`; confirmar que polling/AXObserver herdam a supressão pelo gate composto.

### Fase 2: Guarda em transform_selection
- [x] T2.1: [LOW] `commands.rs`: `let _guard = runtime.pause.begin_action();` no topo de `transform_selection`, antes da checagem de readiness/snapshot; liberação por RAII em todos os caminhos.

### Fase 3: Fallback de geometria escopado
- [x] T3.1: [MEDIUM] `coordinator.rs`: `last_bounds` + atualização em candidate/toolbar/result + LIMPEZA em `Invalidated` + `last_known_bounds()`.
- [x] T3.2: [LOW] `commands.rs`: helper `error_bounds` (fallback escopado por `is_action_in_flight()`) usado por `show_readiness` e `show_provider_unavailable`.

### Fase 4: Testes (test-engineer)
- [x] T4.1: [MEDIUM] Unit `runtime_pause`: supressão in-flight de polling/observer/mouse-dismiss; reentrância (contador); graça pós-Drop com clock injetável (CA01/CA02/CA06).
- [x] T4.2: [MEDIUM] Unit `coordinator`: `last_known_bounds` presente enquanto ativo e `None` após `Invalidated`; latest-wins/revalidação intactos (CA03).
- [x] T4.3: [MEDIUM] Unit/integração `commands`: fallback de erro só com `is_action_in_flight()`; snapshot `None` sem ação → sem nota (RF04); `dismiss_overlays`/pause/undo ainda invalidam (CA04/CA05).

### Fase 5: Gates & QA
- [x] T5.1: Rodar todos os gates.
- [x] T5.2: QA review.

## ✅ Verificação (gates obrigatórios antes do handoff)
```
npm test
npm run test:coverage
npm run test:e2e
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
deno test supabase/functions/transform/contract_test.ts
npm run tauri -- build --debug --bundles app   # bundle smoke
```

## 🔬 Verificação manual pelo usuário (gate AX — NÃO automatizável aqui)
O comportamento AX real exige bundle assinado + permissão de Acessibilidade (não pode ser afirmado a partir de unit/Playwright — CLAUDE.md "Known manual gates"). Após instalar o bundle debug:
1. Conceder Acessibilidade ao bundle atual (atenção a TCC stale — `docs/004`; nunca resetar TCC).
2. `export VERBALIX_DIAGNOSTICS=1` e abrir o app pelo terminal para coletar o trace.
3. No TextEdit, selecionar texto → clicar "Traduzir" na toolbar.
4. **Correção confirmada se:** a nota permanece aberta e exibe a tradução; e no trace NÃO aparece `coordinator invalidated` entre o clique e `coordinator result_ready`. Sequência esperada aprox.: `detection ...` → `ai_readiness ready` → `coordinator ... processing` → `coordinator result_ready`, SEM `invalidated` no meio.
5. **Confirmar qual entrypoint disparava (diagnóstico da causa raiz):** rodar o cenário na versão ANTES do fix (ou inspecionar trace anterior) e observar se o `invalidated` vinha de `detection polling`/`detection ax_observer` (esperado #1/#2) e não de `detection mouse_dismiss` (#3, improvável).
6. **Não regredir o dismiss:** com a nota aberta, Escape / botão fechar → nota some (`coordinator invalidated`). Clique fora após a ação → some.
7. Repetir em Chrome e VS Code além do TextEdit.
