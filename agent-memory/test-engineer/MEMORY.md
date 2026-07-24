# Agent Memory — test-engineer

## Padrões de Teste
- O domínio Rust usa testes inline com fakes de `SelectionPort`, `OverlayPort` e `AiProvider`.
- O frontend usa Vitest; contratos IPC são testados com mock hoisted de `invoke`.
- Contratos da Edge Function usam `deno test` sem dependências externas.
- Smoke tests validam configuração e também executam o bundle `.app` pelo CLI do Tauri.
- Boundaries macOS que não podem ser exercitados sem permissão real usam uma combinação de funções puras Rust para geometria e contratos estáticos Vitest para garantir APIs AX/Core Graphics, ausência de AppKit no worker e lifecycle do shell.
- Fluxos críticos de recuperação visual usam Playwright com `__TAURI_INTERNALS__` simulado e verificam tanto invocações IPC quanto clipping pelo bounding box.

## Estratégias de Mock
- Seleções mutáveis ficam em `Arc<Mutex<SelectionSnapshot>>` para simular mudança durante requests.
- Providers falsos retornam sucesso, timeout ou request ID divergente sem acessar a rede.
- O limite de Keychain é verificado pelo payload IPC; testes não gravam credenciais reais.
- Wiring com efeitos Tauri pode ser testado por callbacks `FnOnce` que contam separadamente abertura de janela e publicação de nota, mantendo o mesmo branch usado por produção sem construir `AppHandle`.

## Erros Recorrentes & Soluções
- Factories de `vi.mock` são hoisted; mocks compartilhados devem usar `vi.hoisted`.
- Clipboard e Accessibility reais não devem ser acionados em testes automatizados, pois alteram estado global do macOS.
- Coordenadas globais negativas são válidas em monitores secundários; validação geométrica deve rejeitar valores não finitos e dimensões inválidas sem rejeitar a origem negativa.

## Cobertura & Métricas
- O escopo instrumentado do cliente frontend (`native.ts` e `types.ts`) mantém 100% em statements, branches, functions e lines.
- A suíte Rust cobre state machine, latest-wins, stale selection, falhas seguras, Unicode, settings, readiness e geometria, mas `cargo-llvm-cov` não está instalado.

## Observações
- Preview/apply/undo possuem integração mockada; a matriz AX e o fallback de clipboard ainda precisam de validação manual em um app com permissão de Acessibilidade.
- Configuração pública do Supabase é testada como build-time embutido com override de runtime; OpenAI/service-role não podem aparecer nos arquivos do runtime público.
