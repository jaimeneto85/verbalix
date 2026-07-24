# Entrega — Fallback geométrico do cursor

## Resultado

O resolver de geometria agora preserva a seguinte prioridade:

1. bounds válidos da seleção;
2. cursor finito, somente quando contido no frame válido do elemento focado;
3. frame do elemento focado;
4. ausência de geometria.

O cursor isolado deixou de ser aceito. A contenção usa bordas inclusivas, margem zero, rejeita overflow dos extremos do frame e preserva coordenadas globais negativas.

## Escopo preservado

Nenhuma FFI, conversão AppKit, captura de texto, foco, mutação, posicionamento final da janela, Auth ou IA foi alterada.

## Evidências automatizadas

- Rust: 101 testes aprovados;
- `cargo fmt`, `cargo check` e Clippy estrito: aprovados;
- Vitest: 47 testes aprovados;
- cobertura frontend configurada: 100%;
- Playwright: 6 testes aprovados;
- build e `git diff --check`: aprovados;
- arquivos de produção: até 300 linhas;
- Trivy: zero HIGH/CRITICAL.

A matriz pura cobre prioridade, cursor interno e externo, quatro bordas e pontos imediatamente externos, `NaN`/infinito, frames inválidos, overflow, coordenadas X/Y negativas e frames cruzando a origem global.

## QA

Verdict: `APPROVED` para código e testes automatizados.

As análises pessimista e otimista convergiram no mesmo risco residual: contenção espacial não prova que o cursor ainda representa uma seleção feita por teclado dentro de um editor grande.

## Gate operacional pendente

Antes de merge ou release, repetir o smoke real no Slack por Computer Use:

- seleção por mouse;
- seleção por teclado;
- cursor movido dentro do mesmo editor;
- confirmação visual e diagnostic `geometry_source=cursor`;
- processo estável e overlay associado à seleção.

Se esse gate falhar, a próxima correção deve introduzir um sinal causal temporal próprio; não deve ampliar margem nem alterar a conversão AppKit.
