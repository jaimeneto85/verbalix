# Hotfix: toolbar de seleção não aparece

## 0. SCOPE

Restaurar a exibição confiável da toolbar após uma seleção válida no macOS, mantendo todo acesso AppKit na main thread.

Incluído:

- rastrear, sem conteúdo selecionado, as etapas AXObserver/polling, captura, coordenador, dispatch main-thread e visibilidade;
- corrigir a causa comprovada no pipeline;
- adicionar regressões automatizadas para identidade/debounce, ordem de comandos e falhas;
- validar o bundle e realizar smoke manual local quando as permissões TCC permitirem.

Fora do escopo:

- mudanças em tradução, aprimoramento, autenticação ou UI visual;
- novos provedores ou dependências;
- merge ou push antes de aprovação explícita.

Fluxo simplificado: trata-se de hotfix localizado em um pipeline já especificado. Mantemos especificação, implementação, testes e QA, mas sem reabrir a arquitetura do MVP.

## 1. REQUIREMENTS

- R1: uma seleção válida e estável deve produzir exatamente uma intenção efetiva de `ShowToolbar`.
- R2: capturas equivalentes não podem trocar o ID usado pelo debounce.
- R3: invalidações anteriores à seleção não podem ocultar uma toolbar posterior por reordenação assíncrona.
- R4: `ShowToolbar` deve criar/reusar, posicionar, mostrar e confirmar visibilidade da janela na main thread.
- R5: erros devem degradar sem panic e ficar diagnosticáveis.
- R6: nenhuma telemetria/log pode conter texto selecionado, tokens ou credenciais.
- R7: seleção indisponível continua ocultando overlays.

Critérios de aceite:

- testes determinísticos cobrem recaptura equivalente e debounce concorrente;
- testes cobrem ordenação/execução dos comandos de overlay;
- Rust tests, Clippy, frontend tests/build e bundle passam;
- smoke confirma processo vivo e, se TCC permitir, toolbar visível no TextEdit.

## 2. DESIGN

### Diagnóstico por estágios

Eventos estruturados e sanitizados devem identificar:

1. origem da detecção (`ax_observer`, `polling`, `shortcut`);
2. captura (`pid`, localização/comprimento UTF-16, bounds, writable, snapshot ID);
3. decisão do coordenador (novo alvo, alvo equivalente, debounce aceito/ignorado, invalidação);
4. comando de overlay agendado e executado na main thread;
5. criação/reuso, posição, `show` e `is_visible`.

O rastreio deve ser opt-in via ambiente e não registrar `SelectionSnapshot.text`.

### Invariantes

- A identidade estável pertence ao candidato armazenado pelo coordenador. Ao recapturar o mesmo alvo, os callers recebem o snapshot ativo, não um novo UUID descartado.
- O estado só avança para `ToolbarVisible` depois que a solicitação de exibição é aceita.
- O dispatcher preserva a ordem de comandos enfileirados na main thread.
- Toda consulta ou mutação de janela permanece no closure de `run_on_main_thread`.

### Riscos e oportunidades

Riscos:

- polling e AXObserver podem intercalar capturas e debounces;
- o monitor global de mouse e capturas transitórias podem invalidar estado;
- bounds válidos em AX podem estar fora do frame visível;
- `show()` pode retornar sucesso sem a janela permanecer visível.

Oportunidades:

- concentrar a estabilização no coordenador reduz duplicação nos três entrypoints;
- um trace sanitizado reutilizável reduz o custo de diagnosticar futuras diferenças entre apps;
- fakes existentes permitem reproduzir a corrida sem depender de TCC.

## 3. TASKS

- [ ] T1 Adicionar regressões que reproduzam recaptura equivalente/debounce e ordem do overlay.
- [ ] T2 Implementar rastreio opt-in e sanitizado dos cinco estágios.
- [ ] T3 Corrigir a identidade retornada por `refresh_selection` e qualquer causa adicional comprovada pelo trace.
- [ ] T4 Confirmar criação, posicionamento, show e visibilidade somente na main thread.
- [ ] T5 Executar gates Rust/frontend/Clippy/build/bundle e smoke local.
- [ ] T6 QA independente revisar segurança, concorrência, regressões e emitir verdict.
- [ ] T7 Documentar resultado e evidência em `docs/`.
