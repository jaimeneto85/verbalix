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

### 🟢 Oportunidades incorporadas

- Reuso de `TransformOperation`, `request_id`, state machine, ports/fakes, `NoteResultState` e diagnósticos privacy-safe.
- Testes paramétricos de Translate/Improve e extensão dos harnesses existentes, sem criar arquitetura paralela.
- Command nativo como authority de readiness, reduzindo round-trip e janela de corrida.
- Timeline sanitizada por `request_id + snapshot_id`, útil para diagnosticar cada estágio sem conteúdo do usuário.
- TextEdit como baseline AX e Slack como editor complexo na matriz real.

### Síntese

A correção será tratada como uma transação ligada a `snapshot.id + request_id`, e não como um simples clique seguido de chamada remota. O ganho rápido é remover o mascaramento de erro e fixar a ação antes dos awaits; o núcleo de segurança é revalidar o alvo AX original sem relaxar identidade. Nenhum sleep ou atraso artificial será aceito como solução.

## 🔄 Parallelization Synthesis

- 🔴 Estimativa pessimista: 1 agente, devido ao acoplamento entre state machine, command, AX e feedback.
- 🟢 Estimativa otimista: 2 agentes após fundação serial, separando transação/runtime de resolução AX.
- Decisão: 1 agente sequencial. Os contratos centrais se sobrepõem e o limite físico de threads exigiu serializar as análises no mesmo thread reutilizado.
- Risco de conflito: baixo.
