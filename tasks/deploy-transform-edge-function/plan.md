# Deploy: Supabase Edge Function `transform`

## 0. SCOPE

Concluir, validar e implantar a Edge Function `transform` no projeto Supabase configurado pelo par público `VITE_SUPABASE_*`.

Incluído:

- finalizar o handler HTTP e tornar suas dependências testáveis;
- preservar autenticação JWT, validação, timeout e contrato de erro;
- verificar acesso ao projeto e presença de `OPENAI_API_KEY`/`OPENAI_MODEL` sem revelar valores;
- implantar a função autorizada pelo usuário;
- executar smoke real autenticado com texto técnico sintético;
- documentar versão, evidências e rollback sem registrar secrets/tokens.

Fora do escopo:

- commitar `.env`, tokens Supabase, JWT ou secrets OpenAI;
- imprimir URL completa, project ref, keys, request body ou resposta textual nos logs;
- criar um novo projeto Supabase;
- escolher/substituir `OPENAI_API_KEY` ou modelo ausente sem autorização específica;
- merge ou push; o agente raiz fará o merge após aprovação.

## 1. REQUIREMENTS

- R1: somente `POST` autenticado pode transformar; `verify_jwt=true` permanece habilitado.
- R2: payload inválido retorna código estável sem chegar ao provider.
- R3: texto continua limitado a 12.000 caracteres Unicode e improvement exige preferências completas.
- R4: provider tem timeout total de 20 segundos e mapeia rate limit/timeout/resposta inválida.
- R5: resposta válida preserva `requestId` e contém `sourceLanguage`, `targetLanguage` e resultado não vazio.
- R6: prompts tratam texto selecionado como dado não confiável e preservam tokens técnicos.
- R7: ausência de `OPENAI_API_KEY` ou `OPENAI_MODEL` falha fechada e bloqueia deploy/smoke real.
- R8: nenhum log, teste, documento ou output de ferramenta expõe valores de `.env`, secrets, JWT ou texto transformado.
- R9: project ref é derivado localmente da URL canônica e usado sem ser ecoado.
- R10: deploy só é considerado concluído quando a função listada responde sem 404 e um request autenticado chega ao provider.
- R11: smoke real usa texto sintético sem dados pessoais, credenciais, código proprietário ou conteúdo de clipboard.
- R12: falha de deploy/smoke preserva evidência sanitizada e fornece rollback pela versão/commit anterior.

## 2. DESIGN

### Handler testável

`index.ts` exporta uma factory/handler que recebe:

- factory do `AiProvider`;
- getter de secrets;
- timeout scheduler/cancelamento.

`Deno.serve` apenas conecta essas dependências de produção. Testes chamam o handler em memória e não usam rede/OpenAI.

Fluxo:

1. rejeitar método não `POST`;
2. exigir `Authorization: Bearer ...` como defesa adicional ao gateway;
3. parsear JSON e validar contrato;
4. confirmar os dois secrets por presença;
5. executar provider com `AbortController`;
6. normalizar somente códigos permitidos;
7. responder JSON com `Cache-Control: no-store`.

### Segurança e custo

- JWT é verificado pelo Supabase gateway e nunca repassado ao OpenAI.
- A key OpenAI existe somente como secret da função.
- O provider envia apenas o texto do request explícito, delimitado como não confiável.
- Limite de tamanho, timeout e mapeamento 429 limitam custo/abuso básico do MVP.
- Logs operacionais, se necessários, usam apenas request ID, operação, status e duração; nunca texto ou headers.

### Descoberta e deploy

1. validar apenas a presença do par `VITE_SUPABASE_*` no `.env` ignorado do checkout raiz;
2. derivar o project ref em memória a partir do host, sem output;
3. verificar autenticação da CLI e que o projeto alvo é acessível;
4. listar secrets remotamente e reduzir o resultado a dois booleanos/nome ausente, sem valores;
5. executar todos os gates locais;
6. implantar `transform` explicitamente no project ref derivado;
7. confirmar função listada/ativa e endpoint diferente de 404;
8. executar smoke autenticado com UUID novo e texto técnico sintético;
9. confirmar somente status, requestId correspondente, idiomas presentes e resultado não vazio.

### Rollback

Registrar commit SHA e versão remota sanitizada. Se o smoke falhar após deploy, corrigir e redeployar a versão aprovada anterior; não apagar a função automaticamente. Como o estado inicial é 404/ausente, qualquer rollback destrutivo exige confirmação adicional.

## 3. TASKS

- [ ] T1 Criar draft, análise dual e síntese final do SDD.
- [ ] T2 Refatorar handler para injeção sem alterar o contrato público.
- [ ] T3 Adicionar testes unitários/integrados para HTTP, secrets, timeout e provider.
- [ ] T4 Executar Deno, Rust, frontend, E2E, Edge, analyzer e scans.
- [ ] T5 Descobrir acesso/projeto e verificar secrets por presença.
- [ ] T6 Implantar `transform` no projeto autorizado.
- [ ] T7 Executar smoke não autenticado e autenticado sem expor dados.
- [ ] T8 QA independente emitir verdict de código e deploy.
- [ ] T9 Documentar versão, evidência, bloqueios e rollback.
