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
- R2: entradas preexistentes da allow-list permanecem byte-a-byte e na mesma ordem.
- R3: a callback é adicionada no máximo uma vez; rerun é idempotente.
- R4: PATCH envia somente `uri_allow_list`; nenhum outro campo remoto é escrito.
- R5: `site_url` e demais configurações permanecem inalterados.
- R6: nenhuma URI wildcard é criada.
- R7: GET/PATCH/GET ocorre por HTTPS com a credencial da CLI já autorizada, mantida fora de output e arquivos do projeto.
- R8: verificação sanitizada reporta apenas contagem anterior/posterior, presença exata e igualdade dos demais campos.
- R9: teste funcional usa um magic link novo e nunca reutiliza o code anterior.
- R10: sucesso exige que o redirect novo abra `verbalix://auth/callback?...`, e não `http://localhost:3000`.

## 2. DESIGN

### Evidência de código

O cliente já chama `signInWithOtp` com `emailRedirectTo: "verbalix://auth/callback"`, registra o scheme `verbalix` no Tauri e troca um `code` recebido via `onOpenUrl`. Os testes existentes cobrem os três contratos. Portanto, nenhum patch local é necessário nesta etapa.

### Mutação mínima

1. derivar project ref em memória da URL canônica, sem eco;
2. GET `/v1/projects/{ref}/config/auth`;
3. manter a resposta original apenas em memória/arquivo temporário protegido;
4. interpretar `uri_allow_list` preservando entradas e ordem;
5. se a callback exata já existir, não executar PATCH;
6. caso contrário, anexar uma única entrada exata;
7. PATCH `/v1/projects/{ref}/config/auth` com body contendo apenas `uri_allow_list`;
8. GET novamente e comparar todos os demais campos com o snapshot original;
9. reportar somente booleanos e contagens.

### Verificação funcional

- solicitar um magic link novo pelo app;
- inspecionar somente o destino/scheme do novo link, sem registrar query/code;
- abrir o link uma vez;
- confirmar callback no app e sessão salva no Keychain por estado booleano;
- invalidar qualquer dado temporário e não reutilizar o code.

### Rollback

O valor original de `uri_allow_list` é mantido apenas durante a operação. Se o PATCH alterar outro campo ou a verificação estrutural falhar, restaurar somente o valor original da allow-list e interromper. Não alterar `site_url` para mascarar falha.

## 3. TASKS

- [ ] T1 Concluir análise dual e síntese.
- [ ] T2 Ler configuração Auth atual e produzir baseline sanitizada.
- [ ] T3 Aplicar append idempotente via PATCH mínimo.
- [ ] T4 Verificar preservação integral e callback exata sem wildcard.
- [ ] T5 Executar smoke com magic link novo e callback no app.
- [ ] T6 QA independente emitir verdict.
- [ ] T7 Documentar evidências sanitizadas e rollback.
