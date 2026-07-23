# Verbalix macOS MVP — Test Report

## Resultado

- Rust: 21 testes aprovados.
- Frontend: 9 testes aprovados.
- Edge Function: 6 testes aprovados.
- Bundle: `Verbalix.app` debug gerado com sucesso.
- Total: 36 testes, 0 falhas.

## Pirâmide

- Unitários: domínio, Unicode/UTF-16, settings, prompts e validação de contratos.
- Integração: coordinator com adapters falsos, IPC Tauri e persistência em arquivo.
- Smoke: configuração do bundle e geração real do `.app`.

## Cobertura instrumentada

O cliente frontend testável em isolamento (`native.ts` e `types.ts`) atingiu 100% de statements, branches, functions e lines. A cobertura Rust não foi instrumentada porque `cargo-llvm-cov` não está instalado; a suíte Rust executou 21 cenários.

## Gates executados

```text
cargo test
npm run test:coverage
npm run build
deno test supabase/functions/transform/contract_test.ts
npm run tauri -- build --debug --bundles app
```

## Gaps que exigem QA manual ou implementação

- O preview antes de substituir ainda não existe no fluxo de produção.
- Undo possui validação estrita de conteúdo alterado, mas a UI temporária para acioná-lo ainda não existe.
- Clipboard copy-only precisa ser validado em processo real para confirmar restauração com conteúdos não textuais.
- AXObserver híbrido, clamp completo por visible frame e painel `NSPanel` genuinamente não ativante não estão concluídos.
- Chrome, Safari, VS Code, Slack, Notes e TextEdit exigem permissão de Acessibilidade e execução manual; T5.4 e T5.5 permanecem abertos.
