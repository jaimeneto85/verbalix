# Verbalix macOS MVP — Test Report

## Resultado

- Rust: 27 testes aprovados.
- Frontend: 21 testes aprovados.
- Edge Function: 6 testes aprovados.
- Bundle: `Verbalix.app` debug gerado com sucesso.
- Total: 54 testes, 0 falhas.

## Pirâmide

- Unitários: domínio, Unicode/UTF-16, settings, prompts e validação de contratos.
- Integração: coordinator com adapters falsos, fluxos React, IPC Tauri e persistência em arquivo.
- Smoke: configuração do bundle e geração real do `.app`.

## Regressões finais

- Pausa bloqueia polling, callback do AXObserver, atalho global e fallback de clipboard; retomada reabilita os quatro caminhos.
- A nota registra o resultado antes do evento e o frontend registra o listener antes de consultar o estado atual, cobrindo resultados criados antes e depois da prontidão.
- O fluxo integrado automatizado percorre toolbar, transformação, preview, apply e undo com adapters de domínio.
- O adapter de overlay aceita chamadas vindas de worker threads e encaminha todas as operações de janela ao executor principal.

## Cobertura instrumentada

O cliente frontend testável em isolamento (`native.ts` e `types.ts`) atingiu 100% de statements, branches, functions e lines. A cobertura Rust não foi instrumentada porque `cargo-llvm-cov` não está instalado; a suíte Rust executou 27 cenários.

## Gates executados

```text
cargo test
npm test
npm run test:coverage
npm run build
deno test supabase/functions/transform/contract_test.ts
npm run tauri -- build --debug --bundles app
cargo clippy --all-targets --all-features -- -D warnings
```

O analisador de tamanho não encontrou arquivos acima do limite de 300 linhas efetivas. O Trivy não encontrou vulnerabilidades críticas ou altas; há uma vulnerabilidade média em dependência transitiva sem correção compatível neste escopo.

## Gaps que exigem QA manual

- Executar toolbar → transformar → preview/aplicar → undo em TextEdit com o bundle assinado, permissão AX e credenciais válidas; esses requisitos impedem automação confiável no ambiente de CI atual.
- Validar restauração integral do clipboard em processos macOS reais.
- Confirmar o comportamento do AXObserver híbrido, o clamp pelo `NSScreen.visibleFrame` e o `NSPanel` não ativante com múltiplos monitores.
- Chrome, Safari, VS Code, Slack, Notes e TextEdit exigem permissão de Acessibilidade e execução manual; T5.4 e T5.5 permanecem abertos.

## Revisão do overlay transparente

Os gates independentes da terceira revisão passaram: Rust 86/86, Vitest 44/44, Playwright 5/5, cobertura frontend configurada em 100%, fmt/check/clippy estrito, build, diff-check e limite de 300 linhas.

O veredito de código é `REJECTED_CODE`. O protocolo de readiness ainda precisa vincular cada ACK à geração do documento/WebView e impor um teto rígido de três tentativas. Sem essa correlação, um invoke que excedeu o timeout pode marcar como pronta uma janela recriada com o mesmo label.

## Revisão do protocolo geracional

A remediação vincula cada documento de overlay a uma geração UUID emitida pelo Rust e valida geração, label e identidade da WebView chamadora antes de aceitar readiness. Reload invalida a geração e esconde a janela; rotas de overlay sem geração mantêm a superfície transparente com root vazio.

O frontend removeu `Promise.race`: tentativas são sequenciais, limitadas rigidamente a três, executadas somente após ACK falso ou erro, interrompidas no primeiro sucesso e reportadas após exaustão.

Gates independentes desta revisão:

- Rust: 87/87.
- Vitest: 47/47.
- Playwright: 6/6.
- Cobertura frontend configurada: 100%.
- Build, fmt, check, clippy estrito, diff-check e limite de 300 linhas: aprovados.

## Revisão da recuperação após reload

O segundo início de carregamento agora invalida a geração e destrói a WebView. Uma solicitação posterior detecta qualquer janela sem documento atual, remove essa instância e cria UUID/URL novos; ACK da geração anterior permanece rejeitado. Falhas de invalidação, destruição e fallback de ocultação possuem diagnósticos próprios.

O bootstrap aceita somente UUID v4. Geração ausente ou inválida mantém a rota transparente, sem renderizar toolbar, nota ou aplicação principal.

Gates independentes desta revisão:

- Rust: 88/88.
- Vitest: 47/47.
- Playwright: 6/6.
- Cobertura frontend configurada: 100%.
- Build, fmt, check, clippy estrito, diff-check e limite de 300 linhas: aprovados.

## Quinta revisão dual

A recuperação após reload foi aprovada: o segundo início de carregamento invalida a geração antes da destruição, registra falhas de invalidação/destruição/ocultação, e uma solicitação posterior cria UUID/URL novos. ACK antigo, UUID ausente ou inválido e janela sem documento falham fechados.

Os gates foram reexecutados independentemente:

- Rust: 88/88.
- Vitest: 47/47.
- Playwright: 6/6.
- Cobertura frontend configurada: 100%.
- Build, fmt, check, clippy estrito, diff-check e limite de 300 linhas: aprovados.

O veredito é `REJECTED_CODE`. Se a configuração AppKit falhar após a WebView ser construída, a janela e a geração continuam registradas e podem ser reutilizadas sem composição nativa válida. A criação deve fazer rollback transacional, com invalidação, destruição e fallback de ocultação diagnosticados. O lifecycle real de reload permanece como gate de Computer Use do CA6 antes do release.
