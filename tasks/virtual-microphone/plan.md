# M3 — Microfone virtual "Verbalix Microphone"

Terceiro marco da interpretação ao vivo. Cria o dispositivo de entrada virtual
"Verbalix Microphone" para que Zoom/Meet/Slack selecionem a saída traduzida (voz clonada
do M1, pipeline frase-a-frase do M2) como microfone. Sobre M1 (`docs/012`) e M2 (`docs/013`),
ambos mergeados na `main`.

Worktree dedicado: `.worktrees/virtual-microphone` (branch `virtual-microphone`). NÃO mergear em main.

## Restrição de ambiente (crítica para escopo)

O usuário AINDA NÃO tem conta Apple Developer. Portanto:
- **DENTRO do escopo**: tudo que funciona com **build local**. Um driver HAL (AudioServerPlugIn)
  compilado localmente e copiado para `/Library/Audio/Plug-Ins/HAL` CARREGA no próprio Mac de
  desenvolvimento — Gatekeeper/notarização só bloqueiam artefatos **baixados** (quarantine bit),
  não um `.driver` construído e instalado localmente com sudo.
- **FORA do escopo (pré-requisitos de distribuição, documentar no doc de entrega)**: Developer ID
  (app + driver + installer), notarização, installer `.pkg` privilegiado assinado com uninstall/repair.

## 🎯 SCOPE

### Arquivos a criar
- `virtual-mic-driver/` — fonte do driver HAL vendorizado (fork rebrandeado do BlackHole), **fora**
  do crate Tauri, com `LICENSE` (GPL-3) preservada e `README.md` explicando origem/rebrand.
- `scripts/build-virtual-mic.sh` — compila o `.driver` (xcodebuild) para `build/` local.
- `scripts/install-virtual-mic.sh` — PEDE sudo ao usuário; copia `.driver` p/ `/Library/Audio/Plug-Ins/HAL`
  + `sudo killall coreaudiod`. NUNCA embute senha.
- `scripts/uninstall-virtual-mic.sh` — PEDE sudo; remove o `.driver` + recarrega coreaudiod.
- `src-tauri/src/platform/virtual_mic.rs` (+ `_tests` se puro) — adapters macOS `VirtualMicDevice`/`VirtualMicOutput`.
- `src-tauri/src/application/playback_router.rs` (+ `_tests`) — roteamento de playback (PURO/testável).
- `src-tauri/src/commands_virtual_mic.rs` — comandos `virtual_mic_status`, evento de status.

### Arquivos a modificar
- `src-tauri/src/application/ports.rs` — novos ports `VirtualMicDevicePort`, `VirtualMicOutputPort`.
- `src-tauri/src/application/live_interpretation.rs` — enter/leave_live abrem/fecham a sessão do vmic
  e definem o roteamento (fail-open p/ alto-falante se vmic falhar; NUNCA falhar a sessão por causa do vmic).
- `src-tauri/src/application/mod.rs` — re-exports.
- `src-tauri/src/platform/mod.rs` — stub não-macOS dos dois ports.
- `src-tauri/src/platform/audio_capture.rs` — EXCLUIR o UID do Verbalix da enumeração de dispositivos de
  CAPTURA (anti-feedback-loop). Ver EC.
- `src-tauri/src/domain/settings.rs` — `output_to_virtual_mic: bool` (`#[serde(default)]`, macOS-only, não sync).
- `src-tauri/src/domain/error.rs` — variantes sanitizadas (`VirtualMicUnavailable`, etc.).
- `src-tauri/src/application/remote_preferences.rs` — preservar `output_to_virtual_mic` no struct-literal `apply_remote`.
- `src-tauri/src/runtime.rs` — construir vmic device/output + router; wiring no `AppRuntime` e `build_live_coordinator`.
- `src-tauri/src/lib.rs` — registrar comandos novos; registrar listener de device-list e emitir evento.
- `src-tauri/src/diagnostics.rs` — status do device, buffer depth, underruns (sem conteúdo).
- `src-tauri/Cargo.toml` — deps CoreAudio (ex. `coreaudio-sys`/`core-foundation`) p/ UID + property listener.
- `src/native.ts`, `src/types.ts` — `virtualMicStatus()`, `onVirtualMicStatusChange()`, tipos.
- `src/components/LivePanel.tsx` (e/ou `InterpretationPanel.tsx`) — seção do microfone virtual + toggle de saída.
- `src/components/SettingsPanel.tsx` — toggle `output_to_virtual_mic` (se colocado em settings).
- `src/styles/panels.css` — estilos da seção.

### Fora do escopo
- Developer ID / notarização / installer `.pkg` assinado (pré-requisito de distribuição — documentar).
- Streaming incremental, jitter buffer adaptativo, cancelamento de eco (M4).
- Sync remoto de `output_to_virtual_mic` e espelho iOS.
- Correção da divergência de nome de evento `live-state` vs `live-state-changed` (pré-existente do M2, fora do escopo).
- Alterar o `SelectionCoordinator` (M1) ou o pipeline de rede do M2.

### Riscos de impacto
- R1: BlackHole é **GPL-3** — decisão de licença é bloqueante de DESIGN (ver seção DESIGN).
- R2: driver HAL é uma base grande em C/Obj-C; build via xcodebuild pode não estar disponível no sandbox de
  CI → o gate "build do driver" pode cair para gate manual se o toolchain Xcode não existir no ambiente.
- R3: property listener de device-list (`AudioObjectAddPropertyListener`) é CoreAudio puro, além do `cpal` já
  usado — requer `coreaudio-sys`/FFI atrás de `cfg(macos)`; risco de unsafe mal isolado.
- R4: feedback loop se o vmic virar dispositivo de captura do próprio app (ver EC).
- R5: mudanças em `live_interpretation.rs` (já em 340 linhas efetivas) podem estourar o gate de ~300 linhas.
- R6: `lib.rs` tem gate real de ≤301 linhas (`src/bundle-smoke.test.ts`) — novos comandos/listener devem entrar
  extraindo p/ `runtime.rs`/`commands_virtual_mic.rs`, não inflando `lib.rs`.

## 📋 REQUIREMENTS

### Requisitos Funcionais
- RF01: Decisão de licença documentada (fork BlackHole vs clean-room) com recomendação justificada.
- RF02: Fonte do driver vendorizado em `virtual-mic-driver/` com LICENSE preservada, rebrandeado:
  device name "Verbalix Microphone", bundle id `com.verbalix.virtualmic`, UID estável, 48 kHz, 2ch.
- RF03: Script de build gera o `.driver`; scripts de install/uninstall pedem sudo e recarregam coreaudiod.
- RF04: `VirtualMicDevicePort`: status `NotInstalled | Installed | IncompatibleVersion` + listener de
  mudanças na lista de dispositivos Core Audio.
- RF05: `VirtualMicOutputPort`: abre o device Verbalix pelo UID, enfileira PCM 48 kHz, expõe buffer depth
  e underruns, e NUNCA altera o dispositivo de saída padrão do sistema.
- RF06: No M2, destino de playback selecionável — alto-falante (monitor, comportamento atual) OU mic virtual
  quando instalado + on-air. Setting `output_to_virtual_mic` (bool). Se driver não instalado → comportamento
  atual + CTA de instalação (sem falhar a sessão).
- RF07: Frontend — seção do microfone virtual no painel Interpretação: status do driver (não instalado /
  instalado / incompatível), instruções apontando para o script (o app NÃO executa sudo), indicação de para
  onde o áudio está saindo.
- RF08: Diagnostics: status do device, buffer depth, underruns; nunca conteúdo/voz.

### Requisitos Não-Funcionais
- RNF01: Adapters macOS atrás de `cfg(target_os="macos")` com stub compilando fora do macOS (padrão do projeto).
- RNF02: Arquivos <~300 linhas efetivas; `lib.rs` ≤301 (gate real); sem comentários; IPC camelCase.
- RNF03: AppKit/CoreAudio thread-safety — stream contínuo do vmic possuído por thread dedicada (padrão M2 do
  `MacAudioPlayback`); surface `Send`-safe via canal.
- RNF04: Fail-closed: silêncio no device quando não on-air ou em falha; fail-open só do ROTEAMENTO (cai p/
  alto-falante) — a sessão nunca cai por causa do vmic.
- RNF05: `output_to_virtual_mic` NÃO sincronizado (macOS-only) e preservado em `apply_remote`.
- RNF06: Nenhuma dependência nova de rede; nenhum segredo.

### Critérios de Aceitação
- CA01: Todos os gates verdes (lista abaixo) DENTRO do worktree.
- CA02: `virtual-mic-driver/LICENSE` presente e não removida; README documenta o rebrand.
- CA03: Com driver não instalado, `enter_live` funciona igual ao M2 (alto-falante) e a UI mostra CTA de instalação.
- CA04: `select` do vmic como captura é impossível pelo app (UID excluído da enumeração de input).
- CA05: Coordinator/worker do M2 não regridem (testes M2 continuam passando).

### Edge Cases
- EC01: Driver instalado mas versão incompatível (UID presente, versão ≠) → status `IncompatibleVersion`, UI orienta reinstalar.
- EC02: Device desconectado/removido durante on-air (usuário desinstala) → fail-closed (silêncio) + fallback p/
  alto-falante + evento de status; sessão não trava.
- EC03: vmic virtual aparece na lista de dispositivos de CAPTURA — o adapter de captura DEVE filtrar pelo UID do
  Verbalix para não realimentar (mic→traduz→vmic→captura→loop infinito).
- EC04: `output_to_virtual_mic=true` mas driver não instalado → roteia p/ alto-falante + emite necessidade de instalação.
- EC05: sleep/wake — coreaudiod reinicia; o listener de device-list deve re-detectar o device (gate manual).
- EC06: Underrun (pipeline atrasa e o ring esvazia) → device emite silêncio, incrementa contador de underrun; nunca estala/repete buffer.
- EC07: enter_live com routing ativo e `open()` do vmic falha → log sanitizado + fallback alto-falante + UI indica saída real.
- EC08: leave_live/crash → `close()` do vmic para o stream (silêncio), nunca deixa o device preso emitindo áudio velho.
- EC09: toggle `output_to_virtual_mic` MID-sessão → só vale no próximo `enter_live`; a UI deve indicar "aplica na
  próxima sessão" (evita QA reportar "toguei e nada mudou").
- EC10: `IncompatibleVersion` em `enter_live` com routing → fail-open p/ alto-falante (igual `NotInstalled`) + UI reinstalar.
- EC11: outro driver da família BlackHole (BlackHole/Loopback/Soundflower) coexistindo → bundle id/UID distintos devem
  conviver; gate manual documentado.
- EC12: settings.json de instalações M1/M2 pré-existentes sem o campo novo → `#[serde(default)]` cobre; teste explícito.
- EC13: thread do `MacVirtualMicOutput` morre (panic na callback) mid-sessão → sem supervisor a v1 apenas silencia
  (fail-closed); documentar como limitação (igual `MacAudioPlayback`). Evitar `unwrap` na callback do stream.

## 🏗️ DESIGN

### RF01 — Decisão de licença (BLOQUEANTE)

**Nota GPL-3 / repositório (síntese da análise dual):** publicar o fonte vendorizado em um remote git
SATISFAZ a GPL-3 (o fonte fica disponível), não a viola — desde que a `LICENSE` GPL-3 seja preservada no
subdiretório. O risco real não é "push = distribuição proibida", e sim **mistura de licenças**: o
`virtual-mic-driver/LICENSE` (GPL-3) governa EXCLUSIVAMENTE aquele subtree e NÃO relicencia o app Tauri/Rust
(que permanece sob a licença própria do Verbalix). O driver é um artefato SEPARADO, processo à parte carregado
pelo coreaudiod, nunca linkado no binário do app → sem contaminação de copyleft do produto. Documentar isso
explicitamente no README de `virtual-mic-driver/` e no doc de entrega. Como derivative work, TODO o fork
(inclusive as partes rebrandeadas) permanece GPL-3 — o que já é o caso por manter a LICENSE.

**Recomendação: fork rebrandeado do BlackHole (opção a) para a v1.** Justificativa:

1. BlackHole é um AudioServerPlugIn HAL maduro e battle-tested (loopback duplex). Um driver clean-room
   equivalente é semanas de trabalho CoreAudio de alto risco — desproporcional para a v1.
2. **GPL-3 só impõe obrigações na DISTRIBUIÇÃO.** Para o build/uso **local** de desenvolvimento (que é todo
   o escopo desta tarefa, dado que não há conta Apple Developer) NÃO há distribuição → nenhuma obrigação
   dispara. O fork é seguro agora.
3. Quando a distribuição existir, a GPL-3 exige oferecer o **fonte do driver** distribuído — já satisfeito por
   vendorizar o fonte com LICENSE em `virtual-mic-driver/`. É a rota de menor atrito.
4. A opção (b) clean-room fica documentada como caminho futuro CASO o usuário queira um driver proprietário
   (relicenciável), mas NÃO é necessária para funcionar localmente.

Consequência de design: o fork vive em `virtual-mic-driver/` **separado do crate Tauri**, com LICENSE preservada;
o rebrand troca device name → "Verbalix Microphone", bundle id → `com.verbalix.virtualmic`, UID estável (constante
`com.verbalix.virtualmic:0`), formato 48 kHz / 2ch. Nenhum código GPL entra no binário Rust/Tauri (o driver é um
processo separado carregado pelo coreaudiod), evitando contaminação de licença do app.

### Padrões reutilizados
- **Thread dedicada dona do stream** (de `MacAudioPlayback`, M2): o `VirtualMicOutput` mantém um `cpal::Stream`
  de OUTPUT no device Verbalix, possuído por uma thread; comunica por canal + ring buffer. Emite silêncio quando
  o ring esvazia (fail-closed).
- **Port + adapter cfg(macos) + stub** (M1/M2): dois ports novos com stub não-macOS.
- **Setting macOS-only não-sync** (padrão `shortcut`): `output_to_virtual_mic` com `#[serde(default)]`, preservado
  em `apply_remote` via struct-literal (força tratamento em compile-time).
- **Evento publish/listen** (M2 `live-state`): novo evento `virtual-mic-status`.
- **Variantes sanitizadas de `VerbalixError`** (M1/M2): nada de conteúdo/UID sensível.

### Resolução de device por UID vs cpal (spike T3.0 — decisão de DESIGN)
`cpal` NÃO abre device por UID (só por nome via `host.output_devices()` + `Device::name()`). Decisão adotada
para a v1 (menor unsafe, pragmática): **nós controlamos o nome** do device no fork ("Verbalix Microphone"), então
o `MacVirtualMicOutput` casa `output_devices()` por NOME constante. O UID (via `coreaudio-sys`
`kAudioHardwarePropertyDeviceForUID`) é usado para o `VirtualMicDevicePort` (status + listener de device-list),
onde precisamos de identidade estável. Limitação conhecida (documentar): se o usuário RENOMEAR o device no
System Settings, o match por nome quebra → mitigação: re-resolução periódica + status via evento. `coreaudio-sys`
JÁ está no `Cargo.lock` (transitivo via cpal→coreaudio-rs) — adicionar como dep direta. `Xcode 26.6` presente no
ambiente → build do `.driver` roda localmente (não é gate manual neste ambiente).

### Interfaces (Rust)
```
enum VirtualMicStatus { NotInstalled, Installed, IncompatibleVersion }   // camelCase no IPC

trait VirtualMicDevicePort: Send + Sync {
    fn status(&self) -> VirtualMicStatus;
    fn watch(&self, on_change: Box<dyn Fn(VirtualMicStatus) + Send + Sync>);   // CoreAudio device-list listener
}

trait VirtualMicOutputPort: Send + Sync {
    fn open(&self) -> Result<(), VerbalixError>;          // resolve device por nome/UID, inicia stream contínuo (silêncio)
    fn enqueue(&self, samples_48k: Vec<f32>, channels: u16);  // não-bloqueante; ring buffer bounded
    fn close(&self);                                      // para o stream (silêncio total)
    fn metrics(&self) -> VirtualMicMetrics;              // { buffer_depth, underruns }
}
```

### Ring buffer, overflow e drift (RF05/EC06 — concretizado)
- Capacidade BOUNDED: ~2 s de áudio a 48 kHz (constante nomeada). Nunca cresce sem limite.
- Overflow (segmentos chegam mais rápido que o device consome, ex. burst pós-recuperação de rede): **descartar o
  MAIS ANTIGO** ainda não reproduzido (mantém a fala mais fresca; incrementa contador de drop) — fail-forward.
- Underrun (ring esvazia): device emite SILÊNCIO e incrementa `underruns`; nunca estala nem repete buffer.
- Dois clocks (chegada WAV vs. callback do device): sem correção de drift ativa na v1 (aceitável frase-a-frase
  com pausas de silêncio entre enunciados que naturalmente re-sincronizam o ring). Drift acumulado é mitigado
  pelo fato de o ring drenar até vazio entre frases → silêncio → reset natural. Documentar como limitação v1.

### Contrato play() bloqueante vs enqueue não-bloqueante (verificado contra live_worker.rs)
No M2, `process_queue_events` (live_worker.rs) chama `playback.play(wav)` DENTRO da task tokio que segura o
permit do semáforo (MAX_IN_FLIGHT=2); `MacAudioPlayback::play` BLOQUEIA até o áudio terminar. A ordenação é
garantida pelo `LiveQueue` (reorder buffer), NÃO pelo bloqueio. Para o vmic, `enqueue` retorna já e o device
clock pacena a reprodução — **correto por design** para um device contínuo, e a ordem de EMISSÃO ao ring é a
ordem do reorder buffer (preservada). O engenheiro DEVE preservar essa ordem (enqueue na ordem do `LiveQueue`)
e NÃO deve assumir que o não-bloqueio quebra o M2 — mas DEVE rodar os testes de ordering do M2 para confirmar.

### Re-roteamento mid-sessão em perda de device (EC02 — wiring concreto)
O `route: Arc<AtomicBool>` é a única fonte de verdade de "áudio indo pro vmic AGORA". Fluxo:
- No `runtime.rs`, o `VirtualMicDevicePort::watch` é registrado UMA vez no startup com uma callback que detém
  `Arc` clones de `route` e do `virtual_mic`. Se o device SUMIR da lista enquanto `route==true`: a callback faz
  `route.store(false)` + `virtual_mic.close()` + emite `virtual-mic-status` (fail-open p/ alto-falante). A thread
  da callback do CoreAudio NUNCA toca AppKit nem bloqueia.
- `PlaybackRouter::play` lê `route` a cada chamada → a próxima frase já sai no alto-falante. Sessão nunca trava.
- Três estados (setting `output_to_virtual_mic` / `device.status()` / `route`) são derivados deterministicamente:
  `route` só vira true em `enter_live` quando setting==true E status==Installed E `open()` ok. Consolidar a
  decisão em UMA função (`resolve_route(...)`) evita divergência.

### Detecção de versão do driver (EC01 — mecanismo concreto)
`status()` lê `CFBundleVersion` do Info.plist do bundle instalado em
`/Library/Audio/Plug-Ins/HAL/VerbalixMicrophone.driver/Contents/Info.plist` e compara com uma constante
`EXPECTED_DRIVER_VERSION`. Ausente → `NotInstalled`; presente e igual → `Installed`; presente e ≠ →
`IncompatibleVersion`. `IncompatibleVersion` em `enter_live` com routing = fail-open p/ alto-falante (igual a
`NotInstalled`) + UI orienta reinstalar.

### Roteamento de playback (integração M2 — minimamente invasiva)
`PlaybackRouter` (application, `playback_router.rs`) implementa `AudioPreviewPort` e é passado ao
`LiveInterpretationCoordinator` no lugar do `MacAudioPlayback` cru. Ele detém:
- `speaker: Arc<dyn AudioPreviewPort>` (o `MacAudioPlayback` atual — monitor),
- `virtual_mic: Arc<dyn VirtualMicOutputPort>`,
- `route: Arc<AtomicBool>` (roteando p/ vmic?).

`play(wav)`: se `route` ativo → decodifica WAV (reusar `decode`/helpers de `audio_wav.rs`) → reamostra p/ 48 kHz
→ `virtual_mic.enqueue(...)` (retorna já; o stream contínuo do device consome); senão → `speaker.play(wav)`.
`stop()`: `speaker.stop()` (o vmic continua emitindo silêncio até `close`).

O `LiveInterpretationCoordinator` ganha `virtual_mic: Arc<dyn VirtualMicOutputPort>` + `route: Arc<AtomicBool>`:
- `enter_live`: lê `output_to_virtual_mic` (via closure/param) E `device.status()==Installed`. Se ambos →
  `virtual_mic.open()`; sucesso ⇒ `route=true`; falha ⇒ `route=false` + fallback alto-falante + evento sanitizado
  (NUNCA propaga erro que derrube a sessão — RNF04/EC07).
- `leave_live`: `route=false` + `virtual_mic.close()` (EC08).

Isso mantém `live_worker.rs` e `live_queue.rs` **intactos** (continuam vendo `Arc<dyn AudioPreviewPort>`), reduzindo
regressão do M2. Como `enter_live` já está em 340 linhas, extrair a decisão de roteamento p/ um helper
(`fn resolve_playback_route(...)`) ou p/ `playback_router.rs` para respeitar o gate (R5).

### Anti-feedback (EC03/CA04)
O `MacAudioCapture` (M1/M2) enumera devices de input. Adicionar filtro: **nunca** selecionar o device cujo UID ==
`com.verbalix.virtualmic:0` como fonte de captura. Se for o default input, cair para o próximo device físico e/ou
sinalizar erro claro ("selecione um microfone físico").

### Frontend
Seção "Microfone virtual" no painel Interpretação:
- Estado do driver: Não instalado (CTA: "Como instalar" → mostra o caminho do script `scripts/install-virtual-mic.sh`;
  o app NÃO roda sudo) / Instalado / Incompatível (CTA reinstalar).
- Toggle "Enviar áudio traduzido para o microfone virtual" (`output_to_virtual_mic`).
- Indicador de destino atual do áudio: "Alto-falante (monitor)" vs "Verbalix Microphone".
- Escuta `onVirtualMicStatusChange` para refletir plug/unplug em tempo real.

### Diagnostics (RF08)
`diagnostics::virtual_mic(status, buffer_depth, underruns)` — só números/enum, sem PCM/UID textual sensível.

## 📝 TASKS

### Fase 1 — Driver vendorizado + scripts (independente do Rust)
- [x] T1.1: [MEDIUM] Vendorizar fork do BlackHole em `virtual-mic-driver/` com LICENSE + README; rebrand (device
  name, bundle id `com.verbalix.virtualmic`, UID estável, 48 kHz 2ch).
- [x] T1.2: [LOW] `scripts/build-virtual-mic.sh` (xcodebuild → `.driver` em `build/`), idempotente, falha loud.
- [x] T1.3: [LOW] `scripts/install-virtual-mic.sh` e `uninstall-virtual-mic.sh` — pedem sudo, copiam/removem em
  `/Library/Audio/Plug-Ins/HAL`, `sudo killall coreaudiod`; nunca embutem senha.

### Fase 2 — Ports + domain + settings + erros
- [x] T2.1: [LOW] `domain/settings.rs`: `output_to_virtual_mic: bool` (`#[serde(default)]`) + Default + preservar em `apply_remote`.
- [x] T2.2: [LOW] `domain/error.rs`: variantes sanitizadas (`VirtualMicUnavailable`).
- [x] T2.3: [LOW] `application/ports.rs`: `VirtualMicDevicePort` + `VirtualMicOutputPort` + `VirtualMicStatus`/`VirtualMicMetrics` (camelCase).

### Fase 3 — Adapter macOS + stub
- [x] T3.0: [MEDIUM] SPIKE de resolução de device: confirmar `coreaudio-sys` `kAudioHardwarePropertyDeviceForUID`
  (UID→AudioObjectID) + match por NOME em `cpal::host.output_devices()`; congelar a assinatura dos ports (T2.3)
  só após o spike. Sem device real, provar a resolução com um device qualquer existente.
- [x] T3.1: [MEDIUM] `platform/virtual_mic.rs`: `MacVirtualMicDevice` (status por Info.plist `CFBundleVersion` +
  property listener de device-list `AudioObjectAddPropertyListener`) atrás de `cfg(macos)`. Testes NÃO exigem
  driver real instalado (lógica de comparação de versão isolada/pura).
- [x] T3.2: [HIGH] `platform/virtual_mic_output.rs`: `MacVirtualMicOutput` (resolve device por nome, thread dedicada
  dona do `cpal::Stream` de output — molde de `MacAudioPlayback`; `RingBuffer` puro+testável bounded ~2 s 48 kHz,
  overflow=drop oldest, silêncio quando vazio, métricas buffer_depth/underruns). Construção LAZY (só em `open()`).
- [x] T3.3: [LOW] Stubs não-macOS em `platform/mod.rs` (status `NotInstalled`, `watch` no-op que nunca dispara,
  `open`→Err, enqueue/close no-ops); re-export de `MacVirtualMicDevice`/`MacVirtualMicOutput` atrás de cfg(macos).
- [x] T3.4: [MEDIUM] `platform/audio_capture.rs`: exclusão ampliada para `starts_with(VERBALIX_MIC_DEVICE_NAME)` em
  ambos os pontos do `resolve_physical_input_device` — cobre "Verbalix Microphone" e "Verbalix Microphone Mirror".

### Fase 4 — Roteamento + integração M2
- [x] T4.0: [LOW] PRÉ-REQUISITO de gate: extrair a lógica de `enter_live`/`leave_live` de `live_interpretation.rs`
  (já em 340 linhas > gate ~300) para helpers/módulo ANTES de adicionar routing, senão o gate falha na 1ª tentativa (R5).
- [x] T4.1: [MEDIUM] `application/playback_router.rs`: `PlaybackRouter` (impl `AudioPreviewPort`, PURO/testável;
  decode+resample→48k → enqueue vs speaker por `Arc<AtomicBool>`). Reuso DRY: promover `decode_wav_f32` de
  `audio_playback.rs` para `pub` em `audio_wav.rs`; generalizar `resample_to_16k` → `resample(samples, src, target, ch)`
  com `resample_to_16k` virando wrapper fino (não regride o M2).
- [x] T4.2: [MEDIUM] `live_interpretation.rs`: injetar `virtual_mic` + `route`; enter/leave_live abrem/fecham vmic e
  resolvem roteamento via `resolve_route(...)` única (fail-open p/ speaker; EC02/EC07/EC08). Preservar ordem do
  `LiveQueue` (enqueue não-bloqueante). Rodar testes de ordering do M2 (não regride).

### Fase 5 — Commands + wiring + eventos + diagnostics
- [x] T5.1: [LOW] `commands_virtual_mic.rs`: `virtual_mic_status`; emitter do evento `virtual-mic-status`.
- [x] T5.2: [MEDIUM] `runtime.rs`: construir device/output/router; wiring no `AppRuntime`/`build_live_coordinator`. (parcial: virtual_mic_status command fica pro próximo round)
- [x] T5.3: [LOW] `lib.rs`: registrar comandos + listener de device-list; manter `lib.rs` ≤301 (R6).
- [x] T5.4: [LOW] `diagnostics.rs`: `virtual_mic(status, buffer_depth, underruns)` sanitizado.

### Fase 6 — Frontend
- [x] T6.1: [LOW] `types.ts` + `native.ts`: `VirtualMicStatus`, `virtualMicStatus()`, `onVirtualMicStatusChange()`.
- [x] T6.2: [MEDIUM] Seção "Microfone virtual" no painel Interpretação: status + CTA de instalação (aponta script,
  não roda sudo) + toggle `output_to_virtual_mic` + indicador de destino do áudio; estilos em `panels.css`.

### Fase 7 — Testes (test-engineer) + QA
- [x] T7.1: Rust — `playback_router` (roteamento, fail-open), settings default/preservação, stub, filtro de captura.
- [x] T7.2: Rust — `live_interpretation` enter/leave com vmic (mock ports): routing on/off, open falha → fallback, close no leave.
  Auditoria (round test-engineer): adicionados `resolve_route_true_when_setting_on_and_open_succeeds` (route ON +
  `open()` sucesso → `route==true`) e `leave_live_zeroes_route_after_successful_routing` (`leave_live` zera `route`
  e chama `virtual_mic.close()` partindo de um roteamento ativo) em `live_interpretation_tests.rs`. Adicionado
  `legacy_settings_json_without_output_to_virtual_mic_defaults_to_false` (EC12) em `domain/settings.rs`. Adicionado
  `virtual_mic_metadata_*` (sanitização) em `diagnostics_tests.rs` (extraído de `diagnostics.rs` para respeitar o
  gate de ~300 linhas após a adição).
- [x] T7.3: Vitest — `native.test.ts` cobertura de `virtualMicStatus`/listener; `types.ts` 100%.
- [x] T7.4: Vitest — painel: estados not-installed/installed/incompatible, toggle, indicador de destino.
- [x] T7.5: Playwright — sequência de comandos da seção do vmic (status + toggle).
- [ ] T7.6: QA (@qa-reviewer com análise dual) → verdict.

## Gates antes do handoff (DENTRO do worktree)
`npm test`, `npm run test:coverage`, `npm run test:e2e`, `npm run build`, `cargo test`,
`cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`,
`deno test supabase/functions/` (regressão), `npm run tauri -- build --debug --bundles app`,
e **build do driver** (`scripts/build-virtual-mic.sh` gerando o `.driver`).

## Gates manuais (listar no doc, NÃO alegar)
- Instalar o driver com sudo e ver "Verbalix Microphone" nas Preferências de Som.
- Selecionar "Verbalix Microphone" como microfone no Zoom/Meet (Chrome/Safari)/Slack e o interlocutor ouvir a tradução.
- sleep/wake (coreaudiod reinicia) → device re-detectado.
- Apps que cacheiam a lista de dispositivos (reabrir o app cliente após instalar).
- Crash/leave → silêncio no device.
- Se `xcodebuild`/Xcode não existir no ambiente de CI, o build do `.driver` vira gate manual.

## Pré-requisitos de distribuição (FORA do escopo — documentar)
Conta Apple Developer; Developer ID (app + driver + installer); notarização; installer `.pkg` privilegiado
assinado com uninstall/repair. Sem isso o driver não carrega em máquinas de terceiros (só local).

## Análise Dual

### 🟢 Oportunidades incorporadas (downsideup)
- `PlaybackRouter` como decorator de `AudioPreviewPort` mantém `live_worker.rs`/`live_queue.rs` INTACTOS (grep
  confirma: só o coordinator referencia o trait) — vitória de design validada.
- Molde thread-dedicada de `MacAudioPlayback` (audio_playback.rs:16-134) reusado quase mecanicamente no vmic output.
- Reuso DRY concreto (movido p/ T4.1): promover `decode_wav_f32`→`audio_wav.rs` pub; generalizar `resample_to_16k`.
- Filtro anti-feedback são só 2 call-sites (audio_capture.rs:62,180) — cirúrgico.
- `VirtualMicMetrics { buffer_depth, underruns }` já é payload de diagnostics (sanitizado por construção).
- Patterns copy-and-rename: setting não-sync (`voice_profile_id`), evento publish/listen (`live-state`), stub cfg.
- Paralelização: Fase 1 (driver) ‖ Fase 2 (ports/domain) desde o início; T3.1/T3.4 disjuntos; T4.1 (router puro)
  pode adiantar em paralelo à Fase 3; T6.1 (types/native) adianta com o contrato congelado. Fase 4→5 acopladas (serial).

### 🔴 Riscos críticos mitigados (upsidedown)
- **cpal não abre por UID** → decisão: match por NOME (controlamos o nome no fork) + UID via coreaudio-sys só p/
  status/listener. Spike T3.0 congela a assinatura dos ports antes da Fase 2 fechar. Limitação (rename) documentada.
- **T3.4 não é filtro de enumeração** — é rewrite de resolução de device (`default_input_device()` nas 2 linhas),
  rescopado p/ MEDIUM com fallback + erro sanitizado.
- **GPL push-to-remote** — esclarecido: push com LICENSE preservada CUMPRE a GPL; risco real é mistura de licenças →
  driver é artefato GPL-3 separado, LICENSE própria no subtree, app não relicenciado.
- **Ring buffer/overflow/drift** — concretizado: ~2 s bounded, drop-oldest no overflow, silêncio no underrun, sem
  correção de drift ativa na v1 (re-sync natural pelo silêncio entre frases). Contrato play() bloqueante vs enqueue
  não-bloqueante verificado contra live_worker (ordem garantida pelo LiveQueue, não pelo bloqueio).
- **live_interpretation.rs já > 300 (340)** — T4.0 extração é PRÉ-REQUISITO, não caveat.
- **EC02 mid-sessão** — wiring concreto: watch() no startup detém Arc de `route`+`virtual_mic`; device some →
  route=false + close + evento (thread CoreAudio nunca toca AppKit).
- **Versão do driver (EC01)** — mecanismo concreto: `CFBundleVersion` do Info.plist instalado vs constante.
- **Construção lazy do vmic** (só em `open()`); listener registrado no startup (barato). Sem vazar property listener.
- **T3.2 é HIGH** (não MEDIUM) — rescopado. **T1.1 rebrand** pode ser MEDIUM-HIGH (nome/UID espalhados em .c/.h/Info.plist).
- Testes Rust do vmic NÃO exigem driver real instalado (lógica pura/mock) — igual à constraint do driver build.

### Decisões de escopo mantidas
- Playback é OU alto-falante OU vmic (spec do usuário), não ambos simultâneos. Alternativa "tocar em ambos (monitor
  sempre)" documentada como possível evolução M4, não adotada agora.
- Driver build É gate automático NESTE ambiente (Xcode 26.6 presente); documentar como manual em CI sem Xcode.
- Débito técnico a registrar no doc: manutenção do fork vs upstream BlackHole a cada major do macOS; fragilidade do
  match por nome; `sudo killall coreaudiod` derruba TODO o áudio do sistema (avisar no prompt do script/README).
