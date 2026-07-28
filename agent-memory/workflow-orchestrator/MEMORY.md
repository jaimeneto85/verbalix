# Agent Memory — workflow-orchestrator

## Padrões do Projeto
- O Verbalix usa Tauri 2 como shell desktop e Rust para a lógica central e integrações nativas do macOS.
- Funcionalidades dependentes de seleção global são isoladas atrás de contratos testáveis e adapters de plataforma.
- Toda tarefa é executada em worktree dedicado e só é integrada à branch de origem após aprovação explícita.

## Decisões Arquiteturais
- O MVP tem macOS 14 como versão mínima e distribuição direta, assinada e notarizada fora da Mac App Store.
- Settings e onboarding usam a WebView do Tauri; observação de seleção, geometria e overlays usam APIs nativas do macOS.
- Resultados atrasados nunca podem alterar uma seleção nova: toda transformação referencia e revalida um snapshot.
- O caminho primário usa Accessibility API. O fallback copy-only preserva/restaura o clipboard, só ocorre pelo atalho Option+Shift+Space e nunca simula colagem.
- A transformação usa Supabase Edge Function com OpenAI atrás de `AiProvider`; o modelo é definido por ambiente e a chave não chega ao cliente.
- Supabase Auth usa magic link, a sessão fica no Keychain e o histórico opcional tem RLS owner-only e retenção de 30 dias.
- Magic links do app desktop exigem a entrada exata `verbalix://auth/callback` na allow-list remota do Supabase Auth; ausência da entrada faz o serviço retornar ao `site_url`, mesmo quando `emailRedirectTo` está correto no cliente.

## Erros Recorrentes & Soluções
- Repositórios sem commit não permitem o worktree convencional: criar primeiro um commit-base vazio em `main`.
- Nem todo aplicativo implementa todos os atributos AX: tratar ausência, timeout e elemento invalidado como falhas recuperáveis.
- O ícone carregado pelo Tauri no startup precisa ser PNG 8-bit RGBA. Um PNG 16-bit causou panic em `did_finish_launching`, antes da UI abrir.
- Ao validar correções de bundle, reconstruir o `.app` e conferir `Contents/Resources`, Info.plist, `codesign --verify --deep --strict` e launch smoke; artefatos antigos podem mascarar a correção.
- Callbacks do AXObserver não executam na main thread. Toda criação, configuração, posicionamento, emissão e show/hide de NSWindow/NSPanel deve passar pelo dispatcher `run_on_main_thread`; AppKit fora desse boundary encerra o processo com `Must only be used from the main thread`.
- Recapturas AX equivalentes criam novos UUIDs; `refresh_selection` deve retornar o snapshot ativo quando `same_target` para que polling/AXObserver debouncem o ID armazenado no coordenador.
- Uma ação de toolbar (`transform_selection`) leva segundos (refresh de sessão + provider, abort 20s). Durante ela, os entrypoints automáticos de detecção (polling, AXObserver, monitor global de mouse) precisam ser SUSPENSOS, senão uma falha de captura AX no meio despacha `Invalidated → hide_all → Idle` e fecha a nota recém-aberta ("abre e fecha"). Solução: estender `RuntimePause` (o single gate) com contador atômico `in_flight` + `ActionGuard` RAII aberto no topo de `transform_selection`; compor `!is_action_in_flight()` em `run_polling`/`run_ax_observer`/`run_mouse_dismiss`.
- O dismiss legítimo (`dismiss_overlays` de Escape/botão da nota, tray "Pausar", `undo`) despacha `Invalidated` DIRETO no coordinator, sem passar pelos entrypoints automáticos — por isso um gate aplicado só a polling/observer/mouse-dismiss preserva o dismiss do usuário integralmente.
- ATENÇÃO ao re-check pós-debounce (após o `thread::sleep(150ms)`): tanto a thread de polling quanto o callback do AXObserver têm um segundo dispatch de `DebounceElapsed` DEPOIS do sleep; ambos precisam checar `!is_paused() && !is_action_in_flight()`. QA pegou o AXObserver com o check interno faltando (só polling tinha) — sempre alinhar os dois pontos.
- O gate in-flight precisa de um curto período de graça pós-`Drop` da guarda (relógio injetável para teste determinístico, NÃO sleep) para cobrir o gap IPC+render entre `transform_selection` retornar e o frontend exibir a nota; sem isso uma falha de captura pós-retorno ainda reproduz o bug.
- Fallback de "última geometria conhecida" para nota de erro (`last_known_bounds`) DEVE ser escopado por `is_action_in_flight()` e o cache limpo em `Invalidated`. Caso contrário vaza para `ai_readiness` standalone (nota fantasma em posição obsoleta sem seleção) e reabre nota "zumbi" após dismiss legítimo durante a ação. Mouse-dismiss gate SÓ por `!is_action_in_flight()`, nunca por `is_paused()`, para não mudar a semântica de pausa.
- Um bundle ad-hoc sem `TeamIdentifier` usa requisito designado por `cdhash`; uma entrada antiga habilitada em Acessibilidade pode estar stale para o build atual. A recuperação é remover a entrada antiga, adicionar o bundle exato, habilitar e reabrir — nunca resetar TCC automaticamente.
- Depois de trocar dinamicamente uma `WebviewWindow` para `NSPanel`, setters do wrapper Tauri que dependem dos ivars da classe original podem causar panic; configure o painel inteiramente no boundary AppKit e não chame `set_focusable` após o swizzle.
- `AXBoundsForRange` pode falhar em apps como Slack. Nunca materializar a falha como retângulo sentinela: validar o range, tentar frame/posição+tamanho AX e por último o cursor global via Core Graphics.
- Superfícies Tauri transparentes também precisam neutralizar o fundo e as dimensões mínimas de `html/body/#root`; `transparent(true)` sozinho não remove o canvas CSS opaco.
- Coordenadas AX globais não devem passar por `LogicalPosition` no macOS. Converter uma vez para Cocoa usando `NSScreen.screens.firstObject()` como zero screen e aplicar `setFrameOrigin:` em pontos evita dupla escala Retina e a tela da key window.
- Quando `AXBoundsForRange` não existe, a geometria segue `SelectedRange → Cursor contido no frame focado → FocusedElement → None`; cursor global sem frame válido nunca é aceito e não recebe margem implícita.
- Contenção cursor-frame é uma heurística espacial, não temporal. Em editores grandes, validar por Computer Use seleção por mouse, teclado e cursor movido antes de merge/release; staleness exige um sinal causal separado.
- Readiness de overlay precisa de UUID por documento, caller `NSView`, ACK após a main thread e compare-and-invalidate. Reload e rollback devem destruir/inutilizar apenas a própria geração, nunca o documento atual.
- A criação de overlay é transacional: falha depois do build invalida a geração e destrói a janela, com hide diagnosticado como fallback.
- Publicação visual guardada precisa separar a lifetime cancelável da ação de um permit single-use por comando. Um claim único na lifetime bloqueia feedbacks sequenciais legítimos como Preview → erro de Apply.
- O boundary visual correto é `prepare → claim do permit → emit/show`: cancelamento durante preparação vence com zero efeito; cancelamento depois do claim lineariza `publish → hide` e termina oculto.
- `AXSecureTextField` é subrole. Gates de privacidade precisam classificar `AXRole + AXSubrole` antes de identifier, bounds, settable, token ou qualquer leitura de conteúdo, inclusive no último boundary do setter e em reconcile.
- Eventos de foco/destruição precisam revogar a geração antes de qualquer leitura AX auxiliar. Eventos de seleção própria exigem correlação one-shot forte; ausência ou mismatch deve falhar como evento externo imediatamente.
- AXIdentifier é identidade causal interna e não pertence a DTO/serde/IPC/Debug. Redigir somente o token não basta se o snapshot ainda serializa a mesma informação.
- Mutation ledgers devem expor outcomes tipados por operação; uma API genérica de terminalização permite transições cruzadas inválidas mesmo quando os callers atuais parecem corretos.

## Aprendizados de QA
- A matriz de compatibilidade precisa cobrir seleção por mouse e teclado, campos editáveis e somente leitura, múltiplos monitores e conteúdo Unicode.
- Testar separadamente detecção, leitura, bounds e escrita evita mascarar incompatibilidades específicas dos aplicativos.
- Pausar precisa bloquear todos os entrypoints: polling, AXObserver, atalho global e fallback de clipboard.
- Eventos de overlay não são enfileirados para listeners futuros; resultados de nota precisam de estado persistido e state pull após registrar o listener.
- Aprovação automatizada de código não substitui o spike manual AX/AppKit na matriz antes da distribuição.
- Mudança frontend, mesmo restrita a UX de permissão, precisa do gate E2E além de Vitest; o E2E simulado deve declarar explicitamente que não comprova o estado real do TCC.
- Smoke de botão sem backend deve provar erro visível/acionável, não IA funcional. Tradução real exige configuração pública do Supabase embutida no bundle, Edge implantada e sessão válida.
- Corridas de overlay devem ser testadas com sincronização determinística, incluindo primeira e segunda publicação da mesma ação, ACK tardio e visibilidade final; sleeps não provam a ordem.

## Dependências & Integrações
- Accessibility e AppKit exigem permissão de Acessibilidade concedida pelo usuário.
- Conteúdo selecionado só pode sair da máquina depois de ação explícita.
- Segredos nunca pertencem à WebView, ao repositório ou aos logs.
- Finder não herda variáveis do shell; configuração pública necessária ao cliente deve ser embutida no build do bundle ou carregada de fonte persistida, enquanto segredos permanecem no backend.
- `VITE_SUPABASE_URL` e `VITE_SUPABASE_ANON_KEY` são o par canônico compartilhado. O nativo aceita `VERBALIX_*` apenas como par legado completo; nunca misturar URL de uma fonte com key de outra.
- Para embutir configuração pública sem expô-la no output do build script, gerar fonte Rust em `OUT_DIR` e incluí-la no binário; não transportar valores por `cargo:rustc-env`.
- Worktrees não recebem arquivos ignorados como `.env`; o smoke pré-merge precisa provisionar o arquivo localmente sem logar valores, enquanto o checkout principal resolve `../.env` normalmente.
- Deploy de Edge Function com provider externo é bloqueado antes da publicação quando qualquer secret obrigatório está ausente; nunca publicar deliberadamente um endpoint que só responderá 500.
- Chave pública Supabase no formato JWT legado não prova sessão de usuário. Além de `verify_jwt`, confirmar o bearer no Auth endpoint e rejeitar papel/token anônimo antes de chamar o provider.
- A Edge Function `transform` foi implantada com `verify_jwt=true`; endpoint non-404 e rejeições de request sem autenticação/token anônimo foram comprovados. O smoke autenticado de IA permanece gate operacional dependente de sessão de usuário.
- A matriz de MVP é Chrome, Safari, VS Code, Slack, Notes e TextEdit.
- Para mutações pontuais da configuração Auth, usar GET → revalidação → PATCH contendo somente o campo necessário → GET, com evidências por booleanos/contagens, token do CLI consumido do Keychain sem output e rerun idempotente.
- Evidência de configuração remota precisa ser contemporânea e persistida para QA: separar relato histórico não auditável de um novo ciclo GET/no-op/GET, registrar categorias HTTP, hashes canônicos, contagens, decisão de payload e cleanup sem persistir respostas ou identificadores.

## Observações
- A promessa do produto é mensurável na matriz de aplicativos suportados e best-effort nos demais; não existe evento universal de seleção no macOS.
- O código do MVP recebeu verdict final `APPROVED`; T5.4 e T5.5 permanecem gates manuais de pré-release.
- RF42 foi aprovado: todos os sinais causais cancelam `Armed|Authorizing` antes do bump; writer-wins preserva `InSetter|Committed`. Gates: Rust 229/229, Vitest 55/55, Playwright 6/6, Deno 38/38 e limite de 300 linhas.
- Ao limpar worktrees, auditar novamente imediatamente antes da remoção: arquivos não rastreados podem surgir depois da auditoria inicial. Preservar trabalho incompleto em bundle/cópia fora do repositório antes de excluir branch ou usar `--force`.
- Os masters da marca vivem em `branding/`; ícones derivados para targets Tauri vivem em `src-tauri/icons/`.
- Existe um gate de tamanho de arquivo real e enforced: `src/bundle-smoke.test.ts` assert `lib.rs` (runtime composition root) com `split("\n").length <= 301` (~300 linhas). Mudanças em `lib.rs` que adicionem linhas quebram esse teste vitest — reduzir extraindo responsabilidade (ex.: inline de função single-use como `trigger_shortcut`, ou mover fiação), nunca comprimir artificialmente nem comentar.
- O ambiente pode limpar/resetar worktrees e avançar `main` ENTRE chamadas de ferramenta (aconteceu neste projeto: worktree recriado sumiu e `main` andou de f9fd6d1 para a7febf8). Verificar `git worktree list` ao retomar; se o worktree sumiu, recriá-lo a partir do `main` atual e revalidar que o código-alvo do plano ainda bate antes de delegar.
- Sub-agentes de implementação podem retornar mensagem final TRUNCADA (status intermediário) em tarefas longas com muitos gates. Não confiar só no texto retornado: inspecionar o worktree (`git log`, `git status`, checkboxes do plan.md) para o estado real e, se preciso, re-delegar uma continuação bounded. Os gates pesados (`test:coverage`, `e2e`, `tauri build --debug`) podem ser rodados pelo próprio orquestrador (verificação read-only) para evitar timeouts do sub-agente.
