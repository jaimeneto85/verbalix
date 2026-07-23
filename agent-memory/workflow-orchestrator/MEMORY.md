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

## Aprendizados de QA
- A matriz de compatibilidade precisa cobrir seleção por mouse e teclado, campos editáveis e somente leitura, múltiplos monitores e conteúdo Unicode.
- Testar separadamente detecção, leitura, bounds e escrita evita mascarar incompatibilidades específicas dos aplicativos.

## Dependências & Integrações
- Accessibility e AppKit exigem permissão de Acessibilidade concedida pelo usuário.
- Conteúdo selecionado só pode sair da máquina depois de ação explícita.
- Segredos nunca pertencem à WebView, ao repositório ou aos logs.
- A matriz de MVP é Chrome, Safari, VS Code, Slack, Notes e TextEdit.

## Observações
- A promessa do produto é mensurável na matriz de aplicativos suportados e best-effort nos demais; não existe evento universal de seleção no macOS.
