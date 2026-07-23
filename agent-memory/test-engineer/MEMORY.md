# Agent Memory — test-engineer

## Padrões de Teste
- O domínio Rust usa testes inline com fakes de `SelectionPort`, `OverlayPort` e `AiProvider`.
- O frontend usa Vitest; contratos IPC são testados com mock hoisted de `invoke`.
- Contratos da Edge Function usam `deno test` sem dependências externas.
- Smoke tests validam configuração e também executam o bundle `.app` pelo CLI do Tauri.

## Estratégias de Mock
- Seleções mutáveis ficam em `Arc<Mutex<SelectionSnapshot>>` para simular mudança durante requests.
- Providers falsos retornam sucesso, timeout ou request ID divergente sem acessar a rede.
- O limite de Keychain é verificado pelo payload IPC; testes não gravam credenciais reais.

## Erros Recorrentes & Soluções
- Factories de `vi.mock` são hoisted; mocks compartilhados devem usar `vi.hoisted`.
- Clipboard e Accessibility reais não devem ser acionados em testes automatizados, pois alteram estado global do macOS.

## Cobertura & Métricas
- O escopo instrumentado do cliente frontend (`native.ts` e `types.ts`) mantém 100% em statements, branches, functions e lines.
- A suíte Rust cobre state machine, latest-wins, stale selection, falhas seguras, Unicode, settings e geometria, mas `cargo-llvm-cov` não está instalado.

## Observações
- Preview ainda não existe no fluxo de produção e, portanto, não possui teste de aceitação.
- A matriz AX e o fallback de clipboard precisam de validação manual em um app com permissão de Acessibilidade.
