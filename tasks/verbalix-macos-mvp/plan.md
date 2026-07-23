# Verbalix macOS MVP

## 🎯 SCOPE

### Incluído
- Aplicativo Tauri 2 com React, TypeScript e Vite para macOS 14+, executado como menu-bar/accessory app.
- Onboarding para permissão de Acessibilidade e tela de configurações.
- Detecção best-effort de seleção textual global com toolbar flutuante nativa.
- Ações `Traduzir` e `Aprimorar`, processadas por IA somente após clique explícito.
- Tradução português ↔ inglês e aprimoramento no idioma original.
- Substituição da seleção quando o alvo AX permitir escrita; nota flutuante nos demais casos.
- Preferências de formalidade, extensão, tom e preview; tratamento seguro de concorrência, falhas e conteúdo protegido.
- Autenticação Supabase por magic link, sessão no Keychain e histórico sincronizado opcional com retenção de 30 dias.
- Matriz inicial de validação: Chrome, Safari, VS Code, Slack, Notes e TextEdit.

### Fora do Escopo
- Windows, Linux, Mac App Store, OCR, seleção em imagens/canvas e suporte universal a PDFs.
- Billing e telemetria de texto.
- Preservação universal de rich text, múltiplas seleções e fallback automático por clipboard/Cmd+V.
- Funcionamento offline.

### Riscos de Impacto
- Implementações incompletas da Accessibility API podem impedir leitura, bounds ou escrita em apps específicos.
- Uma janela comum pode roubar foco; overlays precisam ser painéis não ativantes.
- Ranges AX usam UTF-16, enquanto Rust usa UTF-8.
- Respostas atrasadas podem escrever no alvo errado sem snapshot, cancelamento e revalidação.

## 📋 REQUIREMENTS

### Requisitos Funcionais
- [ ] RF01: Solicitar, verificar e explicar a permissão de Acessibilidade sem operar silenciosamente quando negada.
- [ ] RF02: Detectar seleção estável automaticamente ou sob o atalho Option+Shift+Space e capturar texto, range, bounds, PID e capacidade de escrita.
- [ ] RF03: Exibir toolbar com exatamente `Traduzir` e `Aprimorar` perto da seleção, sem ativar o Verbalix nem remover o foco do alvo.
- [ ] RF04: Traduzir PT→EN, EN→PT e outros idiomas→PT.
- [ ] RF05: Aprimorar no idioma original, preservando significado técnico, código, Markdown, identificadores, números e URLs.
- [ ] RF06: Substituir exatamente a seleção original somente quando o snapshot ainda for válido e o atributo AX for gravável.
- [ ] RF07: Para conteúdo somente leitura, mostrar nota ancorada com resultado, `Copiar` e `Fechar`.
- [ ] RF08: Permitir fechar overlays por Esc, clique externo, seleção vazia, mudança de aplicativo ou nova seleção.
- [ ] RF09: Persistir formalidade 1..5, extensão `Concise|Balanced|Detailed`, tom `Neutral|Friendly|Assertive|Technical` e preview opcional; defaults 3, `Balanced`, `Technical` e substituição direta.
- [ ] RF10: Cancelar/inutilizar respostas antigas quando a seleção mudar.
- [ ] RF11: No atalho, quando AX não expuser texto, simular somente a cópia, preservar/restaurar o clipboard e nunca simular colagem.
- [ ] RF12: Autenticar por magic link, persistir sessão no Keychain e transformar texto exclusivamente pela Supabase Edge Function.
- [ ] RF13: Quando habilitado, sincronizar histórico por 30 dias e permitir excluir um item ou todo o histórico.
- [ ] RF14: Após substituição direta, oferecer undo temporário do resultado aplicado.

### Requisitos Não-Funcionais
- [ ] RNF01: Nenhum texto selecionado, segredo ou resultado deve aparecer em logs.
- [ ] RNF02: O texto só é enviado à IA após ação explícita.
- [ ] RNF03: Timeout, offline, rate limit ou resposta inválida nunca podem escrever no alvo.
- [ ] RNF04: Toolbar deve aparecer em até 300 ms após uma seleção estável em apps compatíveis.
- [ ] RNF05: Overlays devem respeitar múltiplos monitores, Retina, bordas e visible frame.
- [ ] RNF06: Campos protegidos e seleções vazias devem ser ignorados.
- [ ] RNF07: Componentes dependentes de macOS e IA devem aceitar adapters falsos em testes.
- [ ] RNF08: Rejeitar seleções acima de 12.000 caracteres antes de qualquer chamada remota.
- [ ] RNF09: RLS deve garantir acesso owner-only ao histórico, que deve conter `expires_at`.

### Critérios de Aceitação
- [ ] CA01: Sem permissão AX, o onboarding explica como concedê-la e o observador permanece inativo.
- [ ] CA02: Seleção por mouse e teclado exibe a toolbar sem trocar o aplicativo ativo.
- [ ] CA03: Tradução e aprimoramento preservam tokens técnicos e não aplicam respostas antigas.
- [ ] CA04: Campo editável recebe somente a transformação da seleção capturada e revalidada.
- [ ] CA05: Conteúdo read-only permanece intacto e a nota permite copiar o resultado.
- [ ] CA06: Erros de rede, provider, AX e idioma são recuperáveis e não destrutivos.
- [ ] CA07: Campos seguros não exibem toolbar nem enviam conteúdo.

### Edge Cases
- EC01: Texto misto, código puro, identificador isolado ou idioma diferente de PT/EN.
- EC02: Emoji, acentos, surrogate pairs e grapheme clusters.
- EC03: Elemento AX invalidado, app travado, atributo não implementado ou permissão revogada.
- EC04: Seleção muda, app troca ou request termina fora de ordem.
- EC05: Seleção próxima às bordas, em monitor secundário ou fullscreen.
- EC06: Duplo clique na ação, resposta vazia e conteúdo acima do limite.

## 🏗️ DESIGN

### Padrões Utilizados
- Ports and adapters para separar macOS, UI, armazenamento e provider de IA.
- State machine `Idle → CandidateSelection → ToolbarVisible → Processing → ResultVisible/Idle`.
- Snapshot imutável e latest-wins para segurança de escrita.
- Capability detection para decidir entre substituição e nota, sem listas hardcoded por aplicativo.

### Componentes
- `SelectionService`: permissão, observação, captura, geometria e revalidação AX.
- `SelectionCoordinator`: debounce, estado, cancelamento, ações e prevenção de loops.
- `OverlayService`: toolbar, loading, erros e nota por painéis nativos não ativantes.
- `TextTransformer`: contrato único para `Translate` e `Improve`, implementado remotamente por Edge Function atrás de `AiProvider`.
- `SettingsRepository`: formalidade, estilo e preferências não sensíveis.
- `AuthRepository` e `HistoryRepository`: Supabase Auth, sessão no Keychain e histórico protegido por RLS.
- UI Tauri: onboarding, autenticação, histórico, status da permissão e configurações.

### Fluxo de Dados
`evento global/AX ou atalho → debounce/captura → SelectionSnapshot → toolbar → ação → Edge Function/AiProvider → validação → revalidação do snapshot → preview opcional → substituir/undo ou nota → histórico opcional`

### Contratos Principais
- `SelectionSnapshot`: identificador, PID, app, texto, range UTF-16, bounds, `writable` e instante de captura.
- `TransformRequest`: `{ requestId, operation, text, preferences? }`.
- `TransformResult`: `{ requestId, sourceLanguage, targetLanguage, result }`.
- `AppSettings`: formalidade 1..5, extensão, tom, preview e histórico habilitado.
- `transform_history`: owner ID, operação, idiomas, conteúdo necessário ao histórico, timestamps e `expires_at`, com RLS owner-only.
- Erros são tipados por permissão, AX, idioma, provider, timeout, stale selection e validação.

### Segurança e Privacidade
- Nunca enviar conteúdo na detecção; somente após clique.
- Delimitar o texto como dado não confiável no prompt e aceitar apenas transformação textual.
- Validar request ID, idioma e resultado não vazio antes de qualquer escrita.
- Persistir conteúdo somente quando o usuário habilitar histórico; aplicar retenção de 30 dias e exclusão explícita.
- Manter a chave OpenAI exclusivamente na Edge Function; modelo configurável por variável de ambiente.

## 📝 TASKS

### Fase 1: Fundação
- [ ] T1.1: [LOW] Inicializar workspace Tauri 2, frontend, manifests e configurações macOS.
- [ ] T1.2: [LOW] Definir tipos, erros, ports e configuração compartilhada.
- [ ] T1.3: [LOW] Implementar store local de settings e validação.
- [ ] T1.4: [MEDIUM] Criar schema/migration Supabase, RLS owner-only e contrato da Edge Function.

### Fase 2: Seleção e Estado
- [ ] T2.1: [MEDIUM] Implementar adapter de confiança/permissão Accessibility.
- [ ] T2.2: [MEDIUM] Implementar captura AX de elemento, texto, range, bounds e writability.
- [ ] T2.3: [MEDIUM] Implementar observação híbrida, debounce e filtros de segurança.
- [ ] T2.4: [MEDIUM] Implementar state machine, snapshots, cancelamento e revalidação.
- [ ] T2.5: [MEDIUM] Implementar escrita AX estrita para seleção gravável.
- [ ] T2.6: [MEDIUM] Implementar atalho e fallback copy-only com restauração segura do clipboard.

### Fase 3: Overlays e UI
- [ ] T3.1: [MEDIUM] Implementar toolbar nativa não ativante e posicionamento/clamp.
- [ ] T3.2: [MEDIUM] Implementar nota read-only com Copiar/Fechar e estados de erro/loading.
- [ ] T3.3: [MEDIUM] Implementar onboarding e Settings no frontend Tauri.
- [ ] T3.4: [LOW] Integrar menu bar, ciclo de vida e abertura das configurações.
- [ ] T3.5: [MEDIUM] Implementar login magic link, histórico, exclusão e estados de sessão.

### Fase 4: IA
- [ ] T4.1: [MEDIUM] Implementar `TextTransformer`, prompts técnicos e política PT/EN.
- [ ] T4.2: [MEDIUM] Implementar Edge Function com `AiProvider`, OpenAI por env, erros padronizados, timeout e limite de 12k.
- [ ] T4.3: [MEDIUM] Implementar cliente Supabase, sessão no Keychain e repositories de auth/histórico.
- [ ] T4.4: [MEDIUM] Integrar ações, latest-wins, preview, aplicação segura e undo temporário.

### Fase 5: Verificação
- [ ] T5.1: [MEDIUM] Criar testes unitários de domínio, Unicode, prompts, settings e geometria.
- [ ] T5.2: [MEDIUM] Criar testes de integração com adapters falsos para AX/provider/concorrência.
- [ ] T5.3: [MEDIUM] Validar build e smoke tests do bundle macOS.
- [ ] T5.4: [MEDIUM] Executar checklist manual da matriz de aplicativos e registrar limitações.
- [ ] T5.5: [MEDIUM] Executar spike obrigatório de detectar, ler, obter bounds e substituir na matriz aprovada.

## Análise Dual

### Riscos incorporados
- Universalidade foi substituída por matriz explícita e comportamento best-effort.
- Clipboard automático, colagem simulada, OCR e rich text universal ficaram fora do MVP; copy-only é acionado apenas pelo atalho.
- O design exige painel não ativante, snapshot revalidado, tratamento UTF-16 e falhas não destrutivas.
- A permissão e a privacidade são gates de produto, não detalhes de implementação.

### Oportunidades incorporadas
- Tauri fica concentrado em shell/settings, enquanto Rust expõe capacidades nativas testáveis.
- Ports pequenos permitem providers alternativos e novas ações futuramente.
- Capability detection amplia compatibilidade sem hardcode por bundle ID.
- Fixtures de AX e provider permitem testar a maior parte do fluxo sem automação de aplicativos reais.
