# virtual-mic-driver — Verbalix Microphone HAL Driver

## Origem

Fork rebrandeado do [BlackHole](https://github.com/ExistentialAudio/BlackHole) (Existential Audio Inc.),
um AudioServerPlugIn HAL para macOS que cria um dispositivo de áudio virtual loopback.

## Licença

Este subtree é distribuído sob a **GPL-3** — veja `LICENSE`.

A GPL-3 governa **exclusivamente** este diretório (`virtual-mic-driver/`).
O driver é um artefato separado, carregado pelo `coreaudiod` como processo independente,
e **nunca** é linkado no binário Tauri/Rust do Verbalix. Portanto, a GPL-3 deste subtree
NÃO relicencia o aplicativo Verbalix (que permanece sob sua própria licença).

Ao distribuir este driver (junto com o app ou de forma independente), a GPL-3 exige que
o fonte esteja disponível — o que já é satisfeito pela presença deste diretório no repositório
com a LICENSE preservada.

## O que foi rebrandeado

Diferenças em relação ao BlackHole upstream (branch master):

| Item | BlackHole (upstream) | Verbalix Microphone (fork) |
|------|---------------------|---------------------------|
| `kDriver_Name` | `"BlackHole"` | `"VerbalixMicrophone"` |
| `kPlugIn_BundleID` | `"audio.existential.BlackHole2ch"` | `"com.verbalix.virtualmic"` |
| `kManufacturer_Name` | `"Existential Audio Inc."` | `"Verbalix"` |
| `kDevice_Name` | `"BlackHole 2ch"` (dinâmico) | `"Verbalix Microphone"` (fixo) |
| `kDevice_UID` | `"BlackHole2ch_UID"` (dinâmico) | `"com.verbalix.virtualmic:0"` (fixo) |
| `kBox_UID` | `"BlackHole2ch_UID"` (dinâmico) | `"com.verbalix.virtualmic:0"` (fixo) |
| `kHas_Driver_Name_Format` | `true` (UID/nome com `%ich`) | `false` (strings fixas) |
| `CFBundleVersion` (Info.plist) | `"596"` | `"1.0"` |
| `MARKETING_VERSION` (xcodeproj) | `"0.7.1"` | `"1.0"` |
| `PRODUCT_BUNDLE_IDENTIFIER` | `audio.existential.BlackHole` | `com.verbalix.virtualmic` |
| Target/bundle name | `BlackHole.driver` | `VerbalixMicrophone.driver` |
| Factory function (plist + .c) | `BlackHole_Create` | `VerbalixMicrophone_Create` |
| Canais (`kNumber_Of_Channels`) | 2 (padrão) | 2 (sem alteração) |
| Sample rate padrão | 48000 Hz | 48000 Hz (sem alteração) |

## Bundle instalado

O driver compilado deve ser instalado em:

```
/Library/Audio/Plug-Ins/HAL/VerbalixMicrophone.driver
```

O `CFBundleVersion = "1.0"` deve corresponder à constante
`VERBALIX_DRIVER_EXPECTED_VERSION = "1.0"` em `src-tauri/src/platform/virtual_mic_constants.rs`.

## Como compilar

```bash
bash scripts/build-virtual-mic.sh
```

Requer Xcode com suporte a macOS SDK. O `.driver` gerado fica em `virtual-mic-driver/build/`.

## Como instalar (requer sudo)

```bash
bash scripts/install-virtual-mic.sh
```

O script pede a senha do usuário interativamente e **reinicia o `coreaudiod`**,
o que interrompe brevemente todo o áudio do sistema.

## Como desinstalar

```bash
bash scripts/uninstall-virtual-mic.sh
```

## Aviso importante

`killall coreaudiod` reinicia todo o subsistema de áudio do macOS.
Chamadas em andamento (VoIP, streaming) perdem o áudio momentaneamente.
Os scripts de install/uninstall exibem este aviso e solicitam confirmação antes de prosseguir.

## Artefato separado — sem contaminação de copyleft

O driver é carregado pelo `coreaudiod` como plugin HAL, em processo separado do Verbalix.
Não há linkagem estática ou dinâmica entre o driver GPL e o binário do app.
O copyleft da GPL-3 não se propaga para o aplicativo Verbalix.
