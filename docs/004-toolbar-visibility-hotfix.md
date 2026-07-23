# 004 — Hotfix de visibilidade da toolbar

## Diagnóstico

A ausência da toolbar observada no bundle local tem como causa primária uma autorização de Acessibilidade stale. O bundle é assinado ad-hoc, sem `TeamIdentifier`, e cada rebuild ou mudança de caminho pode alterar seu requisito designado. O macOS pode manter uma entrada antiga habilitada enquanto `AXIsProcessTrusted` retorna falso para o bundle atual.

Também foi comprovado um defeito independente no debounce: recapturas equivalentes criavam um UUID novo, mas o coordenador mantinha o candidato anterior. O caller recebia o UUID descartado e `DebounceElapsed` era ignorado.

## Correções

- Recapturas equivalentes agora retornam o snapshot ativo e seu UUID estável.
- `VERBALIX_DIAGNOSTICS=1` habilita tracing por detecção, captura, coordenador e overlay.
- O tracing registra apenas origem, UUID, PID, range UTF-16, bounds, writability, sequência, visibilidade e códigos de erro.
- O dispatcher confirma criação/reuso, posição, `show`, `hide` e `is_visible` na main thread.
- A tela de permissão orienta remover a entrada antiga, adicionar o `Verbalix.app` atual, habilitar e reabrir.
- Nenhum reset automático de TCC é executado.

## Evidência

- A regressão falhou antes da correção mostrando UUIDs diferentes e passou depois.
- Rust: 32 testes aprovados.
- Frontend: 22 testes aprovados.
- E2E Playwright: 1 cenário aprovado com adapter Tauri simulado para o estado sem permissão.
- Edge Function: 6 testes aprovados.
- Clippy com warnings como erro, Vite build e bundle Tauri aprovados.
- `codesign --verify --deep --strict` aprovado no bundle recém-gerado.
- O E2E automatizado prova somente a orientação e a rechecagem da UI no estado simulado sem permissão; não prova o estado real do TCC.
- O smoke visual da toolbar no TextEdit continua bloqueado até o bundle atual ser autorizado em Acessibilidade.

## Operação local

Se o Verbalix já aparece habilitado, remova a entrada antiga em Privacidade e Segurança → Acessibilidade, adicione o bundle exato em `src-tauri/target/debug/bundle/macos/Verbalix.app`, habilite e reabra. Para identidade persistente entre builds, use Apple Development ou Developer ID; assinatura ad-hoc não oferece identidade TCC estável.

## Status

Implementação e testes concluídos na branch `hotfix-toolbar-visibility`. A revisão QA independente permanece pendente e nenhum merge ou push foi realizado.
