# Hotfix: runtime visível e reprodução diagnóstica

## 0. SCOPE

Tornar o Verbalix observável como aplicativo macOS normal durante o MVP, reproduzir o encerramento/ausência da toolbar no bundle exato com tracing sanitizado e corrigir somente a causa comprovada.

Incluído:

- usar `ActivationPolicy::Regular` para exibir o app no Dock e no Cmd+Tab;
- construir e executar o bundle deste worktree com `VERBALIX_DIAGNOSTICS=1`;
- registrar identidade/caminho/hash do bundle, confiança AX, lifecycle e pipeline da toolbar;
- reproduzir seleção no TextEdit com o bundle efetivamente autorizado;
- corrigir a causa demonstrada pelo trace/crash report;
- regressões, bundle, smoke e QA independente.

Fora do escopo:

- novas funcionalidades de tradução/aprimoramento;
- reset ou edição automática do TCC;
- esconder novamente o app no Dock antes do diagnóstico ser encerrado;
- merge ou push sem aprovação explícita.

## 1. REQUIREMENTS

- R1: o app deve aparecer no Dock e Cmd+Tab enquanto esta política MVP estiver ativa.
- R2: fechar a janela principal não deve tornar o processo impossível de reencontrar; o Dock/tray deve permitir reabrir Configurações.
- R3: o bundle reproduzido deve ser o desta branch e ter caminho, `CDHash`, assinatura e confiança AX verificados antes do teste.
- R4: o tracing deve cobrir startup/lifecycle, permissão, captura, coordenador, agendamento main-thread, criação/posição/show/visibilidade e encerramento.
- R5: tracing nunca inclui texto selecionado, tokens, credenciais ou conteúdo de clipboard.
- R6: qualquer encerramento deve ser correlacionado com exit status e crash report do mesmo executável.
- R7: a toolbar deve aparecer após seleção válida no TextEdit com processo vivo.
- R8: a correção deve atacar somente uma falha comprovada; hipóteses não comprovadas permanecem observações.
- R9: nenhum acesso AppKit ocorre fora da main thread.

## 2. DESIGN

### Procedimento de reprodução

1. Limpar somente artefatos de build deste worktree e gerar um bundle debug novo.
2. Registrar caminho absoluto, `codesign -dvvv`, designated requirement e hash do executável.
3. Remover manualmente a entrada TCC antiga, adicionar o bundle exato, habilitar e encerrar/reabrir.
4. Confirmar `AXIsProcessTrusted` pelo status exibido no app.
5. Executar `Contents/MacOS/verbalix` a partir do Terminal com `VERBALIX_DIAGNOSTICS=1`, preservando stdout/stderr e exit status em arquivo temporário sem conteúdo.
6. Abrir TextEdit, selecionar texto por mouse e teclado e observar sequência completa do trace.
7. Se houver encerramento, correlacionar timestamp/PID/CDHash com o relatório em `~/Library/Logs/DiagnosticReports`.
8. Se o processo permanecer vivo sem toolbar, usar os últimos estágios registrados para localizar o primeiro boundary ausente ou divergente.

### Lifecycle observável

`ActivationPolicy::Regular` deve ser configurada na main thread durante setup. O diagnóstico registra startup, política aplicada, janela principal disponível, eventos de abertura/fechamento e solicitação explícita de quit sem registrar dados do usuário.

### Hipóteses a discriminar

- autorização AX ainda stale para o bundle executado;
- captura falha antes de criar candidato;
- candidato/debounce é invalidado por mouse/polling;
- comando main-thread é agendado mas não executado;
- janela é criada/mostrada e imediatamente ocultada;
- processo encerra por violação AppKit, panic ou lifecycle.

## 3. TASKS

- [ ] T1 Concluir análise dual e sintetizar riscos/oportunidades neste plano.
- [ ] T2 Adicionar regressões para política de ativação/lifecycle e eventos diagnósticos sem conteúdo.
- [ ] T3 Alterar a política para `Regular` e manter reabertura via Dock/tray.
- [ ] T4 Construir e identificar o bundle exato desta branch.
- [ ] T5 Reautorizar manualmente o bundle e reproduzir com tracing.
- [ ] T6 Corrigir a causa comprovada e adicionar regressão específica.
- [ ] T7 Executar Rust, Clippy, frontend, E2E, Edge, build, bundle e codesign.
- [ ] T8 Executar QA independente com análise dual e verdict.
- [ ] T9 Documentar evidências, limitações e operação manual.
