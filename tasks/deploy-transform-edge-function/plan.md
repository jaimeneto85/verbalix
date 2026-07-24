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
- R13: a chave pública atual é JWT legado; o handler deve validar o bearer como usuário real e rejeitar token/papel anônimo mesmo após `verify_jwt`.
- R14: body HTTP tem limite em bytes antes de `JSON.parse`; `formality` precisa ser inteiro.
- R15: resultado do provider tem limite e invariantes por operação; JSON inválido nunca vira 500 genérico.

## 2. DESIGN

### Handler testável

`index.ts` exporta uma factory/handler que recebe:

- factory do `AiProvider`;
- autenticador de usuário;
- getter de secrets;
- timeout scheduler/cancelamento.

`Deno.serve` fica em um entrypoint mínimo separado; importar o handler em teste não inicia servidor. Testes chamam o handler em memória e não usam rede/OpenAI.

Fluxo:

1. rejeitar método não `POST`;
2. exigir `Authorization: Bearer ...` como defesa adicional ao gateway;
3. validar o bearer como usuário Supabase via Auth, sem logar token ou identidade;
4. ler body com limite de bytes, parsear JSON e validar contrato;
5. confirmar os dois secrets por presença;
6. executar provider com `AbortController`;
7. normalizar somente códigos permitidos;
8. responder JSON com `Cache-Control: no-store`.

### Segurança e custo

- JWT é verificado pelo Supabase gateway e confirmado como sessão de usuário pelo Auth endpoint; nunca é repassado ao OpenAI.
- A key OpenAI existe somente como secret da função.
- O provider envia apenas o texto do request explícito, delimitado como não confiável.
- Limite de tamanho, timeout e mapeamento 429 limitam custo/abuso básico do MVP.
- Logs operacionais, se necessários, usam apenas request ID, operação, status e duração; nunca texto ou headers.
- Rate limit por usuário permanece risco residual do MVP; autenticação, body cap, limite de texto/saída, timeout e rate limit do provider são os controles desta entrega.

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

T5 bloqueia T6/T7: ausência de acesso CLI, `OPENAI_API_KEY` ou `OPENAI_MODEL` impede deploy/smoke, sem tentar inventar ou recuperar valores. A máquina inicialmente não possui CLI nem sessão Supabase detectável; autenticação explícita será necessária.

### Rollback

Registrar commit SHA e versão remota sanitizada. Esta é a primeira implantação, portanto não existe versão remota anterior. Se o smoke falhar após deploy, corrigir e redeployar; voltar ao estado ausente exigiria apagar a função e requer confirmação adicional.

### Síntese da análise dual

Riscos (upsidedown):

- JWT de usuário precisa ser obtido para smoke sem aparecer em comando/output; anon key não substitui sessão.
- `verify_jwt` com chave legada não basta para autorização de usuário.
- primeira implantação não tem rollback remoto não destrutivo.
- secrets ausentes precisam bloquear deploy antes de produzir endpoint quebrado.
- body ilimitado, formality fracionária e output sem limite permitem custo ou contrato inválido.
- import de testes não pode disparar `Deno.serve`.

Oportunidades (downsideup):

- `AiProvider`, `OpenAiProvider`, `parseRequest`, prompt/schema e contrato Rust já são reutilizáveis.
- factory de handler com provider/auth/scheduler fake cobre o pipeline sem OpenAI.
- sequência gates → secrets → deploy de uma função → non-404 → auth → IA reduz blast radius.
- smoke pode reportar apenas booleanos/status/invariantes, mantendo tokens e conteúdo fora do output.

## 3. TASKS

- [x] T1 Criar draft, análise dual e síntese final do SDD.
- [ ] T2 Refatorar handler para injeção sem alterar o contrato público.
- [ ] T3 Adicionar testes unitários/integrados para HTTP, secrets, timeout e provider.
- [ ] T4 Executar Deno, Rust, frontend, E2E, Edge, analyzer e scans.
- [ ] T5 Descobrir acesso/projeto e verificar secrets por presença.
- [ ] T6 Implantar `transform` no projeto autorizado.
- [ ] T7 Executar smoke não autenticado e autenticado sem expor dados.
- [ ] T8 QA independente emitir verdict de código e deploy.
- [ ] T9 Documentar versão, evidência, bloqueios e rollback.
