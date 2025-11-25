# Quick Notes - Ventana Flotante de Notas Rápidas

Las Quick Notes son una funcionalidad de NotNative que permite acceder a notas rápidas desde cualquier lugar, incluso mientras juegas o usas otras aplicaciones a pantalla completa.

## 🚀 Características

- **Always-on-top**: La ventana se mantiene visible sobre otras aplicaciones
- **Acceso global**: Funciona incluso con NotNative minimizado en la bandeja
- **Auto-guardado**: Las notas se guardan automáticamente cada 5 segundos
- **Múltiples notas**: Crea y gestiona varias quick notes
- **Diseño minimalista**: Interfaz limpia que no distrae

## 📁 Ubicación

Las quick notes se guardan en:
```
~/.local/share/notnative/notes/quick-notes/
```

Cada nota es un archivo `.md` independiente, lo que facilita su backup y sincronización.

## 🎮 Configuración para Hyprland

Agrega estas líneas a tu `~/.config/hypr/hyprland.conf`:

```conf
# ===== NotNative Quick Notes =====

# Keybindings
bind = SUPER, N, exec, echo 'quicknote' > /tmp/notnative.control
bind = SUPER SHIFT, N, exec, echo 'quicknote-new' > /tmp/notnative.control

# Reglas de ventana para Quick Notes (flotante, siempre visible, esquina superior derecha)
windowrulev2 = float, class:^(com.notnative.app)$, title:^(Quick Note)$
windowrulev2 = pin, class:^(com.notnative.app)$, title:^(Quick Note)$
windowrulev2 = size 450 400, class:^(com.notnative.app)$, title:^(Quick Note)$
windowrulev2 = move 100%-470 50, class:^(com.notnative.app)$, title:^(Quick Note)$

# Opcional: animación suave
windowrulev2 = animation slide, class:^(com.notnative.app)$, title:^(Quick Note)$

# Opcional: sin sombra para look más limpio
windowrulev2 = noshadow, class:^(com.notnative.app)$, title:^(Quick Note)$
```

### Reglas alternativas (posición central)

```conf
# Ventana centrada
windowrulev2 = center, class:^(com.notnative.app)$, title:^(Quick Note)$
windowrulev2 = size 500 450, class:^(com.notnative.app)$, title:^(Quick Note)$
```

## 🪟 Configuración para i3/Sway

Agrega a tu `~/.config/i3/config` o `~/.config/sway/config`:

```conf
# ===== NotNative Quick Notes =====

# Keybindings
bindsym $mod+n exec echo 'quicknote' > /tmp/notnative.control
bindsym $mod+Shift+n exec echo 'quicknote-new' > /tmp/notnative.control

# Regla para que sea flotante y sticky (visible en todos los workspaces)
for_window [title="Quick Note"] floating enable, sticky enable, resize set 450 400, move position center
```

## 🖥️ Comandos de Control

Puedes controlar Quick Notes desde la terminal o scripts:

| Comando | Acción |
|---------|--------|
| `echo 'quicknote' > /tmp/notnative.control` | Toggle ventana (abrir/cerrar) |
| `echo 'quicknote-new' > /tmp/notnative.control` | Crear nueva quick note |

## ⌨️ Atajos dentro de Quick Notes

| Atajo | Acción |
|-------|--------|
| `Esc` | Volver a lista / Cerrar ventana |
| `Ctrl + S` | Guardar nota manualmente |
| Click en `+` | Crear nueva quick note |
| Click en `←` | Volver a la lista de notas |
| Click en `📌` | Toggle pin (visual) |

## 💡 Casos de Uso

### Durante gaming
Mantén una nota con:
- Controles del juego
- Tips y estrategias
- Lista de misiones pendientes

### Mientras trabajas
- Notas temporales de reuniones
- Snippets de código rápidos
- TODOs urgentes

### Para estudiar
- Fórmulas importantes
- Definiciones clave
- Preguntas para revisar

## 🔧 Solución de Problemas

### La ventana no aparece flotante
Verifica que las reglas de ventana estén configuradas correctamente y que el título sea exactamente "Quick Note".

### El keybinding no funciona
1. Asegúrate de que NotNative esté corriendo (aunque sea minimizado)
2. Verifica que el archivo `/tmp/notnative.control` se puede crear
3. Comprueba los logs: `journalctl -f -t notnative`

### La ventana no se mantiene arriba en Hyprland
Asegúrate de tener la regla `pin`:
```conf
windowrulev2 = pin, class:^(com.notnative.app)$, title:^(Quick Note)$
```

## 📝 Notas Técnicas

- Las quick notes usan el mismo formato Markdown que las notas normales
- Se almacenan en una subcarpeta especial (`quick-notes/`)
- El nombre de archivo incluye timestamp para ordenación cronológica
- Compatible con el sistema de tags y menciones de NotNative
