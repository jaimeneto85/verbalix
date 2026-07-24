# 006 — Edge Function `transform`: implantada

## Implementação

O handler foi separado de `Deno.serve` e recebe autenticação, provider, secrets e scheduler por injeção. O boundary agora:

- exige usuário Supabase real e rejeita anon key/token anônimo;
- limita body antes do parse;
- exige formality inteira e contrato completo;
- limita e valida output por operação;
- cancela o provider após 20 segundos;
- normaliza erros sem registrar token ou conteúdo.

## Evidência de qualidade

- re-QA independente de código: `APPROVED`.
- Deno fmt/lint/check: aprovado.
- Deno tests: 34/34.
- Worktree sem alterações de produção após o verdict.

## Estado remoto

A Supabase CLI foi autenticada pela conta já ativa no navegador e o projeto derivado do par `VITE_SUPABASE_*` foi confirmado sem exibir identificadores ou valores.

Depois do provisionamento explícito, os dois secrets obrigatórios foram confirmados somente por presença e a função `transform` foi implantada com `verify_jwt=true`.

Evidências sanitizadas:

- endpoint ativo e diferente de 404;
- request sem autenticação rejeitado;
- token/papel anônimo rejeitado;
- nenhum secret, JWT, identificador de projeto, request ou resposta textual registrado.

## Gate operacional e rollback

O smoke autenticado de IA ainda precisa de uma sessão Supabase de usuário. Ele deve usar texto técnico sintético e validar apenas status, correlação do request, idiomas e resultado não vazio, sem imprimir conteúdo.

Esta foi a primeira implantação; não há versão remota anterior para rollback. Se o smoke autenticado revelar defeito, corrigir e redeployar. Remover a função para voltar ao estado 404 é destrutivo e exige confirmação adicional.

Classificação: segura para merge, com smoke autenticado pós-merge explicitamente pendente.
