# Entrega — transação de Traduzir e Aprimorar

## Resultado

O fluxo de transformação agora permanece ligado ao snapshot e request originais, revalida o alvo Accessibility imediatamente antes da escrita e impede que resultados atrasados alterem uma seleção posterior.

Traduzir e Aprimorar preservam os comportamentos esperados:

- conteúdo editável válido é substituído;
- conteúdo somente leitura recebe nota;
- confirmação habilitada exige Aplicar;
- falhas de sessão, provider, seleção, Accessibility, pin, Aplicar e Desfazer geram feedback tipado;
- histórico é persistido fora do caminho crítico, com timeout e diagnóstico sanitizado.

## Concorrência e overlay

Cada transformação possui uma lifetime cancelável. Cada comando visual guardado recebe um `PublicationPermit` independente, criado antes da preparação da janela.

O executor segue `prepare → claim → emit/show`:

- cancelamento durante `get/create/place` produz zero emissão ou exibição;
- permits pendentes e futuros são revogados pelo cancelamento;
- comandos sequenciais legítimos da mesma ação não se bloqueiam;
- se o claim vencer, o supersede posterior preserva `publish → hide` e termina oculto;
- ACK tardio não ressuscita uma superfície cancelada.

No boundary Accessibility, Focus, Destroyed, Selected externo e mouse convergem em um protocolo: expectativas `Armed|Authorizing` são canceladas e removidas antes do incremento causal. Se o writer já venceu em `InSetter|Committed`, a expectation permanece válida para uma única self-notification.

## Evidências

- Rust: 229/229
- Vitest: 55/55, cobertura 100%
- Deno: 38/38
- Playwright: 6/6
- `cargo fmt`, `cargo check`, `cargo clippy -D warnings`, build e `git diff --check`: aprovados
- limite de 300 linhas efetivas por arquivo modificado: aprovado
- QA dual RF42: `APPROVED`

## Gate operacional restante

Antes de merge/release, executar smoke macOS real com Accessibility e backend configurado:

- Traduzir e Aprimorar em seleção editável;
- nota em conteúdo somente leitura;
- preview seguido de Aplicar;
- supersede durante transformação;
- TextEdit e Slack;
- confirmação do requisito de assinatura estável e permissão TCC do bundle final.
