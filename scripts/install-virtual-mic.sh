#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DRIVER_SRC="$REPO_ROOT/virtual-mic-driver/build/Products/Release/VerbalixMicrophone.driver"
DRIVER_DEST="/Library/Audio/Plug-Ins/HAL/VerbalixMicrophone.driver"

if [ ! -d "$DRIVER_SRC" ]; then
    echo "ERROR: Driver bundle not found at $DRIVER_SRC" >&2
    echo "Run 'bash scripts/build-virtual-mic.sh' first." >&2
    exit 1
fi

echo ""
echo "=========================================================="
echo "  AVISO: esta operação reinicia o coreaudiod"
echo "=========================================================="
echo ""
echo "  'killall coreaudiod' interrompe TODOS os dispositivos de"
echo "  áudio do sistema por alguns segundos. Chamadas em curso"
echo "  (VoIP, streaming, gravações) perderão o áudio momentaneamente."
echo ""
read -r -p "Deseja continuar? [s/N] " resposta
case "$resposta" in
    [sS][iI][mM]|[sS])
        ;;
    *)
        echo "Instalação cancelada."
        exit 0
        ;;
esac

echo ""
echo "Copiando VerbalixMicrophone.driver para $DRIVER_DEST ..."
sudo cp -R "$DRIVER_SRC" "$DRIVER_DEST"

echo "Reiniciando coreaudiod..."
sudo killall coreaudiod

echo ""
echo "Driver instalado com sucesso."
echo "O dispositivo 'Verbalix Microphone' deve aparecer nas preferências de som em alguns segundos."
