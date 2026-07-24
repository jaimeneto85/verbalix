# Plano — Corrigir latência do provider de transformação

## 0. SCOPE

### Incluído

- [x] Ajustar o payload da Responses API para resposta rápida e limitada.
- [x] Usar `reasoning.effort: "none"` explicitamente.
- [x] Tornar `max_output_tokens` proporcional à entrada, com piso 500 e teto 8.000 compatível com o contrato de 12.000 caracteres.
- [x] Preservar timeout total de 20 segundos e erro tipado/acionável.
- [ ] Após QA de código, atualizar `OPENAI_MODEL` remoto para `gpt-5.4-nano`, implantar a Edge Function e executar smoke autenticado.
- [ ] Verificar que transformações bem-sucedidas geram histórico e que insert/list respeitam o usuário autenticado/RLS.

### Arquivos/módulos afetados

- `supabase/functions/transform/provider.ts` e testes.
- `supabase/functions/transform/handler.ts` e testes apenas se o roteamento de timeout exigir ajuste.
- Configuração remota `OPENAI_MODEL` e deploy da função, somente após QA.

### Fora do escopo

- Alterar prompts, contrato público HTTP ou timeout total de 20 segundos.
- Remover autenticação, limites de entrada/saída ou isolamento RLS.
- Persistir chave OpenAI no cliente/repositório.

### Riscos

- Limite de saída baixo demais truncar conteúdo técnico.
- Modelo inválido/indisponível impedir o deploy smoke.
- Reasoning configurado incorretamente ser ignorado ou rejeitado.
- Transformação funcionar, mas histórico falhar independentemente por RLS/UI.

## 1. REQUIREMENTS

### Requisitos funcionais

- [ ] RF01: Translate e Improve usam `gpt-5.4-nano` configurado por secret remoto.
- [x] RF02: O payload envia `reasoning: { effort: "none" }`.
- [x] RF03: `max_output_tokens` usa orçamento proporcional testável, com piso 500 e teto 8.000.
- [x] RF04: Timeout continua retornando `PROVIDER_TIMEOUT` 504 com feedback acionável no app.
- [ ] RF05: Transformação autenticada bem-sucedida é seguida de insert e aparece no listHistory do mesmo usuário.
- [x] RF06: Resposta incompleta por `max_output_tokens` é rejeitada explicitamente e nunca aplicada como sucesso truncado.

### Requisitos não funcionais

- [x] RNF01: Nenhum segredo, token ou conteúdo selecionado em logs/commits.
- [x] RNF02: Testes Deno isolados não dependem de rede/secrets.
- [ ] RNF03: Deploy só ocorre após QA automatizado verde.

### Critérios de aceitação

- [x] CA01: Teste do provider valida model, reasoning none e orçamento de 500 tokens para entrada curta.
- [x] CA02: Teste de timeout continua vencendo provider não cooperativo.
- [ ] CA03: Smoke autenticado real de Translate e Improve conclui abaixo do hard timeout.
- [ ] CA04: Histórico lista os dois resultados para o usuário e não expõe registros de outro usuário.
- [ ] CA05: Erro remoto continua mapeado por código tipado, sem “falha silenciosa”.
- [x] CA06: Entrada técnica longa próxima de 12.000 caracteres recebe orçamento conservador até 8.000 e output truncado falha fechado.

### Edge cases

- Resposta idêntica à entrada.
- Conteúdo técnico próximo do limite permitido.
- Timeout, 429, 5xx, JSON inválido e output vazio.
- Insert de histórico falha depois da transformação.
- Responses API retorna `incomplete_details.reason = "max_output_tokens"`.

## 2. DESIGN

### Fluxo

`request autenticada → handler 20s → OpenAI Responses(gpt-5.4-nano, reasoning none, output limitado) → validate result → client replace/note → history insert → listHistory`

### Decisões

- O modelo permanece configurável por `OPENAI_MODEL`; somente o secret remoto muda.
- `reasoning.effort: none` explicita o perfil de baixa latência suportado pelo modelo.
- O orçamento usa caracteres Unicode da entrada, piso 500 e teto 8.000. A fórmula deve reservar expansão conservadora para PT↔EN, manter o teto compatível com o limite atual de resultado e ser testada nos boundaries.
- Timeout total não aumenta: latência é resolvida por modelo/payload, preservando UX e contenção de custos.
- Histórico continua após sucesso da transformação; falha independente de history deve ser reportada como nova subtask, sem mascarar a transformação concluída.
- `incomplete_details` por limite de output é resposta inválida tipada; conteúdo parcial nunca chega ao replace/history.

## 3. TASKS

### Especificação e testes

- [x] T1.1 `[LOW]` Definir política de saída: piso 500 comprovado para entrada curta, orçamento conservador proporcional e teto 8.000 para preservar o contrato de 12.000 caracteres. Chamada direta oficial com `gpt-5.4-nano`, reasoning none e 500 tokens retornou HTTP 200 estruturado em 1,550139 s; o payload anterior com `gpt-5-mini` excedeu 20 s.
- [x] T1.2 `[LOW]` Atualizar teste do payload para reasoning none e limite.
- [x] T1.3 `[MEDIUM]` Cobrir timeout, erro provider e outputs limítrofes.
- [x] T1.4 `[MEDIUM]` Cobrir entrada longa e resposta `incomplete_details/max_output_tokens`.

### Implementação

- [x] T2.1 `[LOW]` Ajustar payload do provider.
- [x] T2.2 `[LOW]` Preservar roteamento tipado de timeout.
- [x] T2.3 `[MEDIUM]` Integrar testes de insert/list de histórico ao sucesso de Translate/Improve.

### QA e operação

- [x] T3.1 `[LOW]` Executar Deno lint/test e gates completos do projeto.
- [ ] T3.2 `[MEDIUM]` Atualizar secret remoto para `gpt-5.4-nano` e implantar função após QA.
- [ ] T3.3 `[MEDIUM]` Executar smoke autenticado Translate/Improve e histórico/RLS.
- [ ] T3.4 `[LOW]` Registrar evidências sanitizadas e verdict.

## Análise Dual

### 🔴 Riscos incorporados

- Um teto universal de 500 quebraria o contrato atual de entrada de 12.000 caracteres; foi substituído por orçamento proporcional com piso/teto.
- O benchmark de 1,55 s é uma amostra curta; o smoke deve cobrir Translate, Improve e tamanhos distintos sem aumentar o timeout.
- Histórico pode falhar depois de transformação concluída; sucesso de apply e sucesso de sincronização serão evidências separadas.
- RLS exige isolamento real; a validação operacional deve usar contextos autenticados distintos ou evidência equivalente contemporânea.
- Secret/deploy precisam de valor anterior verificável, rollback e smoke imediato, sem revelar valores.

### 🟢 Oportunidades incorporadas

- A principal redução de latência vem de `gpt-5.4-nano + reasoning none`, preservando modelo configurável e timeout de 20 s.
- Provider/handler injetáveis, testes recentes de histórico e roteamento tipado já cobrem a maior parte da infraestrutura.
- Uma função pura de orçamento permite testar entrada curta (500), proporcional e longa (8.000).
- Detecção explícita de `incomplete_details` evita transformar truncamento em conteúdo aplicado.

### Síntese

O payload de baixa latência será otimizado sem enfraquecer o contrato de conteúdo longo: reasoning fica explicitamente desabilitado e o orçamento deixa de ser 8.000 para todos os casos, mas mantém esse teto quando a entrada realmente exige. O deploy remoto terá rollback e só ocorrerá depois do QA de código.

## 🔄 Parallelization Synthesis — Software Engineer

- 🔴 Estimativa pessimista: 1 agente; orçamento, envelope e testes alteram o mesmo boundary.
- 🟢 Estimativa otimista: 2 agentes; orçamento puro poderia ser separado dos fixtures do provider.
- Decisão: 1 agente sequencial. Os arquivos são pequenos, a sobreposição supera 30% e a implementação precisa manter payload e validação atômicos.
- Risco de conflito: baixo.
