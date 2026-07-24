# Fix: composição transparente e posição do overlay

## 0. SCOPE

Corrigir a toolbar flutuante do Verbalix no macOS para:

- eliminar o fundo retangular claro visível fora do cartão arredondado;
- manter janela e WebView realmente transparentes nos cantos;
- ancorar a toolbar ao centro do `bounds` real da seleção, na mesma tela;
- preservar painel não ativante, foco do aplicativo alvo, tamanho visual, ações e marca existentes.

Fora do escopo:

- redesenhar a toolbar, marca, ícones ou conteúdo;
- alterar captura, transformação por IA, autenticação ou backend;
- fazer merge, release ou alterar permissões TCC;
- usar um deslocamento específico para uma máquina como correção.

## 1. REQUIREMENTS

### Requisitos funcionais

- R1: `html`, `body`, `#root`, WebView e janela nativa dos overlays não podem pintar fundo fora do cartão.
- R2: os cantos externos ao `border-radius` da toolbar devem revelar integralmente o conteúdo do aplicativo abaixo.
- R3: a posição horizontal deve ser `selection.centerX - toolbar.width / 2`, limitada apenas à área visível da tela que contém a seleção.
- R4: a posição vertical deve ficar acima da seleção com o espaçamento atual e usar fallback abaixo somente quando necessário para permanecer visível.
- R5: coordenadas AX, AppKit e Tauri devem ser convertidas explicitamente em pontos da mesma base global; escala Retina e monitores com origens positivas ou negativas não podem deslocar a toolbar para outra região/tela.
- R6: a escolha da tela deve considerar a interseção/centro do `bounds`, não depender da tela atual da janela escondida.
- R7: toolbar e note continuam não ativantes, always-on-top, sem roubar foco.
- R8: o ajuste não pode alterar a aparência interna do cartão nem suas ações.

### Critérios de aceitação

- CA1: screenshot real mostra apenas o cartão arredondado, sem retângulo branco/cinza nos cantos.
- CA2: em Slack ou alvo equivalente, a toolbar fica centralizada sobre a seleção com erro máximo de um ponto antes do clamp.
- CA3: testes cobrem transparência do documento de overlay e posicionamento em tela Retina/múltiplos monitores.
- CA4: testes cobrem clamp nas quatro bordas e seleção com coordenadas globais negativas.
- CA5: Rust, Vitest, Playwright relevante, clippy estrito, build e limite de 300 linhas passam.
- CA6: Computer Use final confirma transparência, posição e estabilidade do processo; este plano entrega apenas código/QA para o agente raiz executar esse gate.

## 2. DESIGN

### Hipóteses a comprovar

- H1: `:root { background: #eef1f5 }` pinta a superfície da WebView, apesar de a janela Tauri usar `transparent(true)`.
- H2: `LogicalPosition` reinterpreta coordenadas globais AX com o fator de escala associado à janela/tela atual, produzindo deslocamento em Retina ou múltiplos monitores.
- H3: a conversão manual de `NSScreen.visibleFrame` e a API de posicionamento não compartilham a mesma origem/base em todos os layouts.

### Direção proposta

- Marcar `document.documentElement` como overlay sincronicamente, antes de `createRoot`, e aplicar transparência explícita a `html`, `body` e `#root` somente nessa rota. A rota também neutraliza o `min-width: 320px`, dimensões mínimas, margin e overflow globais que conflitam com a toolbar de 236 pontos.
- Reforçar no boundary AppKit que `NSWindow` não é opaca e usa `NSColor.clearColor`, preservando o painel não ativante. APIs privadas adicionais ou KVC não fazem parte da correção sem evidência.
- Extrair `overlay_geometry.rs`, com modelos explícitos de `full_frame` e `visible_frame`. A tela é escolhida pelo centro da seleção; se não houver contenção, vence a maior interseção com `full_frame`, seguida da tela mais próxima como fallback determinístico.
- Converter uma única vez o retângulo AX top-left para Cocoa bottom-left com a transformação global `cocoa_y = main_screen.frame.maxY - ax_y - ax_height`. O round-trip precisa ser testado em telas à esquerda, acima e abaixo da principal.
- Calcular em pontos Cocoa: `x = selection.midX - width / 2`; preferir `selection.maxY + gap` para posicionar acima; usar `selection.minY - gap - height` abaixo quando acima não couber; se nenhum lado couber, aplicar clamp determinístico no `visible_frame`.
- No macOS, aplicar o ponto final diretamente ao `NSPanel` por `setFrameOrigin:` dentro do dispatcher da main thread. Não usar `LogicalPosition`, `PhysicalPosition` nem `scale_factor` no caminho macOS.
- Manter o cálculo puro de ancoragem/clamp separado da aplicação nativa para permitir testes determinísticos e preservar o caminho Tauri fora do macOS.
- Extrair `macos_overlay_panel.rs` para configuração/composição/posição nativas e manter cada arquivo de produção com no máximo 300 linhas.
- Fora do macOS, preservar o caminho Tauri existente.

### Riscos

- Tornar toda a aplicação transparente por CSS pode afetar a janela principal; a regra deve ser restrita à rota de overlay.
- Trocar somente `LogicalPosition` por `PhysicalPosition` sem controlar a tela pode duplicar ou remover escala.
- Confundir altura da tela com o `maxY` global da principal falha em monitores acima/abaixo; a transformação deve operar na base global AppKit.
- Manipulação AppKit fora da main thread causa crash; toda configuração e posição permanecem no dispatcher atual.
- Alterar classe/ivars após o swizzle para `NSPanel` pode causar panic; usar apenas mensagens AppKit no boundary já estabelecido.
- `visibleFrame` não serve para escolher a tela porque exclui menu bar/Dock; escolha usa `full_frame` e clamp usa `visible_frame`.
- Sombras nativas ou da WebView podem simular uma moldura; a validação visual precisa conferir pixels nos quatro cantos sem remover o shadow interno existente às cegas.

### Síntese da análise dual

#### Riscos incorporados

- H1 foi confirmada: `:root` pinta `#eef1f5` e `body` impõe 320 pontos a uma janela de 236 pontos.
- O boundary atual mistura AX/AppKit em pontos com `LogicalPosition`, que pode reaplicar escala usando a tela associada à janela escondida/reutilizada.
- O algoritmo antigo escolhe tela pelo canto superior esquerdo e não implementa fallback real abaixo.
- `overlay_dispatcher.rs` já ultrapassa 300 linhas e deve ser separado antes de crescer.
- Transparência precisa existir antes da primeira pintura e não pode atingir a janela principal.

#### Oportunidades incorporadas

- O dispatcher atual já oferece o boundary correto de main thread e será preservado.
- A fórmula horizontal existente pode ser reutilizada após unificar o sistema de coordenadas.
- A separação em geometria pura e adapter AppKit permite reproduzir Retina/múltiplos monitores sem GUI.
- A correção não exige alterações em captura AX, domínio, autenticação, IA ou conteúdo visual.

## 3. TASKS

- [x] T1: executar análise dual de riscos e oportunidades e sintetizar o design final.
- [x] T2: adicionar testes que reproduzam documento opaco e erro de posição em Retina/múltiplos monitores.
- [x] T3: implementar transparência restrita às rotas de overlay no frontend.
- [x] T4: implementar composição nativa transparente de `NSWindow`/WebView no macOS.
- [x] T5: corrigir seleção de tela, conversão de coordenadas e posicionamento nativo em pontos.
- [x] T6: executar gates automatizados e análise de segurança/regressão.
- [x] T7: QA dual emitir `APPROVED`, `REJECTED_CODE` ou `REJECTED_TESTS`.
- [x] T8: registrar evidências e atualizar memórias; deixar Computer Use, merge e release para o agente raiz.

### Casos de teste obrigatórios

- Frontend: classe/atributo antes do render; backgrounds computados transparentes em `html/body/#root`; `min-width` neutralizado; rota principal preserva o fundo.
- Geometria: centro sem clamp; quatro bordas; acima quando cabe; abaixo quando não cabe acima; nenhuma direção cabe.
- Telas: origem X negativa; telas acima/abaixo; escalas 1x/2x sem multiplicação; seleção cruzando telas; seleção fora do `visible_frame` mas dentro do `full_frame`.
- Conversão: AX → Cocoa → AX preserva retângulo; janela toolbar e note usam alturas distintas.
- Lifecycle: janela reutilizada muda entre telas sem herdar a escala/posição anterior.
- Contrato: caminho macOS não usa APIs Tauri de posição/escala; painel continua não ativante e transparente.

## 4. QA

### Veredito da primeira revisão

`REJECTED_CODE`

- A conversão AX → Cocoa usa `NSScreen.mainScreen.frame.maxY`, mas o SDK define `mainScreen` como a tela da key window. Como o painel não ativa, essa tela pode ser a secundária do aplicativo alvo. A referência global correta precisa vir da zero screen, `NSScreen.screens.first`.
- O caso nominal 1x/2x executa duas chamadas idênticas e não protege o boundary que escolhe a referência vertical. É necessário testar explicitamente que a zero screen, e não a key-window screen, alimenta a conversão.
- A classe transparente é instalada antes do render React, porém não há sincronização que impeça `show()` antes do bootstrap do documento. O caminho deve tornar a primeira pintura determinística ou fornecer um handshake de readiness.

### Evidências aprovadas

- 14/14 testes focados de geometria Rust passaram.
- 7/7 testes focados de superfície/contrato Vitest passaram.
- `cargo clippy --all-targets --all-features -- -D warnings` passou na análise dual.
- Geometria pura, clamp, fallback vertical, seleção determinística de tela, transparência em três camadas e aplicação via `setFrameOrigin:` são direções corretas.
- Arquivos de produção do escopo permanecem abaixo de 300 linhas.

### Remediação obrigatória

- [x] Usar a zero screen como referência global AX → Cocoa e cobrir key window em uma tela secundária com origem/altura diferentes.
- [x] Substituir o teste Retina tautológico por um contrato que exerça o boundary de coordenadas em pontos e impeça o uso de `mainScreen` como zero screen.
- [x] Impedir a exibição do overlay antes de a superfície transparente estar pronta, sem alterar a janela principal.
- [x] Impedir que um handshake atrasado reabra um overlay invalidado antes de ficar pronto.
- [x] Reexecutar todos os gates antes de submeter nova revisão dual, sem Computer Use, merge ou release.

### Veredito da segunda revisão

`REJECTED_CODE`

- As remediações de zero screen, posicionamento nativo em pontos, transparência e lifecycle foram aprovadas pela análise dual.
- `src-tauri/src/lib.rs` foi modificado pelo registro de `overlay_surface_ready` e possui 309 linhas, violando o limite obrigatório de 300 linhas de CA5 e o gate do projeto para arquivos modificados.
- O handshake mantém a janela oculta até `ready && requested`; `HideAll` cancela solicitações antes de esconder e um `SurfaceReady` tardio não reabre a superfície.
- A sinalização ocorre antes do commit React, mas a classe e o CSS transparentes são aplicados sincronicamente antes do handshake; este ponto permanece como observação não bloqueante para a validação visual final.

### Remediação da segunda revisão

- [ ] Extrair responsabilidade de `src-tauri/src/lib.rs` até o arquivo ficar com no máximo 300 linhas, sem alterar o comportamento do runtime.
- [ ] Reexecutar Rust, Vitest, cobertura, Playwright, clippy estrito, build, diff-check e limite de linhas.
- [ ] Submeter nova revisão QA antes de Computer Use, merge ou release.
