# 006 — Edge Function `transform`: pronta para deploy, bloqueada por secrets

## Implementação

O handler foi separado de `Deno.serve` e recebe autenticação, provider, secrets e scheduler por injeção. O boundary agora:

- exige usuário Supabase real e rejeita anon key/token anônimo;
- limita body antes do parse;
- exige formality inteira e contrato completo;
- limita e valida output por operação;
- cancela o provider após 20 segundos;
- normaliza erros sem registrar token ou conteúdo.

## Evidência pré-deploy

- QA independente de código: `APPROVED`.
- Deno fmt/lint/check: aprovado.
- Deno tests: 34/34.
- Worktree sem alterações de produção após o verdict.

## Descoberta remota

A Supabase CLI foi autenticada pela conta já ativa no navegador e o projeto derivado do par `VITE_SUPABASE_*` foi confirmado sem exibir identificadores ou valores.

O deploy não foi executado porque os dois requisitos obrigatórios estão ausentes:

- `OPENAI_API_KEY`;
- `OPENAI_MODEL`.

Também não há valores locais no ambiente ou no repositório. Implantar nesse estado faria toda transformação falhar fechada com erro interno.

## Retomada e rollback

Para retomar:

1. provisionar os dois secrets por canal seguro;
2. repetir a verificação somente por presença;
3. executar a matriz completa de gates;
4. implantar apenas `transform`;
5. confirmar non-404/401 e smoke com JWT de usuário;
6. validar transformação sintética sem imprimir request ou resultado.

Esta é a primeira implantação; não há versão remota anterior para rollback. Se o smoke pós-deploy falhar, corrigir e redeployar. Remover a função para voltar ao estado 404 exige confirmação adicional.
