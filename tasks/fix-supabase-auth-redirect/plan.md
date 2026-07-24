# Fix: redirect do magic link para o app

## 0. SCOPE

Adicionar `verbalix://auth/callback` à allow-list remota do Supabase Auth sem alterar código ou sobrescrever configuração existente.

Incluído:

- confirmar que cliente, deep-link plugin e PKCE já usam a callback exata;
- ler a configuração Auth remota atual;
- anexar idempotentemente apenas `verbalix://auth/callback` a `uri_allow_list`;
- aplicar PATCH mínimo no projeto derivado do par `VITE_SUPABASE_*`;
- verificar presença exata, ausência de wildcard e novo fluxo magic link.

Fora do escopo:

- alterar `site_url`, templates, providers, SMTP, rate limits ou demais campos Auth;
- adicionar `verbalix://*`, `verbalix://**` ou qualquer wildcard;
- reutilizar code/token de magic link já emitido;
- imprimir project ref, PAT, anon key, sessão, code de uso único ou allow-list completa;
- alterar código sem evidência de divergência;
- merge/push; a configuração remota é o artefato principal.

## 1. REQUIREMENTS

- R1: callback permitida é exatamente `verbalix://auth/callback`.
- R2: os valores e a ordem lógica das entradas preexistentes permanecem inalterados; nenhuma normalização, ordenação ou deduplicação é feita.
- R3: a callback é adicionada no máximo uma vez; rerun é idempotente.
- R4: PATCH envia somente `uri_allow_list`; nenhum outro campo remoto é escrito.
- R5: `site_url` e demais configurações permanecem inalterados.
- R6: nenhuma URI wildcard é criada. Se o baseline já contiver wildcard, a operação aborta sem tentar removê-lo.
- R7: GET/PATCH/GET ocorre por HTTPS com a credencial da CLI já autorizada, mantida fora de output e arquivos do projeto.
- R8: verificação sanitizada reporta apenas contagem anterior/posterior, presença exata e igualdade dos demais campos.
- R9: teste funcional usa um magic link novo e nunca reutiliza o code anterior.
- R10: sucesso exige que o redirect novo abra `verbalix://auth/callback?...`, e não `http://localhost:3000`.
- R11: o GET inicial precisa retornar 2xx, JSON válido e `uri_allow_list` em representação conhecida; campo ausente, nulo ou ambíguo aborta sem PATCH.
- R12: a identidade do projeto é derivada somente de uma URL canônica `https://<ref>.supabase.co`, sem path/query, e é validada sem expor o ref.
- R13: antes do PATCH, um segundo GET precisa confirmar que a allow-list não mudou; qualquer drift aborta como conflito.
- R14: se a callback já existir uma ou mais vezes, a operação é no-op. Mais de uma ocorrência preexistente aborta a validação final em vez de higienizar configuração fora do escopo.
- R15: timeout ou resposta 401/403/404/409/429/5xx não autoriza retry cego; um GET sanitizado reconcilia o estado antes de qualquer decisão.
- R16: stdout/stderr usam allow-list de evidências sanitizadas. Bodies, headers, URLs completas e diffs nunca são impressos.
- R17: temporários usam `umask 077`, diretório dedicado e limpeza em sucesso, erro ou sinal.
- R18: rollback só pode restaurar a lista original se uma leitura confirmar que o estado remoto ainda corresponde exatamente ao payload produzido por esta operação; conflito gera `REMOTE_STATE_UNCERTAIN`.

## 2. DESIGN

### Evidência de código

O cliente já chama `signInWithOtp` com `emailRedirectTo: "verbalix://auth/callback"`, registra o scheme `verbalix` no Tauri e troca um `code` recebido via `onOpenUrl`. Os testes existentes cobrem os três contratos. Portanto, nenhum patch local é necessário nesta etapa.

### Mutação mínima

1. derivar project ref em memória da URL canônica, sem eco;
2. GET `/v1/projects/{ref}/config/auth`;
3. manter a resposta original apenas em memória/arquivo temporário protegido por modo `0600` e cleanup via trap;
4. validar o tipo real de `uri_allow_list`; preservar entradas e ordem conforme a representação observada, sem conversão destrutiva;
5. se a callback exata já existir, não executar PATCH;
6. abortar se houver wildcard preexistente, duplicata exata ou schema inesperado;
7. repetir o GET imediatamente antes da escrita e abortar se houver drift;
8. caso contrário, anexar uma única entrada exata;
9. PATCH `/v1/projects/{ref}/config/auth` com body contendo somente `uri_allow_list`;
10. GET novamente e comparar estruturalmente todos os demais campos, sem serializar diff;
11. verificar que a sequência anterior é prefixo exato da posterior e que o único delta esperado é a callback;
12. reconciliar timeout/resposta ambígua por leitura, sem repetir escrita;
13. reportar somente booleanos, contagens e categorias de status.

### Gates sanitizados

- `present_before`: callback exata encontrada no baseline;
- `patch_performed`: escrita executada somente quando necessária;
- `present_after`: callback exata presente após a operação;
- `exact_callback_count`: exatamente `1`;
- `existing_order_preserved`: sequência anterior preservada;
- `only_expected_append`: delta lógico `+1` quando ausente ou `0` quando já presente;
- `other_fields_equal`: igualdade estrutural dos demais campos;
- `wildcard_present`: `false`;
- `cleanup_ok`: temporários removidos;
- rerun: `patch_needed=false`.

### Verificação funcional

- solicitar um magic link novo pelo app;
- observar o redirect efetivamente entregue ao sistema, e não assumir que o href bruto do e-mail já é a callback;
- inspecionar somente scheme, host e path esperados, sem registrar query/code;
- abrir o link uma vez;
- confirmar callback no app aberto e em cold start, além da sessão salva no Keychain por estado booleano;
- invalidar qualquer dado temporário e não reutilizar o code.

### Rollback

O valor original de `uri_allow_list` é mantido apenas durante a operação. Se a verificação estrutural falhar, restaurar somente quando uma leitura confirmar que o estado atual ainda é exatamente o produzido por esta execução. Se houver drift concorrente, não sobrescrever e reportar `REMOTE_STATE_UNCERTAIN`. Não alterar `site_url` para mascarar falha.

## 2.1 DUAL ANALYSIS SYNTHESIS

### 🔴 Riscos incorporados

- schema de `uri_allow_list` precisa ser observado e validado antes de construir o PATCH;
- preservação é medida por valores e ordem lógica, não pela serialização JSON;
- GET/PATCH/GET tem janela de corrida, reduzida por revalidação imediatamente antes da escrita;
- rollback cego é proibido;
- temporários, credenciais e respostas seguem redaction por allow-list;
- smoke mockado não comprova entrega real do deep link, especialmente em cold start.

### 🟢 Oportunidades incorporadas

- cliente, scheme e exchange PKCE já implementam a callback exata, eliminando mudança de código;
- o delta remoto esperado é determinístico: `+1` quando ausente ou `0` quando presente;
- GET → append literal → PATCH mínimo → GET permite provar idempotência com gates booleanos;
- um segundo run deve resultar em `patch_needed=false`;
- testes existentes protegem o contrato local, enquanto um magic link novo fecha a verificação end-to-end.

### Decisão

Autorizar somente a mutação remota mínima quando todos os preconditions forem satisfeitos. Qualquer schema inesperado, wildcard, duplicata, drift, falha HTTP ou impossibilidade de redaction resulta em abort sem escrita.

## 3. TASKS

- [ ] T1 Concluir análise dual e síntese.
- [ ] T2 Ler configuração Auth atual e produzir baseline sanitizada.
- [ ] T3 Aplicar append idempotente via PATCH mínimo.
- [ ] T4 Verificar preservação integral e callback exata sem wildcard.
- [ ] T5 Executar smoke com magic link novo e callback no app.
- [ ] T6 QA independente emitir verdict.
- [ ] T7 Documentar evidências sanitizadas e rollback.
