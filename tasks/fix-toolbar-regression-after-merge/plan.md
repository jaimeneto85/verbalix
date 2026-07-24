# Fix: regressão da toolbar após merge

## 0. SCOPE

Restaurar a exibição da toolbar flutuante quando texto é selecionado em apps suportados no macOS, sem regredir segurança, lifecycle, posicionamento ou escrita.

Incluído:

- diagnosticar por estágio a aquisição do app/elemento focado via Accessibility;
- preservar o caminho system-wide quando funcional e adicionar fallback seguro pelo app frontmost;
- excluir o próprio Verbalix como alvo de captura;
- manter polling, AXObserver, debounce e overlay no main thread;
- criar testes de resolução/fallback e regressão;
- validar em app real por Computer Use;
- após QA aprovada, integrar na `main` preservando mudanças user-owned e gerar build release.

Fora do escopo:

- alterar Auth, Supabase Edge Function ou prompts de IA;
- resetar TCC automaticamente;
- ampliar captura para clipboard implícito;
- mascarar falha de captura forçando a toolbar sem snapshot válido;
- remover gates manuais de permissão, assinatura e release.

## 1. REQUIREMENTS

- R1: seleção real no TextEdit deve produzir snapshot e toolbar visível.
- R2: `AXUIElementCreateSystemWide` + `AXFocusedUIElement` permanece caminho primário.
- R3: se o caminho primário falhar, resolver o app frontmost, criar seu elemento AX e consultar `AXFocusedUIElement`.
- R4: o fallback nunca captura Verbalix nem um PID inválido/ausente.
- R5: falhas precisam indicar o estágio sanitizado (`system_wide`, `frontmost_app`, `focused_element`, `selected_text`, `selected_range`) sem conteúdo selecionado.
- R6: ausência de permissão continua falhando fechada e não dispara fallback que contorne TCC.
- R7: captura bem-sucedida preserva texto, range Unicode, writability e geometria existente.
- R8: observer, polling, refresh/apply/restore usam a mesma resolução de elemento focado.
- R9: nenhum AppKit/UI é executado fora da main thread.
- R10: testes cobrem primário, fallback, self-exclusion, app/PID ausente e falha total.
- R11: teste real por Computer Use comprova seleção e toolbar; AX tree do app alvo e screenshot/estado visual formam evidência.
- R12: merge só ocorre após testes e QA `APPROVED`; mudanças locais user-owned na `main` permanecem byte-identical.
- R13: release build só ocorre após merge validado e deve passar bundle, codesign e launch smoke.

## 2. DESIGN

### Evidência reproduzida

- processo Verbalix permaneceu vivo;
- bundle instalado e bundle debug tinham o mesmo executável e assinatura válida;
- `AX trusted=true`;
- TextEdit expôs `Selected text: technical sentence`;
- polling registrou repetidamente `selection_unavailable`;
- coordinator invalidou a seleção e executou `overlay hide`;
- captura falhou antes da geometria.

Os merges recentes não alteraram captura AX, coordinator ou overlay. A causa está concentrada na aquisição do elemento focado no runtime/macOS atual, não no rendering da janela.

### Resolução em dois estágios

1. tentar o elemento focado do objeto AX system-wide;
2. se indisponível e a aplicação continua trusted, obter o PID da aplicação frontmost;
3. rejeitar PID do Verbalix, zero ou ausente;
4. criar `AXUIElement` da aplicação e consultar seu `AXFocusedUIElement`;
5. retornar um elemento owned com lifecycle seguro;
6. prosseguir pelo pipeline atual de texto, range, writability e geometria.

O fallback deve ficar atrás de um boundary testável. A lógica de decisão recebe resultados abstratos do primário, app frontmost e elemento da aplicação; FFI/AppKit apenas fornece adapters.

### Diagnóstico

Emitir somente estágio, origem e categoria de erro. Nunca emitir texto, range bruto, bundle path, token ou conteúdo de clipboard.

### Gate real

1. buildar e instalar o bundle corrigido;
2. confirmar assinatura e processo estável;
3. confirmar permissão AX do bundle atual;
4. selecionar texto técnico no TextEdit por Computer Use;
5. observar toolbar com Traduzir/Aprimorar;
6. repetir seleção por teclado e mouse;
7. confirmar hide ao limpar seleção;
8. repetir ao menos em Notes ou Chrome;
9. registrar evidência sem conteúdo sensível.

### Merge e release

Após QA:

1. registrar hashes de `package.json`, `vitest.config.ts`, `CLAUDE.md` e `README.md` na `main`;
2. merge `--no-ff` da branch;
3. repetir testes críticos;
4. confirmar hashes user-owned inalterados;
5. gerar bundle release;
6. validar recursos, `codesign --verify --deep --strict` e launch smoke;
7. remover worktree/branch somente após entrega confirmada.

## 3. TASKS

- [x] T1 Criar worktree isolada e ler memória.
- [x] T2 Reproduzir e localizar o estágio da falha.
- [ ] T3 Executar análise dual do plano.
- [ ] T4 Sintetizar plano final.
- [ ] T5 Engenharia implementar boundary/fallback e diagnóstico.
- [ ] T6 Testes cobrir resolução e regressão.
- [ ] T7 QA dual revisar código, concorrência, FFI e testes.
- [ ] T8 Build/install debug e validação real por Computer Use.
- [ ] T9 Corrigir rejeições e obter verdict `APPROVED`.
- [ ] T10 Merge preservando dirty main user-owned.
- [ ] T11 Gerar e validar build release.
- [ ] T12 Documentar entrega, evidência e gates residuais.
