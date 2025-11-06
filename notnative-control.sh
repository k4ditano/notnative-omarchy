#!/bin/bash
# Script de control para NotNative
# Úsalo desde waybar o la terminal para controlar la app en segundo plano

CONTROL_FILE="/tmp/notnative.control"

case "$1" in
    show)
        echo "show" > "$CONTROL_FILE"
        echo "📱 Mostrando NotNative..."
        ;;
    hide)
        echo "hide" > "$CONTROL_FILE"
        echo "📱 Ocultando NotNative..."
        ;;
    toggle)
        echo "toggle" > "$CONTROL_FILE"
        echo "📱 Alternando NotNative..."
        ;;
    quit)
        echo "quit" > "$CONTROL_FILE"
        echo "👋 Cerrando NotNative..."
        ;;
    *)
        echo "Uso: $0 {show|hide|toggle|quit}"
        echo ""
        echo "Ejemplos:"
        echo "  $0 show    - Mostrar la ventana"
        echo "  $0 hide    - Ocultar la ventana"
        echo "  $0 toggle  - Alternar entre mostrar/ocultar"
        echo "  $0 quit    - Cerrar completamente"
        echo ""
        echo "Para usar en waybar, agrega un módulo custom:"
        echo '  "custom/notnative": {'
        echo '    "format": "📝 NotNative",'
        echo '    "on-click": "'"$0"' toggle",'
        echo '    "on-click-right": "'"$0"' quit"'
        echo '  }'
        exit 1
        ;;
esac
