# Agent Memory — workflow-orchestrator

## Padrões do Projeto
- O Verbalix usa Tauri 2 como shell desktop e Rust para a lógica central e integrações nativas do macOS.
- Funcionalidades dependentes de seleção global são isoladas atrás de contratos testáveis e adapters de plataforma.
- Toda tarefa é executada em worktree dedicado e só é integrada à branch de origem após aprovação explícita.

## Decisões Arquiteturais
- O companion iOS começa como Swift Package puro `ios/VerbalixKit` (só Foundation + Security, zero deps externas),
  com `platforms: [.iOS(.v17), .macOS(.v14)]` — macOS existe só para `swift test` rodar no host sem simulador.
  supabase-swift só entra na Fase 3 (app SwiftUI/extensões), ainda pendente de decisão de tooling.
- O contrato iOS (`Transform.swift`) espelha `supabase/functions/transform/contract.ts` no wire (camelCase),
  com `requestId` serializado em lowercase via `CodingKeys`+`encode(to:)`. Guardas 12.000 unicode scalars e
  64 KiB do corpo JSON são INDEPENDENTES; o guard de 64 KiB é pré-check defensivo do cliente (index.ts não
  impõe tamanho de corpo). Fixtures Swift derivam dos mesmos literais de `contract_test.ts` para evitar drift.
- Sync de preferências: só campos de IA (`formality`, `length`, `tone`, `history_enabled`) trafegam;
  `shortcut`/`automatic_toolbar`/`confirm_before_replace` são macOS-only e nunca entram/saem pelo sync.
  `settings.json` continua a fonte de verdade; qualquer falha de rede é não-fatal e nunca propaga erro.
  `load_settings` virou `async fn` (transparente ao frontend, que usa `invoke()` retornando Promise);
  `save_settings` segue não-bloqueante — grava local, re-registra shortcut e só então dispara upsert detached
  via `tauri::async_runtime::spawn`. Adapter `remote_preferences.rs` usa timeout estrito de 4s (bootstrap path).
- LWW de preferências: `updated_at` é server-authoritative (default now() + trigger BEFORE INSERT/UPDATE que
  força now()); merge trata remoto ausente/nulo como "infinitamente antigo" (local vence) e empate mantém local.
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

## Erros Recorrentes & Soluções (iOS/Swift)
- `swift test` bare no host NÃO tem entitlement de keychain-access-groups nem de App Group: `SecItem*` com
  `kSecAttrAccessGroup` e `FileManager.containerURL(forSecurityApplicationGroupIdentifier:)` falham. Solução:
  separar via protocolo (`SessionPersisting`) com double em memória e diretório tmp injetável; o caminho real
  de Keychain/App Group compila mas não roda em CI.
- `XCTAssertEqual(_:_:accuracy:)` exige `FloatingPoint` NÃO-opcional; comparar `TimeInterval?` quebra a
  compilação. Desembrulhar com `try XCTUnwrap` antes de comparar com tolerância.
- `NSLock.unlock()` em contexto assíncrono é warning no Swift 5 e ERRO no Swift 6 language mode. Usar locking
  com escopo (`withLock`) numa seção síncrona, sem `lock()/unlock()` cruzando `await`, em stubs de transporte.

## Aprendizados de QA
- Relatórios finais de sub-agentes podem chegar truncados/otimistas; o orquestrador DEVE re-rodar os gates
  empiricamente (swift test, cargo test/clippy/fmt, npm test/build, deno, e os 3 xcodebuild) antes de aceitar
  qualquer verdict. Ao longo desta entrega os sub-agentes repetidamente: deixaram `Tests/` vazio; entregaram
  teste que não compilava (accuracy sobre Optional); pararam com código quebrado NÃO-commitado (RefreshLock
  com `flock` ambíguo; SessionRefresher lançando `.localFailure`); e terminaram sem commitar apesar de "tudo
  verde". Padrão de mitigação que funcionou: (1) escopo por rodada com "pouse SEMPRE verde+commitado"; (2)
  o orquestrador verifica git log/status + roda os gates a cada retorno; (3) preservar trabalho commitando
  quando o sub-agente esquece.
- Fase 2 (sync de prefs) tinha DEFEITO real de "remoto sempre vence": LWW exige timestamp local. Solução
  aprovada: SIDECAR `preferences_sync.json` (`{updatedAt, syncedAt, sequence}`) fora do `AppSettings` (que
  cruza IPC), com sequence-guard contra race entre `save_settings` síncrono e o spawn de `load_settings`;
  `load_settings` retorna local na hora e emite evento `preferences-synced` (padrão listen/emit de
  `note-result`, com comando de pull de fallback). No iOS, TODA edição local deve chamar `touch()` senão o
  bug reaparece.

## iOS — Prontidão para App Store
- Em `.xcconfig`, `//` inicia COMENTÁRIO: gravar `VerbalixSupabaseURL = https://x.supabase.co` faz o valor
  resolver para `https:` (sem host). O build passa; só o Info.plist COMPILADO revela. `bootstrap.sh` escapa
  a barra (VERBALIX_SLASH). LIÇÃO GERAL: onde valores atravessam camadas (xcconfig→Info.plist→BackendConfig),
  verificar o VALOR FINAL COMPILADO com `plutil -p "<Verbalix.app>/Info.plist"`, não só que compila.
- Versões: `MARKETING_VERSION`/`CURRENT_PROJECT_VERSION` no `settings.base` do project.yml SÓ têm efeito se os
  Info.plist referenciam `$(MARKETING_VERSION)`/`$(CURRENT_PROJECT_VERSION)` — literais `1.0`/`1` ignoram a
  build setting. Centralizar em settings.base garante paridade app↔extensões (divergência = rejeição no upload).
- AppIcon: bloqueio duro. Asset catalog single-size (Xcode 14+): um PNG 1024x1024 com
  `Contents.json {idiom: universal, platform: ios, size: 1024x1024}` + `ASSETCATALOG_COMPILER_APPICON_NAME`.
  Ícone da App Store NÃO pode ter alpha: achatar com `magick "<src>" -background white -alpha remove -alpha off`
  (ImageMagick em /opt/homebrew; `sips` só zera alpha via round-trip JPEG lossy; PIL/pngcrush ausentes) e
  validar `sips -g hasAlpha` == no. COMMITAR o PNG achatado no appiconset para o build não depender de
  ImageMagick. Provar no `.app`: `AppIcon60x60@2x.png` presente e `Assets.car` gerado.
- `UILaunchScreen` (dict vazio basta) é bloqueio duro (SDK iOS 14+); provar no Info.plist compilado.
- Release signing: `CODE_SIGN_STYLE = Automatic` no `Release.xcconfig` (aplica aos 3 targets, pois `configFiles`
  é por-projeto) + `DEVELOPMENT_TEAM` via `Local.xcconfig` gitignored. NÃO mexer no `CODE_SIGNING_ALLOWED = NO`
  do Debug (é o que permite build de simulador sem Team). Build de simulador verde NÃO valida signing de
  Release/device — isso é gate manual.
- Simulador: há `iPhone 17 Pro` duplicados e runtimes 26.3/26.4/26.5 (SDK 26.5); fixar `OS=26.5` no
  `-destination` evita casar com runtime indisponível.

## iOS App + Extensões (Fases 3-5)
- Tooling: XcodeGen (`ios/project.yml` versionado → `ios/Verbalix.xcodeproj` gitignored). Targets: app
  `Verbalix`, `VerbalixAction` (com.apple.ui-services), `VerbalixKeyboard` (com.apple.keyboard-service),
  extensões embedadas, todos dependem do package local `ios/VerbalixKit`. Deployment iOS 17.0.
- Build de simulador SEM Team: entitlements de App Group/Keychain quebram assinatura, então a config de
  simulador usa `CODE_SIGNING_ALLOWED=NO`/`CODE_SIGNING_REQUIRED=NO`/`CODE_SIGN_IDENTITY=""` (em
  `Config/Debug.xcconfig` e/ou na linha de comando do xcodebuild). `DEVELOPMENT_TEAM` via `ios/Local.xcconfig`
  gitignored (`.example` versionado); vazio compila no simulador. Os 3 schemes buildam verdes assim.
- Config Supabase: `ios/scripts/bootstrap.sh` gera `ios/Config/Supabase.xcconfig` (gitignored) do `.env` da
  raiz e roda `xcodegen`. Em WORKTREE o `.env` (gitignored) não existe localmente; resolver a raiz do checkout
  via `git rev-parse --git-common-dir` (pai do `.git` comum), NUNCA `ios/../..` fixo. O script deve falhar
  loud se ausente e NUNCA ecoar URL/anon key.
- supabase-swift entra só aqui (produto `Auth`, resolvido via SPM 2.53.0). `AuthLocalStorage`/`AuthService`
  sobre `SharedSessionStore`; refresh serializado por `RefreshLock` (fcntl `F_SETLK`, NÃO `flock` — `flock`
  colide com o `struct flock` do Darwin) com expiração de lock órfão. Para testar no host, o lock precisa ser
  INJETÁVEL (init com `lockPath` em tmp); a versão de produção usa `containerURL(App Group)`, que no host
  retorna URL inacessível (não-nil) e faz `open(O_CREAT)` falhar com `.localFailure` — mesmo trap do M3.
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
