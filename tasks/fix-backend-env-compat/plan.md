# Fix: configuração pública compatível no bundle

## 0. SCOPE

Corrigir a integração entre o `.env` existente e o runtime nativo sem criar duas configurações públicas concorrentes.

Incluído:

- adotar `VITE_SUPABASE_URL` e `VITE_SUPABASE_ANON_KEY` como nomes canônicos;
- aceitar `VERBALIX_SUPABASE_URL` e `VERBALIX_SUPABASE_ANON_KEY` somente como aliases legados opcionais;
- carregar o `.env` da raiz durante o build Rust/Tauri e embutir os dois valores públicos no bundle;
- atualizar testes, `.env.example` e documentação;
- validar presença/configuração sem imprimir os valores.

Fora do escopo:

- adicionar service-role, chave OpenAI ou qualquer segredo ao cliente;
- implantar projeto Supabase/Edge Function;
- autenticar uma conta;
- merge ou push sem aprovação explícita.

Fluxo SDD simplificado: correção localizada de configuração/build, sem alteração da arquitetura do provider.

## 1. REQUIREMENTS

- R1: `VITE_SUPABASE_*` é a única nomenclatura documentada e presente no `.env.example`.
- R2: o nativo resolve primeiro os nomes canônicos e usa `VERBALIX_*` apenas quando o par canônico correspondente está ausente/vazio.
- R3: URL e anon key devem vir da mesma fonte/par; não combinar URL canônica com key legada silenciosamente.
- R4: variáveis do processo têm precedência sobre valores embutidos para desenvolvimento.
- R5: `src-tauri/build.rs` deve observar as variáveis e o `.env` da raiz, disponibilizando valores públicos por arquivo Rust gerado em `OUT_DIR`.
- R6: o build não registra URL, anon key ou conteúdo do `.env` em stdout/stderr, snapshots, docs ou commits.
- R7: `.env` real permanece ignorado pelo Git.
- R8: ausência/incompletude continua produzindo `provider_not_configured`; par válido produz o próximo estado de readiness sem revelar valores.
- R9: aliases legados permanecem cobertos por teste, mas não são duplicados no example.

## 2. DESIGN

### Resolução

`PublicBackendConfig` recebe pares completos nesta ordem:

1. `VITE_SUPABASE_*` do processo;
2. `VERBALIX_SUPABASE_*` do processo;
3. `VITE_SUPABASE_*` embutido pelo build;
4. `VERBALIX_SUPABASE_*` embutido legado.

Um par só é elegível quando URL e anon key estão ambos não vazios. URL inválida mantém `configured=false`. Essa resolução por par evita misturar ambientes/projetos.

### Build do bundle

O build script:

- marca `cargo:rerun-if-env-changed` para os quatro aliases e `cargo:rerun-if-changed=../.env`;
- lê o `.env` da raiz sem modificar o processo do usuário;
- captura separadamente os pares canônico e legado, preferindo para cada um variáveis já exportadas e depois o arquivo;
- gera um arquivo Rust em `OUT_DIR`, incluído pelo runtime sem transportar valores por stdout;
- nunca imprime valores em mensagens diagnósticas.

O frontend continua obtendo `PublicBackendConfig` pelo comando nativo; não existe uma segunda resolução divergente na WebView.

### Validação segura

- testes unitários usam placeholders fictícios para precedência, pares incompletos e alias legado;
- teste estrutural confirma os nomes e a política do build script, sem ler `.env` real;
- bundle smoke verifica apenas `configured/readiness` e exit status, nunca os campos de configuração;
- scanner confirma que service-role/OpenAI secrets não aparecem no runtime público.

## 3. TASKS

- [x] T1 Implementar resolução canônica por pares com fallback legado.
- [x] T2 Carregar `VITE_SUPABASE_*` do `.env` no build Rust/Tauri sem logar valores.
- [x] T3 Restaurar `.env.example` para somente os dois nomes `VITE_*`.
- [x] T4 Atualizar regressões Rust, bundle-smoke e documentação.
- [ ] T5 Executar Rust, Clippy, Vitest, Playwright, Edge, build Tauri e analyzer.
- [ ] T6 Smoke do bundle confirma readiness configurada sem expor URL/key.
- [ ] T7 QA independente emite verdict.
