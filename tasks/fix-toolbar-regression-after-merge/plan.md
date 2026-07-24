# Fix: regressão da toolbar após merge

## 0. SCOPE

Restaurar a exibição da toolbar flutuante quando texto é selecionado em apps suportados no macOS, sem regredir segurança, lifecycle, posicionamento ou escrita.

Incluído:

- diagnosticar por estágio a aquisição do app/elemento focado via Accessibility;
- preservar o elemento focado system-wide comprovadamente funcional e adicionar extração segura por range quando `AXSelectedText` não está disponível;
- excluir o próprio Verbalix como alvo de captura;
- manter polling, AXObserver, debounce e overlay no main thread;
- criar testes de resolução, extração por range e regressão;
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
- R3: quando `AXSelectedText` retornar `no_value` ou `attribute_unsupported`, classificar a representação do range no mesmo elemento.
- R4: para `AXTextMarkerRange`, extrair texto e bounds usando os atributos parametrizados públicos `AXStringForTextMarkerRange` e `AXBoundsForTextMarkerRange`.
- R5: falhas precisam indicar o estágio sanitizado (`system_wide`, `focused_element`, `selected_text`, `selected_range_type`, `text_marker_string`, `text_marker_bounds`, `text_marker_index`) sem conteúdo selecionado.
- R6: ausência de permissão continua falhando fechada e não dispara extração alternativa que contorne TCC.
- R7: captura bem-sucedida preserva texto, range Unicode, writability e geometria existente.
- R8: observer, polling e refresh reutilizam o mesmo elemento focado; apply/restore continuam vinculados ao PID/range/texto do snapshot.
- R9: nenhum AppKit/UI é executado fora da main thread.
- R10: testes cobrem selected text direto, CFRange clássico, text marker range, self-exclusion, PID/range inválido e falha total.
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

Os merges recentes não alteraram captura AX, coordinator ou overlay. O spike no mesmo bundle demonstrou que a aquisição do elemento focado funciona; a falha está na estratégia de leitura do texto selecionado.

### Evidência causal confirmada

Primeiro smoke:

- `AXFocusedUIElement` system-wide: sucesso;
- PID e role: sucesso;
- `AXSelectedText`: `no_value`, depois `attribute_unsupported`;
- toolbar ausente e processo estável.

Segundo smoke:

- `AXFocusedApplication` e focused element da aplicação: sucesso;
- PID primary/secondary coerente e role em ambos: sucesso;
- `AXSelectedText` falha com as mesmas categorias nos dois elementos.

Conclusão: fallback por aplicação focada foi refutado e não será implementado.

Terceiro smoke:

- fetch de `AXSelectedTextRange`: sucesso;
- interpretação como `AXValue<CFRange>`: falha nas duas rotas de foco;
- `AXStringForRange`: não elegível porque não existe CFRange comprovado.

Conclusão: não usar CFRange presumido nem ler `AXValue` do documento inteiro. A próxima sonda classifica o tipo e trata text markers como tokens opacos.

### Spike causal antes da correção

Antes de habilitar qualquer extração alternativa, instrumentar o mesmo processo/bundle para classificar cada chamada sem payload:

1. trust check;
2. criação do objeto system-wide;
3. leitura de `AXFocusedUIElement`;
4. leitura de `AXFocusedApplication`;
5. role;
6. selected text;
7. selected range;
8. PID;
9. geometry.

Os dois primeiros spikes demonstraram que o caminho primário de foco funciona e que o caminho por aplicação focada não recupera o texto. A próxima sonda, ainda read-only, deve provar:

1. classificar corretamente `AXSelectedTextRange` com `CFGetTypeID`, `AXValueGetTypeID`/`AXValueGetType` e `AXTextMarkerRangeGetTypeID`;
2. consultar `AXSelectedTextMarkerRange` quando a representação clássica não for CFRange;
3. se o marker opaco produz texto por `AXStringForTextMarkerRange`;
4. se o mesmo marker produz bounds por `AXBoundsForTextMarkerRange`;
5. se `AXTextMarkerRangeCopyStartMarker`/`CopyEndMarker` + `AXIndexForTextMarker` e `AXLengthForTextMarkerRange` fornecem location/length coerentes para `TextRange`;
6. se PID, role e foco permanecem coerentes durante toda a leitura;
7. se `AXSelectedText` é settable no alvo editável.

Se nenhuma extração por range funcionar, parar novamente e revisar o plano; não materializar seleção nem toolbar artificial.

### Extração validada por representação

1. resolver `AXFocusedUIElement` system-wide;
2. validar PID `> 0`, não-self e role não protegida;
3. tentar `AXSelectedText`;
4. se indisponível por `no_value | attribute_unsupported`, ler `AXSelectedTextRange` e classificar `RangeRepresentation::{CfRange, TextMarker, Unsupported}`;
5. para CFRange comprovado, manter `AXStringForRange` e geometry existentes;
6. para text marker comprovado, manter o marker opaco e passá-lo no mesmo elemento/thread a `AXStringForTextMarkerRange` e `AXBoundsForTextMarkerRange`;
7. copiar start/end markers via APIs públicas do SDK e obter índices parametrizados; validar location/length também contra `AXLengthForTextMarkerRange`;
8. rejeitar índice negativo, range vazio, overflow, divergência de length, tipo CF inesperado, conteúdo vazio ou bounds inválidos;
9. confirmar PID, role, foco e representação no mesmo elemento antes de materializar o snapshot.

Trust falso, API disabled/not authorized, elemento inválido, `cannot_complete`, tipo CF inesperado e falhas estruturais encerram a captura. O próximo polling pode tentar novamente; não existe retry interno ilimitado.

O caminho usa apenas APIs públicas de Accessibility/Core Foundation presentes nos headers do macOS 26.5 SDK, sem AppKit ou nova dependência. `AXSelectedText` continua o caminho rápido quando disponível. Leitura integral de `AXValue` e slice do documento ficam explicitamente fora do MVP por privacidade e custo.

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
- focused element provider;
- selected-range/text provider;
- parameterized marker string/bounds/index provider;
- resultado tipado com origem de extração, estágio e categoria AX.

Todos os valores vindos de funções `Create`/`Copy`, inclusive marker range e start/end markers, seguem RAII com exatamente um `CFRelease` em sucesso e em cada saída de erro. `OwnedAxElement` e markers ficam restritos à operação e à thread em que foram criados.

### Diagnóstico

Emitir somente estágio, origem tipada e categoria AX estável. Nunca emitir texto, range bruto, bundle path, token, conteúdo de clipboard ou status AX não classificado.

Estágios mínimos: `trust`, `system_wide_focused_element`, `role`, `selected_text`, `selected_range_type`, `selected_text_marker_range`, `string_for_text_marker_range`, `bounds_for_text_marker_range`, `index_for_text_marker`, `length_for_text_marker_range`, `settable`, `pid` e `geometry`.

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

Antes do gate completo, uma fixture descartável no TextEdit deve provar set/restore reversível com o mesmo target identity. Se `AXSelectedText` não for settable ou a restauração falhar, a captura marker pode ser entregue apenas como read-only/nota; não classificar TextEdit editável silenciosamente como sucesso total.

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

- A evidência inicial agregava todos os erros como `selection_unavailable`; foram adicionados spikes causais por estágio antes de qualquer extração alternativa.
- A extração alternativa possui matriz fechada de elegibilidade por categoria AX e nunca contorna TCC.
- A descoberta do app focado usa AX, não AppKit em worker.
- Mutação e restore ficam vinculados ao PID/snapshot esperado para eliminar TOCTOU e escrita cross-app.
- Ownership Core Foundation, identidade intra-PID, coerência temporal, rate limiting diagnóstico, secure fields e concorrência entraram como gates explícitos.
- A leitura integral de `AXValue` foi removida por risco de exposição/custo; text markers permanecem opacos.
- A preservação da main cobre todo o dirty state user-owned, não uma whitelist fixa.

### 🟢 Oportunidades incorporadas

- Reusar o boundary `focused_element` e o RAII existentes mantém a correção concentrada.
- A sonda por `AXFocusedApplication` refutou rapidamente uma correção especulativa antes de entrar em produção.
- Origem tipada permite provar em runtime qual extração (`selected_text | cf_range | text_marker`) restaurou a toolbar.
- Polling existente absorve mudanças de app no ciclo seguinte, evitando retries internos e novos schedulers.
- APIs públicas do SDK permitem derivar start/end/index/length de markers sem decodificar bytes privados ou ler o documento inteiro.
- Testes puros da matriz de decisão cobrem caminho direto, CFRange, text marker, trust, self-exclusion e corrida com baixo custo.

### Decisão sintetizada

Manter o resolver system-wide e implementar um adapter mínimo de text marker somente depois da quarta prova causal completa (texto, bounds, índices, length e settable). Compartilhar aquisição/ownership em capture e observer, mas vincular replace/restore ao snapshot e identidade forte do elemento esperado. A correção só é aprovada com testes automatizados, QA dual e smoke real correlacionado por Computer Use.

## 3. TASKS

- [x] T1 [LOW] Criar worktree isolada e ler memória.
- [x] T2 [LOW] Reproduzir e localizar o estágio agregado da falha.
- [x] T3 [LOW] Executar análise dual do plano.
- [x] T4 [LOW] Sintetizar plano final.
- [x] T5 [LOW] Adicionar diagnóstico tipado e executar três spikes que refutaram fallback de foco e CFRange presumido.
- [ ] T6 [LOW] Executar sonda read-only de tipo, text marker string/bounds/index/length e settable.
- [ ] T7 [MEDIUM] Implementar tipos puros de decisão, origem de extração e categoria AX.
- [ ] T8 [MEDIUM] Implementar extração por representação no mesmo elemento com RAII e type-check estrito.
- [ ] T9 [MEDIUM] Integrar extração em capture/observer e vincular replace/restore ao mesmo target esperado.
- [ ] T10 [LOW] Limitar diagnostics repetidos sem conteúdo selecionado.
- [ ] T11 [MEDIUM] Testar matriz direct/parameterized/value-slice/trust/self/race/ownership e regressões existentes.
- [ ] T12 [LOW] Testar equivalência/debounce, secure field, range, geometry e mutation target.
- [ ] T13 [MEDIUM] Executar QA dual de código, concorrência, FFI e testes.
- [ ] T14 [MEDIUM] Build/install debug e validação real por Computer Use em TextEdit e segundo app.
- [ ] T15 [MEDIUM] Corrigir rejeições e obter verdict `APPROVED`.
- [ ] T16 [LOW] Atualizar memória e gerar documento de entrega.
- [ ] T17 [LOW] Merge autorizado preservando todo dirty state user-owned.
- [ ] T18 [MEDIUM] Gerar e validar build release, recursos, codesign e launch smoke.
