use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Spanish,
    English,
}

impl Language {
    pub fn from_code(code: &str) -> Self {
        match code {
            "en" | "en_US" | "en_GB" => Language::English,
            "es" | "es_ES" | "es_MX" => Language::Spanish,
            _ => {
                // Detectar por prefijo
                if code.starts_with("en") {
                    Language::English
                } else if code.starts_with("es") {
                    Language::Spanish
                } else {
                    Language::Spanish // Default
                }
            }
        }
    }

    pub fn from_env() -> Self {
        std::env::var("LANG")
            .ok()
            .and_then(|lang| lang.split('.').next().map(String::from))
            .map(|code| Self::from_code(&code))
            .unwrap_or(Language::Spanish)
    }

    pub fn code(&self) -> &'static str {
        match self {
            Language::Spanish => "es",
            Language::English => "en",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Language::Spanish => "Español",
            Language::English => "English",
        }
    }
}

#[derive(Debug, Clone)]
pub struct I18n {
    language: Language,
    translations: HashMap<&'static str, (&'static str, &'static str)>,
}

impl I18n {
    pub fn new(language: Language) -> Self {
        let mut translations = HashMap::new();

        // (key, (spanish, english))
        translations.insert("app_title", ("NotNative", "NotNative"));
        translations.insert("untitled", ("Sin título", "Untitled"));
        translations.insert("notes", ("Notas", "Notes"));
        translations.insert("new_note", ("Nueva nota", "New Note"));
        translations.insert("search", ("Buscar", "Search"));
        translations.insert("search_notes", ("Buscar (Ctrl+F)", "Search (Ctrl+F)"));
        translations.insert("search_placeholder", ("Buscar notas...", "Search notes..."));
        translations.insert(
            "show_hide_notes",
            ("Mostrar/ocultar lista de notas", "Show/hide notes list"),
        );
        translations.insert("preferences", ("Preferencias", "Preferences"));
        translations.insert(
            "keyboard_shortcuts",
            ("Atajos de teclado", "Keyboard Shortcuts"),
        );
        translations.insert("about", ("Acerca de", "About"));
        translations.insert("settings", ("Ajustes", "Settings"));
        translations.insert("tags", ("Tags", "Tags"));
        translations.insert("tags_note", ("Tags de la nota", "Note tags"));
        translations.insert("no_tags", ("No hay tags", "No tags"));
        translations.insert(
            "search_tag",
            ("Buscar notas con este tag", "Search notes with this tag"),
        );
        translations.insert("remove_tag", ("Eliminar tag", "Remove tag"));
        translations.insert("close", ("Cerrar", "Close"));

        // Diálogos
        translations.insert("create_note_title", ("Nueva nota", "New Note"));
        translations.insert(
            "note_name_hint",
            (
                "ejemplo: proyectos/nueva-idea",
                "example: projects/new-idea",
            ),
        );
        translations.insert(
            "create_folder_hint",
            (
                "Usa '/' para crear en carpetas",
                "Use '/' to create in folders",
            ),
        );
        translations.insert("create", ("Crear", "Create"));
        translations.insert("cancel", ("Cancelar", "Cancel"));
        translations.insert("rename", ("Renombrar", "Rename"));
        translations.insert("delete", ("Eliminar", "Delete"));
        translations.insert(
            "confirm_delete",
            (
                "¿Estás seguro de eliminar",
                "Are you sure you want to delete",
            ),
        );

        // Preferencias
        translations.insert("theme", ("Tema", "Theme"));
        translations.insert(
            "theme_sync",
            (
                "La aplicación sincroniza automáticamente con el tema Omarchy",
                "The app automatically syncs with Omarchy theme",
            ),
        );
        translations.insert(
            "markdown_rendering",
            ("Renderizado Markdown", "Markdown Rendering"),
        );
        translations.insert(
            "markdown_enabled",
            (
                "Activado por defecto en modo Normal",
                "Enabled by default in Normal mode",
            ),
        );
        translations.insert("language", ("Idioma", "Language"));
        translations.insert(
            "language_description",
            (
                "Elige el idioma de la interfaz",
                "Choose the interface language",
            ),
        );
        translations.insert(
            "restart_required",
            (
                "Se requiere reiniciar la aplicación",
                "Application restart required",
            ),
        );

        // Workspace
        translations.insert("workspace", ("Directorio de trabajo", "Workspace"));
        translations.insert(
            "workspace_description",
            (
                "Ubicación donde se guardan las notas y recursos",
                "Location where notes and resources are saved",
            ),
        );
        translations.insert(
            "open_workspace_folder",
            ("Abrir carpeta de trabajo", "Open workspace folder"),
        );
        translations.insert("change_workspace", ("Cambiar ubicación", "Change location"));
        translations.insert("workspace_location", ("Ubicación", "Location"));
        translations.insert(
            "select_workspace_folder",
            ("Seleccionar carpeta de trabajo", "Select workspace folder"),
        );
        translations.insert("select", ("Seleccionar", "Select"));

        // Audio
        translations.insert("audio_output", ("Salida de audio", "Audio Output"));
        translations.insert(
            "audio_output_description",
            (
                "Seleccionar dispositivo de salida de audio",
                "Select audio output device",
            ),
        );
        translations.insert(
            "audio_output_default",
            ("Por defecto del sistema", "System default"),
        );
        translations.insert(
            "audio_output_detecting",
            ("Detectando dispositivos...", "Detecting devices..."),
        );
        translations.insert(
            "audio_output_changed",
            ("Salida de audio cambiada", "Audio output changed"),
        );
        translations.insert(
            "audio_output_error",
            (
                "Error cambiando salida de audio",
                "Error changing audio output",
            ),
        );

        // Atajos de teclado
        translations.insert("shortcuts_general", ("General", "General"));
        translations.insert("shortcuts_modes", ("Modos de edición", "Editing Modes"));
        translations.insert("shortcuts_navigation", ("Navegación", "Navigation"));
        translations.insert("shortcuts_editing", ("Edición", "Editing"));

        translations.insert("shortcut_new_note", ("Nueva nota", "New note"));
        translations.insert("shortcut_save", ("Guardar nota", "Save note"));
        translations.insert("shortcut_search", ("Buscar notas", "Search notes"));
        translations.insert(
            "shortcut_toggle_sidebar",
            ("Alternar sidebar", "Toggle sidebar"),
        );
        translations.insert("shortcut_escape", ("Volver al editor", "Back to editor"));

        translations.insert("shortcut_insert_mode", ("Modo Insert", "Insert mode"));
        translations.insert("shortcut_normal_mode", ("Modo Normal", "Normal mode"));
        translations.insert("shortcut_command_mode", ("Modo Command", "Command mode"));
        translations.insert("shortcut_visual_mode", ("Modo Visual", "Visual mode"));

        translations.insert(
            "shortcut_movement",
            ("Izquierda/Abajo/Arriba/Derecha", "Left/Down/Up/Right"),
        );
        translations.insert("shortcut_next_word", ("Siguiente palabra", "Next word"));
        translations.insert("shortcut_prev_word", ("Palabra anterior", "Previous word"));
        translations.insert("shortcut_line_start", ("Inicio de línea", "Start of line"));
        translations.insert("shortcut_line_end", ("Fin de línea", "End of line"));
        translations.insert(
            "shortcut_doc_start",
            ("Inicio del documento", "Start of document"),
        );
        translations.insert("shortcut_doc_end", ("Fin del documento", "End of document"));

        translations.insert(
            "shortcut_delete_char",
            ("Eliminar carácter", "Delete character"),
        );
        translations.insert("shortcut_delete_line", ("Eliminar línea", "Delete line"));
        translations.insert("shortcut_undo", ("Deshacer", "Undo"));
        translations.insert("shortcut_redo", ("Rehacer", "Redo"));

        // About
        translations.insert(
            "app_description",
            (
                "Editor de notas markdown con estilo vim",
                "Vim-style markdown note editor",
            ),
        );
        translations.insert("website", ("Sitio web", "Website"));
        translations.insert("authors", ("Autores", "Authors"));
        translations.insert("version", ("Versión", "Version"));
        translations.insert("license", ("Licencia", "License"));

        // Búsqueda
        translations.insert(
            "no_results",
            ("No se encontraron resultados para", "No results found for"),
        );
        translations.insert("searching", ("Buscando", "Searching"));

        // Estados
        translations.insert("lines", ("líneas", "lines"));
        translations.insert("words", ("palabras", "words"));
        translations.insert("characters", ("caracteres", "characters"));
        translations.insert("saved", ("Guardado", "Saved"));
        translations.insert(
            "unsaved_changes",
            ("Cambios sin guardar", "Unsaved changes"),
        );

        // Mensajes
        translations.insert("note_created", ("Nota creada", "Note created"));
        translations.insert("note_deleted", ("Nota eliminada", "Note deleted"));
        translations.insert("note_renamed", ("Nota renombrada", "Note renamed"));
        translations.insert("error", ("Error", "Error"));
        translations.insert("success", ("Éxito", "Success"));

        // Visor de imágenes
        translations.insert("image_viewer", ("Visor de imagen", "Image Viewer"));
        translations.insert("open_file_location", ("Abrir ubicación", "Open Location"));

        // TODOs
        translations.insert("todos", ("TODOs", "TODOs"));
        translations.insert("todos_note", ("TODOs de la nota", "Note TODOs"));
        translations.insert(
            "no_todos",
            ("No hay TODOs en esta nota", "No TODOs in this note"),
        );
        translations.insert("completed", ("completo", "completed"));
        translations.insert("no_section", ("Sin sección", "No section"));

        // YouTube
        translations.insert(
            "transcribe_youtube",
            (
                "¿Transcribir video de YouTube?",
                "Transcribe YouTube video?",
            ),
        );
        translations.insert(
            "youtube_detected",
            (
                "Se ha detectado un enlace de YouTube",
                "A YouTube link has been detected",
            ),
        );
        translations.insert("only_link", ("Solo enlace", "Only link"));
        translations.insert(
            "transcribe_and_insert",
            ("Transcribir e insertar", "Transcribe and insert"),
        );
        translations.insert(
            "downloading_transcript",
            ("Descargando transcripción...", "Downloading transcript..."),
        );
        translations.insert(
            "loading_transcript",
            ("Cargando transcripción...", "Loading transcript..."),
        );
        translations.insert(
            "transcript_error",
            (
                "Error al obtener transcripción",
                "Error fetching transcript",
            ),
        );
        translations.insert(
            "transcript_unavailable",
            (
                "Transcripción no disponible para este video",
                "Transcript unavailable for this video",
            ),
        );
        translations.insert("transcript_section", ("📝 Transcripción", "📝 Transcript"));

        // Music Player
        translations.insert(
            "music_search_placeholder",
            ("Buscar música en YouTube...", "Search music on YouTube..."),
        );
        translations.insert(
            "no_music_playing",
            ("No hay música reproduciéndose", "No music playing"),
        );
        translations.insert(
            "music_play_pause",
            ("Reproducir/Pausar (Espacio)", "Play/Pause (Space)"),
        );
        translations.insert("music_stop", ("Detener", "Stop"));
        translations.insert("music_seek_back", ("Retroceder 5s", "Seek back 5s"));
        translations.insert("music_seek_forward", ("Avanzar 5s", "Seek forward 5s"));
        translations.insert("music_volume_down", ("Bajar volumen", "Lower volume"));
        translations.insert("music_volume_up", ("Subir volumen", "Raise volume"));
        translations.insert("music_previous_song", ("Canción anterior", "Previous song"));
        translations.insert("music_next_song", ("Siguiente canción", "Next song"));
        translations.insert("music_repeat_off", ("Repetir: OFF", "Repeat: OFF"));
        translations.insert("music_repeat_one", ("Repetir: UNA", "Repeat: ONE"));
        translations.insert("music_repeat_all", ("Repetir: TODAS", "Repeat: ALL"));
        translations.insert("music_shuffle_off", ("Aleatorio: OFF", "Shuffle: OFF"));
        translations.insert("music_shuffle_on", ("Aleatorio: ON", "Shuffle: ON"));
        translations.insert(
            "music_manage_playlists",
            ("Gestionar playlists", "Manage playlists"),
        );
        translations.insert(
            "music_playback_queue",
            ("Cola de reproducción", "Playback queue"),
        );
        translations.insert("music_loading", ("Cargando...", "Loading..."));
        translations.insert("music_add_to_queue", ("Añadir a cola", "Add to queue"));
        translations.insert(
            "music_remove_from_queue",
            ("Eliminar de cola", "Remove from queue"),
        );
        translations.insert("music_new_playlist", ("Nueva playlist", "New playlist"));
        translations.insert("music_load_playlist", ("Cargar playlist", "Load playlist"));
        translations.insert("music_save_playlist", ("Guardar playlist", "Save playlist"));
        translations.insert(
            "music_playlist_name",
            ("Nombre de la playlist", "Playlist name"),
        );

        // System Tray
        translations.insert("tray_show_window", ("Mostrar ventana", "Show window"));
        translations.insert("tray_hide_window", ("Ocultar ventana", "Hide window"));
        translations.insert("tray_quit", ("Salir", "Quit"));

        // AI Chat
        translations.insert("ai_chat", ("Chat IA", "AI Chat"));
        translations.insert(
            "chat_input_placeholder",
            ("Escribe tu mensaje aquí...", "Type your message here..."),
        );
        translations.insert("chat_send", ("Enviar", "Send"));
        translations.insert(
            "chat_model_default",
            ("Modelo: OpenAI GPT-4", "Model: OpenAI GPT-4"),
        );
        translations.insert(
            "chat_subtitle",
            (
                "Combina tus notas con el asistente en tiempo real",
                "Combine your notes with the assistant in real time",
            ),
        );
        translations.insert("chat_context", ("Contexto", "Context"));
        translations.insert(
            "chat_attach_note",
            ("Adjuntar nota actual", "Attach current note"),
        );
        translations.insert(
            "chat_attach_note_dialog_title",
            ("Adjuntar nota al contexto", "Attach note to context"),
        );
        translations.insert("chat_attach_button", ("Adjuntar", "Attach"));
        translations.insert("chat_clear_context", ("Limpiar contexto", "Clear context"));
        translations.insert("chat_clear_history", ("Borrar historial", "Clear history"));
        translations.insert(
            "chat_clear_history_confirm_title",
            (
                "¿Borrar todo el historial de chat?",
                "Delete all chat history?",
            ),
        );
        translations.insert(
            "chat_clear_history_confirm_message",
            (
                "Esta acción eliminará permanentemente todo el historial de conversaciones guardado. No se puede deshacer.",
                "This action will permanently delete all saved conversation history. This cannot be undone.",
            ),
        );
        translations.insert(
            "chat_history_cleared",
            ("Historial borrado", "History cleared"),
        );
        translations.insert(
            "chat_history_cleared_message",
            (
                "Se ha eliminado todo el historial de conversaciones",
                "All conversation history has been deleted",
            ),
        );
        translations.insert(
            "music_player_title",
            ("Reproductor de Música", "Music Player"),
        );
        translations.insert(
            "ai_chat_placeholder",
            (
                "Escribe un mensaje para el asistente IA...",
                "Type a message for the AI assistant...",
            ),
        );
        translations.insert("ai_send_message", ("Enviar mensaje", "Send message"));
        translations.insert("ai_thinking", ("Pensando...", "Thinking..."));
        translations.insert("ai_model", ("Modelo", "Model"));
        translations.insert("ai_temperature", ("Temperatura", "Temperature"));
        translations.insert("ai_api_key", ("API Key", "API Key"));
        translations.insert("ai_openai", ("OpenAI", "OpenAI"));
        translations.insert("ai_openrouter", ("OpenRouter", "OpenRouter"));
        translations.insert(
            "ai_api_key_placeholder",
            ("Ingresa tu API key...", "Enter your API key..."),
        );
        translations.insert("ai_save", ("Guardar", "Save"));
        translations.insert("ai_cancel", ("Cancelar", "Cancel"));
        translations.insert(
            "ai_no_key_configured",
            ("No hay API key configurada", "No API key configured"),
        );
        translations.insert(
            "ai_configure_key",
            (
                "Configurar API key en Preferencias",
                "Configure API key in Preferences",
            ),
        );
        translations.insert(
            "ai_free_models",
            ("═══ MODELOS GRATUITOS ═══", "═══ FREE MODELS ═══"),
        );
        translations.insert(
            "ai_paid_models",
            ("═══ MODELOS DE PAGO ═══", "═══ PAID MODELS ═══"),
        );

        // MCP Messages
        translations.insert(
            "mcp_note_created",
            (
                "✓ Nota '{}' creada exitosamente",
                "✓ Note '{}' created successfully",
            ),
        );
        translations.insert(
            "mcp_note_read",
            (
                "✓ Nota '{}' leída correctamente",
                "✓ Note '{}' read successfully",
            ),
        );
        translations.insert(
            "mcp_note_updated",
            (
                "✓ Nota '{}' actualizada exitosamente",
                "✓ Note '{}' updated successfully",
            ),
        );
        translations.insert(
            "mcp_note_deleted",
            (
                "✓ Nota '{}' eliminada exitosamente",
                "✓ Note '{}' deleted successfully",
            ),
        );
        translations.insert(
            "mcp_content_appended",
            (
                "✓ Contenido agregado a '{}' exitosamente",
                "✓ Content appended to '{}' successfully",
            ),
        );
        translations.insert(
            "mcp_notes_found",
            ("✓ {} notas encontradas", "✓ {} notes found"),
        );
        translations.insert(
            "mcp_search_results",
            ("✓ {} resultados para '{}'", "✓ {} results for '{}'"),
        );
        translations.insert(
            "mcp_notes_with_tag",
            ("✓ {} notas con tag #{}", "✓ {} notes with tag #{}"),
        );
        translations.insert(
            "mcp_tags_found",
            ("✓ {} tags encontrados", "✓ {} tags found"),
        );
        translations.insert(
            "mcp_tags_added",
            ("✓ Tags agregados a '{}'", "✓ Tags added to '{}'"),
        );
        translations.insert(
            "mcp_note_renamed",
            (
                "✓ Nota renombrada de '{}' a '{}'",
                "✓ Note renamed from '{}' to '{}'",
            ),
        );
        translations.insert(
            "mcp_note_duplicated",
            (
                "✓ Nota '{}' duplicada como '{}'",
                "✓ Note '{}' duplicated as '{}'",
            ),
        );
        translations.insert(
            "mcp_folder_created",
            (
                "✓ Carpeta '{}' creada exitosamente",
                "✓ Folder '{}' created successfully",
            ),
        );
        translations.insert(
            "mcp_note_not_found",
            ("Nota '{}' no encontrada", "Note '{}' not found"),
        );
        translations.insert(
            "mcp_folders_found",
            ("✓ {} carpetas encontradas", "✓ {} folders found"),
        );

        Self {
            language,
            translations,
        }
    }

    pub fn t(&self, key: &str) -> String {
        self.translations
            .get(key)
            .map(|(es, en)| match self.language {
                Language::Spanish => *es,
                Language::English => *en,
            })
            .unwrap_or(key)
            .to_string()
    }

    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }

    pub fn current_language(&self) -> Language {
        self.language
    }

    /// Obtiene todas las traducciones disponibles para una clave
    pub fn all_translations(&self, key: &str) -> Option<(String, String)> {
        self.translations
            .get(key)
            .map(|(es, en)| (es.to_string(), en.to_string()))
    }
}
