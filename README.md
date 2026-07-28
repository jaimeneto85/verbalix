# Verbalix

Companheiro de escrita técnica para macOS. Selecione texto em qualquer aplicativo, escolha **Traduzir** ou **Aprimorar** em uma toolbar flutuante e receba o resultado no lugar — sem copiar, colar ou trocar de janela.

- Tradução PT→EN, EN→PT e outros idiomas→PT.
- Aprimoramento de texto técnico no idioma original, com formalidade, extensão e tom configuráveis.
- Substituição direta em campos editáveis, com desfazer temporário e preview opcional antes de aplicar.
- Nota somente leitura quando o campo não é editável.
- Histórico opcional (opt-in), owner-only e com retenção de 30 dias.

Requer **macOS 14+** e permissão de Acessibilidade.

## Como funciona

O Verbalix roda como app de menu-bar. Ele observa a seleção ativa via Accessibility API (AXUIElement + AXObserver com polling de apoio) e, após um debounce, mostra uma toolbar `NSPanel` não ativante ancorada na seleção — o foco nunca sai do aplicativo original.

Ao acionar uma operação, o texto vai para uma Edge Function Supabase autenticada, que é a única a falar com a OpenAI. O resultado volta, é revalidado contra a seleção original e só então aplicado.

Atalho global padrão: `Option+Shift+Space`. Quando a Accessibility API não expõe a seleção, o atalho aciona um fallback por clipboard que preserva e restaura o conteúdo original do pasteboard.

## Arquitetura

Arquitetura hexagonal no núcleo Rust. Dependências apontam sempre para dentro:

```
src-tauri/src/
├── domain/        lógica pura: SelectionSnapshot/State/Event, settings,
│                  contratos de transformação (trait AiProvider), VerbalixError
├── application/   SelectionCoordinator (máquina de estados), ports
│                  (SelectionPort, OverlayPort, ClipboardPort), readiness de IA,
│                  adapters remotos (Supabase, Keychain, settings em arquivo)
├── platform/      adapters macOS: acessibilidade, observer, geometria,
│                  clipboard, TauriOverlay + MainThreadOverlayDispatcher
├── commands.rs    superfície #[tauri::command]
├── diagnostics.rs tracing opt-in e sanitizado
└── lib.rs         wiring, tray, atalho global, threads de detecção
```

O frontend React fala com o Rust exclusivamente por `src/native.ts`, tipado em `src/types.ts`. `src/main.tsx` monta uma de duas raízes a partir do mesmo bundle, conforme o parâmetro `?overlay=` da URL: `Overlay` (toolbar e nota, abertas como janelas Tauri separadas) ou `App` (janela principal de configurações e histórico).

O backend fica em `supabase/`: a Edge Function `transform` (contrato, provider OpenAI e handler com JWT obrigatório) e as migrations da tabela de histórico com RLS.

### Garantias do núcleo

- **Toda operação de janela roda na main thread.** Callbacks do AXObserver e a thread de polling executam em background; qualquer NSWindow/NSPanel passa por `MainThreadOverlayDispatcher` como um `OverlayCommand`.
- **Latest-wins com revalidação.** Snapshots são imutáveis; toda escrita revalida alvo e `request_id` ativos. Falhas após `Processing` retornam ao estado de toolbar sem alterar o documento.
- **`RuntimePause` é o gate único** de polling, AXObserver, atalho global e fallback de clipboard.
- **Texto selecionado e segredos nunca são registrados.** Falhas externas viram variantes de `VerbalixError` sem conteúdo. Sessões ficam no Keychain; só preferências não sensíveis vão para `settings.json`.

## Configuração

Copie `.env.example` para `.env` na raiz e preencha:

```
VITE_SUPABASE_URL=
VITE_SUPABASE_ANON_KEY=
```

Esses são os nomes canônicos, usados tanto pelo Vite quanto pelo build Rust. O `build.rs` lê o par completo do `.env` e embute a configuração pública em `OUT_DIR`, sem passar valores por stdout ou `cargo:rustc-env`. Variáveis completas do ambiente do processo têm precedência em desenvolvimento. `VERBALIX_SUPABASE_URL` / `VERBALIX_SUPABASE_ANON_KEY` seguem aceitos apenas como aliases legados e nunca são combinados parcialmente com o par canônico.

A Edge Function recebe `OPENAI_API_KEY` e `OPENAI_MODEL` como secrets do Supabase. A chave da OpenAI nunca chega ao cliente.

Sem configuração válida o app abre normalmente e reporta `provider_not_configured`, orientando o usuário em vez de falhar em silêncio.

## Desenvolvimento

```bash
npm install
npm run tauri -- dev        # app completo
npm run dev                 # apenas a webview em :1420
```

Permissão de Acessibilidade precisa ser concedida ao bundle exato que está sendo executado. A tela de permissão do app explica o procedimento.

### Testes

```bash
npm test                                            # frontend (vitest)
npm run test:coverage                               # cobertura de native.ts e types.ts
npm run test:e2e                                    # Playwright, adapter Tauri simulado
npx vitest run src/native.test.ts                   # um arquivo
npx vitest run -t "applies a preview"               # um teste
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml coordinator
deno test supabase/functions/transform/
```

### Qualidade e bundle

```bash
npm run build
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml
npm run tauri -- build --debug --bundles app
codesign --verify --deep --strict src-tauri/target/debug/bundle/macos/Verbalix.app
```

### Diagnóstico

`VERBALIX_DIAGNOSTICS=1` habilita tracing por detecção, captura, coordenador, overlay e ciclo de vida. O trace registra apenas origem, UUID, PID, range UTF-16, bounds, writability, sequência, visibilidade e códigos de erro — nunca o texto selecionado.

## Estado atual

Gates automatizados, verificados na revisão mais recente:

| Gate | Resultado |
|---|---:|
| Rust | 52 aprovados |
| Frontend (vitest) | 28 aprovados |
| Cobertura (`native.ts`, `types.ts`) | 100% |
| E2E (Playwright) | 3 aprovados |
| Edge Function (deno) | 6 aprovados |
| `tsc` + build Vite | aprovado |
| Clippy `-D warnings` | aprovado |
| Bundle `.app` debug + `codesign` | aprovado |
| Smoke de execução | processo estável, degradação segura sem AX |

O que **não** está coberto por automação e continua sendo gate manual:

- Fluxo real toolbar → transformar → preview/aplicar → desfazer em apps externos (Chrome, Safari, VS Code, Slack, Notes, TextEdit), que exige bundle autorizado em Acessibilidade e sessão válida.
- Restauração integral do clipboard em processos macOS reais.
- `NSPanel` não ativante com múltiplos monitores e fullscreen.
- Transformação por IA de ponta a ponta, que depende de Edge Function implantada e sessão autenticada.

O bundle é assinado ad-hoc (`-`), sem identidade TCC estável entre rebuilds. Para identidade persistente use Apple Development ou Developer ID. Notarização permanece fora do escopo atual.

## Documentação

`docs/` contém um registro por escopo entregue, em ordem cronológica — do MVP (`001`) aos hotfixes de bundle, main thread, visibilidade da toolbar e runtime visível (`002`–`005`), além do relatório de testes. Planos de especificação ficam em `tasks/<nome>/plan.md`.
