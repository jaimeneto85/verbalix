# Verbalix macOS MVP — Test Report

## Resultado

- Rust: 17 testes aprovados.
- Frontend: 19 testes aprovados.
- Edge Function: 6 testes aprovados.
- Bundle: `Verbalix.app` debug gerado com sucesso.
- Total: 42 testes, 0 falhas.

## Pirâmide

- Unitários: domínio, Unicode/UTF-16, settings, prompts e validação de contratos.
- Integração: coordinator com adapters falsos, fluxos React, IPC Tauri e persistência em arquivo.
- Smoke: configuração do bundle e geração real do `.app`.

## Cobertura instrumentada

O cliente frontend testável em isolamento (`native.ts` e `types.ts`) atingiu 100% de statements, branches, functions e lines. A cobertura Rust não foi instrumentada porque `cargo-llvm-cov` não está instalado; a suíte Rust executou 17 cenários.

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

- Validar preview, aplicar, undo temporário e restauração integral do clipboard em processos macOS reais.
- Confirmar o comportamento do AXObserver híbrido, o clamp pelo `NSScreen.visibleFrame` e o `NSPanel` não ativante com múltiplos monitores.
- Chrome, Safari, VS Code, Slack, Notes e TextEdit exigem permissão de Acessibilidade e execução manual; T5.4 e T5.5 permanecem abertos.
