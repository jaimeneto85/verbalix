# Agent Memory — test-engineer

## Padrões de Teste
- O domínio Rust usa testes inline com fakes de `SelectionPort`, `OverlayPort` e `AiProvider`.
- O frontend usa Vitest; contratos IPC são testados com mock hoisted de `invoke`.
- Contratos da Edge Function usam `deno test` sem dependências externas.
- Smoke tests validam configuração e também executam o bundle `.app` pelo CLI do Tauri.
- Boundaries macOS que não podem ser exercitados sem permissão real usam uma combinação de funções puras Rust para geometria e contratos estáticos Vitest para garantir APIs AX/Core Graphics, ausência de AppKit no worker e lifecycle do shell.
- Text markers permanecem sem mocks FFI: a suíte combina matriz pura e exaustiva de categorias AX, validação de índices/length em UTF-16, fluxo read-only até nota e contrato estático das APIs públicas/RAII; o gate real cobre o adapter nativo.
- O fluxo composto de fallback AX deve combinar matriz pura de categorias com contrato de integração do source: falha estrutural de CFRange não pode alcançar `marker_selection`, enquanto somente falhas explícitas de capacidade podem fazê-lo.
- Restore precisa de testes separados e combinados para PID, identidade estável do elemento, texto selecionado e range UTF-16 atual; coincidência de texto/range em outro campo do mesmo PID deve continuar falhando fechada.
- Replace e restore exigem identidade forte antes de qualquer lookup/write AX; testes devem cobrir `identifier=None`, string vazia e somente whitespace, todos com zero escrita.
- Fluxos críticos de recuperação visual usam Playwright com `__TAURI_INTERNALS__` simulado e verificam tanto invocações IPC quanto clipping pelo bounding box.
- Superfícies de overlay transparentes exigem teste antes do render: a classe de rota deve existir durante o callback de bootstrap e `html`, `body` e `#root` devem computar fundo transparente, dimensões mínimas zero e overflow oculto sem alterar a rota principal.
- Posicionamento macOS em múltiplos monitores é testado como geometria pura em pontos Cocoa: conversão AX round-trip, escolha por centro/interseção, clamp nas quatro bordas, fallback vertical e coordenadas globais negativas. Um contrato estático separado impede reintroduzir `LogicalPosition`, `PhysicalPosition` ou `scale_factor` no caminho macOS.

## Estratégias de Mock
- Seleções mutáveis ficam em `Arc<Mutex<SelectionSnapshot>>` para simular mudança durante requests.
- Providers falsos retornam sucesso, timeout ou request ID divergente sem acessar a rede.
- O limite de Keychain é verificado pelo payload IPC; testes não gravam credenciais reais.
- Wiring com efeitos Tauri pode ser testado por callbacks `FnOnce` que contam separadamente abertura de janela e publicação de nota, mantendo o mesmo branch usado por produção sem construir `AppHandle`.

## Erros Recorrentes & Soluções
- Factories de `vi.mock` são hoisted; mocks compartilhados devem usar `vi.hoisted`.
- Clipboard e Accessibility reais não devem ser acionados em testes automatizados, pois alteram estado global do macOS.
- Coordenadas globais negativas são válidas em monitores secundários; validação geométrica deve rejeitar valores não finitos e dimensões inválidas sem rejeitar a origem negativa.
- Doubles Deno que implementam interfaces assíncronas sem executar `await` devem retornar `Promise.resolve`/`Promise.reject` explicitamente para satisfazer `deno lint require-await`.
- Limites superiores de índices marker em macOS arm64 devem considerar que `isize::MAX == i64::MAX`; o maior location válido antes de um range de length 1 é `isize::MAX - 1`.
- O analyzer QA considera linhas efetivas e impõe máximo de 300 por arquivo modificado; os boundaries macOS foram divididos e devem permanecer abaixo desse limite.

## Cobertura & Métricas
- O escopo instrumentado do cliente frontend (`native.ts` e `types.ts`) mantém 100% em statements, branches, functions e lines.
- A suíte Rust cobre state machine, latest-wins, stale selection, falhas seguras, Unicode/UTF-16, matriz AX, marker read-only, identidade forte de replace/restore, settings, readiness e geometria. `cargo-llvm-cov` não está instalado; os 70 testes Rust e os gates `clippy -D warnings` são usados como evidência.
- O frontend possui 37 testes Vitest e mantém 100% em statements, branches, functions e lines no escopo instrumentado (`native.ts`, `types.ts`); os 5 testes Playwright E2E também passam.
- A suíte Rust possui 82 testes, incluindo 14 casos determinísticos da geometria do overlay.

## Observações
- Preview/apply/undo possuem integração mockada; a matriz AX e o fallback de clipboard ainda precisam de validação manual em um app com permissão de Acessibilidade.
- Configuração pública do Supabase é testada como build-time embutido com override de runtime; OpenAI/service-role não podem aparecer nos arquivos do runtime público.
- Limites HTTP da Edge são testados em bytes com JSON válido seguido de whitespace até exatamente 64 KiB; isso separa o boundary de transporte do limite de 12.000 caracteres do domínio.
- Testes pré-deploy da Edge mantêm Auth, scheduler e OpenAI totalmente injetados: cobrem usuário real/anon, timeout que vence provider não cooperativo, respostas por operação e limites exatos sem rede ou secrets reais.
- O hard gate Trivy pode ser reutilizado quando executado contemporaneamente na mesma worktree: scan `vuln,misconfig` HIGH/CRITICAL com `--ignore-unfixed --exit-code 1` passou para `package-lock`, `Cargo.lock` e configurações.
