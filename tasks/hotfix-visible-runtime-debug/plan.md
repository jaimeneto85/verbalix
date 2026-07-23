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
- R10: bounds inválidos de `AXBoundsForRange` não podem virar silenciosamente `(0,0,1,1)`; a origem geométrica escolhida deve ser válida, rastreável e próxima ao alvo.

## 2. DESIGN

### Causa comprovada

O trace do bundle exato chegou a:

1. seleção TextEdit capturada;
2. candidato/debounce aceito;
3. comando executado na main thread;
4. toolbar criada e posicionada;
5. panic na main thread em `WebviewWindow::set_focusable(false)`.

`configure_nonactivating_panel` troca dinamicamente a classe da janela para `NSPanel` com `AnyObject::set_class`. Depois desse swizzle, o wrapper Tauri ainda executa `set_focusable(false)` esperando um ivar da classe original; o ivar não existe no objeto convertido e o processo encerra. A ausência da toolbar é consequência desse panic, não de captura, bounds, TCC ou debounce nesta reprodução.

A correção remove a chamada Tauri pós-swizzle. O builder já cria a janela com `.focused(false)` e a configuração nativa aplica `NSWindowStyleMaskNonactivatingPanel`, `setBecomesKeyOnlyIfNeeded` e nível adequado, suficientes para o comportamento não ativante.

### Causa adicional comprovada no smoke

Após remover o panic, o bundle permaneceu vivo e o trace confirmou `toolbar visible=true`. No Slack, porém, a seleção `range_length=3` produziu `bounds=0.0,1117.0,1.0,1.0`: `AXBoundsForRange` falhou, o adapter converteu a falha em retângulo sentinela e o overlay foi corretamente — mas inutilmente — ancorado no canto inferior esquerdo.

A captura passa a resolver geometria nesta ordem:

1. `AXBoundsForRange` quando retornar retângulo finito, positivo e não sentinela;
2. frame do elemento AX, combinando posição e tamanho válidos;
3. posição atual do cursor obtida por API thread-safe e convertida ao mesmo sistema global em pontos usado pelo overlay.

Cada snapshot registra apenas a fonte geométrica (`selected_range`, `focused_element` ou `cursor`) e os números já sanitizados. O conteúdo continua fora do trace. O fallback do cursor é amostrado no momento da captura para manter proximidade com seleção por mouse; seleção por teclado pode cair no frame do elemento.

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

Um helper único reabre e foca a janela principal a partir do tray e de `RunEvent::Reopen`. `CloseRequested` na janela principal impede o encerramento e apenas oculta a janela; quit continua exclusivo à ação explícita do tray/processo.

### Hipóteses a discriminar

- Confirmada: chamada `set_focusable(false)` do Tauri após swizzle para `NSPanel`.
- Rejeitadas nesta reprodução: TCC stale, captura ausente, debounce divergente, bounds inválidos e comando main-thread não executado.
- A monitorar após a correção: janela criada/mostrada e imediatamente ocultada ou lifecycle de fechar/reabrir.

### Análise dual

Riscos (upsidedown):

- `Regular` altera foco, Cmd+Tab e semântica de fechamento; fechar/reabrir precisa ser testado explicitamente.
- rebuilds ad-hoc mudam `CDHash`; congelar caminho/artefato durante toda a reprodução evita reautorizar o bundle errado.
- logs de lifecycle e overlay devem distinguir encerramento real de janela apenas oculta.
- remover `set_focusable` exige comprovar que o painel continua não ativante e clicável.
- frame AX pode cobrir um editor inteiro; deve ser preferido ao cursor apenas quando a seleção não fornece bounds, e o smoke deve avaliar proximidade no Slack/TextEdit.
- coordenadas AX, Core Graphics e Tauri devem ser comparadas em pontos globais, incluindo monitores com escala/origem diferentes.

Oportunidades (downsideup):

- a causa está isolada em uma chamada pós-swizzle e não exige redesenhar o dispatcher.
- `.focused(false)` no builder mais a configuração `NSPanel` existente permitem remover o acesso incompatível.
- `show_main_window`, `CloseRequested` e `RunEvent::Reopen` centralizam um lifecycle previsível e testável.
- o tracing já existente permite confirmar o caminho `created → positioned → visibility=true` sem conteúdo do usuário.

## 3. TASKS

- [x] T1 Concluir análise dual e sintetizar riscos/oportunidades neste plano.
- [x] T2 Adicionar regressões para política de ativação/lifecycle e eventos diagnósticos sem conteúdo.
- [x] T3 Alterar a política para `Regular` e manter reabertura via Dock/tray.
- [ ] T4 Construir e identificar o bundle exato desta branch.
- [x] T5 Reautorizar manualmente o bundle e reproduzir com tracing.
- [x] T6 Corrigir a causa comprovada e adicionar regressão específica.
- [ ] T6b Implementar fallback `AXBoundsForRange → frame AX → cursor`, com validação e trace da fonte.
- [ ] T7 Executar Rust, Clippy, frontend, E2E, Edge, build, bundle e codesign.
- [ ] T8 Executar QA independente com análise dual e verdict.
- [ ] T9 Documentar evidências, limitações e operação manual.
- [ ] T10 Executar smoke de posição no Slack e TextEdit, por mouse e teclado.
