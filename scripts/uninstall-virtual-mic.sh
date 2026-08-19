#!/usr/bin/env bash
set -euo pipefail

DRIVER_DEST="/Library/Audio/Plug-Ins/HAL/VerbalixMicrophone.driver"

if [ ! -d "$DRIVER_DEST" ]; then
    echo "VerbalixMicrophone.driver não está instalado em $DRIVER_DEST"
    echo "Nada a fazer."
    exit 0
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
read -r -p "Deseja continuar com a desinstalação? [s/N] " resposta
case "$resposta" in
    [sS][iI][mM]|[sS])
        ;;
    *)
        echo "Desinstalação cancelada."
        exit 0
        ;;
esac

echo ""
echo "Removendo $DRIVER_DEST ..."
sudo rm -rf "$DRIVER_DEST"

echo "Reiniciando coreaudiod..."
sudo killall coreaudiod

echo ""
echo "Driver removido com sucesso."
echo "O dispositivo 'Verbalix Microphone' não deve mais aparecer nas preferências de som."
