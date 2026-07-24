# Fix: regressão da toolbar após merge

## 0. SCOPE

Restaurar a exibição da toolbar flutuante quando texto é selecionado em apps suportados no macOS, sem regredir segurança, lifecycle, posicionamento ou escrita.

Incluído:

- diagnosticar por estágio a aquisição do app/elemento focado via Accessibility;
- preservar o caminho system-wide quando funcional e adicionar fallback seguro pelo app frontmost;
- excluir o próprio Verbalix como alvo de captura;
- manter polling, AXObserver, debounce e overlay no main thread;
- criar testes de resolução/fallback e regressão;
- validar em app real por Computer Use;
- após QA aprovada, integrar na `main` preservando mudanças user-owned e gerar build release.

Fora do escopo:

- alterar Auth, Supabase Edge Function ou prompts de IA;
- resetar TCC automaticamente;
- ampliar captura para clipboard implícito;
- mascarar falha de captura forçando a toolbar sem snapshot válido;
- remover gates manuais de permissão, assinatura e release.

## 1. REQUIREMENTS

- R1: seleção real no TextEdit deve produzir snapshot e toolbar visível.
- R2: `AXUIElementCreateSystemWide` + `AXFocusedUIElement` permanece caminho primário.
- R3: se o caminho primário falhar, resolver o app frontmost, criar seu elemento AX e consultar `AXFocusedUIElement`.
- R4: o fallback nunca captura Verbalix nem um PID inválido/ausente.
- R5: falhas precisam indicar o estágio sanitizado (`system_wide`, `frontmost_app`, `focused_element`, `selected_text`, `selected_range`) sem conteúdo selecionado.
- R6: ausência de permissão continua falhando fechada e não dispara fallback que contorne TCC.
- R7: captura bem-sucedida preserva texto, range Unicode, writability e geometria existente.
- R8: observer, polling, refresh/apply/restore usam a mesma resolução de elemento focado.
- R9: nenhum AppKit/UI é executado fora da main thread.
- R10: testes cobrem primário, fallback, self-exclusion, app/PID ausente e falha total.
- R11: teste real por Computer Use comprova seleção e toolbar; AX tree do app alvo e screenshot/estado visual formam evidência.
- R12: merge só ocorre após testes e QA `APPROVED`; mudanças locais user-owned na `main` permanecem byte-identical.
- R13: release build só ocorre após merge validado e deve passar bundle, codesign e launch smoke.

## 2. DESIGN

### Evidência reproduzida

- processo Verbalix permaneceu vivo;
- bundle instalado e bundle debug tinham o mesmo executável e assinatura válida;
- `AX trusted=true`;
- TextEdit expôs `Selected text: technical sentence`;
- polling registrou repetidamente `selection_unavailable`;
- coordinator invalidou a seleção e executou `overlay hide`;
- captura falhou antes da geometria.

Os merges recentes não alteraram captura AX, coordinator ou overlay. A causa está concentrada na aquisição do elemento focado no runtime/macOS atual, não no rendering da janela.

### Spike causal antes da correção

Antes de habilitar o fallback, instrumentar o mesmo processo/bundle para classificar cada chamada sem payload:

1. trust check;
2. criação do objeto system-wide;
3. leitura de `AXFocusedUIElement`;
4. leitura de `AXFocusedApplication`;
5. role;
6. selected text;
7. selected range;
8. PID;
9. geometry.

O spike deve demonstrar no mesmo intervalo que o caminho primário falha e que o caminho por aplicação focada retorna o elemento do TextEdit. Caso a falha esteja em outro estágio, a implementação deve parar e o plano precisa ser revisado.

### Resolver AX em dois estágios

1. tentar `AXFocusedUIElement` no objeto AX system-wide;
2. somente se o erro primário for `no_value` ou `attribute_unsupported`, e `AXIsProcessTrusted` continuar verdadeiro, consultar `AXFocusedApplication` no mesmo objeto system-wide;
3. obter o PID do elemento da aplicação focada, rejeitando PID `<= 0` e `getpid()`;
4. criar `AXUIElementCreateApplication(pid)` e consultar seu `AXFocusedUIElement`;
5. exigir que `AXUIElementGetPid(focused) == pid`;
6. retornar elemento owned e origem tipada `system_wide | focused_application`, sem persistir ou transferir o elemento entre threads;
7. prosseguir pelo pipeline atual de role, texto, range, writability e geometria.

`kAXErrorAPIDisabled`, trust falso, erro de autorização, argumento inválido, elemento inválido, `cannot_complete` e falhas estruturais não habilitam fallback. O próximo ciclo de polling pode tentar novamente; não existe retry interno ilimitado.

O caminho usa somente Accessibility/Core Foundation no worker. Não introduzir `NSWorkspace`, `NSRunningApplication` ou outra chamada AppKit fora da main thread. O caminho primário vence sempre e o fallback não deve ser consultado quando ele funciona.

### Descoberta versus mutação

O boundary de aquisição e ownership é compartilhado por captura e observer. `replace` e `restore` não podem redescobrir um alvo frontmost arbitrário:

- resolver pelo PID esperado do snapshot;
- revalidar PID, role, texto, range e writability no mesmo elemento owned imediatamente antes do set;
- se foco, PID, texto ou range divergir, retornar `StaleSelection` sem escrever;
- interação com a toolbar nunca autoriza escrever no próprio Verbalix ou em outro app.

Essa separação elimina a corrida atual entre `capture()` e uma segunda chamada independente a `focused_element()`.

### Boundary testável e ownership

Separar a função pura de decisão dos adapters FFI por contratos internos equivalentes a:

- trust provider;
- focused application/PID provider;
- focused element provider;
- resultado tipado com origem, estágio e categoria AX.

Todos os valores vindos de funções `Create`/`Copy` seguem RAII com exatamente um `CFRelease` em sucesso e em cada saída de erro. `OwnedAxElement` fica restrito à operação e à thread em que foi criado.

### Diagnóstico

Emitir somente estágio, origem tipada e categoria AX estável. Nunca emitir texto, range bruto, bundle path, token, conteúdo de clipboard ou status AX não classificado.

Estágios mínimos: `trust`, `system_wide_focused_element`, `focused_application`, `application_focused_element`, `role`, `selected_text`, `selected_range`, `pid` e `geometry`.

Os diagnostics do loop devem ser limitados por transição/categoria para não gerar spam a cada 120 ms.

### Gate real

1. buildar e instalar o bundle corrigido;
2. confirmar assinatura e processo estável;
3. confirmar permissão AX do bundle atual;
4. selecionar texto técnico no TextEdit por Computer Use;
5. correlacionar a seleção do TextEdit com diagnostic de origem, capture success, candidate/debounce e overlay visible;
6. observar toolbar com Traduzir/Aprimorar sem roubar foco;
7. repetir seleção por teclado e mouse e conferir bounds coerentes;
8. executar ação -> preview -> apply -> undo em campo editável;
9. confirmar hide ao limpar seleção;
10. alternar rapidamente de app e provar falha fechada sem escrita cross-app;
11. provar que secure field não é capturado;
12. repetir captura/hide em Notes ou Chrome;
13. confirmar que o processo permanece vivo;
14. registrar evidência sem conteúdo sensível.

### Merge e release

Após QA:

1. registrar `git status --porcelain` e hashes de todos os arquivos user-owned na `main`;
2. merge `--no-ff` da branch;
3. repetir testes críticos;
4. confirmar hashes user-owned inalterados;
5. gerar bundle release;
6. validar recursos, `codesign --verify --deep --strict` e launch smoke;
7. remover worktree/branch somente após entrega confirmada.

## Análise Dual

### 🔴 Riscos incorporados

- A evidência inicial agregava todos os erros como `selection_unavailable`; foi adicionado spike causal por estágio antes do fallback.
- O fallback agora possui matriz fechada de elegibilidade por categoria AX e nunca contorna TCC.
- A descoberta do app focado usa AX, não AppKit em worker.
- Mutação e restore ficam vinculados ao PID/snapshot esperado para eliminar TOCTOU e escrita cross-app.
- Ownership Core Foundation, coerência temporal de PID, rate limiting diagnóstico, secure fields e concorrência entraram como gates explícitos.
- A preservação da main cobre todo o dirty state user-owned, não uma whitelist fixa.

### 🟢 Oportunidades incorporadas

- Reusar o boundary `focused_element` e o RAII existentes mantém a correção concentrada.
- `AXFocusedApplication` oferece fallback nativo sem nova dependência AppKit.
- Origem tipada permite provar em runtime qual rota restaurou a toolbar.
- Polling existente absorve mudanças de app no ciclo seguinte, evitando retries internos e novos schedulers.
- Testes puros da matriz de decisão cobrem primary-wins, fallback, trust, self-exclusion e corrida com baixo custo.

### Decisão sintetizada

Implementar fallback mínimo no boundary AX somente depois da prova causal. Compartilhar aquisição/ownership em capture e observer, mas vincular replace/restore ao snapshot esperado. A correção só é aprovada com testes automatizados, QA dual e smoke real correlacionado por Computer Use.

## 3. TASKS

- [x] T1 [LOW] Criar worktree isolada e ler memória.
- [x] T2 [LOW] Reproduzir e localizar o estágio agregado da falha.
- [x] T3 [LOW] Executar análise dual do plano.
- [x] T4 [LOW] Sintetizar plano final.
- [ ] T5 [LOW] Adicionar diagnóstico tipado por estágio e executar spike causal no bundle atual.
- [ ] T6 [MEDIUM] Implementar tipos puros de decisão, origem e categoria AX.
- [ ] T7 [MEDIUM] Implementar fallback por `AXFocusedApplication` com RAII e coerência de PID.
- [ ] T8 [MEDIUM] Integrar resolver em capture e observer sem AppKit em worker.
- [ ] T9 [MEDIUM] Vincular replace/restore ao PID e snapshot esperados, eliminando TOCTOU.
- [ ] T10 [LOW] Limitar diagnostics repetidos sem conteúdo selecionado.
- [ ] T11 [MEDIUM] Testar matriz primary/fallback/trust/self/race/ownership e regressões existentes.
- [ ] T12 [LOW] Testar equivalência/debounce, secure field, range, geometry e mutation target.
- [ ] T13 [MEDIUM] Executar QA dual de código, concorrência, FFI e testes.
- [ ] T14 [MEDIUM] Build/install debug e validação real por Computer Use em TextEdit e segundo app.
- [ ] T15 [MEDIUM] Corrigir rejeições e obter verdict `APPROVED`.
- [ ] T16 [LOW] Atualizar memória e gerar documento de entrega.
- [ ] T17 [LOW] Merge autorizado preservando todo dirty state user-owned.
- [ ] T18 [MEDIUM] Gerar e validar build release, recursos, codesign e launch smoke.
