use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::mcp::protocol::MCPTool;

/// Enum con todas las herramientas disponibles
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", content = "args")]
pub enum MCPToolCall {
    // === Gestión de notas ===
    CreateNote {
        name: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
    },
    UpdateNote {
        name: String,
        content: String,
    },
    AppendToNote {
        name: String,
        content: String,
    },
    DeleteNote {
        name: String,
    },
    ReadNote {
        name: String,
    },
    ListNotes {
        #[serde(skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
    },
    RenameNote {
        old_name: String,
        new_name: String,
    },
    DuplicateNote {
        name: String,
        new_name: String,
    },

    // === Búsqueda y navegación ===
    SearchNotes {
        query: String,
    },
    SearchByTag {
        tag: String,
    },
    GetNotesWithTag {
        tag: String,
    },
    SearchByDateRange {
        start_date: String,
        end_date: String,
    },
    FuzzySearch {
        query: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<i32>,
    },

    // === Búsqueda Semántica ===
    SemanticSearch {
        query: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        min_similarity: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
    },
    FindSimilarNotes {
        note_path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        min_similarity: Option<f32>,
    },
    GetEmbeddingStats,
    IndexNote {
        note_path: String,
    },
    ReindexAllNotes,

    // === Organización ===
    MoveNote {
        name: String,
        folder: String,
    },
    CreateFolder {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
    },
    DeleteFolder {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        recursive: Option<bool>, // Si true, elimina contenido; si false, solo si está vacía
    },
    RenameFolder {
        old_name: String,
        new_name: String,
    },
    MoveFolder {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        new_parent: Option<String>, // None = mover a raíz
    },
    AddTag {
        note: String,
        tag: String,
    },
    RemoveTag {
        note: String,
        tag: String,
    },
    CreateTag {
        tag: String,
    },
    AddMultipleTags {
        note: String,
        tags: Vec<String>,
    },
    AnalyzeAndTagNote {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_tags: Option<i32>, // Límite de tags a sugerir
    },
    ArchiveNote {
        name: String,
    },

    // === Análisis y Estadísticas ===
    GetNoteStats {
        name: String,
    },
    AnalyzeNoteStructure {
        name: String,
    },
    GetWordCount {
        name: String,
    },
    FindBrokenLinks {
        #[serde(skip_serializing_if = "Option::is_none")]
        note_name: Option<String>,
    },
    SuggestRelatedNotes {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<i32>,
    },
    GetRecentNotes {
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<i32>,
    },
    GetAllTags {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        _dummy: Option<()>,
    },
    ListFolders {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        _dummy: Option<()>,
    },
    GetNoteGraph {
        #[serde(skip_serializing_if = "Option::is_none")]
        max_depth: Option<i32>,
    },
    FindEmptyItems {
        #[serde(skip_serializing_if = "Option::is_none")]
        item_type: Option<String>, // "notes", "folders", "all" (default: "all")
    },
    GetSystemDateTime {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        _dummy: Option<()>,
    },

    // === Transformaciones de Contenido ===
    GenerateTableOfContents {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_level: Option<i32>,
    },
    ExtractCodeBlocks {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
    FormatNote {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<String>,
    },
    MergeNotes {
        note_names: Vec<String>,
        output_name: String,
    },
    SplitNote {
        name: String,
        split_by: String, // "heading", "paragraph", "separator"
    },

    // === Control de UI (DESHABILITADAS - pendiente de implementar) ===
    // OpenNote {
    //     name: String,
    // },
    // ShowNotification {
    //     message: String,
    //     #[serde(skip_serializing_if = "Option::is_none")]
    //     level: Option<String>, // "info", "warning", "error", "success"
    // },
    // HighlightNote {
    //     name: String,
    // },
    // ToggleSidebar,
    // SwitchMode {
    //     mode: String, // "normal", "insert", "chat"
    // },
    // RefreshSidebar,
    // FocusSearch,

    // === Exportación e Importación ===
    ExportNote {
        name: String,
        format: String, // "html", "pdf", "json", "txt"
        #[serde(skip_serializing_if = "Option::is_none")]
        output_path: Option<String>,
    },
    ExportMultipleNotes {
        note_names: Vec<String>,
        format: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        output_dir: Option<String>,
    },
    BackupNotes {
        #[serde(skip_serializing_if = "Option::is_none")]
        output_path: Option<String>,
    },
    ImportFromUrl {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        note_name: Option<String>,
    },

    // === Multimedia ===
    InsertImage {
        note: String,
        image_path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        alt_text: Option<String>,
    },
    InsertYouTubeVideo {
        note: String,
        video_url: String,
    },
    ExtractYouTubeTranscript {
        video_url: String,
    },

    // === Automatización ===
    CreateDailyNote {
        #[serde(skip_serializing_if = "Option::is_none")]
        template: Option<String>,
    },
    BatchRename {
        pattern: String,
        replacement: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
    },
    FindAndReplace {
        find: String,
        replace: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        note_names: Option<Vec<String>>,
    },

    // === Sistema ===
    GetAppInfo,
    GetWorkspacePath,
    ListRecentFiles {
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<i32>,
    },

    // === Recordatorios / Reminders ===
    CreateReminder {
        title: String,
        due_date: String, // "2025-11-20 15:00", "hoy 18:00", "mañana", "today 18:00", "tomorrow"
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        priority: Option<String>, // "baja", "media", "alta", "urgente" / "low", "medium", "high", "urgent"
        #[serde(skip_serializing_if = "Option::is_none")]
        repeat: Option<String>, // "diario", "semanal", "mensual" / "daily", "weekly", "monthly"
        #[serde(skip_serializing_if = "Option::is_none")]
        note_name: Option<String>, // Nota a la que vincular el recordatorio
    },
    ListReminders {
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>, // "pending", "completed", "all" (default: "pending")
        #[serde(skip_serializing_if = "Option::is_none")]
        limit: Option<i32>,
    },
    UpdateReminder {
        id: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        due_date: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        priority: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        repeat: Option<String>,
    },
    DeleteReminder {
        id: i64,
    },
    SnoozeReminder {
        id: i64,
        minutes: i32, // Posponer por X minutos
    },
    CompleteReminder {
        id: i64,
    },
}

/// Resultado de la ejecución de una herramienta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl MCPToolResult {
    pub fn success(data: Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// Registry de herramientas
#[derive(Debug, Clone)]
pub struct MCPToolRegistry {
    tools: Vec<Value>,
}

impl MCPToolRegistry {
    /// Crea un registro con solo las herramientas esenciales (mejor para modelos más lentos)
    pub fn new_core() -> Self {
        Self {
            tools: crate::mcp::tool_schemas::get_core_tool_definitions(),
        }
    }

    /// Crea un registro con todas las herramientas disponibles
    pub fn new() -> Self {
        Self {
            tools: crate::mcp::tool_schemas::get_all_tool_definitions_as_values(),
        }
    }

    fn default_tools_deprecated() -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "create_note".to_string(),
                description: "Crea una nueva nota en NotNative".to_string(),
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
                description: "Lee el contenido completo de una nota".to_string(),
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
                description: "Actualiza el contenido de una nota existente".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Nombre de la nota"
                        },
                        "content": {
                            "type": "string",
                            "description": "Nuevo contenido"
                        }
                    },
                    "required": ["name", "content"]
                }),
            },
            MCPTool {
                name: "delete_note".to_string(),
                description: "Elimina una nota".to_string(),
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
                description: "Lista todas las notas o las de una carpeta específica".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "folder": {
                            "type": "string",
                            "description": "Carpeta específica (opcional)"
                        }
                    }
                }),
            },
            MCPTool {
                name: "search_notes".to_string(),
                description: "Busca notas por contenido o nombre".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Texto a buscar"
                        }
                    },
                    "required": ["query"]
                }),
            },
            MCPTool {
                name: "get_notes_with_tag".to_string(),
                description: "Obtiene todas las notas que tienen un tag específico".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "tag": {
                            "type": "string",
                            "description": "Tag a buscar (sin #)"
                        }
                    },
                    "required": ["tag"]
                }),
            },
            MCPTool {
                name: "get_all_tags".to_string(),
                description: "Obtiene la lista de todos los tags usados en las notas".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
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
        ]
    }

    /// Obtiene todas las herramientas en formato OpenAI (ya listas para enviar a la API)
    pub fn get_tools(&self) -> &[Value] {
        &self.tools
    }
}

impl Default for MCPToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
