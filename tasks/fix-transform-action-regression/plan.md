# Plano — Corrigir ações Traduzir e Aprimorar

## 0. SCOPE

### Incluído

- [ ] Reproduzir e instrumentar a cadeia `overlay click → IPC Tauri → transformação autenticada → coordinator → escrita AX`.
- [ ] Corrigir Traduzir e Aprimorar para enviarem a operação correta.
- [ ] Restaurar a substituição do texto selecionado quando o conteúdo for editável.
- [ ] Preservar comportamento fail-closed para seleção stale, identidade AX divergente e conteúdo somente leitura.
- [ ] Cobrir erros acionáveis de sessão/backend sem expor texto selecionado ou credenciais.
- [ ] Validar em app macOS real com Accessibility e backend configurado.

### Arquivos/módulos potencialmente afetados

- `src/Overlay.tsx`, `src/native.ts` e testes frontend do overlay/IPC.
- `src-tauri/src/commands.rs` e runtime/comandos Tauri.
- `src-tauri/src/application/coordinator.rs` e respectivos testes.
- `src-tauri/src/platform/macos_accessibility*.rs` e testes de replace/identidade.
- Diagnósticos sanitizados estritamente necessários à reprodução.

### Dependências diretas

- Tauri IPC, React, Supabase Auth/Edge Function e Accessibility API do macOS.

### Fora do escopo

- Alterar prompts, modelo de IA, UI visual do overlay ou geometria já aprovada.
- Afrouxar validações de identidade/staleness para forçar a escrita.
- Merge em `main`, geração de release, mudanças na Edge Function ou no contrato público sem evidência de necessidade.

### Riscos de impacto

- Corrigir o clique sem corrigir a revalidação AX pode produzir sucesso aparente sem substituir.
- Uma seleção muda enquanto a IA responde; a correção deve continuar rejeitando a escrita stale.
- O overlay pode tomar foco e invalidar a seleção antes do comando.
- Traduzir e Aprimorar podem compartilhar um defeito comum, mas ainda precisam de cobertura independente.
- Testes com mocks podem passar sem provar sessão remota, Accessibility e substituição reais.

## 1. REQUIREMENTS

### Requisitos funcionais

- [ ] RF01: Clicar em Traduzir invoca exatamente uma transformação `translate`.
- [ ] RF02: Clicar em Aprimorar invoca exatamente uma transformação `improve`.
- [ ] RF03: Em seleção editável ainda válida, o resultado substitui exatamente o texto selecionado.
- [ ] RF04: Em seleção somente leitura, o resultado é exibido como nota, sem tentativa de escrita.
- [ ] RF05: Falhas de backend, autenticação, seleção stale ou escrita AX geram estado/erro acionável e não desaparecem silenciosamente.
- [ ] RF06: A transformação usa o snapshot capturado antes do clique e revalida sua identidade antes da escrita.

### Requisitos não funcionais

- [ ] RNF01: Nenhum texto selecionado, token ou segredo em logs.
- [ ] RNF02: Nenhum arquivo modificado ultrapassa 300 linhas efetivas.
- [ ] RNF03: `cargo fmt`, `cargo check`, `cargo test`, `cargo clippy -D warnings`, Vitest, cobertura, Playwright e build passam.
- [ ] RNF04: A correção preserva transparência, posicionamento e lifecycle geracional do overlay.

### Critérios de aceitação

- [ ] CA01: Teste frontend prova ambos os botões, payloads e tratamento de falha.
- [ ] CA02: Teste Rust prova as duas operações chegando ao provider e substituindo seleção editável.
- [ ] CA03: Teste Rust prova que seleção read-only recebe nota e zero writes.
- [ ] CA04: Testes AX provam escrita no handle original com identidade/range atuais e zero escrita após divergência.
- [ ] CA05: Computer Use em seleção editável comprova Traduzir e Aprimorar alterando textos de teste distintos.
- [ ] CA06: Diagnósticos sanitizados identificam estágios sem registrar conteúdo ou credenciais.

### Edge cases

- EC01: Duplo clique/ações concorrentes.
- EC02: Overlay perde/reordena readiness durante o clique.
- EC03: Sessão ausente/expirada e refresh transitório.
- EC04: Backend retorna erro, timeout ou resultado vazio.
- EC05: Seleção, foco, range ou conteúdo muda durante a transformação.
- EC06: Unicode e ranges UTF-16.
- EC07: Elemento editável sem setter AX suportado.

## 2. DESIGN

### Estratégia

- Reproduzir primeiro com diagnósticos sanitizados para localizar a primeira quebra observável.
- Manter a UI como entrypoint explícito e o comando Tauri como boundary de validação/readiness.
- Manter o `SelectionCoordinator` como dono de latest-wins, revalidação e decisão `replace` versus `note`.
- Manter o adapter Accessibility como único responsável pela escrita real e pela revalidação do mesmo handle.

### Fluxo de dados esperado

`click(operation) → native.transformSelection(operation, settings) → transform_selection → session/readiness → coordinator.transform(snapshot, operation) → provider → recapture/revalidate → replace(editável) | note(read-only) → feedback`

### Contratos e invariantes

- Cada ação explícita gera no máximo uma request ativa.
- O snapshot/request ID enviado ao provider deve ser o mesmo validado no retorno.
- `replace` só ocorre quando `writable=true`, identidade forte coincide, texto/range atuais coincidem e setter AX é suportado.
- Falha em qualquer invariável é terminal e observável, nunca convertida em “sucesso”.
- Não ocultar o toolbar de forma que o clique destrua o snapshot antes do comando assumir a operação.

### Componentes reutilizáveis

- `Overlay` e `native.transformSelection`.
- `commands::transform_selection`.
- `SelectionCoordinator` e fakes existentes.
- Matriz AX de identidade, replace/restore e diagnósticos tipados.

## 3. TASKS

### Fase 1 — Reprodução e causa

- [ ] T1.1 `[MEDIUM]` Reproduzir ambos os botões e registrar o primeiro estágio que falha.
- [ ] T1.2 `[LOW]` Inspecionar payload IPC, readiness/session e lifecycle do snapshot.
- [ ] T1.3 `[MEDIUM]` Inspecionar revalidação e setter AX no mesmo handle.

### Fase 2 — Implementação

- [ ] T2.1 `[MEDIUM]` Corrigir a causa comum sem enfraquecer identidade/staleness.
- [ ] T2.2 `[LOW]` Garantir feedback acionável para falhas reais.
- [ ] T2.3 `[LOW]` Preservar note read-only e overlay lifecycle.

### Fase 3 — Testes

- [ ] T3.1 `[LOW]` Cobrir os dois botões e payloads no frontend.
- [ ] T3.2 `[MEDIUM]` Cobrir translate/improve e replace/note no coordinator.
- [ ] T3.3 `[MEDIUM]` Cobrir revalidação e escrita AX, incluindo Unicode/stale/unsupported.
- [ ] T3.4 `[LOW]` Executar gates automatizados e limite de linhas.

### Fase 4 — QA real

- [ ] T4.1 `[MEDIUM]` Validar Traduzir em texto editável via Computer Use.
- [ ] T4.2 `[MEDIUM]` Validar Aprimorar em texto editável via Computer Use.
- [ ] T4.3 `[LOW]` Verificar logs sanitizados, ausência de regressão visual e verdict formal.

