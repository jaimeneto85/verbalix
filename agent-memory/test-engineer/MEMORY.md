# Agent Memory — test-engineer

## Padrões de Teste

- **Framework Deno (Edge Functions)**: testes em `*_test.ts` ao lado dos arquivos fonte. Sem framework de assertion externo — usa função local `assertEquals` (JSON.stringify comparison). Cada `Deno.test()` é independente.
- **Framework frontend**: Vitest + React Testing Library. Arquivos `*.test.tsx` / `*.test.ts` em `src/`.
- **E2E**: Playwright em `e2e/*.e2e.ts`, rodado contra Vite real com `window.__TAURI_INTERNALS__` stubado.
- **Rust**: `cargo test` padrão; testes em módulo `#[cfg(test)]` dentro dos próprios arquivos ou em `tests/`.

## Estratégias de Mock

- **Deno handlers**: injeção de dependência via `deps: HandlerDeps`. Criar `createState(options)` que retorna instâncias fake das interfaces (authenticator, provider, serviceClient, timeout). Contadores (authCalls, enrollCalls, setReadyCalls, setFailedCalls) em closures para verificação de chamadas.
- **Opções de erro em createState**: adicionar campos `xyzError?: Error` na `StateOptions` e checar no mock antes de resolver. Padrão: `if (options.xyzError) return Promise.reject(options.xyzError)`.
- **Frontend**: `vi.mock` para módulos Tauri (`@tauri-apps/api/core`). IPC wrappers em `src/native.ts` são o único ponto de contato — mocká-lo isola testes de componentes.
- **Playwright**: script de init injeta `window.__TAURI_INTERNALS__`; assertivas em chamadas `invoke` gravadas.

## Erros Recorrentes & Soluções

- **Deno: assertEquals usa JSON.stringify** — arrays vazios `[]` e `0` são distintos; verificar exatamente o tipo esperado.
- **setReadyCalls conta mesmo quando setReady lança**: o contador é incrementado antes do check de erro no mock — é intencional para verificar que a chamada aconteceu antes da exceção.
- **cpal::Stream não é Send**: `MacAudioCapture` usa thread de captura dedicada + `mpsc` + `Arc<AtomicU32>`. O caminho de erro `start()` sem device não pode ser testado sem injeção de `cpal::Host` — documentar como gate manual.

## Cobertura & Métricas

- **Limiar**: 80% mínimo; 100% em `src/native.ts` e `src/types.ts` (enforced por `npm run test:coverage`).
- **Deno**: não há threshold automático; cobertura verificada por inspeção dos cenários (happy path, auth error, parse error, timeout, idempotência, replace, orphan cleanup, DB conflict).
- Áreas de cobertura difícil: `platform/audio_capture.rs` (cpal, macOS-only), `platform/audio_permission.rs` (AVFoundation, requer hardware real).

## Observações

- O projeto tem gate de linhas em `lib.rs` (≤301) verificado por `bundle-smoke.test.ts` — não adicionar código inline lá.
- Commits NUNCA devem mencionar Claude, IA ou qualquer ferramenta de IA.
- Worktree ativo em `.worktrees/voice-enrollment` (branch `voice-enrollment`).
- Gates manuais documentados em `tasks/voice-enrollment/plan.md` na seção "Gates Manuais".
