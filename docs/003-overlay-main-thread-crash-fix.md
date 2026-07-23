# 003 — Correção do crash ao selecionar texto

## Diagnóstico

O crash report `verbalix-2026-07-23-191310.ips` registrou `Must only be used from the main thread`. O callback do AXObserver executava em uma worker thread e seguia o fluxo:

`observer_callback → RuntimePause → coordinator.dispatch → TauriOverlay::show_toolbar → configure_nonactivating_panel`

Esse caminho criava e alterava NSWindow/NSPanel fora da main thread, causando `SIGTRAP`.

## Solução

- `TauriOverlay` passou a produzir comandos independentes de AppKit.
- `MainThreadOverlayDispatcher` encaminha toolbar, nota, preview, undo e hide por `AppHandle::run_on_main_thread`.
- Criação, conversão em `NSPanel`, posição, eventos e visibilidade ficam dentro do mesmo boundary da main thread.
- O callback AXObserver continua em background e não espera sincronamente pela UI, evitando deadlock.
- Falhas de agendamento ou execução degradam para erro local sem panic.

## Validação

| Gate | Resultado |
|---|---:|
| Rust | 27 testes aprovados |
| Frontend | 21 testes aprovados |
| Edge Function | 6 testes aprovados |
| Build e Clippy | Aprovados |
| Bundle debug | Reconstruído |
| Smoke real com seleção em TextEdit | Processo permaneceu vivo, sem panic/output |

O smoke real criou e selecionou `technical sentence` no TextEdit. A inspeção visual posterior ficou limitada pela permissão ScreenCapture TCC da ferramenta de automação, mas o processo permaneceu vivo por mais de cinco segundos e o crash reproduzido anteriormente não ocorreu.

## Status

Hotfix concluído na branch `verbalix-macos-mvp`, sem novo push. Como a versão anterior já havia sido enviada antes da suspensão, este commit precisa de nova aprovação explícita antes de atualizar `main` remoto.
