# Entrega: callback do Supabase Auth

Data: 2026-07-23

## Resultado

A allow-list remota do Supabase Auth passou a conter exatamente:

`verbalix://auth/callback`

Nenhuma alteração de código foi necessária. O cliente já enviava a callback exata, o Tauri já registrava o scheme `verbalix` e o fluxo PKCE já trocava o `code` recebido por sessão.

## Mutação anterior — histórico não auditável

Uma execução anterior reportou a inclusão da callback por PATCH. Como aquela execução não deixou um artefato contemporâneo suficiente para auditoria independente, este documento não usa o relato anterior para provar o conteúdo do payload, as chaves enviadas ou o estado preexistente.

Em particular, não se alega como evidência auditada que a allow-list estivesse vazia antes, que um PATCH específico tenha sido enviado ou que a callback tenha sido introduzida por aquela execução.

## Estado atual e idempotência — auditados

Um novo ciclo estritamente read-only foi executado em `2026-07-24T01:55:54Z`. Ele realizou GET inicial, decisão de idempotência e GET de revalidação. Nenhum PATCH ocorreu e nenhum payload foi construído ou enviado.

- categorias HTTP pre/post: `2xx` / `2xx`;
- schema remoto validado: `true`;
- contagem de entradas pre/post: `1` / `1`;
- contagem exata da callback pre/post: `1` / `1`;
- wildcard pre/post: `false` / `false`;
- PATCH necessário: `false`;
- conjunto de chaves do payload: vazio;
- payload não enviado: `true`;
- allow-list estável entre leituras: `true`;
- hash canônico dos demais campos pre/post: `a159b4cc32ff6c1435292866ed628aeeecedd1f49cccd1c233533c2187453d10`;
- demais campos estáveis: `true`;
- temporários removidos: `true`;
- auditoria final: `true`.

O artefato estruturado está em `docs/evidence/supabase-auth-redirect-current-state.json`. Credenciais, project ref, allow-list completa, headers, respostas remotas e tokens não foram persistidos nem impressos.

## Rollback

Não há baseline histórico auditável suficiente para executar rollback automático. Qualquer rollback exigiria autorização explícita, nova leitura remota e definição humana do estado desejado. Drift concorrente exige interrupção sem sobrescrita.

## Gate pendente

O smoke end-to-end precisa de um magic link novo. O link anterior não deve ser reutilizado. O teste deve cobrir app aberto e cold start, confirmar a callback efetivamente entregue como `verbalix://auth/callback` sem registrar query/code e observar apenas o estado booleano da sessão.

## QA e integração

- testes Auth/deep-link: 8/8;
- re-QA independente após o artefato contemporâneo: `APPROVED`;
- classificação: segura para merge;
- pendência operacional pós-merge: smoke com magic link novo.
