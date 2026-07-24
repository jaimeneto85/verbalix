# Entrega: callback do Supabase Auth

Data: 2026-07-23

## Resultado

A allow-list remota do Supabase Auth passou a conter exatamente:

`verbalix://auth/callback`

Nenhuma alteração de código foi necessária. O cliente já enviava a callback exata, o Tauri já registrava o scheme `verbalix` e o fluxo PKCE já trocava o `code` recebido por sessão.

## Evidências sanitizadas

- identidade canônica do projeto validada: `true`;
- schema remoto validado como string: `true`;
- callback presente antes: `false`;
- contagem de entradas antes: `0`;
- wildcard presente antes: `false`;
- PATCH mínimo executado: `true`;
- callback presente depois: `true`;
- contagem exata da callback depois: `1`;
- contagem de entradas depois: `1`;
- entradas e ordem preexistentes preservadas: `true`;
- único delta foi o append esperado: `true`;
- demais campos Auth permaneceram estruturalmente iguais: `true`;
- wildcard presente depois: `false`;
- nova escrita necessária no rerun: `false`;
- temporários protegidos removidos: `true`;
- verificação estrutural final: `true`.

Credenciais, project ref, allow-list completa, headers, respostas remotas e tokens não foram persistidos nem impressos.

## Rollback

O baseline remoto tinha a allow-list vazia. Se um rollback for explicitamente autorizado, ele deve ocorrer somente após nova leitura confirmar que o estado atual ainda contém exclusivamente o delta desta entrega. Qualquer drift concorrente exige interrupção sem sobrescrita.

## Gate pendente

O smoke end-to-end precisa de um magic link novo. O link anterior não deve ser reutilizado. O teste deve cobrir app aberto e cold start, confirmar a callback efetivamente entregue como `verbalix://auth/callback` sem registrar query/code e observar apenas o estado booleano da sessão.
