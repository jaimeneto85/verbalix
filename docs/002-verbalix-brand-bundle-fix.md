# 002 — Marca e correção de abertura do Verbalix

## Diagnóstico

O bundle anterior encerrava durante `did_finish_launching` porque `src-tauri/icons/icon.png` era um PNG 16-bit RGBA incompatível com o decoder usado pelo runtime. O panic reportava dimensões 1024×1024 e buffer RGBA com 2.097.152 bytes. O artefato antigo também não continha `Contents/Resources` e falhava na verificação de assinatura.

## Solução

- Marca aprovada registrada em `branding/` com master 1024×1024, mark, wordmark e guia.
- Icon set Tauri regenerado a partir de PNG 8-bit RGBA.
- `icon.png` substituído por 512×512, 8-bit RGBA.
- `icon.icns` configurado explicitamente para o bundle macOS.
- Mark e paleta integrados à interface existente.
- Bundle reconstruído com `Contents/Resources/icon.icns` e `_CodeSignature`.

## Validação

| Gate | Resultado |
|---|---:|
| Frontend | 21 testes aprovados |
| Rust | 24 testes aprovados |
| Edge Function | 6 testes aprovados |
| Build Vite | Aprovado |
| Clippy `-D warnings` | Aprovado |
| `codesign --verify --deep --strict` | Aprovado |
| Launch smoke | Processo vivo por 5 segundos, sem panic/output |

O novo Info.plist referencia `icon.icns`, o bundle contém `Contents/Resources` e o crash do icon decoder não se reproduziu.

## Status

Correção pronta na branch/worktree `verbalix-macos-mvp`, sem merge em `main`. Assinatura Developer ID e notarização continuam sendo etapas de distribuição, fora deste escopo.
