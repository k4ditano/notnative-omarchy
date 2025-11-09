# Referencia Completa de Herramientas MCP - NotNative

Documentación completa de todas las herramientas disponibles en el **NotNative MCP Server** para crear workflows con n8n, Make.com, o cualquier herramienta de automatización.

## 📋 Índice

1. [Conexión al MCP Server](#conexión-al-mcp-server)
2. [Gestión de Notas](#gestión-de-notas)
3. [Búsqueda y Navegación](#búsqueda-y-navegación)
4. [Organización](#organización)
5. [Análisis y Estadísticas](#análisis-y-estadísticas)
6. [Transformaciones de Contenido](#transformaciones-de-contenido)
7. [Control de UI](#control-de-ui)
8. [Exportación e Importación](#exportación-e-importación)
9. [Multimedia](#multimedia)
10. [Automatización](#automatización)
11. [Sistema](#sistema)
12. [Ejemplos de Workflows n8n](#ejemplos-de-workflows-n8n)

---

## Conexión al MCP Server

### Endpoint HTTP
```
http://localhost:8765
```

### Método de Comunicación
- **Protocolo**: HTTP POST
- **Content-Type**: `application/json`
- **Formato**: JSON-RPC 2.0

### Ejemplo de Request
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "create_note",
    "arguments": {
      "name": "Mi Nota",
      "content": "# Título\n\nContenido de la nota"
    }
  }
}
```

### Ejemplo de Response
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "success": true,
    "data": {
      "message": "Nota creada exitosamente",
      "path": "/ruta/a/Mi Nota.md"
    }
  }
}
```

---

## Gestión de Notas

### 📝 create_note
Crea una nueva nota en NotNative.

**Parámetros:**
```json
{
  "name": "string (requerido)",      // Nombre de la nota (con/sin .md)
  "content": "string (requerido)",   // Contenido markdown
  "folder": "string (opcional)"      // Carpeta destino
}
```

**Ejemplo n8n:**
```json
{
  "name": "Reunión 2025-11-08",
  "content": "# Reunión de Equipo\n\n- Tema 1\n- Tema 2",
  "folder": "Meetings"
}
```

---

### 📖 read_note
Lee el contenido completo de una nota.

**Parámetros:**
```json
{
  "name": "string (requerido)"  // Nombre de la nota
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "name": "Mi Nota",
    "content": "# Contenido...",
    "path": "/ruta/completa.md"
  }
}
```

---

### ✏️ update_note
Sobrescribe el contenido de una nota existente.

**Parámetros:**
```json
{
  "name": "string (requerido)",     // Nombre de la nota
  "content": "string (requerido)"   // Nuevo contenido completo
}
```

**⚠️ Importante:** Reemplaza TODO el contenido. Para agregar al final usa `append_to_note`.

---

### ➕ append_to_note
Agrega contenido al final de una nota sin borrar lo existente.

**Parámetros:**
```json
{
  "name": "string (requerido)",     // Nombre de la nota
  "content": "string (requerido)"   // Contenido a agregar
}
```

**Ejemplo n8n - Log de actividades:**
```json
{
  "name": "Daily Log",
  "content": "\n## {{ $now.format('HH:mm') }}\n{{ $json.activity }}"
}
```

---

### 🗑️ delete_note
Elimina permanentemente una nota.

**Parámetros:**
```json
{
  "name": "string (requerido)"  // Nombre de la nota
}
```

**⚠️ Advertencia:** Esta acción no se puede deshacer.

---

### 📋 list_notes
Lista todas las notas o las de una carpeta específica.

**Parámetros:**
```json
{
  "folder": "string (opcional)"  // Carpeta específica
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "notes": [
      {
        "name": "Nota 1",
        "path": "/ruta/Nota 1.md",
        "modified": "2025-11-08T10:30:00Z"
      }
    ],
    "total": 42
  }
}
```

---

### 🔄 rename_note
Renombra una nota existente.

**Parámetros:**
```json
{
  "old_name": "string (requerido)",  // Nombre actual
  "new_name": "string (requerido)"   // Nuevo nombre
}
```

---

### 📄 duplicate_note
Crea una copia de una nota.

**Parámetros:**
```json
{
  "name": "string (requerido)",      // Nota a duplicar
  "new_name": "string (requerido)"   // Nombre de la copia
}
```

---

## Búsqueda y Navegación

### 🔍 search_notes
Búsqueda de texto completo en todas las notas.

**Parámetros:**
```json
{
  "query": "string (requerido)"  // Texto a buscar
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "results": [
      {
        "name": "Nota encontrada",
        "matches": 3,
        "preview": "...contexto del match..."
      }
    ]
  }
}
```

---

### 🏷️ get_notes_with_tag
Obtiene todas las notas con un tag específico.

**Parámetros:**
```json
{
  "tag": "string (requerido)"  // Tag sin #
}
```

**Ejemplo:**
```json
{
  "tag": "importante"  // Busca notas con #importante
}
```

---

### 🔎 fuzzy_search
Búsqueda difusa que tolera errores de escritura.

**Parámetros:**
```json
{
  "query": "string (requerido)",  // Texto aproximado
  "limit": "integer (opcional)"   // Max resultados (default: 10)
}
```

**Ejemplo:**
```json
{
  "query": "reunon equpo",  // Encuentra "reunión equipo"
  "limit": 5
}
```

---

### 🕒 get_recent_notes
Obtiene las notas modificadas recientemente.

**Parámetros:**
```json
{
  "limit": "integer (opcional)"  // Número de notas (default: 10)
}
```

---

### 🏷️ get_all_tags
Lista todos los tags usados en todas las notas.

**Parámetros:**
```json
{}  // Sin parámetros
```

**Response:**
```json
{
  "success": true,
  "data": {
    "tags": [
      {"name": "importante", "count": 15},
      {"name": "trabajo", "count": 23}
    ]
  }
}
```

---

## Organización

### 📁 create_folder
Crea una nueva carpeta.

**Parámetros:**
```json
{
  "name": "string (requerido)",    // Nombre de la carpeta
  "parent": "string (opcional)"    // Carpeta padre
}
```

**Ejemplo - Crear subcarpeta:**
```json
{
  "name": "2025",
  "parent": "Proyectos"
}
```

---

### 📂 list_folders
Lista todas las carpetas.

**Parámetros:**
```json
{}  // Sin parámetros
```

---

### 🚚 move_note
Mueve una nota a otra carpeta.

**Parámetros:**
```json
{
  "name": "string (requerido)",    // Nombre de la nota
  "folder": "string (requerido)"   // Carpeta destino
}
```

---

### 🏷️ add_tag
Agrega un tag a una nota.

**Parámetros:**
```json
{
  "note": "string (requerido)",  // Nombre de la nota
  "tag": "string (requerido)"    // Tag a agregar (sin #)
}
```

---

### 🏷️❌ remove_tag
Elimina un tag de una nota.

**Parámetros:**
```json
{
  "note": "string (requerido)",  // Nombre de la nota
  "tag": "string (requerido)"    // Tag a eliminar (sin #)
}
```

---

### 📦 archive_note
Archiva una nota (mueve a carpeta Archive).

**Parámetros:**
```json
{
  "name": "string (requerido)"  // Nombre de la nota
}
```

---

## Análisis y Estadísticas

### 📊 get_note_stats
Obtiene estadísticas de una nota.

**Parámetros:**
```json
{
  "name": "string (requerido)"  // Nombre de la nota
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "words": 1523,
    "characters": 8945,
    "lines": 142,
    "headings": 12,
    "links": 8,
    "images": 3,
    "code_blocks": 5
  }
}
```

---

### 🔬 analyze_note_structure
Analiza la estructura de una nota.

**Parámetros:**
```json
{
  "name": "string (requerido)"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "headings": [
      {"level": 1, "text": "Título Principal"},
      {"level": 2, "text": "Sección 1"}
    ],
    "lists": 5,
    "code_blocks": [
      {"language": "python", "lines": 10}
    ],
    "links": [...],
    "images": [...]
  }
}
```

---

### 📝 get_word_count
Cuenta palabras, caracteres y líneas.

**Parámetros:**
```json
{
  "name": "string (requerido)"
}
```

---

### 🔗 find_broken_links
Encuentra enlaces rotos en notas.

**Parámetros:**
```json
{
  "note_name": "string (opcional)"  // Si no se especifica, busca en todas
}
```

---

### 🤝 suggest_related_notes
Sugiere notas relacionadas por contenido similar.

**Parámetros:**
```json
{
  "name": "string (requerido)",   // Nota de referencia
  "limit": "integer (opcional)"   // Max sugerencias (default: 5)
}
```

---

### 🕸️ get_note_graph
Obtiene el grafo de relaciones entre notas.

**Parámetros:**
```json
{
  "max_depth": "integer (opcional)"  // Profundidad máxima (default: 2)
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "nodes": [
      {"id": "nota1", "label": "Nota 1", "tags": ["tag1"]}
    ],
    "edges": [
      {"from": "nota1", "to": "nota2", "type": "link"}
    ]
  }
}
```

---

## Transformaciones de Contenido

### 📑 generate_table_of_contents
Genera un índice automático basado en headings.

**Parámetros:**
```json
{
  "name": "string (requerido)",      // Nombre de la nota
  "max_level": "integer (opcional)"  // Nivel máx headings (1-6)
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "toc": "## Índice\n\n- [Título 1](#titulo-1)\n  - [Subtítulo](#subtitulo)"
  }
}
```

---

### 💻 extract_code_blocks
Extrae bloques de código de una nota.

**Parámetros:**
```json
{
  "name": "string (requerido)",      // Nombre de la nota
  "language": "string (opcional)"    // Filtrar por lenguaje
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "blocks": [
      {
        "language": "python",
        "code": "def hello():\n    print('Hello')",
        "line": 42
      }
    ]
  }
}
```

---

### ✨ format_note
Formatea una nota según un estilo.

**Parámetros:**
```json
{
  "name": "string (requerido)",   // Nombre de la nota
  "style": "string (opcional)"    // "compact", "spacious", "clean"
}
```

---

### 🔗 merge_notes
Fusiona múltiples notas en una.

**Parámetros:**
```json
{
  "note_names": ["string"] (requerido),  // Array de notas
  "output_name": "string (requerido)"    // Nombre nota resultante
}
```

**Ejemplo:**
```json
{
  "note_names": ["Parte 1", "Parte 2", "Parte 3"],
  "output_name": "Documento Completo"
}
```

---

### ✂️ split_note
Divide una nota en múltiples partes.

**Parámetros:**
```json
{
  "name": "string (requerido)",     // Nota a dividir
  "split_by": "string (requerido)"  // "heading", "paragraph", "separator"
}
```

---

## Control de UI

### 📖 open_note
Abre una nota en el editor.

**Parámetros:**
```json
{
  "name": "string (requerido)"  // Nombre de la nota
}
```

---

### 🔔 show_notification
Muestra una notificación en NotNative.

**Parámetros:**
```json
{
  "message": "string (requerido)",  // Mensaje
  "level": "string (opcional)"      // "info", "warning", "error", "success"
}
```

**Ejemplo:**
```json
{
  "message": "Workflow completado exitosamente",
  "level": "success"
}
```

---

### 👁️ highlight_note
Resalta una nota en la sidebar.

**Parámetros:**
```json
{
  "name": "string (requerido)"
}
```

---

### 🔲 toggle_sidebar
Muestra/oculta la barra lateral.

**Parámetros:**
```json
{}  // Sin parámetros
```

---

### 🔄 refresh_sidebar
Refresca la lista de notas.

**Parámetros:**
```json
{}  // Sin parámetros
```

---

### 🎮 switch_mode
Cambia el modo del editor.

**Parámetros:**
```json
{
  "mode": "string (requerido)"  // "normal", "insert", "chat"
}
```

---

### 🔍 focus_search
Activa el campo de búsqueda.

**Parámetros:**
```json
{}  // Sin parámetros
```

---

## Exportación e Importación

### 📤 export_note
Exporta una nota a otro formato.

**Parámetros:**
```json
{
  "name": "string (requerido)",        // Nombre de la nota
  "format": "string (requerido)",      // "html", "pdf", "json", "txt"
  "output_path": "string (opcional)"   // Ruta de salida
}
```

---

### 📤📤 export_multiple_notes
Exporta múltiples notas.

**Parámetros:**
```json
{
  "note_names": ["string"] (requerido),  // Array de notas
  "format": "string (requerido)",        // Formato
  "output_dir": "string (opcional)"      // Directorio salida
}
```

---

### 💾 backup_notes
Crea un backup de todas las notas.

**Parámetros:**
```json
{
  "output_path": "string (opcional)"  // Ruta del backup
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "backup_file": "/backups/notnative-2025-11-08.zip",
    "notes_count": 156,
    "size_mb": 12.5
  }
}
```

---

### 🌐 import_from_url
Importa contenido desde una URL.

**Parámetros:**
```json
{
  "url": "string (requerido)",       // URL del contenido
  "note_name": "string (opcional)"   // Nombre para la nota
}
```

**Ejemplo:**
```json
{
  "url": "https://example.com/article.html",
  "note_name": "Artículo Importado"
}
```

---

## Multimedia

### 🖼️ insert_image
Inserta una imagen en una nota.

**Parámetros:**
```json
{
  "note": "string (requerido)",        // Nombre de la nota
  "image_path": "string (requerido)",  // Ruta de la imagen
  "alt_text": "string (opcional)"      // Texto alternativo
}
```

---

### 📺 insert_youtube_video
Inserta un video de YouTube.

**Parámetros:**
```json
{
  "note": "string (requerido)",       // Nombre de la nota
  "video_url": "string (requerido)"   // URL del video
}
```

---

### 📝 extract_youtube_transcript
Extrae la transcripción de un video de YouTube.

**Parámetros:**
```json
{
  "video_url": "string (requerido)"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "transcript": "Texto completo de la transcripción...",
    "language": "es",
    "duration": "15:30"
  }
}
```

---

## Automatización

### 📅 create_daily_note
Crea una nota diaria automática.

**Parámetros:**
```json
{
  "template": "string (opcional)"  // Plantilla de contenido
}
```

**Ejemplo:**
```json
{
  "template": "# Daily Log {{ date }}\n\n## Tasks\n- [ ] \n\n## Notes\n"
}
```

**Nota creada:** `2025-11-08.md`

---

### 🔁 batch_rename
Renombra múltiples notas usando patrón.

**Parámetros:**
```json
{
  "pattern": "string (requerido)",     // Patrón regex a buscar
  "replacement": "string (requerido)", // Texto de reemplazo
  "folder": "string (opcional)"        // Carpeta específica
}
```

---

### 🔄 find_and_replace
Busca y reemplaza texto en notas.

**Parámetros:**
```json
{
  "find": "string (requerido)",              // Texto a buscar
  "replace": "string (requerido)",           // Reemplazo
  "note_names": ["string"] (opcional)        // Notas específicas
}
```

**Ejemplo - Actualizar enlaces:**
```json
{
  "find": "http://oldsite.com",
  "replace": "https://newsite.com",
  "note_names": ["Doc1", "Doc2"]
}
```

---

## Sistema

### ℹ️ get_app_info
Obtiene información de NotNative.

**Parámetros:**
```json
{}  // Sin parámetros
```

**Response:**
```json
{
  "success": true,
  "data": {
    "version": "0.1.5-release",
    "workspace": "/home/user/Notes",
    "notes_count": 156,
    "folders_count": 12,
    "tags_count": 45
  }
}
```

---

### 📁 get_workspace_path
Obtiene la ruta del workspace.

**Parámetros:**
```json
{}  // Sin parámetros
```

**Response:**
```json
{
  "success": true,
  "data": {
    "path": "/home/user/NotNative"
  }
}
```

---

### 📋 list_recent_files
Lista archivos recientes.

**Parámetros:**
```json
{
  "limit": "integer (opcional)"  // Número de archivos (default: 10)
}
```

---

## Ejemplos de Workflows n8n

### Workflow 1: Registro Automático de Reuniones

**Trigger:** Webhook cuando termina reunión de Zoom/Meet

**Nodos:**
1. **Webhook** - Recibe datos de la reunión
2. **HTTP Request** → `create_note`
   ```json
   {
     "name": "Reunión {{ $json.date }}",
     "content": "# {{ $json.title }}\n\n**Fecha:** {{ $json.date }}\n**Participantes:**\n{{ $json.participants }}\n\n## Notas\n{{ $json.transcript }}",
     "folder": "Meetings"
   }
   ```
3. **HTTP Request** → `add_tag`
   ```json
   {
     "note": "Reunión {{ $json.date }}",
     "tag": "meeting"
   }
   ```

---

### Workflow 2: Sincronización con Notion/Obsidian

**Trigger:** Cron (cada 1 hora)

**Nodos:**
1. **Schedule Trigger** - Cada hora
2. **HTTP Request** → `list_notes`
3. **Loop Over Items**
4. **HTTP Request** → `read_note`
5. **HTTP Request** → Notion API
6. **If** - Comprobar si existe
7. **HTTP Request** - Create/Update en Notion

---

### Workflow 3: Resumen Diario Automático

**Trigger:** Cron (todos los días a las 20:00)

**Nodos:**
1. **Schedule Trigger** - Diariamente 20:00
2. **HTTP Request** → `get_recent_notes` (limit: 10)
3. **HTTP Request** → `create_note`
   ```json
   {
     "name": "Resumen {{ $now.format('YYYY-MM-DD') }}",
     "content": "# Resumen del Día\n\n## Notas Modificadas\n{{ $json.notes }}",
     "folder": "Daily Summaries"
   }
   ```
4. **HTTP Request** → `show_notification`
   ```json
   {
     "message": "Resumen diario creado",
     "level": "success"
   }
   ```

---

### Workflow 4: Extractor de Transcripciones de YouTube

**Trigger:** Manual o Webhook

**Nodos:**
1. **Webhook** - Recibe URL de YouTube
2. **HTTP Request** → `extract_youtube_transcript`
3. **HTTP Request** → `create_note`
   ```json
   {
     "name": "Transcripción {{ $json.video_title }}",
     "content": "# {{ $json.video_title }}\n\n**URL:** {{ $json.url }}\n**Duración:** {{ $json.duration }}\n\n## Transcripción\n\n{{ $json.transcript }}",
     "folder": "YouTube"
   }
   ```
4. **HTTP Request** → `add_tag`
   ```json
   {
     "note": "Transcripción {{ $json.video_title }}",
     "tag": "youtube"
   }
   ```

---

### Workflow 5: Backup Automático Semanal

**Trigger:** Cron (domingos 23:00)

**Nodos:**
1. **Schedule Trigger** - Domingos 23:00
2. **HTTP Request** → `backup_notes`
3. **Move Binary Data** - Guarda en cloud storage
4. **HTTP Request** → Dropbox/Google Drive API
5. **HTTP Request** → `show_notification`
   ```json
   {
     "message": "Backup completado: {{ $json.notes_count }} notas",
     "level": "success"
   }
   ```

---

### Workflow 6: Monitor de Tags Populares

**Trigger:** Cron (lunes 9:00)

**Nodos:**
1. **Schedule Trigger** - Lunes 9:00
2. **HTTP Request** → `get_all_tags`
3. **Sort** - Ordenar por count DESC
4. **HTTP Request** → `create_note`
   ```json
   {
     "name": "Análisis Tags {{ $now.format('YYYY-MM') }}",
     "content": "# Análisis de Tags\n\n{{ $json.tags_table }}",
     "folder": "Analytics"
   }
   ```

---

### Workflow 7: Búsqueda Inteligente y Organización

**Trigger:** Cron (diario 6:00)

**Nodos:**
1. **Schedule Trigger** - Diario 6:00
2. **HTTP Request** → `search_notes` (query: "TODO")
3. **HTTP Request** → `move_note` (folder: "Pending")
4. **HTTP Request** → `search_notes` (query: "DONE")
5. **HTTP Request** → `archive_note`

---

### Workflow 8: Generador de Informes Mensuales

**Trigger:** Cron (primer día del mes)

**Nodos:**
1. **Schedule Trigger** - 1er día mes 10:00
2. **HTTP Request** → `search_by_date_range`
   ```json
   {
     "start_date": "{{ $now.minus({months: 1}).startOf('month').toISO() }}",
     "end_date": "{{ $now.minus({months: 1}).endOf('month').toISO() }}"
   }
   ```
3. **HTTP Request** → `merge_notes`
   ```json
   {
     "note_names": {{ $json.note_names }},
     "output_name": "Informe {{ $now.minus({months: 1}).format('MMMM YYYY') }}"
   }
   ```
4. **HTTP Request** → `export_note` (format: "pdf")
5. **Email** - Enviar PDF por correo

---

## 🔧 Configuración en n8n

### Paso 1: Crear Credencial HTTP

1. Ve a **Credentials** → **New**
2. Tipo: **Header Auth**
3. Nombre: `NotNative MCP`
4. Headers:
   ```
   Content-Type: application/json
   ```

### Paso 2: Configurar HTTP Request Node

1. Método: **POST**
2. URL: `http://localhost:8765`
3. Authentication: **Header Auth** (credencial creada)
4. Body:
   ```json
   {
     "jsonrpc": "2.0",
     "id": {{ $runIndex }},
     "method": "tools/call",
     "params": {
       "name": "{{ $json.tool_name }}",
       "arguments": {{ $json.arguments }}
     }
   }
   ```

### Paso 3: Procesar Response

Usar **Set Node** para extraer datos:
```javascript
return {
  success: $input.item.json.result.success,
  data: $input.item.json.result.data
};
```

---

## 📝 Plantillas JSON para Copiar

### Template: Crear Nota
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "create_note",
    "arguments": {
      "name": "{{ nombre }}",
      "content": "{{ contenido }}",
      "folder": "{{ carpeta }}"
    }
  }
}
```

### Template: Buscar con Tag
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_notes_with_tag",
    "arguments": {
      "tag": "{{ tag }}"
    }
  }
}
```

### Template: Backup
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "backup_notes",
    "arguments": {}
  }
}
```

---

## 🚀 Mejores Prácticas

1. **Usa nombres descriptivos** para las notas creadas automáticamente
2. **Incluye timestamps** en nombres de notas automáticas
3. **Usa tags consistentes** para facilitar búsquedas
4. **Valida responses** antes de continuar el workflow
5. **Maneja errores** con nodos IF para casos de fallo
6. **Usa folders** para mantener notas organizadas
7. **Crea backups regulares** con workflows programados
8. **Testea workflows** en sandbox antes de producción

---

## 🐛 Troubleshooting

### Error: "Connection refused"
- Verifica que NotNative esté ejecutándose
- Confirma que el MCP Server esté iniciado
- Puerto correcto: 8765

### Error: "Note not found"
- Verifica el nombre exacto de la nota
- Incluye extensión .md si es necesaria
- Usa `list_notes` para confirmar nombres

### Error: "Invalid JSON-RPC"
- Verifica estructura del request
- Confirma que `jsonrpc` sea "2.0"
- ID debe ser número único

---

## 📚 Recursos Adicionales

- **Documentación MCP**: `/docs/MCP_INTEGRATION.md`
- **Guía n8n**: `/docs/QUICK_START_N8N.md`
- **API Reference**: Este documento
- **Ejemplos**: `/docs/N8N_TELEGRAM_INTEGRATION.md`

---

## 🆕 Versión

- **NotNative**: v0.1.5-release
- **MCP Server**: Incluido en NotNative
- **Última actualización**: Noviembre 2025

---

## 🤝 Contribuir

¿Tienes workflows útiles? ¿Ideas para nuevas herramientas? Contribuye al proyecto:

- **GitHub**: https://github.com/k4ditano/notnative-app
- **Issues**: Reporta bugs o sugiere features
- **Pull Requests**: Comparte tus workflows

---

**¡Happy Automation!** 🚀
