# Plano — Corrigir latência do provider de transformação

## 0. SCOPE

### Incluído

- [ ] Ajustar o payload da Responses API para resposta rápida e limitada.
- [ ] Usar `reasoning.effort: "none"` explicitamente.
- [ ] Reduzir `max_output_tokens` de 8.000 para limite apropriado a tradução/aprimoramento.
- [ ] Preservar timeout total de 20 segundos e erro tipado/acionável.
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
- [ ] RF02: O payload envia `reasoning: { effort: "none" }`.
- [ ] RF03: `max_output_tokens` é limitado a 500 e coberto por teste.
- [ ] RF04: Timeout continua retornando `PROVIDER_TIMEOUT` 504 com feedback acionável no app.
- [ ] RF05: Transformação autenticada bem-sucedida é seguida de insert e aparece no listHistory do mesmo usuário.

### Requisitos não funcionais

- [ ] RNF01: Nenhum segredo, token ou conteúdo selecionado em logs/commits.
- [ ] RNF02: Testes Deno isolados não dependem de rede/secrets.
- [ ] RNF03: Deploy só ocorre após QA automatizado verde.

### Critérios de aceitação

- [ ] CA01: Teste do provider valida model, reasoning none e `max_output_tokens: 500`.
- [ ] CA02: Teste de timeout continua vencendo provider não cooperativo.
- [ ] CA03: Smoke autenticado real de Translate e Improve conclui abaixo do hard timeout.
- [ ] CA04: Histórico lista os dois resultados para o usuário e não expõe registros de outro usuário.
- [ ] CA05: Erro remoto continua mapeado por código tipado, sem “falha silenciosa”.

### Edge cases

- Resposta idêntica à entrada.
- Conteúdo técnico próximo do limite permitido.
- Timeout, 429, 5xx, JSON inválido e output vazio.
- Insert de histórico falha depois da transformação.

## 2. DESIGN

### Fluxo

`request autenticada → handler 20s → OpenAI Responses(gpt-5.4-nano, reasoning none, output limitado) → validate result → client replace/note → history insert → listHistory`

### Decisões

- O modelo permanece configurável por `OPENAI_MODEL`; somente o secret remoto muda.
- `reasoning.effort: none` explicita o perfil de baixa latência suportado pelo modelo.
- O limite de output será 500, uma constante única do provider validada por teste de payload.
- Timeout total não aumenta: latência é resolvida por modelo/payload, preservando UX e contenção de custos.
- Histórico continua após sucesso da transformação; falha independente de history deve ser reportada como nova subtask, sem mascarar a transformação concluída.

## 3. TASKS

### Especificação e testes

- [x] T1.1 `[LOW]` Definir limite de saída adequado com justificativa técnica: chamada direta oficial com `gpt-5.4-nano`, reasoning none e 500 tokens retornou HTTP 200 estruturado em 1,550139 s, abaixo do timeout de 20 s; o payload anterior com `gpt-5-mini` excedeu 20 s.
- [ ] T1.2 `[LOW]` Atualizar teste do payload para reasoning none e limite.
- [ ] T1.3 `[MEDIUM]` Cobrir timeout, erro provider e outputs limítrofes.

### Implementação

- [ ] T2.1 `[LOW]` Ajustar payload do provider.
- [ ] T2.2 `[LOW]` Preservar roteamento tipado de timeout.
- [ ] T2.3 `[MEDIUM]` Integrar testes de insert/list de histórico ao sucesso de Translate/Improve.

### QA e operação

- [ ] T3.1 `[LOW]` Executar Deno lint/test e gates completos do projeto.
- [ ] T3.2 `[MEDIUM]` Atualizar secret remoto para `gpt-5.4-nano` e implantar função após QA.
- [ ] T3.3 `[MEDIUM]` Executar smoke autenticado Translate/Improve e histórico/RLS.
- [ ] T3.4 `[LOW]` Registrar evidências sanitizadas e verdict.
