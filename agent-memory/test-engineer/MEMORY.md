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
- Worktree ativo em `.worktrees/live-latency` (branch `live-latency`), Round 2 concluído (Edge Function streaming Deno + Rust T2.2 com 18 testes).
- `futures_util::stream::unfold` NÃO implementa `Unpin`. Para passar a funções `S: Unpin`, usar `Box::pin(unfold(...))` — `Box<T>: Unpin` sempre, então `Pin<Box<...>>: Unpin`.
- `drain_streaming_body` checa `cancel` DEPOIS de `stream.next().await` retornar — para simular "cancel após primeiro chunk", o unfold define `cancel.store(true)` antes de retornar o segundo item; o item é recebido mas o loop faz break antes de processá-lo.
- Gates manuais documentados em `tasks/voice-enrollment/plan.md` e `tasks/virtual-microphone/plan.md` na seção "Gates Manuais"/"Gates antes do handoff".
- **QA exige E2E Playwright para TODA mudança de componente de frontend novo**, mesmo que já tenha Vitest de unidade/integração — rejeição comum se faltar `e2e/*.e2e.ts` cobrindo o componente. Ver `e2e/virtual-mic.e2e.ts` como exemplo do padrão a seguir (stub de `__TAURI_INTERNALS__.invoke` por `page.addInitScript`, parametrizado com um segundo argumento quando o cenário varia por status, ex.: `page.addInitScript(stubTauri, "notInstalled")`).
- Ao montar o stub de `invoke`, sempre inclua `plugin:event|listen` retornando um id numérico (ex. `1`) — os componentes usam `listen()` do Tauri internamente mesmo quando o teste não dispara eventos custom.
- Para alcançar uma seção condicional que só aparece com `profile.status === "ready"` (ex. `VirtualMicSection` dentro de `InterpretationPanel`), já retorne `voice_profile_status` com `status: "ready"` no stub em vez de simular o fluxo completo de gravação/upload — evita testes longos e frágeis quando o objetivo é outro componente.
- **Edge Function tests (Deno)**: quando um handler tem ramos de role check (`anon`/`anonymous`) e normalizeError (`DOMException AbortError` → PROVIDER_TIMEOUT, objeto desconhecido → INTERNAL_ERROR), criar arquivo `*_edge_cases_test.ts` separado para não ultrapassar 300 linhas efetivas do handler_test.ts principal.
- **AbortError em Deno**: `new DOMException("AbortError", "AbortError")` — primeiro argumento é a mensagem, segundo é o nome. Verificar `err instanceof DOMException && err.name === "AbortError"`.
- **stages.ts**: pipeline functions devem ser testadas diretamente (sem passar pelo handler) para cobrir os 6 ramos AbortError→PROVIDER_TIMEOUT (3 por pipeline × 2 pipelines). Arquivo `stages_test.ts` separado para isso.
- **Content-length guard em handler.ts**: testável passando header `Content-Length: <número>` ou `Content-Length: invalid` no Request. O guard atua ANTES de `parseRequest`.
