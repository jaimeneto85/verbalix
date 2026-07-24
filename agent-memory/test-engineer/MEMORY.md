# Agent Memory — test-engineer

## Padrões de Teste
- O domínio Rust usa testes inline com fakes de `SelectionPort`, `OverlayPort` e `AiProvider`.
- O frontend usa Vitest; contratos IPC são testados com mock hoisted de `invoke`.
- Contratos da Edge Function usam `deno test` sem dependências externas.
- Smoke tests validam configuração e também executam o bundle `.app` pelo CLI do Tauri.
- Boundaries macOS que não podem ser exercitados sem permissão real usam uma combinação de funções puras Rust para geometria e contratos estáticos Vitest para garantir APIs AX/Core Graphics, ausência de AppKit no worker e lifecycle do shell.
- Text markers permanecem sem mocks FFI: a suíte combina matriz pura e exaustiva de categorias AX, validação de índices/length em UTF-16, fluxo read-only até nota e contrato estático das APIs públicas/RAII; o gate real cobre o adapter nativo.
- O fluxo composto de fallback AX deve combinar matriz pura de categorias com contrato de integração do source: falha estrutural de CFRange não pode alcançar `marker_selection`, enquanto somente falhas explícitas de capacidade podem fazê-lo.
- Transformações do toolbar delegam readiness exclusivamente ao comando Rust; testes frontend e Playwright exigem uma única chamada `transform_selection`, não chamam `ai_readiness` e não abrem a janela principal para todo erro.
- A transação de transformação é testada com `snapshot.id + request_id`: captura transitória durante `Processing` preserva o alvo pinado, invalidação real bloqueia provider/write, segunda ação é rejeitada e falha de undo após write mantém `Applied`.
- Supersede durante transformação exige testes separados: candidato equivalente preserva exatamente `snapshot.id + request_id`; PID ou identidade AX diferentes substituem o lease antes do provider; resposta de provider já iniciado fica inerte; falha de hide não ressuscita `Processing`; preview superseded falha antes do write. Feedback de erro usa helper puro e só pertence ao snapshot ID original.
- Histórico remoto pode ser testado sem Supabase real com servidor HTTP loopback que cobre `/auth/v1/user`, inserts de `translate`/`improve` e listagem autenticada; o contrato causal do command exige insert somente após `coordinator.transform` bem-sucedido.
- O budget da Responses API usa caracteres Unicode e precisa de boundaries discriminatórios: 558 caracteres ainda resultam no piso 500, 559 produzem 501, 11.806 produzem 7.999 e 11.807 alcançam 8.000; emoji não-BMP deve provar que UTF-16 não é usado.
- Validação de envelope Responses deve ser testada com output parcial que seria semanticamente válido: status ausente, desconhecido ou incomplete e `incomplete_details` não nulo precisam falhar antes do parse; completed aceita details nulo ou ausente.
- Restore precisa de testes separados e combinados para PID, identidade estável do elemento, texto selecionado e range UTF-16 atual; coincidência de texto/range em outro campo do mesmo PID deve continuar falhando fechada.
- Replace e restore exigem identidade forte antes de qualquer lookup/write AX; testes devem cobrir `identifier=None`, string vazia e somente whitespace, todos com zero escrita.
- Fluxos críticos de recuperação visual usam Playwright com `__TAURI_INTERNALS__` simulado e verificam tanto invocações IPC quanto clipping pelo bounding box.
- Superfícies de overlay transparentes exigem teste antes do render: a classe de rota deve existir durante o callback de bootstrap e `html`, `body` e `#root` devem computar fundo transparente, dimensões mínimas zero e overflow oculto sem alterar a rota principal.
- Posicionamento macOS em múltiplos monitores é testado como geometria pura em pontos Cocoa: conversão AX round-trip, escolha por centro/interseção, clamp nas quatro bordas, fallback vertical e coordenadas globais negativas. Um contrato estático separado impede reintroduzir `LogicalPosition`, `PhysicalPosition` ou `scale_factor` no caminho macOS.
- O fallback de geometria da seleção segue `SelectedRange → Cursor contido inclusivamente em FocusedElement → FocusedElement → None`; cursor sem frame válido falha fechado. A matriz cobre quatro cantos, pontos imediatamente externos, não finitos, overflow das somas, coordenadas negativas e frame cruzando a origem.
- A referência AX → Cocoa deve vir da zero screen, `NSScreen.screens.firstObject`, nunca de `mainScreen`, que acompanha a key window. O teste discriminatório usa uma key-window screen secundária com origem e altura diferentes.
- A primeira pintura do overlay usa handshake de readiness: testes separam `ready` de `requested`, comprovam render antes do sinal frontend e garantem que `HideAll` antes de `SurfaceReady` não ressuscita a janela.
- O handshake só deve nascer em `useLayoutEffect` depois do commit dos filhos, e o ACK nativo só pode resolver após a closure da main thread aplicar readiness/visibilidade. Retries precisam ser estritamente sequenciais, limitados a três após ACK falso/erro, parar no primeiro sucesso e reportar exaustão sem deixar invokes órfãos por `Promise.race`.
- Readiness é uma capacidade por documento: Rust emite geração UUID na URL, o estado nativo compara geração atual/pronta e o comando valida label mais identidade da WebView chamadora. ACK antigo, reload e rota sem geração devem falhar fechados.
- Reload não pode apenas girar a geração mantendo a URL antiga: no segundo `PageLoadEvent::Started`, a geração é invalidada e a WebView destruída; a próxima solicitação deve recriar janela, UUID e URL. Testes combinam lifecycle puro e contrato estático de destroy/diagnósticos/fallback.
- Criação de overlay deve ser transacional: `begin_document → build → configure`. Falha de build invalida sem executar rollback de recurso inexistente; falha de configure invalida antes de tentar `destroy → hide`, e uma criação posterior recebe geração nova.
- Invalidação deve ser compare-and-invalidate: callbacks e rollbacks carregam a geração esperada e só removem `current/ready` quando ela ainda coincide. Um rollback stale de G1 nunca pode apagar G2 já pronta.

## Estratégias de Mock
- Seleções mutáveis ficam em `Arc<Mutex<SelectionSnapshot>>` para simular mudança durante requests.
- Providers falsos retornam sucesso, timeout ou request ID divergente sem acessar a rede.
- O limite de Keychain é verificado pelo payload IPC; testes não gravam credenciais reais.
- Wiring com efeitos Tauri pode ser testado por callbacks `FnOnce` que contam separadamente abertura de janela e publicação de nota, mantendo o mesmo branch usado por produção sem construir `AppHandle`.

## Erros Recorrentes & Soluções
- Factories de `vi.mock` são hoisted; mocks compartilhados devem usar `vi.hoisted`.
- Clipboard e Accessibility reais não devem ser acionados em testes automatizados, pois alteram estado global do macOS.
- Coordenadas globais negativas são válidas em monitores secundários; validação geométrica deve rejeitar valores não finitos e dimensões inválidas sem rejeitar a origem negativa.
- Doubles Deno que implementam interfaces assíncronas sem executar `await` devem retornar `Promise.resolve`/`Promise.reject` explicitamente para satisfazer `deno lint require-await`.
- Limites superiores de índices marker em macOS arm64 devem considerar que `isize::MAX == i64::MAX`; o maior location válido antes de um range de length 1 é `isize::MAX - 1`.
- O analyzer QA considera linhas efetivas e impõe máximo de 300 por arquivo modificado; os boundaries macOS foram divididos e devem permanecer abaixo desse limite.

## Cobertura & Métricas
- O escopo instrumentado do cliente frontend (`native.ts` e `types.ts`) mantém 100% em statements, branches, functions e lines.
- A suíte Rust cobre state machine, latest-wins, stale selection, falhas seguras, Unicode/UTF-16, matriz AX, marker read-only, identidade forte de replace/restore, settings, readiness e geometria. `cargo-llvm-cov` não está instalado; os 70 testes Rust e os gates `clippy -D warnings` são usados como evidência.
- O frontend possui 50 testes Vitest e mantém 100% em statements, branches, functions e lines no escopo instrumentado (`native.ts`, `types.ts`); os 6 testes Playwright E2E também passam.
- A suíte Rust possui 121 testes, incluindo pinning e supersede da transformação, invalidação transitória/real, resposta remota fora de ordem, feedback stale, pós-write, histórico insert/list, identidade AX e a matriz de fallback geométrico.

## Observações
- Preview/apply/undo possuem integração mockada; a matriz AX e o fallback de clipboard ainda precisam de validação manual em um app com permissão de Acessibilidade.
- Configuração pública do Supabase é testada como build-time embutido com override de runtime; OpenAI/service-role não podem aparecer nos arquivos do runtime público.
- Limites HTTP da Edge são testados em bytes com JSON válido seguido de whitespace até exatamente 64 KiB; isso separa o boundary de transporte do limite de 12.000 caracteres do domínio.
- Testes pré-deploy da Edge mantêm Auth, scheduler e OpenAI totalmente injetados: cobrem usuário real/anon, timeout que vence provider não cooperativo, respostas por operação e limites exatos sem rede ou secrets reais.
- O 504 `PROVIDER_TIMEOUT` da Edge ainda não prova `ProviderTimeout` no Rust: `RemoteTransformer` converte non-2xx genérico em `ProviderRejected`, enquanto somente timeout de transporte reqwest vira `ProviderTimeout`. Não alegar tipagem ponta a ponta sem teste/correção específica.
- O hard gate Trivy pode ser reutilizado quando executado contemporaneamente na mesma worktree: scan `vuln,misconfig` HIGH/CRITICAL com `--ignore-unfixed --exit-code 1` passou para `package-lock`, `Cargo.lock` e configurações.
