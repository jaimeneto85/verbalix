# Plano — Corrigir fallback geométrico do overlay

## 0. SCOPE

Corrigir somente a decisão pura de geometria usada quando `AXBoundsForRange` não fornece um retângulo válido. A seleção real no Slack comprovou que o boundary AppKit e a conversão de coordenadas estão corretos; o erro é a prioridade atual, que escolhe o frame inteiro do elemento antes do cursor.

Dentro do escopo:

- preservar `SelectedRange` como fonte prioritária;
- usar `Cursor` como segundo fallback somente quando o ponto estiver dentro do frame válido do elemento focado;
- manter `FocusedElement` como último fallback;
- preservar coordenadas globais negativas e múltiplos monitores;
- adicionar testes puros de prioridade e contenção;
- repetir gates automatizados e QA dual;
- atualizar documentação e memória.

Fora do escopo:

- alterar conversão AppKit/AX, posicionamento final da janela ou dimensões do overlay;
- alterar captura de texto, text markers, foco, replace/restore, Auth ou IA;
- usar cursor fora do elemento focado;
- merge, instalação ou release.

## 1. REQUIREMENTS

- R1: `SelectedRange` válido sempre vence cursor e frame.
- R2: sem range válido, cursor finito dentro do frame válido vence o frame.
- R3: cursor fora do frame, não finito ou sem frame associado nunca é usado; o frame válido permanece o último fallback.
- R4: frame ausente ou inválido não autoriza cursor isolado, porque não há vínculo espacial com o alvo focado.
- R5: contenção inclui as bordas do frame e funciona com coordenadas globais negativas.
- R6: a decisão permanece pura, determinística e sem novas chamadas FFI.
- R7: testes cobrem cursor dentro/fora, range sobre cursor, cursor sobre frame, limites e pontos imediatamente externos, entradas inválidas e múltiplos monitores.
- R8: todos os gates existentes e QA dual devem aprovar a mudança.
- R9: o gate real posterior deve validar Slack com seleção por mouse e teclado e reconhecer que contenção espacial não prova causalidade temporal do cursor.

## 2. DESIGN

### Evidência causal

- Slack expôs seleção válida, mas `AXBoundsForRange` ficou indisponível.
- A captura teve sucesso e registrou `geometry_source=focused_element`.
- As coordenadas reais do AppKit coincidiram com os diagnostics.
- O overlay ficou distante porque `select_geometry` atualmente resolve `FocusedElement` antes de considerar `Cursor`.

Logo, não há justificativa para alterar a transformação de coordenadas. A correção pertence exclusivamente à política de fallback.

### Política de resolução

1. validar e retornar `SelectedRange`;
2. validar `FocusedElement`;
3. se o frame for válido, validar o cursor e testar contenção inclusiva;
4. se contido, retornar um retângulo `1x1` com origem no cursor e fonte `Cursor`;
5. caso contrário, retornar o frame com fonte `FocusedElement`;
6. sem range e sem frame válido, retornar `None`.

O cursor não pode ser usado sem frame válido, pois isso aceitaria posição global sem prova de relação com o elemento cuja seleção foi capturada.

Os testes legados que autorizam cursor sem frame válido devem ser reescritos. Cursor válido com frame ausente, `NaN`, infinito, zero ou dimensão negativa retorna `None`.

### Margem

O draft não adiciona margem. As coordenadas do cursor e do frame vêm do mesmo espaço global Core Graphics/AX e a evidência não demonstra drift. Uma margem positiva ampliaria o vínculo para fora do alvo e criaria risco de posicionar o overlay sobre outro elemento. Se a análise dual trouxer evidência concreta, qualquer margem deve ser pequena, constante, testada e justificada na síntese.

A síntese mantém margem zero. A contenção é inclusiva por requisito, embora retângulos contíguos às vezes usem semântica half-open; por isso, as quatro bordas e pontos imediatamente externos devem ser testados explicitamente, sem epsilon oculto.

### Segurança e foco

A mudança não redescobre elementos nem afrouxa os gates de PID, role, foco ou identidade. Ela só escolhe entre três geometrias já capturadas na mesma operação. Falha de associação cursor-frame é fechada no frame, não no cursor.

Contenção é apenas uma heurística espacial. Em elementos grandes, o cursor pode continuar dentro do frame após uma seleção por teclado ou após movimento do mouse, mas longe do texto. Resolver staleness exigiria sinal temporal adicional e está fora deste escopo. O gate real posterior deve testar mouse, teclado e cursor movido dentro do mesmo editor; se o resultado permanecer inadequado, não se deve ampliar margem nem timing nesta tarefa.

### Multi-monitor

A contenção compara valores globais diretamente e não normaliza, arredonda nem limita coordenadas. Assim, frames e cursores com `x/y` negativos, telas acima/abaixo da principal e frames cruzando a origem continuam válidos.

## Análise Dual

### 🔴 Riscos incorporados

- Contenção no frame não prova proximidade da seleção; o risco residual de cursor stale em elementos grandes fica explícito e será exercitado no gate real.
- A leitura sequencial de range, frame e cursor não pode relaxar foco, PID, role, secure field ou identidade; nenhum boundary além da decisão pura será alterado.
- Bordas inclusivas podem coincidir com elementos adjacentes; a decisão é registrada e coberta por pontos exatamente no limite e imediatamente fora.
- Cursor órfão foi removido do contrato: sem frame válido a decisão falha fechada em `None`.
- Multi-monitor cobre X/Y negativos, origem cruzada e não aplica escala ou normalização.

### 🟢 Oportunidades incorporadas

- A correção reutiliza `valid_selected_range`, `valid_frame`, `finite`, `Rect` e `GeometrySource`.
- Um helper puro de contenção inclusiva concentra a regra sem tocar FFI, AppKit, dispatcher ou overlay.
- O diagnóstico `GeometrySource::Cursor` já existe e permite comprovar a nova rota no gate real.
- A matriz de testes substitui expectativas legadas que permitiam cursor sem associação espacial.

### Decisão sintetizada

Implementar somente `SelectedRange → Cursor contido no FocusedElement → FocusedElement → None`, com margem zero e comparação inclusiva. Não tratar staleness, timing ou proximidade textual nesta alteração. O gate real posterior pelo agente raiz decide se a heurística é suficiente no Slack antes de merge/release.

## 3. TASKS

- [x] T1 [LOW] Registrar evidência causal e draft SDD.
- [x] T2 [LOW] Executar análise dual do plano.
- [x] T3 [LOW] Sintetizar o contrato final.
- [ ] T4 [LOW] Executar análise dual de implementação.
- [ ] T5 [LOW] Implementar a política pura de prioridade e contenção.
- [ ] T6 [LOW] Criar testes puros da matriz de decisão.
- [ ] T7 [LOW] Executar gates Rust/frontend/build/diff e scan aplicável.
- [ ] T8 [LOW] Executar QA dual e obter verdict.
- [ ] T9 [LOW] Atualizar memória e documento de entrega.
