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

## Aprendizados de QA
- A matriz de compatibilidade precisa cobrir seleção por mouse e teclado, campos editáveis e somente leitura, múltiplos monitores e conteúdo Unicode.
- Testar separadamente detecção, leitura, bounds e escrita evita mascarar incompatibilidades específicas dos aplicativos.
- Pausar precisa bloquear todos os entrypoints: polling, AXObserver, atalho global e fallback de clipboard.
- Eventos de overlay não são enfileirados para listeners futuros; resultados de nota precisam de estado persistido e state pull após registrar o listener.
- Aprovação automatizada de código não substitui o spike manual AX/AppKit na matriz antes da distribuição.
- Mudança frontend, mesmo restrita a UX de permissão, precisa do gate E2E além de Vitest; o E2E simulado deve declarar explicitamente que não comprova o estado real do TCC.
- Smoke de botão sem backend deve provar erro visível/acionável, não IA funcional. Tradução real exige configuração pública do Supabase embutida no bundle, Edge implantada e sessão válida.

## Dependências & Integrações
- Accessibility e AppKit exigem permissão de Acessibilidade concedida pelo usuário.
- Conteúdo selecionado só pode sair da máquina depois de ação explícita.
- Segredos nunca pertencem à WebView, ao repositório ou aos logs.
- Finder não herda variáveis do shell; configuração pública necessária ao cliente deve ser embutida no build do bundle ou carregada de fonte persistida, enquanto segredos permanecem no backend.
- A matriz de MVP é Chrome, Safari, VS Code, Slack, Notes e TextEdit.

## Observações
- A promessa do produto é mensurável na matriz de aplicativos suportados e best-effort nos demais; não existe evento universal de seleção no macOS.
- O código do MVP recebeu verdict final `APPROVED`; T5.4 e T5.5 permanecem gates manuais de pré-release.
- Os masters da marca vivem em `branding/`; ícones derivados para targets Tauri vivem em `src-tauri/icons/`.
