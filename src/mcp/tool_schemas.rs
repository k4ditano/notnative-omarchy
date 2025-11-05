use crate::mcp::protocol::MCPTool;
use serde_json::{Value, json};

/// Genera las herramientas MCP más comunes (Core Tools)
pub fn get_core_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "create_note",
                "description": "Crea una nueva nota. USA SOLO cuando el usuario diga: 'crea', 'crea una nota', 'escribe una nota', 'nueva nota', 'guarda', 'anota'. NO uses para leer o analizar notas existentes.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Nombre del archivo (con o sin extensión .md)"
                        },
                        "content": {
                            "type": "string",
                            "description": "Contenido de la nota en formato Markdown"
                        }
                    },
                    "required": ["name", "content"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "read_note",
                "description": "Lee el contenido de una nota existente. USA cuando el usuario diga: 'muestra', 'lee', 'qué dice', 'ver contenido'.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Nombre del archivo a leer"
                        }
                    },
                    "required": ["name"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "update_note",
                "description": "Modifica una nota existente. USA cuando el usuario diga: 'edita', 'modifica', 'cambia', 'actualiza la nota'.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Nombre de la nota a actualizar"
                        },
                        "content": {
                            "type": "string",
                            "description": "Nuevo contenido de la nota"
                        }
                    },
                    "required": ["name", "content"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "list_notes",
                "description": "Lista todas las notas. USA cuando el usuario diga: 'lista', 'muestra todas', 'qué notas tengo'.",
                "parameters": {
                    "type": "object",
                    "properties": {}
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "search_notes",
                "description": "Busca notas por texto. USA cuando el usuario diga: 'busca', 'encuentra', 'notas sobre', 'notas que contengan'.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Texto a buscar en las notas"
                        }
                    },
                    "required": ["query"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "delete_note",
                "description": "Elimina una nota. USA cuando el usuario diga: 'elimina', 'borra', 'borrar nota'.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Nombre de la nota a eliminar"
                        }
                    },
                    "required": ["name"]
                }
            }
        }),
    ]
}

/// Genera todas las definiciones de herramientas MCP
pub fn get_all_tool_definitions() -> Vec<MCPTool> {
    vec![
        // ==================== GESTIÓN DE NOTAS ====================
        MCPTool {
            name: "create_note".to_string(),
            description: "Crea una nueva nota en NotNative con el contenido especificado"
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota (con o sin extensión .md)"
                    },
                    "content": {
                        "type": "string",
                        "description": "Contenido de la nota en formato markdown"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Carpeta donde crear la nota (opcional)"
                    }
                },
                "required": ["name", "content"]
            }),
        },
        MCPTool {
            name: "read_note".to_string(),
            description: "Lee el contenido completo de una nota existente".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota a leer"
                    }
                },
                "required": ["name"]
            }),
        },
        MCPTool {
            name: "update_note".to_string(),
            description: "Actualiza o sobrescribe el contenido de una nota existente".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota"
                    },
                    "content": {
                        "type": "string",
                        "description": "Nuevo contenido completo"
                    }
                },
                "required": ["name", "content"]
            }),
        },
        MCPTool {
            name: "delete_note".to_string(),
            description:
                "Elimina permanentemente una nota. ¡Cuidado, esta acción no se puede deshacer!"
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota a eliminar"
                    }
                },
                "required": ["name"]
            }),
        },
        MCPTool {
            name: "list_notes".to_string(),
            description: "Lista todas las notas disponibles o las de una carpeta específica"
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Carpeta específica (opcional, si no se especifica lista todas)"
                    }
                }
            }),
        },
        MCPTool {
            name: "rename_note".to_string(),
            description: "Renombra una nota existente".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "old_name": {
                        "type": "string",
                        "description": "Nombre actual de la nota"
                    },
                    "new_name": {
                        "type": "string",
                        "description": "Nuevo nombre para la nota"
                    }
                },
                "required": ["old_name", "new_name"]
            }),
        },
        MCPTool {
            name: "duplicate_note".to_string(),
            description: "Crea una copia de una nota existente con un nuevo nombre".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota a duplicar"
                    },
                    "new_name": {
                        "type": "string",
                        "description": "Nombre para la copia"
                    }
                },
                "required": ["name", "new_name"]
            }),
        },
        // ==================== BÚSQUEDA ====================
        MCPTool {
            name: "search_notes".to_string(),
            description: "Busca notas por contenido o nombre usando texto completo".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Texto a buscar en las notas"
                    }
                },
                "required": ["query"]
            }),
        },
        MCPTool {
            name: "get_notes_with_tag".to_string(),
            description: "Obtiene todas las notas que contienen un tag específico".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "tag": {
                        "type": "string",
                        "description": "Tag a buscar (sin el símbolo #)"
                    }
                },
                "required": ["tag"]
            }),
        },
        MCPTool {
            name: "fuzzy_search".to_string(),
            description: "Búsqueda difusa que tolera errores de escritura".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Texto aproximado a buscar"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Número máximo de resultados (opcional)"
                    }
                },
                "required": ["query"]
            }),
        },
        MCPTool {
            name: "get_recent_notes".to_string(),
            description: "Obtiene las notas modificadas recientemente".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Número de notas a devolver (por defecto 10)"
                    }
                }
            }),
        },
        // ==================== ANÁLISIS ====================
        MCPTool {
            name: "analyze_note_structure".to_string(),
            description:
                "Analiza la estructura de una nota: headings, listas, bloques de código, etc."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota a analizar"
                    }
                },
                "required": ["name"]
            }),
        },
        MCPTool {
            name: "get_word_count".to_string(),
            description: "Cuenta palabras, caracteres y líneas de una nota".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota"
                    }
                },
                "required": ["name"]
            }),
        },
        MCPTool {
            name: "suggest_related_notes".to_string(),
            description: "Sugiere notas relacionadas basándose en contenido similar".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota de referencia"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Número máximo de sugerencias"
                    }
                },
                "required": ["name"]
            }),
        },
        MCPTool {
            name: "get_all_tags".to_string(),
            description: "Lista todos los tags usados en todas las notas".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        // ==================== TRANSFORMACIONES ====================
        MCPTool {
            name: "generate_table_of_contents".to_string(),
            description: "Genera un índice (TOC) automático basado en los headings de la nota"
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota"
                    },
                    "max_level": {
                        "type": "integer",
                        "description": "Nivel máximo de headings a incluir (1-6)"
                    }
                },
                "required": ["name"]
            }),
        },
        MCPTool {
            name: "extract_code_blocks".to_string(),
            description: "Extrae todos los bloques de código de una nota".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota"
                    },
                    "language": {
                        "type": "string",
                        "description": "Filtrar por lenguaje específico (opcional)"
                    }
                },
                "required": ["name"]
            }),
        },
        MCPTool {
            name: "merge_notes".to_string(),
            description: "Fusiona múltiples notas en una sola".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "note_names": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Lista de nombres de notas a fusionar"
                    },
                    "output_name": {
                        "type": "string",
                        "description": "Nombre de la nota resultante"
                    }
                },
                "required": ["note_names", "output_name"]
            }),
        },
        // ==================== CONTROL DE UI ====================
        MCPTool {
            name: "open_note".to_string(),
            description: "Abre una nota específica en el editor de NotNative".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota a abrir"
                    }
                },
                "required": ["name"]
            }),
        },
        MCPTool {
            name: "show_notification".to_string(),
            description: "Muestra una notificación en la interfaz de NotNative".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Mensaje a mostrar"
                    },
                    "level": {
                        "type": "string",
                        "enum": ["info", "warning", "error", "success"],
                        "description": "Nivel de importancia de la notificación"
                    }
                },
                "required": ["message"]
            }),
        },
        MCPTool {
            name: "toggle_sidebar".to_string(),
            description: "Muestra u oculta la barra lateral de notas".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        MCPTool {
            name: "refresh_sidebar".to_string(),
            description: "Refresca la lista de notas en la barra lateral".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        // ==================== ORGANIZACIÓN ====================
        MCPTool {
            name: "create_folder".to_string(),
            description: "Crea una nueva carpeta para organizar notas".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la carpeta"
                    },
                    "parent": {
                        "type": "string",
                        "description": "Carpeta padre (opcional)"
                    }
                },
                "required": ["name"]
            }),
        },
        MCPTool {
            name: "list_folders".to_string(),
            description: "Lista todas las carpetas de notas".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        MCPTool {
            name: "move_note".to_string(),
            description: "Mueve una nota a una carpeta diferente".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Carpeta de destino"
                    }
                },
                "required": ["name", "folder"]
            }),
        },
        // ==================== AUTOMATIZACIÓN ====================
        MCPTool {
            name: "create_daily_note".to_string(),
            description: "Crea una nota diaria con la fecha actual (formato YYYY-MM-DD)"
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "template": {
                        "type": "string",
                        "description": "Plantilla de contenido para la nota diaria (opcional)"
                    }
                }
            }),
        },
        MCPTool {
            name: "find_and_replace".to_string(),
            description: "Busca y reemplaza texto en una o múltiples notas".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "find": {
                        "type": "string",
                        "description": "Texto a buscar"
                    },
                    "replace": {
                        "type": "string",
                        "description": "Texto de reemplazo"
                    },
                    "note_names": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Notas específicas (opcional, si no se especifica aplica a todas)"
                    }
                },
                "required": ["find", "replace"]
            }),
        },
        // ==================== SISTEMA ====================
        MCPTool {
            name: "get_app_info".to_string(),
            description: "Obtiene información sobre NotNative: versión, configuración, etc."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        MCPTool {
            name: "get_workspace_path".to_string(),
            description: "Obtiene la ruta del directorio de trabajo actual".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}
