# Sistema de Control en Segundo Plano

NotNative ahora puede ejecutarse en segundo plano y ser controlado desde scripts externos, waybar, o hyprland.

## Funcionalidades Implementadas

### 1. Single Instance Detection
- Solo permite una instancia de la app corriendo al mismo tiempo
- Lock file en `/tmp/notnative.lock` con el PID del proceso
- Validación de PID antes de rechazar (detecta procesos muertos)
- Limpieza automática al salir

```bash
# Si intentas abrir otra instancia:
$ notnative-app
❌ NotNative ya está corriendo (PID: 123456)
💡 Si crees que esto es un error, elimina: /tmp/notnative.lock
```

### 2. Window Hide/Show
- Al cerrar la ventana (X o Ctrl+Q), la app se minimiza a segundo plano
- La app sigue corriendo (MCP Server activo, música reproduciéndose)
- Ventana se puede mostrar/ocultar bajo demanda

### 3. Sistema de Control por Archivos
Como GTK4/Wayland no soporta system tray tradicional, usamos un sistema de control basado en archivos:

```bash
# Script helper incluido
./notnative-control.sh show    # Mostrar ventana
./notnative-control.sh hide    # Ocultar ventana  
./notnative-control.sh toggle  # Alternar
./notnative-control.sh quit    # Cerrar completamente
```

O directamente:
```bash
echo "show" > /tmp/notnative.control
echo "hide" > /tmp/notnative.control
echo "quit" > /tmp/notnative.control
```

La app monitorea `/tmp/notnative.control` cada 500ms y ejecuta comandos automáticamente.

### 4. Integración con Waybar

Agregar a tu configuración de waybar (`~/.config/waybar/config`):

```json
{
  "modules-right": ["custom/notnative", "..."],
  
  "custom/notnative": {
    "format": "📝 NotNative",
    "on-click": "/ruta/a/notnative-control.sh toggle",
    "on-click-right": "/ruta/a/notnative-control.sh quit",
    "tooltip": true,
    "tooltip-format": "Click: Mostrar/Ocultar\nClick derecho: Salir"
  }
}
```

### 5. Integración con Hyprland

Agregar atajos de teclado en `~/.config/hypr/hyprland.conf`:

```conf
# Mostrar/ocultar NotNative
bind = SUPER, N, exec, /ruta/a/notnative-control.sh toggle

# Cerrar NotNative completamente
bind = SUPER_SHIFT, N, exec, /ruta/a/notnative-control.sh quit
```

## Casos de Uso

### MCP Server siempre disponible
```bash
# Iniciar NotNative en segundo plano al login
notnative-app &

# Ocultar ventana si está visible
./notnative-control.sh hide

# Ahora el MCP Server está disponible 24/7 en http://localhost:8788
# Puedes crear notas desde n8n, scripts, etc. sin tener la ventana visible
```

### Workflow con Waybar
1. Click en icono waybar → ventana aparece
2. Trabajas en tus notas
3. Cierras la ventana (X) → se minimiza
4. MCP Server sigue activo
5. Click derecho en waybar → app se cierra completamente

### Control desde Scripts
```bash
#!/bin/bash
# Crear nota desde script externo

# Asegurar que NotNative está corriendo
if [ ! -f /tmp/notnative.lock ]; then
    notnative-app &
    sleep 2
fi

# Crear nota vía MCP
curl -X POST http://localhost:8788/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "tool": "CreateNote",
    "name": "Script Note",
    "content": "Creada desde script"
  }'

# Mostrar ventana para ver la nota
./notnative-control.sh show
```

## Archivos de Control

- `/tmp/notnative.lock` - Lock file con PID (evita múltiples instancias)
- `/tmp/notnative.control` - Comandos para controlar la app (show/hide/quit)
- `/tmp/notnative_mcp_update.signal` - Señal de cambios MCP (auto-refresh)

Todos se limpian automáticamente al cerrar la app.

## Limitaciones

- **No hay icono visual en system tray**: GTK4 + Wayland no soportan libappindicator tradicional
- **Solución alternativa**: Usa waybar custom module o scripts de control
- **System tray real**: Requeriría implementar D-Bus StatusNotifierItem (complejo)

## Ventajas del Sistema Actual

✅ Funciona perfectamente en Wayland/Hyprland  
✅ Integrable con waybar, rofi, cualquier script  
✅ No requiere dependencias extra (D-Bus, etc)  
✅ Simple y confiable  
✅ MCP Server siempre disponible en segundo plano  
