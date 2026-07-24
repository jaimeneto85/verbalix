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
- CA2: em Slack ou alvo equivalente, a toolbar fica centralizada sobre a seleção com tolerância visual de 12 pontos antes do clamp.
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

- Marcar o documento como overlay antes da primeira pintura e aplicar transparência explícita a `html`, `body` e `#root` somente nessa rota.
- Reforçar no boundary AppKit que `NSWindow` e a view de conteúdo/WebView não são opacas e usam `clearColor`, preservando o painel não ativante.
- No macOS, posicionar o painel no boundary AppKit em coordenadas Cocoa por pontos, com conversão única entre o sistema AX top-left e o sistema Cocoa bottom-left baseada no frame da tela selecionada.
- Manter cálculo puro de ancoragem/clamp separado da aplicação nativa para permitir testes determinísticos.
- Fora do macOS, preservar o caminho Tauri existente.

### Riscos

- Tornar toda a aplicação transparente por CSS pode afetar a janela principal; a regra deve ser restrita à rota de overlay.
- Trocar somente `LogicalPosition` por `PhysicalPosition` sem controlar a tela pode duplicar ou remover escala.
- Usar altura apenas da tela principal falha em monitores acima/abaixo; a conversão deve usar o frame global AppKit da tela escolhida.
- Manipulação AppKit fora da main thread causa crash; toda configuração e posição permanecem no dispatcher atual.
- Alterar classe/ivars após o swizzle para `NSPanel` pode causar panic; usar apenas mensagens AppKit no boundary já estabelecido.

## 3. TASKS

- [ ] T1: executar análise dual de riscos e oportunidades e sintetizar o design final.
- [ ] T2: adicionar testes que reproduzam documento opaco e erro de posição em Retina/múltiplos monitores.
- [ ] T3: implementar transparência restrita às rotas de overlay no frontend.
- [ ] T4: implementar composição nativa transparente de `NSWindow`/WebView no macOS.
- [ ] T5: corrigir seleção de tela, conversão de coordenadas e posicionamento nativo em pontos.
- [ ] T6: executar gates automatizados e análise de segurança/regressão.
- [ ] T7: QA dual emitir `APPROVED`, `REJECTED_CODE` ou `REJECTED_TESTS`.
- [ ] T8: registrar evidências e atualizar memórias; deixar Computer Use, merge e release para o agente raiz.
