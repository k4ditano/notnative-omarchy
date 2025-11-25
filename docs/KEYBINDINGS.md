# Atajos de Teclado (Keybindings)

Esta guía detalla todos los atajos de teclado disponibles en NotNative, organizados por contexto y modo.

---

## 🚀 Primeros Pasos

### Configurar Quick Notes (Ventana Flotante Global)

Para poder abrir Quick Notes desde cualquier aplicación (incluso juegos fullscreen), necesitas configurar un keybinding en tu gestor de ventanas:

#### Paso 1: Agregar keybindings

**Para Hyprland** (`~/.config/hypr/hyprland.conf`):
```conf
# Quick Notes
bind = SUPER, N, exec, echo 'quicknote' > /tmp/notnative.control
bind = SUPER SHIFT, N, exec, echo 'quicknote-new' > /tmp/notnative.control

# Toggle ventana principal de NotNative
bind = SUPER, M, exec, echo 'toggle' > /tmp/notnative.control
```

**Para i3/Sway** (`~/.config/i3/config` o `~/.config/sway/config`):
```conf
bindsym $mod+n exec echo 'quicknote' > /tmp/notnative.control
bindsym $mod+Shift+n exec echo 'quicknote-new' > /tmp/notnative.control
bindsym $mod+m exec echo 'toggle' > /tmp/notnative.control
```

#### Paso 2: Reglas de ventana (para que Quick Notes sea flotante y siempre visible)

**Para Hyprland**:
```conf
windowrulev2 = float, class:^(com.notnative.app)$, title:^(Quick Note)$
windowrulev2 = pin, class:^(com.notnative.app)$, title:^(Quick Note)$
windowrulev2 = size 450 400, class:^(com.notnative.app)$, title:^(Quick Note)$
windowrulev2 = move 100%-470 50, class:^(com.notnative.app)$, title:^(Quick Note)$
```

**Para i3/Sway**:
```conf
for_window [title="Quick Note"] floating enable, sticky enable
```

#### Paso 3: Recargar configuración
```bash
# Hyprland
hyprctl reload

# i3
i3-msg reload

# Sway
swaymsg reload
```

---

## 🌍 Globales
Estos atajos funcionan en casi cualquier parte de la aplicación.

| Atajo | Acción |
|-------|--------|
| `Ctrl + F` | Abrir búsqueda global flotante (cierra sidebar) |
| `Alt + F` | Abrir búsqueda dentro de la nota actual |
| `Ctrl + Shift + A` | Entrar al modo Chat AI desde cualquier lugar |

### 🪟 Control Global del Sistema (desde cualquier app)

Estos comandos funcionan **incluso cuando NotNative está en segundo plano** o minimizado.

| Comando | Acción |
|---------|--------|
| `echo 'quicknote' > /tmp/notnative.control` | Abrir/cerrar Quick Notes |
| `echo 'quicknote-new' > /tmp/notnative.control` | Crear nueva Quick Note |
| `echo 'show' > /tmp/notnative.control` | Mostrar ventana principal |
| `echo 'hide' > /tmp/notnative.control` | Ocultar a bandeja |
| `echo 'toggle' > /tmp/notnative.control` | Alternar visibilidad |

---

## 📝 Quick Notes (Ventana Flotante)
Notas rápidas accesibles en cualquier momento, incluso sobre juegos/apps fullscreen.

| Atajo | Acción |
|-------|--------|
| `Esc` | Volver a lista / Cerrar ventana |
| `Ctrl + S` | Guardar nota |
| `+` (botón) | Crear nueva quick note |
| `←` (botón) | Volver a la lista de notas |

**Ubicación de las notas:** `~/.local/share/notnative/notes/quick-notes/`

---

## 📝 Editor - Modo Normal (Estilo Vim)
Navegación y comandos rápidos sin editar texto.

### Navegación
| Tecla | Acción |
|-------|--------|
| `h` / `←` | Mover cursor a la izquierda |
| `j` / `↓` | Mover cursor abajo |
| `k` / `↑` | Mover cursor arriba |
| `l` / `→` | Mover cursor a la derecha |
| `0` | Ir al inicio de la línea |
| `$` | Ir al final de la línea |
| `gg` | Ir al inicio del documento |
| `G` | Ir al final del documento |

### Edición y Modos
| Tecla | Acción |
|-------|--------|
| `i` | Entrar en **Modo Insertar** |
| `a` | Entrar en **Modo Chat AI** |
| `v` | Entrar en **Modo Visual** |
| `:` | Entrar en **Modo Comando** |
| `n` | Crear nueva nota |
| `x` | Borrar carácter bajo el cursor |
| `dd` | Borrar línea actual |
| `u` | Deshacer (Undo) |

### Gestión
| Tecla | Acción |
|-------|--------|
| `t` | Abrir barra lateral (Sidebar) |
| `Esc` | Cerrar barra lateral (si está abierta) |
| `Ctrl + s` | Guardar nota |
| `Ctrl + z` | Deshacer |
| `Ctrl + r` | Rehacer |
| `Ctrl + c` | Copiar |
| `Ctrl + x` | Cortar |

---

## ✍️ Editor - Modo Insertar
Escritura y edición de texto estándar.

| Atajo | Acción |
|-------|--------|
| `Esc` | Volver al **Modo Normal** |
| `Ctrl + s` | Guardar nota |
| `Ctrl + c` | Copiar |
| `Ctrl + x` | Cortar |
| `Ctrl + v` | Pegar |
| `Ctrl + z` | Deshacer |
| `Ctrl + r` | Rehacer |
| `Ctrl + t` | Insertar tabla Markdown |
| `Ctrl + Shift + i` | Insertar imagen |
| `Tab` | Insertar tabulación / Autocompletar Tag o Mención (@) |

---

## 🤖 Modo Chat AI
Interacción con el asistente de inteligencia artificial.

| Atajo | Acción |
|-------|--------|
| `Esc` | Salir del Chat (volver a Modo Normal) |
| `i` | Salir del Chat y entrar a **Modo Insertar** |
| `Enter` | Enviar mensaje |
| `Shift + Enter` | Insertar nueva línea en el mensaje |

### Sugerencias (cuando aparecen)
| Tecla | Acción |
|-------|--------|
| `↑` / `↓` | Navegar sugerencias |
| `Tab` / `Enter` | Aceptar sugerencia |
| `Esc` | Cerrar sugerencias |

---

## 📂 Barra Lateral (Sidebar) y Listas
Navegación por la lista de notas.

| Tecla | Acción |
|-------|--------|
| `j` / `↓` | Siguiente nota |
| `k` / `↑` | Nota anterior |
| `Enter` | Abrir nota o carpeta seleccionada |
| `Esc` | Devolver foco al editor |

---

## 🔍 Búsqueda Flotante
Control de la barra de búsqueda global.

| Atajo | Acción |
|-------|--------|
| `Esc` | Cerrar búsqueda |
| `Ctrl` (Izq/Der) | Alternar búsqueda semántica (AI) |
| `↑` / `↓` | Navegar resultados |
| `Enter` | Abrir nota seleccionada |

---

## 💡 Tips

- Las notas se guardan automáticamente en `~/.local/share/notnative/notes/`
- Usa `#tags` para organizar tus notas
- Menciona otras notas con `@nombre_nota` para crear backlinks
- El Chat AI puede leer y modificar tus notas si le das contexto
