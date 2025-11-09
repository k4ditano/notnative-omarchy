# Ejemplos de cURL para NotNative MCP Server

Este archivo contiene ejemplos listos para copiar y pegar de los comandos más útiles para interactuar con el MCP Server.

## Configuración

Establece la URL base (cambia según tu configuración):

```bash
# Local
export MCP_URL="http://localhost:8788"

# Con túnel Cloudflare
export MCP_URL="https://notnative-mcp.tu-dominio.com"

# Con ngrok
export MCP_URL="https://abc123.ngrok.io"
```

---

## Health Check

Verificar que el servidor está activo:

```bash
curl $MCP_URL/health
```

**Respuesta esperada:**
```json
{
  "status": "ok",
  "service": "NotNative MCP Server",
  "version": "1.0.0"
}
```

---

## Listar Herramientas Disponibles

```bash
curl -X POST $MCP_URL/mcp/list_tools \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "list_tools"
  }' | jq '.'
```

---

## Crear Nota

### Nota simple

```bash
curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "create_note",
      "args": {
        "name": "Mi Nota",
        "content": "# Mi Primera Nota\n\nContenido de la nota."
      }
    }
  }' | jq '.'
```

### Nota en carpeta específica

```bash
curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "create_note",
      "args": {
        "name": "Nota en Carpeta",
        "content": "# Contenido",
        "folder": "Proyectos/Web"
      }
    }
  }' | jq '.'
```

### Nota para Telegram (inicial)

```bash
curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "create_note",
      "args": {
        "name": "Telegram Notes",
        "content": "# Notas desde Telegram\n\nTodos los mensajes del bot se agregarán aquí:\n"
      }
    }
  }' | jq '.'
```

---

## Leer Nota

```bash
curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "read_note",
      "args": {
        "name": "Telegram Notes"
      }
    }
  }' | jq -r '.result.data.content'
```

---

## Agregar Contenido a Nota (append_to_note)

### Agregar texto simple

```bash
curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "append_to_note",
      "args": {
        "name": "Telegram Notes",
        "content": "\n\n---\nNuevo contenido agregado al final"
      }
    }
  }' | jq '.'
```

### Formato mensaje de Telegram

```bash
# Con timestamp
TIMESTAMP=$(date "+%d/%m/%Y %H:%M")
USER="Abel"
MESSAGE="Recordar comprar leche"

curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d "{
    \"jsonrpc\": \"2.0\",
    \"id\": 1,
    \"method\": \"call_tool\",
    \"params\": {
      \"tool\": \"append_to_note\",
      \"args\": {
        \"name\": \"Telegram Notes\",
        \"content\": \"\\n\\n---\\n**[$TIMESTAMP]** $USER:\\n$MESSAGE\"
      }
    }
  }" | jq '.'
```

### Ejemplo completo con variables

```bash
#!/bin/bash

NOTE_NAME="Telegram Notes"
TIMESTAMP=$(date "+%d/%m/%Y %H:%M")
USER="Usuario"
MESSAGE="Este es mi mensaje"

curl -X POST "$MCP_URL/mcp/call_tool" \
  -H "Content-Type: application/json" \
  -d "{
    \"jsonrpc\": \"2.0\",
    \"id\": 1,
    \"method\": \"call_tool\",
    \"params\": {
      \"tool\": \"append_to_note\",
      \"args\": {
        \"name\": \"$NOTE_NAME\",
        \"content\": \"\\n\\n---\\n**[$TIMESTAMP]** $USER:\\n$MESSAGE\"
      }
    }
  }" | jq '.'
```

---

## Actualizar Nota Completa

Reemplaza todo el contenido:

```bash
curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "update_note",
      "args": {
        "name": "Mi Nota",
        "content": "# Contenido Completamente Nuevo\n\nEl contenido anterior fue reemplazado."
      }
    }
  }' | jq '.'
```

---

## Listar Notas

### Todas las notas

```bash
curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "list_notes",
      "args": {}
    }
  }' | jq '.result.data.notes[] | .name'
```

### Notas en carpeta específica

```bash
curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "list_notes",
      "args": {
        "folder": "Proyectos"
      }
    }
  }' | jq '.result.data.notes'
```

---

## Buscar Notas

### Búsqueda simple

```bash
curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "search_notes",
      "args": {
        "query": "telegram"
      }
    }
  }' | jq '.result.data.results'
```

### Búsqueda case-sensitive

```bash
curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "search_notes",
      "args": {
        "query": "Telegram",
        "case_sensitive": true
      }
    }
  }' | jq '.result.data.results'
```

---

## Eliminar Nota

⚠️ **Cuidado:** Esta acción es permanente.

```bash
curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "delete_note",
      "args": {
        "name": "Nota a Eliminar"
      }
    }
  }' | jq '.'
```

---

## Agregar Tags a Nota

```bash
curl -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "add_tags",
      "args": {
        "name": "Mi Nota",
        "tags": ["telegram", "importante", "inbox"]
      }
    }
  }' | jq '.'
```

---

## Ejemplos Avanzados

### Script para agregar múltiples mensajes

```bash
#!/bin/bash

NOTE="Telegram Notes"
MESSAGES=(
  "Primer mensaje"
  "Segundo mensaje"
  "Tercer mensaje"
)

for msg in "${MESSAGES[@]}"; do
  TIMESTAMP=$(date "+%d/%m/%Y %H:%M:%S")
  
  curl -X POST "$MCP_URL/mcp/call_tool" \
    -H "Content-Type: application/json" \
    -d "{
      \"jsonrpc\": \"2.0\",
      \"id\": 1,
      \"method\": \"call_tool\",
      \"params\": {
        \"tool\": \"append_to_note\",
        \"args\": {
          \"name\": \"$NOTE\",
          \"content\": \"\\n\\n---\\n**[$TIMESTAMP]**\\n$msg\"
        }
      }
    }" | jq -r '.result.success'
  
  sleep 1
done
```

### Backup de una nota

```bash
#!/bin/bash

NOTE_NAME="Telegram Notes"
BACKUP_FILE="backup_$(date +%Y%m%d_%H%M%S).md"

curl -X POST "$MCP_URL/mcp/call_tool" \
  -H "Content-Type: application/json" \
  -d "{
    \"jsonrpc\": \"2.0\",
    \"id\": 1,
    \"method\": \"call_tool\",
    \"params\": {
      \"tool\": \"read_note\",
      \"args\": {
        \"name\": \"$NOTE_NAME\"
      }
    }
  }" | jq -r '.result.data.content' > "$BACKUP_FILE"

echo "Backup guardado en: $BACKUP_FILE"
```

### Monitorear tamaño de nota

```bash
#!/bin/bash

NOTE_NAME="Telegram Notes"

while true; do
  SIZE=$(curl -s -X POST "$MCP_URL/mcp/call_tool" \
    -H "Content-Type: application/json" \
    -d "{
      \"jsonrpc\": \"2.0\",
      \"id\": 1,
      \"method\": \"call_tool\",
      \"params\": {
        \"tool\": \"read_note\",
        \"args\": {\"name\": \"$NOTE_NAME\"}
      }
    }" | jq -r '.result.data.content' | wc -c)
  
  echo "$(date) - Tamaño de '$NOTE_NAME': $SIZE bytes"
  sleep 60
done
```

---

## Testing Rápido

### Verificar que todo funciona

```bash
# 1. Health check
echo "1. Health Check:"
curl -s $MCP_URL/health | jq '.'

# 2. Crear nota de prueba
echo -e "\n2. Crear nota:"
curl -s -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "create_note",
      "args": {
        "name": "Test",
        "content": "# Test Note\n"
      }
    }
  }' | jq -r '.result.success'

# 3. Agregar contenido
echo -e "\n3. Agregar contenido:"
curl -s -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "append_to_note",
      "args": {
        "name": "Test",
        "content": "\nContenido agregado!"
      }
    }
  }' | jq -r '.result.success'

# 4. Leer contenido
echo -e "\n4. Contenido final:"
curl -s -X POST $MCP_URL/mcp/call_tool \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "call_tool",
    "params": {
      "tool": "read_note",
      "args": {
        "name": "Test"
      }
    }
  }' | jq -r '.result.data.content'
```

---

## Tips

### Pretty print con jq

Siempre agrega `| jq '.'` al final para formatear JSON:

```bash
curl ... | jq '.'
```

### Extraer solo el contenido de la nota

```bash
curl ... | jq -r '.result.data.content'
```

### Verificar solo si tuvo éxito

```bash
curl ... | jq -r '.result.success'
```

### Debug: Ver request completo

```bash
curl -v -X POST ...
```

---

**Última actualización:** 7 de noviembre de 2025
