# Verbalix Overlay Main-Thread Crash Fix

## Scope

- Corrigir o crash ao selecionar texto causado por AppKit/NSWindow fora da main thread.
- Preservar AXObserver em background e despachar exclusivamente as operações de overlay para a main thread.
- Cobrir toolbar, nota, preview, undo e hide sem deadlock ou perda de erro.
- Não alterar UX, IA, autenticação ou compatibilidade de aplicativos.

## Requirements

- [ ] R1: Nenhum método AppKit/NSWindow pode executar no callback AXObserver.
- [ ] R2: `show_toolbar`, `show_note` e `hide_all` devem agendar trabalho pela main thread.
- [ ] R3: O caller deve receber erro de dispatch/setup sem bloquear indefinidamente.
- [ ] R4: Selecionar texto não deve encerrar o processo e deve abrir a toolbar.
- [ ] R5: Suítes existentes e build/bundle devem permanecer verdes.

## Design

- Separar preparação thread-safe do payload da execução AppKit.
- Usar o dispatcher do `AppHandle` para executar criação, configuração e mutação de janelas na main thread.
- Evitar espera síncrona do callback AXObserver; erros assíncronos devem degradar com invalidação segura, sem panic.
- Adicionar regressão que prove que o port pode ser invocado de worker thread e que operações nativas são encaminhadas ao executor principal.

## Tasks

- [ ] T1: Refatorar `TauriOverlay` com dispatcher main-thread.
- [ ] T2: Cobrir toolbar/note/hide e falhas de dispatch.
- [ ] T3: Rodar fmt, Clippy, Rust, frontend, Edge e bundle.
- [ ] T4: Executar smoke real de seleção e inspecionar crash reports novos.
- [ ] T5: Atualizar memória e documentação.

## Dual Analysis Proporcional

- Risco: esperar síncronamente pela main thread a partir de callbacks do sistema pode deadlockar; preferir dispatch assíncrono.
- Risco: mover somente `configure_nonactivating_panel` é insuficiente, pois criação, show/hide, position e emit também tocam janela.
- Oportunidade: centralizar o dispatcher no adapter corrige todos os callers sem contaminar coordinator ou AXObserver.
