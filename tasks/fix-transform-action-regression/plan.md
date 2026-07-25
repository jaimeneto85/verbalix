# Plano — Corrigir ações Traduzir e Aprimorar

## 0. SCOPE

### Incluído

- [x] Reproduzir e instrumentar a cadeia `overlay click → IPC Tauri → transformação autenticada → coordinator → escrita AX`.
- [x] Corrigir Traduzir e Aprimorar para enviarem a operação correta.
- [x] Restaurar a substituição do texto selecionado quando o conteúdo for editável.
- [x] Preservar comportamento fail-closed para seleção stale, identidade AX divergente e conteúdo somente leitura.
- [x] Cobrir erros acionáveis de sessão/backend sem expor texto selecionado ou credenciais.
- [ ] Validar em app macOS real com Accessibility e backend configurado.

### Arquivos/módulos potencialmente afetados

- `src/Overlay.tsx`, `src/native.ts` e testes frontend do overlay/IPC.
- `src-tauri/src/commands.rs` e runtime/comandos Tauri.
- `src-tauri/src/application/coordinator.rs` e respectivos testes.
- `src-tauri/src/platform/macos_accessibility*.rs` e testes de replace/identidade.
- Diagnósticos sanitizados estritamente necessários à reprodução.

### Dependências diretas

- Tauri IPC, React, Supabase Auth/Edge Function e Accessibility API do macOS.

### Fora do escopo

- Alterar prompts, modelo de IA, UI visual do overlay ou geometria já aprovada.
- Afrouxar validações de identidade/staleness para forçar a escrita.
- Merge em `main`, geração de release, mudanças na Edge Function ou no contrato público sem evidência de necessidade.

### Riscos de impacto

- Corrigir o clique sem corrigir a revalidação AX pode produzir sucesso aparente sem substituir.
- Uma seleção muda enquanto a IA responde; a correção deve continuar rejeitando a escrita stale.
- O overlay pode tomar foco e invalidar a seleção antes do comando.
- Traduzir e Aprimorar podem compartilhar um defeito comum, mas ainda precisam de cobertura independente.
- Testes com mocks podem passar sem provar sessão remota, Accessibility e substituição reais.

## 1. REQUIREMENTS

### Requisitos funcionais

- [ ] RF01: Clicar em Traduzir invoca exatamente uma transformação `translate`.
- [ ] RF02: Clicar em Aprimorar invoca exatamente uma transformação `improve`.
- [ ] RF03: Em seleção editável ainda válida, o resultado substitui exatamente o texto selecionado.
- [ ] RF04: Em seleção somente leitura, o resultado é exibido como nota, sem tentativa de escrita.
- [ ] RF05: Falhas de backend, autenticação, seleção stale ou escrita AX geram estado/erro acionável e não desaparecem silenciosamente.
- [ ] RF06: A transformação usa o snapshot capturado antes do clique e revalida sua identidade antes da escrita.
- [ ] RF07: O `snapshot.id` da ação é fixado antes do primeiro `await` e permanece associado ao `request_id` até o resultado.
- [ ] RF08: Com confirmação desligada, o resultado substitui diretamente; com confirmação ligada, zero writes ocorrem antes de Aplicar.
- [ ] RF09: Um erro de seleção/AX não pode ser apresentado como indisponibilidade do provider.
- [ ] RF10: Falha do feedback de undo depois de uma escrita bem-sucedida não pode reclassificar a mutação como inexistente.
- [x] RF11: Candidate diferente ou invalidação real antes do setter revoga a escrita sem depender do mutex de estado.
- [x] RF12: Candidate diferente ou invalidação real deve atualizar o estado enquanto revalidação/escrita/overlay da ação anterior está bloqueado.
- [x] RF13: Nota, preview, undo e erro carregam a guarda da ação até o executor visual; uma guarda cancelada produz zero publicação.
- [x] RF14: O executor visual revalida/lineariza a publicação depois da preparação da janela; cancelamento durante `get/create/place` produz zero `emit/show`.
- [x] RF15: A guarda de vida da ação aceita múltiplos feedbacks legítimos; cada comando visual possui autorização atômica própria, revogada pelo cancelamento global enquanto ainda não reivindicada.
- [ ] RF16: Quando `AXSelectedText` não existe, mas `AXValue` e `AXSelectedTextRange` são válidos, capturar e revalidar a seleção por índices UTF-16 sem afrouxar identidade, bounds, writable ou proteção contra staleness.
- [ ] RF17: Capacidade de leitura e de escrita são independentes: o fallback só é writable quando `AXSelectedText` é comprovadamente settable; `AXValue` completo não será sobrescrito neste escopo.
- [ ] RF18: A estratégia de extração participa de `same_target`, replace e restore; não há revalidação cruzada entre `SelectedText`, `ValueRange` e `TextMarker`.
- [ ] RF19: O fallback acessa no máximo 262.144 code units UTF-16 do valor, copia somente o range selecionado para memória Rust e nunca loga, persiste ou envia prefixo/sufixo ao provider.
- [x] RF20: Protected fields e roles fora da allowlist textual falham antes de qualquer leitura de `AXSelectedText`, `AXStringForRange` ou `AXValue`.
- [x] RF21: Quando `AXIdentifier` não existe, captura, replace e restore usam a mesma referência AX retida causalmente por snapshot, com TTL/capacidade e cleanup; `role + frame` nunca substitui identidade forte.
- [x] RF22: Toda mutação AX confirmada gera receipt lógico durável suficiente para `Applied + undo`; falha pós-setter não deixa alteração órfã nem reclassifica sucesso como ausência de write.
- [x] RF23: A classificação textual/protegida usa `AXRole + AXSubrole`; `AXTextField/AXSecureTextField` falha antes de identifier, bounds, settable ou qualquer leitura de conteúdo.
- [x] RF24: Mudança de seleção/foco revoga a lease por sinal causal não bloqueado atrás de Replace/Restore no actor; o setter revalida essa geração imediatamente antes do claim/write.
- [x] RF25: O actor registra antes do setter o mutation ID, snapshot, original, transformed, strategy, target e undo metadata completos; uma API idempotente reconcilia resposta perdida/commit falho.
- [ ] RF26: Mudança de foco por teclado (`AXFocusedUIElementChanged`) incrementa o epoch fora da FIFO antes que polling/capture possa atrasar a detecção.
- [ ] RF27: Notificação `AXSelectedTextChanged` causada pelo próprio mutation ID é correlacionada/suprimida sem esconder mudanças externas reais.
- [ ] RF28: Estados de restore são monotônicos e tipados; `Rejected` nunca vira `Indeterminate/Confirmed` e o mesmo mutation ID executa no máximo um restore setter.
- [ ] RF29: Toda leitura de reconciliação revalida `AXRole + AXSubrole` antes de conteúdo, inclusive em handles retidos que mudaram para secure.

### Requisitos não funcionais

- [ ] RNF01: Nenhum texto selecionado, token ou segredo em logs.
- [ ] RNF02: Nenhum arquivo modificado ultrapassa 300 linhas efetivas.
- [ ] RNF03: `cargo fmt`, `cargo check`, `cargo test`, `cargo clippy -D warnings`, Vitest, cobertura, Playwright e build passam.
- [ ] RNF04: A correção preserva transparência, posicionamento e lifecycle geracional do overlay.

### Critérios de aceitação

- [ ] CA01: Teste frontend prova ambos os botões, payloads e tratamento de falha.
- [ ] CA02: Teste Rust prova as duas operações chegando ao provider e substituindo seleção editável.
- [ ] CA03: Teste Rust prova que seleção read-only recebe nota e zero writes.
- [ ] CA04: Testes AX provam escrita no handle original com identidade/range atuais e zero escrita após divergência.
- [ ] CA05: Computer Use em seleção editável comprova Traduzir e Aprimorar alterando textos de teste distintos.
- [ ] CA06: Diagnósticos sanitizados identificam estágios sem registrar conteúdo ou credenciais.
- [ ] CA07: Uma recaptura de outro alvo com o mesmo texto nunca herda a ação iniciada.
- [ ] CA08: Falha transitória de captura causada pelo overlay não apaga uma ação em processamento; uma mudança real do alvo continua impedindo a escrita.
- [ ] CA09: O smoke registra o valor de `confirm_before_replace`; no modo preview, Aplicar é parte obrigatória do fluxo.
- [ ] CA10: Persistência de histórico nunca mantém a transformação pendente indefinidamente; falha de sync é observável e não desfaz o resultado.
- [ ] CA11: Falhas de pin, Aplicar e Desfazer exibem feedback tipado guardado e não parecem cliques inertes.
- [ ] CA12: Undo de A concorrente com Candidate B nunca apaga, oculta ou substitui B.
- [ ] CA13: Readiness/falha de A enfileirada antes do provider nunca aparece sobre Candidate B.
- [x] CA14: Candidate/invalidation durante a preparação visual de A vence antes do boundary de publicação e deixa zero evento, zero janela visível e zero payload corrente de A.
- [x] CA15: Preview, undo ou toolbar já publicados não impedem erro subsequente da mesma ação; cancelamento durante a preparação do segundo comando continua produzindo zero efeito stale.
- [ ] CA16: No TextEdit real, seleção editável com `AXSelectedText=attribute_unsupported` produz snapshot válido, exibe toolbar e permite substituir/restaurar exatamente o range selecionado via fallback seguro.
- [ ] CA17: O smoke registra separadamente `identifier_present`, `AXSelectedText_settable`, `AXValue_cfstring` e `AXSelectedTextRange_settable`, sem conteúdo; a árvore real já evidencia o alvo `First Text View` e edição settable.
- [ ] CA18: Campo protegido, role não textual, valor acima do limite ou `AXValue` não-CFString produz zero materialização de texto e zero escrita.
- [ ] CA19: Falha pós-setter não reclassifica a mutação como inexistente; undo só restaura quando o mesmo alvo e o resultado transformado ainda ocupam o range esperado.
- [x] CA20: Trace prova zero APIs de conteúdo chamadas para secure/non-text roles, inclusive caminhos direct/CFRange/value/marker.
- [x] CA21: Snapshot sem identifier substitui/restaura somente pelo handle AX original retido; handle ausente, expirado ou divergente produz zero setter.
- [x] CA22: Setter bem-sucedido seguido de falha de commit/feedback mantém receipt, estado Applied recuperável e undo; setter rejeitado não cria receipt.
- [x] CA23: Trace de `AXTextField + AXSecureTextField` prova zero chamadas a identifier/bounds/settable/SelectedText/StringForRange/AXValue/marker.
- [x] CA24: Scheduler real prova que evento causal de Candidate B durante preparação de A cancela A antes do setter, mesmo com Capture B aguardando na fila AX.
- [x] CA25: Perda de response, expiração durante setter e falha de `finish_receipt`/commit são reconciliadas pelo mesmo mutation ID completo, sem sobrescrever Candidate B.
- [ ] CA26: Teste comportamental move foco por teclado durante preparação A e prova epoch revogado/zero setter, com Capture B ainda pendente.
- [ ] CA27: Self-notification exata mantém Applied/undo/feedback; notificação externa subsequente cancela normalmente.
- [ ] CA28: Restore Rejected/Indeterminate/Confirmed e retry/reconcile provam setter count máximo 1 por mutation ID.
- [ ] CA29: Handle que muda para secure antes de reconcile produz trace zero-read e terminal Rejected.

### Edge cases

- EC01: Duplo clique/ações concorrentes.
- EC02: Overlay perde/reordena readiness durante o clique.
- EC03: Sessão ausente/expirada e refresh transitório.
- EC04: Backend retorna erro, timeout ou resultado vazio.
- EC05: Seleção, foco, range ou conteúdo muda durante a transformação.
- EC06: Unicode e ranges UTF-16.
- EC07: Elemento editável sem setter AX suportado.
- EC08: Outro campo contém exatamente o mesmo texto selecionado.
- EC09: Escrita AX funciona, mas a publicação de undo falha.
- EC10: Clique ocorre antes de `loadSettings()` concluir ou dois cliques chegam no mesmo frame.
- EC11: Restore/undo fica bloqueado enquanto uma nova seleção B é capturada.
- EC12: Histórico remoto não responde depois de uma transformação aplicada.
- EC13: A guarda é cancelada enquanto o executor principal cria ou posiciona a janela, antes do primeiro efeito visual.
- EC14: A mesma ação publica uma superfície inicial e depois precisa publicar feedback de falha de Apply, Undo ou pin.
- EC15: `AXValue` contém Unicode com pares substitutos e o range UTF-16 começa/termina somente em boundary válido.
- EC16: Range negativo, vazio, fora do valor, no meio de surrogate pair ou valor alterado entre captura e escrita deve falhar fechado.
- EC17: `AXValue` é legível, mas `AXSelectedText` não é settable; a seleção deve permanecer read-only e receber nota.

## 2. DESIGN

### Estratégia

- Reproduzir primeiro com diagnósticos sanitizados para localizar a primeira quebra observável.
- Manter a UI como entrypoint explícito e o comando Tauri como boundary de validação/readiness.
- Manter o `SelectionCoordinator` como dono de latest-wins, revalidação e decisão `replace` versus `note`.
- Manter o adapter Accessibility como único responsável pela escrita real e pela revalidação do mesmo handle.
- Tornar o comando nativo a autoridade de readiness, reduzindo a janela criada pela pré-checagem frontend duplicada.
- Fixar `snapshot.id + request_id` antes de refresh de sessão/provider e iniciar o estado Processing antes do primeiro `await`.
- Tratar falha de captura enquanto Processing como sinal transitório somente para retenção de estado; o setter AX ainda deve recapturar/resolver e revalidar o alvo antes de qualquer escrita.

### Fluxo de dados esperado

`click(operation) → native.transformSelection(operation, settings) → transform_selection → pin(snapshot.id, request_id) → session/readiness → provider → recapture/resolve/revalidate → replace(editável) | note(read-only) → feedback`

### Contratos e invariantes

- Cada ação explícita gera no máximo uma request ativa.
- O snapshot ID fixado e o request ID enviado ao provider devem ser os mesmos validados no retorno.
- `replace` só ocorre quando `writable=true`, identidade forte coincide, texto/range atuais coincidem e setter AX é suportado.
- Falha em qualquer invariável é terminal e observável, nunca convertida em “sucesso”.
- Não ocultar o toolbar de forma que o clique destrua o snapshot antes do comando assumir a operação.
- Texto igual não equivale a alvo igual; a ação exige ID, identidade, range e conteúdo compatíveis.
- O alvo AX não pode ser escolhido apenas pelo foco corrente depois da latência remota; a implementação deve reter uma referência com lifecycle seguro ou resolver o alvo original por PID/identidade forte antes da escrita.
- Depois de uma escrita confirmada, o estado lógico permanece Applied mesmo se o feedback de undo falhar; a falha visual é diagnosticada separadamente.
- O mutex protege apenas leitura e transição do estado; provider, AX e overlay nunca executam sob esse mutex.
- Cada ação possui um `TransformLease` seguro com CAS `Active → Claimed | Cancelled`; o claim ocorre após a revalidação final e imediatamente antes do setter.
- Cancelamento após `Claimed` não tenta desfazer a escrita já autorizada, mas impede `Applied`, undo e qualquer publicação visual stale sobre o novo alvo.
- Feedback usa bounds e guarda da ação original. O executor de `ShowResult` revalida a guarda sem consultar o snapshot global.
- Preparação visual pode ocorrer antes do boundary; `emit/show` só ocorre após um claim visual atômico ou revalidação equivalente no último ponto cancelável. Candidate/invalidation concorrente deve linearizar antes ou depois desse boundary, nunca no intervalo.
- `TransformLease` representa a vida cancelável reutilizável da ação; cada `OverlayCommand` guardado recebe um token/claim próprio ligado à mesma vida, evitando que uma publicação legítima consuma a autorização de feedbacks posteriores.
- O fallback clássico lê `AXValue` e fatia exclusivamente por offsets UTF-16 de `AXSelectedTextRange`; nunca trata offsets CFRange como índices de bytes/chars Rust.
- Escrita fallback exige a mesma identidade forte, range e substring atuais e claim imediatamente antes do setter `AXSelectedText`; leitura indisponível não implica setter indisponível.
- `AXValue` é somente fonte de leitura restrita neste escopo. Não haverá setter de documento completo, evitando sobrescrever edições externas, formatação, IME e undo nativo.
- A origem de captura deve acompanhar o snapshot/revalidação para que um snapshot obtido por `AXValue` não seja validado por uma estratégia incompatível ou promovido a writable sem setter comprovado.
- Antes da implementação mutável, um probe sanitizado deve decidir a identidade do TextEdit: `AXIdentifier` forte quando presente; se ausente, usar retenção causal do `AXUIElementRef` original com lifecycle limitado, nunca `role + frame` como substituto.
- A leitura deve obter `range₁ → AXValue → range₂`, exigir igualdade e usar APIs de CFString para validar comprimento e copiar somente o range; valor maior que 262.144 code units falha fechado.
- O gate de role ocorre imediatamente após `AXRole` e antes de qualquer API que possa materializar conteúdo; a allowlist é explícita e coberta por trace.
- O registry causal pertence à instância `MacAccessibility`, retém o `AXUIElementRef` sob `snapshot.id`, é limitado/expirável e só resolve após PID/role/estratégia/range/subtexto revalidados; entradas são removidas ao expirar/consumir e nunca serializadas.
- O setter retorna resultado tipado e receipt. O receipt de mutação é registrado independentemente da apresentação/state machine antes de qualquer operação que possa falhar; `Applied`/undo podem ser reconciliados sem sobrescrever Candidate mais novo.
- `AXSecureTextField` é subrole no SDK macOS. Role e subrole devem ser lidos/classificados antes de qualquer outra propriedade potencialmente sensível.
- A revogação latest-wins não pode depender de uma Capture FIFO atrás do write. Observer/focus/input emite geração atômica ou sinal equivalente fora da fila bloqueada; o actor lê essa geração na preparação e novamente imediatamente antes do setter.
- O actor mantém o registro completo em estado `Prepared → Confirmed | Rejected | Indeterminate`; coordinator consulta/reconcilia por mutation ID após timeout, perda de resposta ou falha de commit.
- Focus change deve ser observado no nível apropriado fora da fila de writes; seleção e foco são sinais causais distintos.
- Supressão de self-notification é por mutation ID/target/generation e consumo único; janela temporal ou supressão global é proibida.
- Restore e reconcile usam a mesma máquina monotônica e o secure gate compartilhado; nenhum caminho de recuperação pode reler conteúdo diretamente.
- Erros são roteados por classe: auth/config, provider, seleção stale, permissão/AX e overlay.

### Componentes reutilizáveis

- `Overlay` e `native.transformSelection`.
- `commands::transform_selection`.
- `SelectionCoordinator` e fakes existentes.
- Matriz AX de identidade, replace/restore e diagnósticos tipados.
- `TransformOperation`, `request_id`, `NoteResultState`, `route_refresh_failure` e `error_code` existentes.

## 3. TASKS

### Fase 1 — Reprodução e causa

- [x] T1.1 `[MEDIUM]` Reproduzir ambos os botões e registrar o primeiro estágio que falha.
- [x] T1.2 `[MEDIUM]` Inspecionar payload IPC, readiness duplicada, settings, sessão e lifecycle do snapshot.
- [x] T1.3 `[MEDIUM]` Inspecionar observer/mouse dismiss durante `ToolbarVisible` e `Processing`.
- [x] T1.4 `[MEDIUM]` Inspecionar como o alvo AX original é retido/resolvido e como o setter é classificado.

### Fase 2 — Implementação

- [x] T2.1 `[MEDIUM]` Fixar snapshot/request antes de awaits e tornar Processing resistente apenas a falhas transitórias do overlay.
- [x] T2.2 `[MEDIUM]` Corrigir a resolução/revalidação do alvo AX original sem afrouxar identidade/staleness.
- [x] T2.3 `[MEDIUM]` Implementar roteamento tipado e feedback acionável para falhas reais.
- [x] T2.4 `[LOW]` Preservar note read-only, preview, undo e overlay lifecycle.
- [x] T2.5 `[LOW]` Garantir consistência de estado quando o write funciona e o feedback falha.
- [x] T2.6 `[HIGH]` Cancelar logicamente a request quando uma captura bem-sucedida identifica outro alvo, preservando apenas falhas transitórias sem candidato.
- [x] T2.7 `[HIGH]` Remover I/O do mutex de estado, introduzir lease com claim CAS no boundary do setter e guardar publicações no executor.
- [x] T2.8 `[HIGH]` Guardar readiness e erros de pin/apply/undo, tornar undo condicional e remover histórico/show_toolbar do caminho bloqueante.
- [x] T2.9 `[HIGH]` Separar preparação de publicação visual e fechar o TOCTOU entre a checagem inicial da guarda e `emit/show`.
- [x] T2.10 `[HIGH]` Substituir o claim visual single-use por lifetime guard reutilizável e claim independente por comando, todos revogáveis pelo cancelamento da ação.
- [ ] T2.11 `[HIGH]` Implementar extração/revalidação/substituição/restauração por `AXValue + AXSelectedTextRange` com conversão UTF-16 validada e fallback restrito às falhas de capacidade de `AXSelectedText`/`AXStringForRange`.
- [ ] T2.12 `[MEDIUM]` Reduzir invalidation spam somente se a captura equivalente pelo fallback comprovar o mesmo alvo/range/texto, preservando invalidação real.
- [ ] T2.13 `[HIGH]` Executar probe sanitizado do TextEdit e implementar identidade forte por identifier ou retenção causal do elemento original, conforme evidência.
- [ ] T2.14 `[MEDIUM]` Generalizar consulta de settable/setter por atributo, mantendo `AXSelectedText` como único writer do fallback e `AXValue` como leitura range-only.
- [x] T2.15 `[HIGH]` Mover a allowlist de role para antes de qualquer leitura de conteúdo e provar zero-read por adapter trace.
- [x] T2.16 `[HIGH]` Implementar registry causal bounded/TTL do `OwnedAxElement` por snapshot e integrá-lo a capture/replace/restore sem unsafe identidade fraca.
- [x] T2.17 `[HIGH]` Introduzir write receipt e reconciliação pós-setter para preservar Applied/undo fora do estado visual latest-wins.
- [x] T2.18 `[CRITICAL]` Classificar role+subrole seguro antes de qualquer leitura/probe de conteúdo ou identidade.
- [x] T2.19 `[HIGH]` Redesenhar revogação do actor com epoch/sinal causal fora da FIFO e rechecagem no boundary do setter, mantendo handle no owner thread.
- [x] T2.20 `[HIGH]` Persistir mutation record completo no actor antes do setter e expor lookup/reconcile idempotente para response/commit failure.
- [ ] T2.21 `[HIGH]` Observar focus-changed fora da FIFO e integrar ao CausalEpoch sem depender do polling.
- [ ] T2.22 `[HIGH]` Correlacionar self-notification ao mutation record e consumir exatamente uma notificação esperada.
- [ ] T2.23 `[HIGH]` Tornar restore terminal monotônico/idempotente e compartilhar secure-gated revalidation no reconcile.

### Fase 3 — Testes

- [x] T3.1 `[LOW]` Cobrir os dois botões e payloads no frontend.
- [x] T3.2 `[MEDIUM]` Cobrir translate/improve, replace/note/preview e erros tipados no boundary do comando/coordinator.
- [x] T3.3 `[MEDIUM]` Cobrir revalidação e escrita AX, incluindo Unicode/stale/unsupported.
- [x] T3.4 `[MEDIUM]` Cobrir polling/observer durante Processing, outro alvo com texto idêntico, duplo clique e falha pós-write.
- [x] T3.5 `[LOW]` Executar gates automatizados e limite de linhas.
- [x] T3.6 `[MEDIUM]` Provar insert/list do histórico após Translate e Improve bem-sucedidos.
- [x] T3.7 `[HIGH]` Provar supersede antes/depois do provider, mesmo texto com PID/identidade diferente, same-target preservado e ausência de feedback stale.
- [x] T3.8 `[HIGH]` Provar com adapters bloqueáveis Candidate/Invalidated antes e depois do claim, apply preview concorrente e `ShowResult` cancelado antes da execução.
- [x] T3.9 `[HIGH]` Provar readiness pré-pin, undo bloqueável versus Candidate B, feedback de pin/apply/undo, history timeout/off-critical-path e show_toolbar sem mutex.
- [x] T3.10 `[HIGH]` Provar deterministicamente, sem sleeps, cancelamento durante preparação visual com zero `emit/show/payload`, além da ordem após o boundary linearizado.
- [x] T3.11 `[HIGH]` Provar Preview→erro Apply, Undo→erro Undo, Toolbar→erro pin, cancelamento durante a segunda preparação e supersede pós-claim terminando oculto.
- [ ] T3.12 `[HIGH]` Cobrir UTF-16 BMP/emoji/combining, ranges inválidos, value read-only/settable, mutação concorrente, replace/restore e zero setter em staleness.
- [ ] T3.13 `[MEDIUM]` Executar smoke macOS real no TextEdit para captura, toolbar, Traduzir, Aprimorar e Desfazer.
- [ ] T3.14 `[HIGH]` Cobrir matriz independente de leitura/setter, `range₁/value/range₂`, limite, CFString inválida, protected field antes do value, identidade ausente/retida e falha pós-write.
- [x] T3.15 `[HIGH]` Cobrir trace zero-read para cada role bloqueada e ausência de regressão nos roles textuais suportados.
- [x] T3.16 `[HIGH]` Cobrir registry: identifier presente/ausente, handle exato, TTL, capacidade, cleanup, divergência e zero setter sem receipt causal.
- [x] T3.17 `[HIGH]` Cobrir setter success/failure/indeterminate, falha de commit/feedback, Candidate concorrente, receipt recuperável e undo sem sobrescrever seleção nova.
- [x] T3.18 `[CRITICAL]` Cobrir secure subrole real com trace zero-read e constantes compatíveis com SDK.
- [x] T3.19 `[HIGH]` Cobrir scheduler actor end-to-end: preparação bloqueada, sinal B fora da FIFO, capture B pendente, zero setter A.
- [x] T3.20 `[HIGH]` Cobrir mutation recovery completo em perda de response, expiry durante setter, commit failure, reconcile repetido e Candidate B preservado.
- [x] T3.21 `[HIGH]` Cobrir behavioralmente focus keyboard, self-notification versus external notification e zero setter stale.
- [x] T3.22 `[HIGH]` Cobrir restore setter count, estados monotônicos e secure transition zero-read com fakes instrumentados; `include_str!` não satisfaz.

### Fase 4 — QA real

- [ ] T4.1 `[MEDIUM]` Validar Traduzir no TextEdit e Slack com confirmação desligada.
- [ ] T4.2 `[MEDIUM]` Validar Aprimorar no TextEdit e Slack com confirmação desligada.
- [ ] T4.3 `[MEDIUM]` Validar preview + Aplicar com confirmação ligada e zero write antes de Aplicar.
- [ ] T4.4 `[LOW]` Verificar logs sanitizados, ausência de regressão visual e verdict formal.

## Análise Dual

### 🔴 Riscos incorporados

- A ação não estava vinculada ao snapshot original através do refresh assíncrono; ID e request agora são invariantes explícitas.
- `replace` reencontrava apenas o elemento focado depois da chamada remota; o plano exige retenção ou resolução segura do alvo original.
- Polling/observer podiam apagar o estado entre clique e resultado; os testes agora distinguem falha transitória do overlay de mudança real.
- Ignorar todo `Candidate` durante `Processing` permitiria escrever no alvo anterior após uma captura real diferente; somente um candidato equivalente preserva o lease.
- O modo `confirm_before_replace` estava ausente dos critérios e agora tem dois fluxos objetivos.
- Erros stale/AX eram mascarados como provider e pelo `catch` frontend; o roteamento tipado passou a requisito.
- Escrita bem-sucedida seguida de falha de undo criava estado parcial; a consistência pós-write passou a invariável.
- Segurar o mutex durante AX/overlay impedia o próprio evento de supersede de revogar a ação; I/O foi separado das transições e a autorização final passou a um CAS no setter.
- Verificar ownership e depois reler o snapshot para publicar erro criava TOCTOU; a publicação agora carrega bounds e guarda imutáveis da ação até a main thread.
- Uma checagem única da guarda antes de `get/create/place` ainda permite publicação stale; o boundary visual final precisa de linearização própria e teste de cancelamento durante a preparação.
- Um claim visual único por ação corrige a primeira publicação, mas bloqueia feedback posterior legítimo; a linearização precisa ser por comando sob uma lifetime guard comum.
- `AXStringForRange` não é universal: TextEdit pode expor `AXValue` e `AXSelectedTextRange` mesmo retornando `attribute_unsupported` para `AXSelectedText`; o fallback deve ser explícito, UTF-16-correto e transacional.
- A nova evidência do TextEdit justifica leitura restrita de `AXValue`, mas não setter integral: somente o trecho selecionado é copiado e o writer continua sendo `AXSelectedText` quando settable.
- O probe Swift externo falhou fechado por TCC (`-25204`) e não tentou mutação; a árvore real via Computer Use evidencia identidade `First Text View`, valor legível e edição settable, suficientes para a primeira implementação conservadora.
- Capacidade deve ser diagnosticada uma vez por transição/categoria no build de smoke, sem texto, range concreto ou identificador.
- O QA RF16 encontrou que a allowlist era aplicada tarde, que o handle causal não sobrevivia à captura e que o commit lógico ocorria depois do setter sem receipt independente; RF20–RF22 tornam esses pontos gates explícitos.
- O QA RF20 mostrou que secure é subrole, que Capture e Replace na mesma FIFO ainda atrasam latest-wins e que IDs pré-setter sem payload completo não permitem recovery; RF23–RF25 fecham esses três pontos.
- O QA RF23 mostrou ausência de focus notification, autocancelamento por self-notification, restore não monotônico e reconcile sem secure gate; RF26–RF29 tornam os quatro comportamentos verificáveis.

### 🟢 Oportunidades incorporadas

- Reuso de `TransformOperation`, `request_id`, state machine, ports/fakes, `NoteResultState` e diagnósticos privacy-safe.
- Testes paramétricos de Translate/Improve e extensão dos harnesses existentes, sem criar arquitetura paralela.
- Command nativo como authority de readiness, reduzindo round-trip e janela de corrida.
- Timeline sanitizada por `request_id + snapshot_id`, útil para diagnosticar cada estágio sem conteúdo do usuário.
- TextEdit como baseline AX e Slack como editor complexo na matriz real.
- Reuso de `CFRange`, `macos_ax::string_value`, `macos_geometry::resolve`, setter `AXSelectedText`, strong identifier e lease existente; coordinator, provider e overlay não precisam mudar.
- Um helper UTF-16 puro sobre code units permite testar slice/range/overflow/surrogates sem depender do macOS.
- Capturas `ValueRange` equivalentes podem reutilizar `same_target`; observer só muda se o smoke ainda provar spam depois da equivalência correta.

### Síntese

A transação permanece ligada a `snapshot.id + request_id`. Para RF16, o adapter adiciona `ValueRange` como estratégia explícita apenas quando as falhas anteriores forem de capacidade. O valor completo não é convertido nem escrito: CFString valida o limite e fornece somente os code units selecionados. Writability é consultada separadamente e o único writer continua `AXSelectedText`; sem setter, o resultado é nota. Identidade forte, range duplo, substring, lease e setter são revalidados no mesmo boundary já aprovado. Nenhum sleep, debounce preventivo ou relaxamento de identidade será aceito.

## 🔄 Parallelization Synthesis

- 🔴 Estimativa pessimista RF16: 1 agente, pois captura, estratégia, replace e restore compartilham o mesmo contrato de revalidação.
- 🟢 Estimativa otimista RF16: helper UTF-16 e testes puros podem ser delegados depois que a estratégia for implementada.
- Decisão RF16: implementação serial por 1 agente; test-engineer independente audita matriz UTF-16/AX e gates depois.
- Risco de conflito: baixo.
