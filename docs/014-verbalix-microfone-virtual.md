# [014] - M3: Microfone virtual "Verbalix Microphone"

## Contexto
Terceiro marco (M3) da interpretação ao vivo do Verbalix. Cria um dispositivo de ENTRADA
virtual "Verbalix Microphone" para que Zoom/Meet/Slack selecionem a saída traduzida (voz
clonada do M1, pipeline frase-a-frase do M2) como microfone. Constrói sobre o M1 (enrollment,
`docs/012`) e o M2 (interpretação ao vivo, `docs/013`), ambos já mergeados na `main`.

Restrição desta entrega: o usuário AINDA NÃO tem conta Apple Developer. O escopo foi limitado a
tudo que funciona com **build local** — um driver HAL (AudioServerPlugIn) compilado localmente e
instalado em `/Library/Audio/Plug-Ins/HAL` CARREGA no próprio Mac de desenvolvimento (Gatekeeper/
notarização só bloqueiam artefatos baixados, não um `.driver` construído e instalado localmente).
Developer ID, notarização e installer `.pkg` assinado ficam FORA do escopo, documentados abaixo
como pré-requisitos de distribuição.

## Escopo

### Incluído
- **Driver**: fork rebrandeado do BlackHole (GPL-3) em `virtual-mic-driver/` (fora do crate Tauri,
  LICENSE preservada), device "Verbalix Microphone", bundle `com.verbalix.virtualmic`, UID estável
  `com.verbalix.virtualmic:0`, 48 kHz, 2ch, `CFBundleVersion` 1.0. Scripts `build`/`install`/
  `uninstall` em `scripts/` (pedem sudo ao usuário, avisam do reset do coreaudiod, nunca embutem senha).
- **Rust**: ports `VirtualMicDevicePort` (status + listener de device-list) e `VirtualMicOutputPort`
  (abre por nome/UID, ring buffer 48 kHz, buffer depth/underruns); adapters macOS (`platform/virtual_mic.rs`,
  `platform/virtual_mic_output.rs`) atrás de `cfg(target_os="macos")` com stub; `PlaybackRouter`
  (roteia alto-falante vs mic virtual); integração no `LiveInterpretationCoordinator` (routing
  fail-open); comando `virtual_mic_status`; setting `output_to_virtual_mic`; filtro anti-feedback na
  captura; diagnostics sanitizado.
- **Frontend**: seção "Microfone virtual" no painel de Interpretação (status do driver, CTA de
  instalação apontando o script SEM rodar sudo, toggle de saída, indicador de destino do áudio + fallback).

### Excluído (fora do escopo)
- Developer ID / notarização / installer `.pkg` assinado — pré-requisitos de DISTRIBUIÇÃO (ver abaixo).
- Streaming incremental, jitter buffer adaptativo, cancelamento de eco (M4).
- Sync remoto de `output_to_virtual_mic` e espelho iOS.
- Correção da divergência de nome de evento `live-state`/`live-state-changed` (pré-existente do M2).

## Decisão de licença (fork BlackHole GPL-3 vs clean-room)

**Recomendação adotada: fork rebrandeado do BlackHole (GPL-3).** Justificativa:
1. BlackHole é um AudioServerPlugIn HAL maduro e battle-tested (loopback duplex). Um driver clean-room
   equivalente seria semanas de trabalho CoreAudio de alto risco — desproporcional para a v1.
2. A GPL-3 só impõe obrigações na **distribuição**. Para o build/uso **local** de desenvolvimento (todo
   o escopo desta entrega) não há distribuição → nenhuma obrigação dispara.
3. Publicar o fonte vendorizado em um remote git **satisfaz** a GPL-3 (o fonte fica disponível), desde
   que a `LICENSE` seja preservada — não é violação.
4. O risco real é mistura de licenças: o `virtual-mic-driver/LICENSE` (GPL-3) governa EXCLUSIVAMENTE
   aquele subtree e NÃO relicencia o app Tauri/Rust (licença própria do Verbalix). O driver é um artefato
   SEPARADO, processo à parte carregado pelo coreaudiod, nunca linkado no binário do app → sem
   contaminação de copyleft do produto.
5. Quando a distribuição existir, a obrigação de "oferecer o fonte do driver" já está satisfeita por
   manter o fonte + LICENSE vendorizados. Clean-room fica documentado como caminho futuro caso se queira
   um driver proprietário (relicenciável).

Débito técnico registrado: manter o fork em dia com o upstream BlackHole a cada major do macOS; o match
de device por NOME (v1) é frágil se o usuário renomear o device (mitigado por re-resolução via evento de
status). O `sudo killall coreaudiod` dos scripts reinicia TODO o áudio do sistema (aviso explícito no script).

## Solução Implementada

### Arquitetura
- **Roteamento minimamente invasivo**: `PlaybackRouter` implementa o `AudioPreviewPort` do M2 e é passado
  ao coordinator no lugar do `MacAudioPlayback`. `play(wav)`: se `route` ativo → decodifica → reamostra
  p/ 48 kHz → `virtual_mic.enqueue`; senão → `speaker.play` (comportamento M2). Isso mantém
  `live_worker.rs`/`live_queue.rs` INTACTOS (ordering do reorder buffer preservado).
- **Fail-open no routing, fail-closed no device**: `resolve_route` só ativa a rota se setting==true E
  device instalado E `open()` ok; qualquer falha cai para o alto-falante SEM derrubar a sessão (evento
  `virtual-mic-fallback`). Falha de `start_stream` após abrir o vmic fecha o vmic e reseta o route
  (corrige estado inconsistente). Perda de device mid-sessão (watch listener) → route=false + close +
  evento. O device emite SILÊNCIO quando o ring esvazia (nunca estala nem repete buffer). Nunca altera o
  dispositivo de saída padrão do sistema.
- **Anti-feedback**: `resolve_physical_input_device` resolve o device de captura explicitamente e nunca
  seleciona um device cujo nome começa com "Verbalix Microphone" (cobre o device principal e o "Mirror"),
  caindo para o próximo device físico ou falhando com erro sanitizado.
- **Adapter macOS**: `MacVirtualMicDevice` lê `CFBundleVersion` do bundle instalado e registra um property
  listener CoreAudio (`AudioObjectAddPropertyListener` em `kAudioHardwarePropertyDevices`); a callback
  nunca toca AppKit, só recomputa status e repassa. `MacVirtualMicOutput` usa thread dedicada dona do
  `cpal::Stream` (molde do `MacAudioPlayback`) com ring buffer bounded (~2 s), overflow = drop-oldest,
  contadores de underrun. Toda `unsafe`/FFI isolada; sem `unwrap` no callback do stream.
- **Privacidade**: diagnostics registra só enum de status + contadores numéricos (buffer depth, underruns),
  nunca áudio/UID/conteúdo (enforçado por teste). Variantes de erro sanitizadas.

### Arquivos Modificados
| Arquivo | Tipo |
|---------|------|
| `virtual-mic-driver/**` (fork BlackHole + LICENSE + README + xcodeproj) | Criado |
| `scripts/{build,install,uninstall}-virtual-mic.sh` | Criado |
| `src-tauri/src/platform/{virtual_mic,virtual_mic_output,virtual_mic_constants,audio_processing}.rs` | Criado |
| `src-tauri/src/application/{playback_router,live_session_setup}.rs` | Criado |
| `src-tauri/src/commands_virtual_mic.rs`, `diagnostics_tests.rs` | Criado |
| `src-tauri/src/application/{ports,mod,remote_preferences,live_interpretation}.rs` | Modificado |
| `src-tauri/src/platform/{mod,audio_capture,audio_wav,audio_playback}.rs` | Modificado |
| `src-tauri/src/{lib,runtime,diagnostics}.rs`, `domain/{settings,error}.rs`, `Cargo.toml` | Modificado |
| `src/components/VirtualMicSection.tsx` (+ test), `src/{native,types}.ts` (+ tests) | Criado/Modificado |
| `e2e/virtual-mic.e2e.ts` | Criado |

## Testes
| Métrica | Valor |
|---------|-------|
| Rust (`cargo test`) | 327 |
| Vitest (frontend) | 106 |
| Cobertura (native.ts + types.ts, threshold enforced) | 100% |
| Playwright e2e | 14 |
| Deno (regressão Edge Functions) | 112 |

## Verificação de Qualidade
| Critério | Status |
|----------|--------|
| `cargo test` | OK (327) |
| `cargo clippy --all-targets --all-features -- -D warnings` | Limpo |
| `cargo fmt --check` | Limpo |
| `npm test` / `npm run test:coverage` | OK / 100% |
| `npm run build` | OK |
| `npm run test:e2e` | OK (14) |
| `deno test supabase/functions/` | OK (112, sem regressão) |
| `tauri build --debug --bundles app` | OK (Verbalix.app assinado ad-hoc) |
| Build do driver (`scripts/build-virtual-mic.sh`) | OK (BUILD SUCCEEDED, VerbalixMicrophone.driver) |
| Gate de tamanho `lib.rs` (≤301) | 270 |
| Trivy (segurança) | 0 CRITICAL / 0 HIGH |
| QA (conformidade + análise dual) | APPROVED (após 1 ciclo REJECTED_CODE) |

### Histórico de QA
Verdict inicial `REJECTED_CODE` com 3 bloqueadores + 1 menor, todos corrigidos e reverificados:
1. `platform/audio_capture.rs` com 325 linhas efetivas (> gate 300) — extraído `audio_processing.rs`
   (`process_audio` + `resolve_physical_input_device`); arquivo ficou em 287.
2. Bug de estado: falha de `start_stream` após abrir o vmic deixava o stream bombeando silêncio e
   `route=true`/`Idle` inconsistente — o `map_err` agora chama `virtual_mic.close()` + `route.store(false)`
   (com teste dedicado).
3. Faltava E2E Playwright do `VirtualMicSection` (T7.5 marcado indevidamente) — adicionado `e2e/virtual-mic.e2e.ts`
   (4 casos: not-installed/installed/incompatible + toggle).
4. (menor) `PlaybackRouter.enqueue` hardcodava 1 canal — passa o `channels` real.

## Como instalar o driver localmente
```bash
bash scripts/build-virtual-mic.sh       # gera virtual-mic-driver/build/.../VerbalixMicrophone.driver
bash scripts/install-virtual-mic.sh      # pede sudo, copia p/ /Library/Audio/Plug-Ins/HAL, recarrega coreaudiod
# (para remover)
bash scripts/uninstall-virtual-mic.sh    # pede sudo, remove e recarrega coreaudiod
```
O app NÃO executa sudo nem roda os scripts — a UI apenas instrui o usuário a rodá-los.

## Gates Manuais Pendentes (NÃO verificados por testes automatizados)
1. Instalar o driver com sudo e ver "Verbalix Microphone" nas Preferências de Som.
2. Selecionar "Verbalix Microphone" como microfone no Zoom/Meet (Chrome/Safari)/Slack e o interlocutor
   ouvir a fala traduzida com a voz clonada.
3. sleep/wake (coreaudiod reinicia) → device re-detectado pelo listener.
4. Apps que cacheiam a lista de dispositivos (reabrir o app cliente após instalar).
5. Crash/sair do ar → silêncio no device (fail-closed).
6. Coexistência com outros drivers da família BlackHole (BlackHole/Loopback/Soundflower já instalados).
7. Auditoria `VERBALIX_DIAGNOSTICS=1` confirmando ausência de áudio/UID/conteúdo nos logs.
8. Warning benigno de build: `MACOSX_DEPLOYMENT_TARGET 10.10` herdado do BlackHole (builda mesmo assim).

## Pré-requisitos de distribuição (FORA do escopo — bloqueiam distribuição a terceiros)
Conta Apple Developer; Developer ID (app + driver + installer); notarização; installer `.pkg` privilegiado
assinado com uninstall/repair. Sem isso o driver não carrega em máquinas de terceiros (só localmente, no
Mac de desenvolvimento). O tauri.conf.json permanece `signingIdentity: "-"` (ad-hoc).

---
**Verificado por:** Workflow Orchestrator (gates re-executados empiricamente a cada handoff e na re-verificação dos bloqueadores)
**Data:** 2026-08-19
**Branch/Worktree:** `virtual-microphone` / `.worktrees/virtual-microphone` (NÃO mergeado)
**Status Final:** APROVADO — pendente de gates manuais e aprovação do usuário para merge
