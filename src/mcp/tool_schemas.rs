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
                "name": "append_to_note",
                "description": "Agrega contenido al final de una nota existente. USA cuando el usuario diga: 'agrega', 'añade al final', 'append', 'continúa la nota'.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Nombre de la nota a la que agregar contenido"
                        },
                        "content": {
                            "type": "string",
                            "description": "Contenido a agregar al final de la nota"
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
                "description": "Búsqueda SQL literal/exacta de texto. USA SOLO para: palabras exactas, código específico, IDs, nombres precisos. Para búsquedas conceptuales (temas, ideas, información sobre...) USA semantic_search en su lugar.",
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
            description: "Crea una nueva nota en NotNative con el contenido especificado. Si quieres crear en una carpeta, usa el parámetro 'folder' (NO incluyas la carpeta en 'name')."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota SOLO (sin carpeta, sin path). Ejemplo: 'Mi Nota' o 'Mi Nota.md'. NO uses 'carpeta/Mi Nota'."
                    },
                    "content": {
                        "type": "string",
                        "description": "Contenido de la nota en formato markdown"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Nombre de la carpeta donde crear la nota. Ejemplo: 'workflows'. Si la carpeta no existe, se creará automáticamente. IMPORTANTE: NO incluyas la carpeta en el parámetro 'name'."
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
            name: "append_to_note".to_string(),
            description: "Añade contenido al final de una nota existente sin borrar lo que ya tiene".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota"
                    },
                    "content": {
                        "type": "string",
                        "description": "Contenido a añadir al final"
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
        // ⭐ BÚSQUEDA SEMÁNTICA PRIMERO (más potente, una búsqueda es suficiente)
        MCPTool {
            name: "semantic_search".to_string(),
            description: "⭐ BÚSQUEDA RECOMENDADA ⭐ Busca notas por similitud semántica usando embeddings. Encuentra contenido relacionado aunque use diferentes palabras. USA POR DEFECTO cuando el usuario pida: 'busca', 'encuentra', 'hay notas sobre', 'información sobre', 'notas relacionadas con'. UNA SOLA búsqueda semántica es suficiente (no repitas). Solo usa search_notes si necesitas texto literal exacto.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Consulta en lenguaje natural (ej: 'machine learning', 'cómo funciona rust ownership', 'información sensible')"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Número máximo de resultados (default: 10)"
                    },
                    "min_similarity": {
                        "type": "number",
                        "description": "Similitud mínima 0.0-1.0 (default: 0.5). Valores altos = más estricto"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Filtrar por carpeta específica (opcional)"
                    }
                },
                "required": ["query"]
            }),
        },
        // Búsqueda SQL literal (solo para texto exacto)
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
        // ==================== UI DESHABILITADAS ====================
        // MCPTool {
        //     name: "show_notification".to_string(),
        //     description: "Muestra una notificación en la interfaz de NotNative".to_string(),
        //     parameters: json!({
        //         "type": "object",
        //         "properties": {
        //             "message": {
        //                 "type": "string",
        //                 "description": "Mensaje a mostrar"
        //             },
        //             "level": {
        //                 "type": "string",
        //                 "enum": ["info", "warning", "error", "success"],
        //                 "description": "Nivel de importancia de la notificación"
        //             }
        //         },
        //         "required": ["message"]
        //     }),
        // },
        // MCPTool {
        //     name: "toggle_sidebar".to_string(),
        //     description: "Muestra u oculta la barra lateral de notas".to_string(),
        //     parameters: json!({
        //         "type": "object",
        //         "properties": {}
        //     }),
        // },
        // MCPTool {
        //     name: "refresh_sidebar".to_string(),
        //     description: "Refresca la lista de notas en la barra lateral".to_string(),
        //     parameters: json!({
        //         "type": "object",
        //         "properties": {}
        //     }),
        // },
        // ==================== ORGANIZACIÓN ====================
        MCPTool {
            name: "create_folder".to_string(),
            description: "Crea una nueva carpeta vacía para organizar notas. Úsala ANTES de mover notas a esa carpeta con move_note.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la carpeta a crear. Ejemplo: 'workflows', 'proyectos', etc."
                    },
                    "parent": {
                        "type": "string",
                        "description": "Carpeta padre si quieres crear una subcarpeta (opcional)"
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
            name: "find_empty_items".to_string(),
            description: "Encuentra notas vacías y/o carpetas vacías. Útil para limpiar el sistema de notas.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "item_type": {
                        "type": "string",
                        "description": "Tipo de elementos a buscar: 'notes' (solo notas vacías), 'folders' (solo carpetas vacías), 'all' (ambos). Por defecto: 'all'",
                        "enum": ["notes", "folders", "all"]
                    }
                }
            }),
        },
        MCPTool {
            name: "get_system_date_time".to_string(),
            description: "Obtiene la fecha y hora actual del sistema. Devuelve fecha completa, hora, día de la semana, zona horaria y timestamp Unix.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        MCPTool {
            name: "move_note".to_string(),
            description: "Mueve una nota existente a una carpeta. Útil para organizar notas en carpetas después de crearlas.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota a mover (sin carpeta actual, solo el nombre)"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Nombre de la carpeta de destino. Si no existe, se creará automáticamente."
                    }
                },
                "required": ["name", "folder"]
            }),
        },
        MCPTool {
            name: "delete_folder".to_string(),
            description: "Elimina una carpeta. Por defecto solo elimina carpetas vacías.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la carpeta a eliminar"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Si true, elimina la carpeta aunque tenga contenido. Por defecto false."
                    }
                },
                "required": ["name"]
            }),
        },
        MCPTool {
            name: "rename_folder".to_string(),
            description: "Renombra una carpeta existente".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "old_name": {
                        "type": "string",
                        "description": "Nombre actual de la carpeta"
                    },
                    "new_name": {
                        "type": "string",
                        "description": "Nuevo nombre para la carpeta"
                    }
                },
                "required": ["old_name", "new_name"]
            }),
        },
        MCPTool {
            name: "move_folder".to_string(),
            description: "Mueve una carpeta a otra ubicación".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la carpeta a mover"
                    },
                    "new_parent": {
                        "type": "string",
                        "description": "Carpeta padre de destino (omitir o null para mover a raíz)"
                    }
                },
                "required": ["name"]
            }),
        },
        MCPTool {
            name: "add_tag".to_string(),
            description: "Añade un tag a una nota. Los tags se agregan en el frontmatter YAML.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "note": {
                        "type": "string",
                        "description": "Nombre de la nota"
                    },
                    "tag": {
                        "type": "string",
                        "description": "Tag a añadir (sin el #)"
                    }
                },
                "required": ["note", "tag"]
            }),
        },
        MCPTool {
            name: "remove_tag".to_string(),
            description: "Elimina un tag de una nota".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "note": {
                        "type": "string",
                        "description": "Nombre de la nota"
                    },
                    "tag": {
                        "type": "string",
                        "description": "Tag a eliminar"
                    }
                },
                "required": ["note", "tag"]
            }),
        },
        MCPTool {
            name: "create_tag".to_string(),
            description: "Crea/registra un tag nuevo (informativo, los tags se crean al usarlos)".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "tag": {
                        "type": "string",
                        "description": "Nombre del tag a crear"
                    }
                },
                "required": ["tag"]
            }),
        },
        MCPTool {
            name: "add_multiple_tags".to_string(),
            description: "Añade múltiples tags a una nota de una vez".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "note": {
                        "type": "string",
                        "description": "Nombre de la nota"
                    },
                    "tags": {
                        "type": "array",
                        "items": {
                            "type": "string"
                        },
                        "description": "Lista de tags a añadir"
                    }
                },
                "required": ["note", "tags"]
            }),
        },
        MCPTool {
            name: "analyze_and_tag_note".to_string(),
            description: "Analiza el contenido de una nota y sugiere tags relevantes basados en frecuencia de palabras. NO los aplica automáticamente.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Nombre de la nota a analizar"
                    },
                    "max_tags": {
                        "type": "integer",
                        "description": "Número máximo de tags a sugerir (por defecto 5)"
                    }
                },
                "required": ["name"]
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
        // ==================== BÚSQUEDA SEMÁNTICA ====================
        MCPTool {
            name: "find_similar_notes".to_string(),
            description: "Encuentra notas similares a una nota específica. Útil para descubrir contenido relacionado o 'notas que podrían interesarte'.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "note_path": {
                        "type": "string",
                        "description": "Ruta de la nota de referencia"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Número máximo de resultados (default: 10)"
                    },
                    "min_similarity": {
                        "type": "number",
                        "description": "Similitud mínima 0.0-1.0 (default: 0.5)"
                    }
                },
                "required": ["note_path"]
            }),
        },
        MCPTool {
            name: "get_embedding_stats".to_string(),
            description: "Obtiene estadísticas del índice de embeddings: notas indexadas, chunks totales, tokens procesados.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        MCPTool {
            name: "index_note".to_string(),
            description: "Indexa o re-indexa una nota específica en el sistema de búsqueda semántica.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "note_path": {
                        "type": "string",
                        "description": "Ruta de la nota a indexar"
                    }
                },
                "required": ["note_path"]
            }),
        },
        MCPTool {
            name: "reindex_all_notes".to_string(),
            description: "Re-indexa todas las notas en el sistema de búsqueda semántica. Operación costosa, usar solo cuando sea necesario.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        // ==================== RECORDATORIOS ====================
        MCPTool {
            name: "CreateReminder".to_string(),
            description: "Crea un nuevo recordatorio. Puede vincularse a una nota específica. Formatos de fecha: 'YYYY-MM-DD HH:MM' o 'hoy 18:00', 'mañana 10:00'. Prioridades: 'baja', 'media', 'alta', 'urgente'. Repetición: 'diario', 'semanal', 'mensual'.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Título del recordatorio"
                    },
                    "due_date": {
                        "type": "string",
                        "description": "Fecha y hora de vencimiento. Formato: 'YYYY-MM-DD HH:MM' (ej: '2025-11-20 15:00')"
                    },
                    "description": {
                        "type": "string",
                        "description": "Descripción detallada del recordatorio (opcional)"
                    },
                    "priority": {
                        "type": "string",
                        "enum": ["baja", "media", "alta", "urgente", "low", "medium", "high", "urgent"],
                        "description": "Prioridad del recordatorio (default: 'media')"
                    },
                    "repeat": {
                        "type": "string",
                        "enum": ["diario", "semanal", "mensual", "daily", "weekly", "monthly"],
                        "description": "Patrón de repetición (opcional)"
                    },
                    "note_name": {
                        "type": "string",
                        "description": "Nombre de la nota a la que vincular el recordatorio (opcional pero recomendado para evitar duplicados)"
                    }
                },
                "required": ["title", "due_date"]
            }),
        },
        MCPTool {
            name: "ListReminders".to_string(),
            description: "Lista todos los recordatorios o filtra por estado. Por defecto muestra solo recordatorios pendientes.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["pending", "completed", "snoozed", "all", "pendiente", "completado", "pospuesto"],
                        "description": "Estado de los recordatorios a listar (default: 'pending')"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Número máximo de resultados (opcional)"
                    }
                },
                "required": []
            }),
        },
        MCPTool {
            name: "CompleteReminder".to_string(),
            description: "Marca un recordatorio como completado.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "integer",
                        "description": "ID del recordatorio a completar"
                    }
                },
                "required": ["id"]
            }),
        },
        MCPTool {
            name: "SnoozeReminder".to_string(),
            description: "Pospone un recordatorio por X minutos.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "integer",
                        "description": "ID del recordatorio a posponer"
                    },
                    "minutes": {
                        "type": "integer",
                        "description": "Minutos a posponer (ej: 15, 30, 60)"
                    }
                },
                "required": ["id", "minutes"]
            }),
        },
        MCPTool {
            name: "DeleteReminder".to_string(),
            description: "Elimina permanentemente un recordatorio.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "integer",
                        "description": "ID del recordatorio a eliminar"
                    }
                },
                "required": ["id"]
            }),
        },
    ]
}

/// Convierte todas las herramientas al formato OpenAI (Vec<Value>)
pub fn get_all_tool_definitions_as_values() -> Vec<Value> {
    get_all_tool_definitions()
        .into_iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters
                }
            })
        })
        .collect()
}
