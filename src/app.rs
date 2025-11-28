use chrono::Local;
use gtk::glib;
use pulldown_cmark::{Options, Parser, html};
use relm4::gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, component, gtk};
use std::cell::RefCell;
use std::rc::Rc;

use crate::core::{
    CommandParser, EditorAction, EditorMode, HtmlRenderer, KeyModifiers, MarkdownParser,
    NoteBuffer, NoteFile, NotesConfig, NotesDatabase, NotesDirectory, PreviewTheme, SearchResult,
    StyleType, extract_all_tags,
};
use crate::i18n::{I18n, Language};
use crate::mcp::{MCPToolCall, MCPToolResult};

use crate::ai::memory::NoteMemory;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct ThemeColors {
    link_color: gtk::gdk::RGBA,
    code_bg: gtk::gdk::RGBA,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            // Azul claro por defecto para links
            link_color: gtk::gdk::RGBA::new(0.4, 0.7, 1.0, 1.0),
            // Gris sutil para fondos de código
            code_bg: gtk::gdk::RGBA::new(0.5, 0.5, 0.5, 0.1),
        }
    }
}

#[derive(Debug, Clone)]
struct LinkSpan {
    start: i32,
    end: i32,
    url: String,
}

#[derive(Debug, Clone)]
struct HeadingAnchor {
    id: String,       // ID del heading (ej: "conexión-al-mcp-server")
    line_offset: i32, // Posición del heading en el buffer
    text: String,     // Texto del heading
}

#[derive(Debug, Clone)]
struct TagSpan {
    start: i32,
    end: i32,
    tag: String,
}

#[derive(Debug)]
struct NoteMentionSpan {
    start: i32,
    end: i32,
    note_name: String,
}

#[derive(Debug, Clone)]
struct YouTubeVideoSpan {
    start: i32,
    end: i32,
    video_id: String,
    url: String,
}

#[derive(Debug, Clone)]
struct TodoItem {
    completed: bool,
    indent_level: usize, // 0 = nivel principal, 1 = primera subtarea, etc.
    text: String,
}

#[derive(Debug, Clone)]
struct TodoSection {
    title: String,
    todos: Vec<TodoItem>,
    total: usize,
    completed: usize,
    percentage: usize,
}

/// Shared user-facing application identifier used by GTK.
pub const APP_ID: &str = "com.notnative.app";

/// High-level preference for the current visual theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePreference {
    FollowSystem,
    Light,
    Dark,
}

#[derive(Debug)]
pub struct MainApp {
    theme: ThemePreference,
    buffer: NoteBuffer,
    mode: Rc<RefCell<EditorMode>>,
    command_parser: CommandParser,
    cursor_position: usize,
    text_buffer: gtk::TextBuffer,
    mode_label: gtk::Label,
    stats_label: gtk::Label,
    window_title: gtk::Label,
    notes_dir: NotesDirectory,
    notes_db: NotesDatabase,
    notes_config: Rc<RefCell<NotesConfig>>,
    current_note: Option<NoteFile>,
    has_unsaved_changes: bool,
    markdown_enabled: bool,
    bit8_mode: bool,
    text_view: gtk::TextView,
    // WebView para preview HTML en modo Normal
    preview_webview: webkit6::WebView,
    editor_stack: gtk::Stack, // Stack para alternar entre TextView y WebView
    editor_scroll: gtk::ScrolledWindow,
    preview_scroll: gtk::ScrolledWindow,
    preview_scroll_percent: Rc<RefCell<f64>>, // Porcentaje de scroll para sincronizar entre modos
    split_view: gtk::Paned,
    notes_list: gtk::ListBox,
    sidebar_visible: bool,
    expanded_folders: std::collections::HashSet<String>,
    is_populating_list: Rc<RefCell<bool>>,
    is_syncing_to_gtk: Rc<RefCell<bool>>,
    context_menu: gtk::PopoverMenu,
    context_item_name: Rc<RefCell<String>>,
    context_is_folder: Rc<RefCell<bool>>,
    renaming_item: Rc<RefCell<Option<(String, bool)>>>, // (nombre, es_carpeta)
    main_window: gtk::ApplicationWindow,
    link_spans: Rc<RefCell<Vec<LinkSpan>>>,
    heading_anchors: Rc<RefCell<Vec<HeadingAnchor>>>,
    tag_spans: Rc<RefCell<Vec<TagSpan>>>,
    note_mention_spans: Rc<RefCell<Vec<NoteMentionSpan>>>,
    youtube_video_spans: Rc<RefCell<Vec<YouTubeVideoSpan>>>,
    tags_menu_button: gtk::MenuButton,
    tags_list_box: gtk::ListBox,
    todos_menu_button: gtk::MenuButton,
    todos_list_box: gtk::ListBox,
    tag_completion_popup: gtk::Popover,
    tag_completion_list: gtk::ListBox,
    current_tag_prefix: Rc<RefCell<Option<String>>>, // Tag que se está escribiendo actualmente
    just_completed_tag: Rc<RefCell<bool>>, // Bandera para evitar reabrir el popover después de completar
    // Sistema de menciones @ para backlinks
    note_mention_popup: gtk::Popover,
    note_mention_list: gtk::ListBox,
    current_mention_prefix: Rc<RefCell<Option<String>>>, // @ de nota que se está escribiendo
    just_completed_mention: Rc<RefCell<bool>>, // Bandera para evitar reabrir después de completar mención
    search_toggle_button: gtk::Button,
    // Barra de búsqueda flotante estilo macOS
    floating_search_bar: gtk::Box,
    floating_search_entry: gtk::SearchEntry,
    floating_search_mode_label: gtk::Label,
    floating_search_results: gtk::ScrolledWindow,
    floating_search_results_list: gtk::ListBox,
    floating_search_rows: Rc<RefCell<Vec<gtk::ListBoxRow>>>,
    floating_search_visible: bool,
    floating_search_in_current_note: Rc<RefCell<bool>>, // true = buscar solo en nota actual, false = buscar en todas
    // Para navegación entre coincidencias en búsqueda dentro de nota
    in_note_search_matches: Rc<RefCell<Vec<(i32, i32)>>>, // Vector de (start_offset, end_offset) de cada coincidencia
    in_note_search_current_index: Rc<RefCell<usize>>,     // Índice de la coincidencia actual
    semantic_search_enabled: bool,                        // Toggle para búsqueda semántica
    semantic_search_timeout_id: Rc<RefCell<Option<gtk::glib::SourceId>>>, // ID del timeout para debounce semántico
    traditional_search_timeout_id: Rc<RefCell<Option<gtk::glib::SourceId>>>, // ID del timeout para debounce tradicional
    semantic_search_answer_box: gtk::Box, // Box para mostrar la respuesta del agente
    semantic_search_answer_row: gtk::ListBoxRow, // Row padre del answer_box
    semantic_search_answer_label: gtk::Label, // Label con la respuesta del agente
    semantic_search_answer_visible: Rc<RefCell<bool>>, // Si la respuesta está visible
    i18n: Rc<RefCell<I18n>>,
    // Widgets para actualización dinámica de idioma
    sidebar_toggle_button: gtk::Button,
    sidebar_notes_label: gtk::Label,
    new_note_button: gtk::Button,
    settings_button: gtk::MenuButton,
    // Widgets de imágenes para modo normal
    image_widgets: Rc<RefCell<Vec<gtk::Picture>>>,
    // Widgets de TODOs para modo normal
    todo_widgets: Rc<RefCell<Vec<gtk::CheckButton>>>,
    // Widgets de videos para modo normal (WebView)
    video_widgets: Rc<RefCell<Vec<gtk::Box>>>,
    // Widgets de tablas para modo normal (WebView)
    table_widgets: Rc<RefCell<Vec<gtk::Box>>>,
    // Widgets de recordatorios para modo normal
    reminder_widgets: Rc<RefCell<Vec<gtk::Box>>>,
    // Sender para comunicación asíncrona desde closures
    app_sender: Rc<RefCell<Option<ComponentSender<Self>>>>,
    // Servidor HTTP local para embeds de YouTube
    youtube_server: Rc<crate::youtube_server::YouTubeEmbedServer>,
    // Reproductor de música (se crea bajo demanda)
    music_player: Rc<RefCell<Option<Rc<crate::music_player::MusicPlayer>>>>,
    music_player_button: gtk::MenuButton,
    music_player_popover: gtk::Popover,
    music_search_entry: gtk::SearchEntry,
    music_results_list: gtk::ListBox,
    music_now_playing_label: gtk::Label,
    music_state_label: gtk::Label,
    music_play_pause_btn: gtk::Button,
    // Gestión de playlists
    playlist_current_list: gtk::ListBox,
    playlist_saved_list: gtk::ListBox,
    // Chat AI
    chat_session: Rc<RefCell<Option<crate::ai_chat::ChatSession>>>,
    chat_session_id: Rc<RefCell<Option<i64>>>,
    content_stack: gtk::Stack,
    chat_ai_container: gtk::Box,
    chat_split_view: gtk::Paned,
    chat_context_list: gtk::ListBox,
    chat_history_scroll: gtk::ScrolledWindow,
    chat_history_list: gtk::ListBox,
    chat_input_view: gtk::TextView,
    chat_input_buffer: gtk::TextBuffer,
    chat_send_button: gtk::Button,
    chat_clear_button: gtk::Button,
    chat_attach_button: gtk::Button,
    chat_model_label: gtk::Label,
    chat_tokens_progress: gtk::ProgressBar,
    // Autocompletado de notas con @
    chat_note_suggestions_popover: gtk::Popover,
    chat_note_suggestions_list: gtk::ListBox,
    chat_current_note_prefix: Rc<RefCell<Option<String>>>,
    chat_just_completed_note: Rc<RefCell<bool>>,
    // Streaming del chat
    chat_streaming_label: Rc<RefCell<Option<gtk::Label>>>, // Label actual que recibe el streaming
    chat_streaming_text: Rc<RefCell<String>>,              // Texto acumulado del stream
    // ReAct steps (pensamiento del agente)
    chat_thinking_container: Rc<RefCell<Option<gtk::Box>>>, // Contenedor de steps expandible
    // MCP (Model Context Protocol)
    mcp_executor: Rc<RefCell<crate::mcp::MCPToolExecutor>>,
    mcp_registry: crate::mcp::MCPToolRegistry,
    mcp_last_update_check: Rc<RefCell<u64>>, // Último timestamp verificado
    // System Tray - Estado de visibilidad compartido
    window_visible: std::sync::Arc<std::sync::atomic::AtomicBool>,
    // File Watcher - Monitorea cambios en el filesystem
    #[allow(dead_code)]
    file_watcher: Option<crate::file_watcher::FileWatcher>,
    // Cache para texto renderizado en modo Normal
    cached_rendered_text: Rc<RefCell<Option<String>>>,
    cached_source_text: Rc<RefCell<Option<String>>>,
    // Router Agent - Sistema multi-agente con ReAct
    router_agent: Rc<RefCell<Option<crate::ai::RouterAgent>>>,
    // Modo de Chat: true = Agente con tools, false = Chat normal sin tools
    chat_agent_mode: Rc<RefCell<bool>>,
    chat_mode_label: gtk::Label,
    // Sistema de notificaciones toast
    notification_revealer: gtk::Revealer,
    notification_label: gtk::Label,
    // Sistema de recordatorios
    reminder_db: std::sync::Arc<std::sync::Mutex<crate::reminders::ReminderDatabase>>,
    reminder_scheduler: std::sync::Arc<crate::reminders::ReminderScheduler>,
    reminder_notifier: std::sync::Arc<crate::reminders::ReminderNotifier>,
    reminder_parser: crate::reminders::ReminderParser,
    reminders_button: gtk::MenuButton,
    reminders_popover: gtk::Popover,
    reminders_list: gtk::ListBox,
    reminders_pending_badge: gtk::Label,
    // Sistema de memoria vectorial RIG (búsqueda semántica unificada)
    #[allow(dead_code)] // No impl Debug
    note_memory: Rc<RefCell<Option<Arc<NoteMemory<rig::providers::openai::EmbeddingModel>>>>>,
    // Quick Notes - Ventana flotante para notas rápidas
    #[allow(dead_code)]
    quick_note_window: Rc<RefCell<Option<crate::quick_note::QuickNoteWindow>>>,
}

#[derive(Debug, Clone)]
pub enum AppMsg {
    ToggleTheme,
    #[allow(dead_code)]
    SetTheme(ThemePreference),
    RefreshTheme, // Nuevo: actualizar cuando el tema del sistema cambia
    Toggle8BitMode,
    ToggleSidebar,
    CloseSidebar,              // Cerrar sidebar si está abierto
    CloseSidebarAndOpenSearch, // Cerrar sidebar si está abierto y abrir búsqueda flotante
    OpenSidebarAndFocus,
    ShowCreateNoteDialog,
    ToggleFolder(String),
    ShowContextMenu(f64, f64, String, bool), // x, y, nombre, es_carpeta
    DeleteItem(String, bool),                // nombre, es_carpeta
    RenameItem(String, bool),                // nombre, es_carpeta
    OpenInFileManager(String, bool),         // nombre, es_carpeta - Abrir en explorador de archivos
    RefreshSidebar,
    ExpandFolder(String), // Expandir una carpeta específica
    CheckMCPUpdates,      // Nuevo: verificar si MCP modificó notas
    IndexNoteEmbeddings {
        path: String,
        content: String,
    }, // Indexar embeddings de una nota
    MinimizeToTray,       // Minimizar a bandeja del sistema
    ShowWindow,           // Mostrar ventana desde bandeja
    QuitApp,              // Cerrar completamente la aplicación
    // Quick Notes - Ventana flotante
    ToggleQuickNote, // Mostrar/ocultar ventana de quick notes
    NewQuickNote,    // Crear nueva quick note
    ToggleChatMode,  // Alternar entre Modo Agente (con tools) y Chat Normal (sin tools)
    NewChatSession,  // Iniciar nueva sesión de chat explícitamente
    KeyPress {
        key: String,
        modifiers: KeyModifiers,
    },
    ProcessAction(EditorAction),
    SaveCurrentNote,
    AutoSave,
    LoadNote {
        name: String,
        highlight_text: Option<String>, // Texto a resaltar después de cargar
    },
    LoadNoteFromSidebar {
        name: String,
    },
    CreateNewNote(String),
    UpdateCursorPosition(usize),
    GtkInsertText {
        offset: usize,
        text: String,
    },
    GtkDeleteRange {
        start: usize,
        end: usize,
    },
    AddTag(String),
    RemoveTag(String),
    RefreshTags,
    CheckTagCompletion,              // Verificar si hay que mostrar autocompletado
    CompleteTag(String),             // Completar tag seleccionado
    CheckNoteMention,                // Verificar si hay que mostrar autocompletado de @notas
    CompleteMention(String),         // Completar mención de nota
    CompleteChatNote(String),        // Completar mención de nota en chat
    ShowChatNoteSuggestions(String), // Mostrar sugerencias de notas en chat
    HideChatNoteSuggestions,         // Ocultar sugerencias de notas en chat
    SearchNotes(String),             // Buscar notas (mantener para tags y menciones)
    ToggleSemanticSearch(bool),      // Toggle búsqueda semántica
    ToggleSemanticSearchWithNotification, // Toggle con notificación de modo
    ToggleFloatingSearch,            // Toggle de la barra flotante (Ctrl+F) - búsqueda global
    ToggleFloatingSearchInNote,      // Toggle de búsqueda solo en nota actual (Alt+F)
    InNoteSearchNext,                // Ir a la siguiente coincidencia en búsqueda dentro de nota
    InNoteSearchPrev,                // Ir a la anterior coincidencia en búsqueda dentro de nota
    FloatingSearchNotes(String),     // Buscar desde la barra flotante
    PerformFloatingSearch(String),   // Ejecutar búsqueda después del debounce
    ExecuteFloatingSearch(String),   // Ejecutar búsqueda real después de mostrar "Buscando..."
    LoadNoteFromFloatingSearch(String), // Cargar nota desde resultado flotante
    SaveAndSearchTag(String),        // Guardar nota actual y luego buscar tag
    ShowPreferences,
    ShowKeyboardShortcuts,
    ShowAboutDialog,
    ShowMCPServerInfo,
    ChangeLanguage(Language),
    SetStartInBackground(bool), // Nuevo: Configurar inicio en segundo plano
    ReloadConfig,               // Recargar configuración desde disco
    InsertImage,                // Abrir diálogo para seleccionar imagen
    InsertImageFromPath(String), // Insertar imagen desde una ruta
    ProcessPastedText(String),  // Procesar texto pegado (puede ser URL de imagen o YouTube)
    // WebView preview messages
    ToggleTodoLine {
        line: usize,
        checked: bool,
    }, // Toggle TODO checkbox desde WebView preview
    SwitchToInsertAtLine {
        line: usize,
    }, // Cambiar a modo Insert en línea específica desde WebView
    ToggleTodo {
        line_number: usize,
        new_state: bool,
    }, // Marcar/desmarcar TODO
    AskTranscribeYouTube {
        url: String,
        video_id: String,
    }, // Preguntar si transcribir video
    InsertYouTubeLink(String), // Insertar solo el enlace del video
    InsertYouTubeWithTranscript {
        video_id: String,
    }, // Insertar video con transcripción
    UpdateTranscript {
        video_id: String,
        transcript: String,
    }, // Actualizar con transcripción obtenida
    ScrollToAnchor(String),    // Hacer scroll a un heading por su ID (anchor link)
    MoveNoteToFolder {
        note_name: String,
        folder_name: Option<String>,
    }, // Mover nota a carpeta
    ReorderNotes {
        source_name: String,
        target_name: String,
    }, // Reordenar notas (drag & drop)
    MoveFolder {
        folder_name: String,
        target_folder: Option<String>,
    }, // Mover carpeta
    CopyText(String),          // Copiar texto al portapapeles
    CreateNoteFromContent(String), // Crear nueva nota con contenido específico
    // Mensajes del reproductor de música
    ToggleMusicPlayer,                    // Abrir/cerrar el reproductor
    MusicSearch(String),                  // Buscar música en YouTube
    MusicPlay(crate::music_player::Song), // Reproducir una canción
    MusicTogglePlayPause,                 // Pausar/reanudar reproducción
    MusicStop,                            // Detener reproducción
    MusicSeekForward,                     // Avanzar 5 segundos
    MusicSeekBackward,                    // Retroceder 5 segundos
    MusicVolumeUp,                        // Subir volumen
    MusicVolumeDown,                      // Bajar volumen
    MusicUpdateState,                     // Actualizar estado del reproductor
    // Mensajes de playlist
    MusicAddToPlaylist(crate::music_player::Song), // Agregar canción a playlist
    MusicRemoveFromPlaylist(usize),                // Eliminar canción de playlist
    MusicClearPlaylist,                            // Limpiar playlist
    MusicNewPlaylist,                              // Crear nueva playlist vacía
    MusicNextSong,                                 // Siguiente canción
    MusicPreviousSong,                             // Canción anterior
    MusicPlayFromPlaylist(usize),                  // Reproducir canción específica
    MusicToggleRepeat,                             // Cambiar modo de repetición
    MusicToggleShuffle,                            // Activar/desactivar shuffle
    MusicSavePlaylist(String),                     // Guardar playlist con nombre
    MusicLoadPlaylist(String),                     // Cargar playlist guardada
    MusicDeletePlaylist(String),                   // Eliminar playlist guardada
    MusicCheckNextSong,                            // Verificar si debe reproducir siguiente
    TogglePlaylistView,                            // Mostrar/ocultar vista de playlist
    // Mensajes del Chat AI
    EnterChatMode,               // Entrar al modo Chat AI
    ExitChatMode,                // Salir del modo Chat AI
    SendChatMessage(String),     // Enviar mensaje a la IA
    ReceiveChatResponse(String), // Recibir respuesta de la IA
    StartChatStream,             // Iniciar un nuevo mensaje streaming
    ReceiveChatChunk(String),    // Recibir chunk de texto en streaming
    EndChatStream,               // Finalizar mensaje streaming
    // Modo Agente: mostrar pensamiento ReAct
    ShowAgentThought(String),     // Mostrar paso de "Pensamiento" del agente
    ShowAgentAction(String),      // Mostrar qué herramienta está usando
    ShowAgentObservation(String), // Mostrar resultado de la herramienta
    UpdateChatStatus(String), // Actualizar el indicador de estado (ej: "Leyendo nota...", "Pensando...")
    ShowAttachNoteDialog,     // Mostrar diálogo para adjuntar nota
    AttachNoteToContext(String), // Adjuntar nota al contexto
    DetachNoteFromContext(String), // Quitar nota del contexto
    ClearChatContext,         // Limprar contexto
    ClearChatHistory,         // Borrar historial de chat de la BD
    ConfirmClearChatHistory,  // Confirmar borrado (después del diálogo)
    UpdateChatTokenCount,     // Actualizar contador de tokens

    // === Mensajes de Recordatorios ===
    ToggleRemindersPopover,   // Abrir/cerrar popover de recordatorios
    ShowCreateReminderDialog, // Mostrar diálogo para crear recordatorio
    CreateReminder {
        title: String,
        description: Option<String>,
        due_date: chrono::DateTime<chrono::Utc>,
        priority: crate::reminders::Priority,
        repeat_pattern: crate::reminders::RepeatPattern,
    },
    RefreshReminders,      // Refrescar lista de recordatorios
    CompleteReminder(i64), // Marcar recordatorio como completado
    DeleteReminder(i64),   // Eliminar recordatorio
    SnoozeReminder {
        id: i64,
        minutes: i32,
    }, // Posponer recordatorio
    EditReminder(i64),     // Abrir diálogo de edición
    UpdateReminder {
        id: i64,
        title: Option<String>,
        description: Option<String>,
        due_date: Option<chrono::DateTime<chrono::Utc>>,
        priority: Option<crate::reminders::Priority>,
        repeat_pattern: Option<crate::reminders::RepeatPattern>,
    },
    ShowNotification(String), // Mostrar toast de notificación
    ReloadCurrentNoteIfMatching {
        path: String,
    },
    ParseRemindersInNote, // Parsear recordatorios de la nota actual

    // === Mensajes de Búsqueda Semántica con IA ===
    PerformSemanticSearchWithAI {
        query: String,
        results: Vec<SearchResult>,
    }, // Realizar búsqueda semántica con IA
    ShowSemanticSearchAnswer(String), // Mostrar respuesta del agente de IA

    // === Mensajes de Iconos Personalizados ===
    ShowIconPicker {
        name: String,
        is_folder: bool,
    }, // Mostrar selector de iconos
    SetNoteIcon {
        note_name: String,
        icon: Option<String>,
        color: Option<String>,
    }, // Establecer icono de nota
    SetFolderIcon {
        folder_path: String,
        icon: Option<String>,
        color: Option<String>,
    }, // Establecer icono de carpeta
}

#[component(pub)]
impl SimpleComponent for MainApp {
    type Input = AppMsg;
    type Output = ();
    type Init = ThemePreference;

    view! {
        main_window = gtk::ApplicationWindow {
            set_title: Some("NotNative"),
            set_default_width: 920,
            set_default_height: 680,

            add_css_class: "compact",

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 0,

                append = header_bar = &gtk::HeaderBar {
                    pack_start = sidebar_toggle_button = &gtk::Button {
                        set_icon_name: "view-list-symbolic",
                        set_tooltip_text: Some("Mostrar/ocultar lista de notas"),
                        add_css_class: "flat",
                        connect_clicked => AppMsg::ToggleSidebar,
                    },

                    #[wrap(Some)]
                    set_title_widget = window_title = &gtk::Label {
                        set_label: "NotNative",
                    },
                },

                append = split_view = &gtk::Paned {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_position: 0,
                    set_vexpand: true,
                    set_wide_handle: false,
                    set_shrink_start_child: true,
                    set_resize_start_child: false,

                    #[wrap(Some)]
                    set_start_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 0,
                        add_css_class: "sidebar",
                        set_width_request: 200,

                        append = &gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,
                            set_margin_all: 12,

                            append = sidebar_notes_label = &gtk::Label {
                                set_label: "Notas",
                                set_xalign: 0.0,
                                set_hexpand: true,
                                add_css_class: "heading",
                            },

                            append = search_toggle_button = &gtk::Button {
                                set_icon_name: "system-search-symbolic",
                                set_tooltip_text: Some("Buscar (Ctrl+F)"),
                                add_css_class: "flat",
                                add_css_class: "circular",
                                connect_clicked[sender] => move |_| {
                                    // Cerrar sidebar (solo si está abierto, se maneja internamente)
                                    sender.input(AppMsg::CloseSidebarAndOpenSearch);
                                },
                            },

                            append = new_note_button = &gtk::Button {
                                set_icon_name: "list-add-symbolic",
                                set_tooltip_text: Some("Nueva nota"),
                                add_css_class: "flat",
                                add_css_class: "circular",
                                connect_clicked => AppMsg::ShowCreateNoteDialog,
                            },
                        },

                        append = &gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_hexpand: false,
                            set_width_request: 190,
                            set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                            #[wrap(Some)]
                            set_child = notes_list = &gtk::ListBox {
                                add_css_class: "navigation-sidebar",
                                set_hexpand: false,
                                set_width_request: 180,
                                set_selection_mode: gtk::SelectionMode::Single,
                                set_activate_on_single_click: false,
                                set_can_focus: true,
                                set_focus_on_click: true,
                            },
                        },
                    },

                    #[wrap(Some)]
                    set_end_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_hexpand: true,
                        set_vexpand: true,

                        append = &gtk::Overlay {
                            set_hexpand: true,
                            set_vexpand: true,

                            #[wrap(Some)]
                            set_child = content_stack = &gtk::Stack {
                                set_hexpand: true,
                                set_vexpand: true,
                                set_transition_type: gtk::StackTransitionType::Crossfade,
                                set_transition_duration: 200,
                            },

                            add_overlay = floating_search_bar = &gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_halign: gtk::Align::Center,
                                set_valign: gtk::Align::Start,
                                set_margin_top: 16,
                                set_width_request: 600,
                                set_visible: false,
                                add_css_class: "floating-search",
                                add_css_class: "card",

                                append = &gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 8,
                                    set_margin_all: 12,

                                    append = floating_search_entry = &gtk::SearchEntry {
                                        set_placeholder_text: Some("Buscar... (Ctrl: cambiar modo)"),
                                        set_hexpand: true,
                                    },

                                    append = floating_search_mode_label = &gtk::Label {
                                        set_markup: "<small>🔍 Normal</small>",
                                        set_tooltip_text: Some("Ctrl para cambiar modo"),
                                        add_css_class: "dim-label",
                                        set_margin_start: 4,
                                        set_margin_end: 4,
                                    },

                                    append = &gtk::Button {
                                        set_icon_name: "window-close-symbolic",
                                        set_tooltip_text: Some("Cerrar (Esc)"),
                                        add_css_class: "flat",
                                        add_css_class: "circular",
                                        connect_clicked => AppMsg::ToggleFloatingSearch,
                                    },
                                },

                                append = floating_search_results = &gtk::ScrolledWindow {
                                    set_vexpand: true,
                                    set_max_content_height: 400,
                                    set_propagate_natural_height: true,
                                    set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                                    #[wrap(Some)]
                                    set_child = floating_search_results_list = &gtk::ListBox {
                                        add_css_class: "boxed-list",
                                        set_selection_mode: gtk::SelectionMode::Single,
                                    },
                                },
                            },

                            add_overlay = notification_revealer = &gtk::Revealer {
                                set_halign: gtk::Align::Center,
                                set_valign: gtk::Align::End,
                                set_margin_bottom: 80,
                                set_transition_type: gtk::RevealerTransitionType::SlideUp,
                                set_transition_duration: 250,
                                set_reveal_child: false,

                                #[wrap(Some)]
                                set_child = &gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,
                                    set_margin_all: 16,
                                    add_css_class: "app",
                                    add_css_class: "card",
                                    add_css_class: "notification-toast",

                                    append = notification_label = &gtk::Label {
                                        set_wrap: true,
                                        set_wrap_mode: gtk::pango::WrapMode::Word,
                                        set_max_width_chars: 50,
                                        set_justify: gtk::Justification::Center,
                                    },
                                },
                            },
                        },

                        append = status_bar = &gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,
                            set_margin_all: 6,
                            add_css_class: "status-bar",

                            append = mode_label = &gtk::Label {
                                set_markup: "<b>NORMAL</b>",
                                set_xalign: 0.0,
                                add_css_class: "mode-indicator",
                            },

                            append = &gtk::Separator {
                                set_orientation: gtk::Orientation::Vertical,
                                set_margin_start: 4,
                                set_margin_end: 4,
                            },

                            append = tags_menu_button = &gtk::MenuButton {
                                set_icon_name: "tag-symbolic",
                                set_tooltip_text: Some("Tags de la nota"),
                                add_css_class: "flat",
                                add_css_class: "circular",
                                set_valign: gtk::Align::Center,
                                set_direction: gtk::ArrowType::Up,

                                #[wrap(Some)]
                                set_popover = &gtk::Popover {
                                    add_css_class: "tags-popover",
                                    set_autohide: true,
                                    set_size_request: (220, -1),

                                    #[wrap(Some)]
                                    set_child = &gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_spacing: 8,
                                        set_margin_all: 12,
                                        set_width_request: 200,

                                        append = &gtk::Label {
                                            set_markup: "<b>Tags</b>",
                                            set_xalign: 0.0,
                                            set_margin_bottom: 4,
                                        },

                                        append = tags_list_box = &gtk::ListBox {
                                            add_css_class: "tags-list",
                                            set_selection_mode: gtk::SelectionMode::None,
                                        },
                                    },
                                },
                            },

                            append = todos_menu_button = &gtk::MenuButton {
                                set_icon_name: "checkbox-checked-symbolic",
                                set_tooltip_text: Some("TODOs de la nota"),
                                add_css_class: "flat",
                                add_css_class: "circular",
                                set_valign: gtk::Align::Center,
                                set_direction: gtk::ArrowType::Up,

                                #[wrap(Some)]
                                set_popover = &gtk::Popover {
                                    add_css_class: "tags-popover",
                                    set_autohide: true,
                                    set_has_arrow: false,
                                    set_size_request: (320, 360),
                                    set_default_widget: gtk::Widget::NONE,

                                    #[wrap(Some)]
                                    set_child = &gtk::ScrolledWindow {
                                        set_width_request: 320,
                                        set_height_request: 360,
                                        set_max_content_width: 320,
                                        set_max_content_height: 360,
                                        set_min_content_width: 320,
                                        set_min_content_height: 360,
                                        set_propagate_natural_height: false,
                                        set_propagate_natural_width: false,
                                        set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
                                        set_hscrollbar_policy: gtk::PolicyType::Never,
                                        set_vscrollbar_policy: gtk::PolicyType::Automatic,
                                        set_kinetic_scrolling: true,
                                        set_overlay_scrolling: false,
                                        set_hexpand: false,
                                        set_vexpand: false,

                                        #[wrap(Some)]
                                        set_child = &gtk::Box {
                                            set_orientation: gtk::Orientation::Vertical,
                                            set_spacing: 8,
                                            set_margin_all: 12,
                                            set_width_request: 296,
                                            set_hexpand: false,
                                            set_vexpand: false,

                                            append = &gtk::Label {
                                                set_markup: "<b>TODOs</b>",
                                                set_xalign: 0.0,
                                                set_margin_bottom: 4,
                                            },

                                            append = todos_list_box = &gtk::ListBox {
                                                add_css_class: "tags-list",
                                                set_selection_mode: gtk::SelectionMode::None,
                                            },
                                        },
                                    },
                                },
                            },

                            append = &gtk::Label {
                                set_hexpand: true,
                                set_label: "",
                            },

                            append = stats_label = &gtk::Label {
                                set_label: "0 líneas | 0 palabras",
                                set_xalign: 1.0,
                            },

                            append = &gtk::Box {
                                set_spacing: 4,

                                append = &gtk::Separator {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_margin_start: 8,
                                    set_margin_end: 8,
                                },

                                // Reproductor de música
                                append = music_player_button = &gtk::MenuButton {
                                    set_icon_name: "audio-x-generic-symbolic",
                                    set_tooltip_text: Some("Reproductor de música"),
                                    add_css_class: "flat",
                                    add_css_class: "circular",
                                    set_valign: gtk::Align::Center,
                                    set_direction: gtk::ArrowType::Up,
                                },

                                // Recordatorios
                                append = reminders_button = &gtk::MenuButton {
                                    set_icon_name: "alarm-symbolic",
                                    set_tooltip_text: Some("Recordatorios (Alt+R)"),
                                    add_css_class: "flat",
                                    add_css_class: "circular",
                                    set_valign: gtk::Align::Center,
                                    set_direction: gtk::ArrowType::Up,
                                },

                                // TODO: Botón 8BIT desactivado temporalmente
                                // append = bit8_button = &gtk::ToggleButton {
                                //     set_label: "8BIT",
                                //     set_tooltip_text: Some("Modo retro 8-bit"),
                                //     add_css_class: "flat",
                                //     connect_toggled[sender] , move |btn| {
                                //         if btn.is_active() {
                                //             sender.input(AppMsg::Toggle8BitMode);
                                //         } else {
                                //             sender.input(AppMsg::Toggle8BitMode);
                                //         }
                                //     },
                                // },

                                append = settings_button = &gtk::MenuButton {
                                    set_icon_name: "emblem-system-symbolic",
                                    set_tooltip_text: Some("Ajustes"),
                                    add_css_class: "flat",
                                    set_direction: gtk::ArrowType::Up,
                                    // El popover se creará dinámicamente después
                                },
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        theme: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        // Crear el TextView manualmente (necesario porque el de la macro no se puede usar)
        let text_view_actual = gtk::TextView::builder()
            .monospace(true)
            .wrap_mode(gtk::WrapMode::WordChar)
            .editable(true)
            .cursor_visible(true)
            .accepts_tab(false)
            .left_margin(24)
            .right_margin(24)
            .top_margin(24)
            .bottom_margin(24)
            .hexpand(true)
            .vexpand(true)
            .build();

        // Contenedor centrado para el TextView (consistente con el modo Normal)
        let editor_center_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        editor_center_box.set_hexpand(true);
        editor_center_box.set_vexpand(true);

        // Spacer izquierdo flexible
        let left_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        left_spacer.set_hexpand(true);
        editor_center_box.append(&left_spacer);

        // Contenedor del TextView con ancho máximo (igual que max-width: 900px del CSS)
        let editor_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        editor_container.set_width_request(900);
        editor_container.set_hexpand(false);
        editor_container.set_vexpand(true);
        editor_container.append(&text_view_actual);
        editor_center_box.append(&editor_container);

        // Spacer derecho flexible
        let right_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        right_spacer.set_hexpand(true);
        editor_center_box.append(&right_spacer);

        // Crear Stack para alternar entre editor (TextView) y preview (WebView)
        let editor_stack = gtk::Stack::new();
        editor_stack.set_hexpand(true);
        editor_stack.set_vexpand(true);
        editor_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        editor_stack.set_transition_duration(150);

        // Agregar el editor (TextView) al Stack interno
        let editor_scroll = gtk::ScrolledWindow::new();
        editor_scroll.set_hexpand(true);
        editor_scroll.set_vexpand(true);
        editor_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        editor_scroll.set_child(Some(&editor_center_box));
        editor_stack.add_named(&editor_scroll, Some("editor"));

        // Crear WebView para preview HTML en modo Normal
        use webkit6::prelude::WebViewExt;
        let preview_webview = webkit6::WebView::new();
        preview_webview.set_hexpand(true);
        preview_webview.set_vexpand(true);
        preview_webview.set_can_focus(true); // Permitir que reciba foco para keybindings
        preview_webview.set_focusable(true);

        // Configurar settings del WebView para preview
        if let Some(settings) = WebViewExt::settings(&preview_webview) {
            settings.set_enable_javascript(true);
            settings.set_enable_developer_extras(false);
            settings.set_javascript_can_access_clipboard(false);
            settings.set_allow_universal_access_from_file_urls(false);
            settings.set_allow_file_access_from_file_urls(true); // Para imágenes locales
            // Deshabilitar funciones innecesarias para preview
            settings.set_enable_media(false);
            settings.set_enable_webaudio(false);
            settings.set_enable_webgl(false);
        }

        // Configurar UserContentManager para recibir mensajes JS→Rust
        if let Some(content_manager) = preview_webview.user_content_manager() {
            // Registrar handler para mensajes desde JavaScript
            content_manager.register_script_message_handler("notnative", None);
        }

        // Scroll para el WebView
        let preview_scroll = gtk::ScrolledWindow::new();
        preview_scroll.set_hexpand(true);
        preview_scroll.set_vexpand(true);
        preview_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        preview_scroll.set_child(Some(&preview_webview));
        editor_stack.add_named(&preview_scroll, Some("preview"));

        // Por defecto mostrar el preview (modo Normal)
        editor_stack.set_visible_child_name("preview");

        // Agregar el Stack interno al Stack principal de contenido
        widgets
            .content_stack
            .add_named(&editor_stack, Some("editor"));
        widgets.content_stack.set_visible_child_name("editor");

        let text_buffer = text_view_actual.buffer();
        let mode = Rc::new(RefCell::new(EditorMode::Normal));

        // Inicializar directorio de notas (por defecto ~/.local/share/notnative/notes)
        let notes_dir = NotesDirectory::default();

        // Inicializar base de datos
        let db_path = notes_dir.db_path();
        let notes_db = NotesDatabase::new(&db_path).expect("No se pudo crear la base de datos");

        // Cargar configuración (necesario antes de crear MCP para tener idioma)
        let config_path = NotesConfig::default_path();
        let notes_config = Rc::new(RefCell::new(
            NotesConfig::load(&config_path).unwrap_or_else(|_| {
                println!("No se pudo cargar configuración, creando una nueva");
                NotesConfig::new()
            }),
        ));

        // Determinar idioma: usar configuración guardada o detectar del sistema
        let language = if let Some(lang_code) = notes_config.borrow().get_language() {
            Language::from_code(lang_code)
        } else {
            Language::from_env()
        };

        let i18n = Rc::new(RefCell::new(I18n::new(language)));
        println!("Idioma detectado: {:?}", language);

        // Inicializar sistema MCP (Model Context Protocol)
        // Crear wrapper Rc<RefCell> para NotesDatabase (necesario para compartir en async)
        let notes_db_rc = Rc::new(RefCell::new(notes_db.clone_connection()));
        let mcp_executor = Rc::new(RefCell::new(crate::mcp::MCPToolExecutor::new(
            notes_dir.clone(),
            notes_db_rc,
            notes_config.clone(),
            i18n.clone(),
        )));
        // Cargar TODAS las herramientas MCP disponibles
        let mcp_registry = crate::mcp::MCPToolRegistry::new();
        println!(
            "Sistema MCP inicializado con {} herramientas",
            mcp_registry.get_tools().len()
        );

        // Iniciar servidor MCP en segundo plano
        let notes_dir_for_server = notes_dir.clone();
        let notes_db_for_server =
            std::sync::Arc::new(std::sync::Mutex::new(notes_db.clone_connection()));
        let notes_config_for_server =
            std::sync::Arc::new(std::sync::Mutex::new(notes_config.borrow().clone()));
        let i18n_for_server = std::sync::Arc::new(std::sync::Mutex::new(i18n.borrow().clone()));

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("No se pudo crear runtime de Tokio");
            rt.block_on(async {
                if let Err(e) = crate::mcp::start_mcp_server(
                    notes_dir_for_server,
                    notes_db_for_server,
                    notes_config_for_server,
                    i18n_for_server,
                )
                .await
                {
                    eprintln!("❌ Error iniciando servidor MCP: {}", e);
                }
            });
        });

        // Indexar todas las notas existentes
        println!("Indexando notas existentes...");
        let mut total_tags = 0;
        if let Ok(notes) = notes_dir.list_notes() {
            for note in &notes {
                if let Ok(content) = note.read() {
                    let folder = notes_dir.relative_folder(note.path());

                    // Indexar la nota
                    if let Ok(note_id) = notes_db.index_note(
                        note.name(),
                        note.path().to_str().unwrap_or(""),
                        &content,
                        folder.as_deref(),
                    ) {
                        // Extraer y almacenar tags (frontmatter + inline #tags)
                        let tags = extract_all_tags(&content);
                        for tag in tags {
                            if let Ok(()) = notes_db.add_tag(note_id, &tag) {
                                total_tags += 1;
                            }
                        }
                    }
                }
            }
            println!("✓ {} notas indexadas con {} tags", notes.len(), total_tags);
        }

        // Crear menú contextual para el sidebar (sin parent inicialmente)
        // Se creará dinámicamente con las traducciones cuando se necesite
        let context_menu = gtk::PopoverMenu::from_model(None::<&gtk::gio::Menu>);
        context_menu.set_has_arrow(false);
        context_menu.add_css_class("context-menu");

        // Helper para crear la nota de keybindings y marcar onboarding como completo
        fn create_keybindings_note(
            notes_dir: &NotesDirectory,
            notes_config: &Rc<RefCell<NotesConfig>>,
            note_name: &str,
        ) {
            let keybindings_content = include_str!("../docs/KEYBINDINGS.md");

            // Crear en carpeta Notnative
            let full_name = format!("Notnative/{}", note_name);

            match notes_dir.create_note(&full_name, keybindings_content) {
                Ok(_) => {
                    println!("✅ Nota '{}' creada", full_name);
                }
                Err(e) => {
                    // Si ya existe una nota con ese nombre, no es error
                    if e.to_string().contains("existe") || e.to_string().contains("exists") {
                        println!("ℹ️ Nota '{}' ya existía", full_name);
                    } else {
                        eprintln!("⚠️ Error creando nota de atajos: {}", e);
                    }
                }
            }

            // Marcar onboarding como completado y guardar config
            {
                let mut config = notes_config.borrow_mut();
                config.set_onboarding_completed(true);
                if let Err(e) = config.save(NotesConfig::default_path()) {
                    eprintln!("⚠️ Error guardando config: {}", e);
                } else {
                    println!("✅ Onboarding marcado como completado");
                }
            }
        }

        // Intentar cargar la última nota abierta, si no la de bienvenida, o crearla si no existe
        let (initial_buffer, current_note) = {
            // Nombre único para la nota de atajos de NotNative
            const KEYBINDINGS_NOTE_NAME: &str = "NotNative_Atajos_de_Teclado";

            // Obtener valores antes para evitar RefCell borrow conflicts
            let last_note = notes_config
                .borrow()
                .get_last_opened_note()
                .map(|s| s.to_string());
            let onboarding_completed = notes_config.borrow().is_onboarding_completed();

            // Primero intentar cargar la última nota abierta
            if let Some(last_note) = last_note {
                match notes_dir.find_note(&last_note) {
                    Ok(Some(note)) => match note.read() {
                        Ok(content) => {
                            println!("Última nota abierta cargada: {}", last_note);

                            // Verificar si necesitamos crear la nota de onboarding
                            if !onboarding_completed {
                                create_keybindings_note(
                                    &notes_dir,
                                    &notes_config,
                                    KEYBINDINGS_NOTE_NAME,
                                );
                            }

                            (NoteBuffer::from_text(&content), Some(note))
                        }
                        Err(_) => {
                            // Si no se puede leer, intentar con bienvenida
                            try_load_or_create_welcome(
                                &notes_dir,
                                &notes_config,
                                onboarding_completed,
                            )
                        }
                    },
                    _ => {
                        // Si la última nota no existe, intentar con bienvenida
                        try_load_or_create_welcome(&notes_dir, &notes_config, onboarding_completed)
                    }
                }
            } else {
                // No hay última nota guardada, intentar con bienvenida
                try_load_or_create_welcome(&notes_dir, &notes_config, onboarding_completed)
            }
        };

        // Helper function para cargar o crear bienvenida
        fn try_load_or_create_welcome(
            notes_dir: &NotesDirectory,
            notes_config: &Rc<RefCell<NotesConfig>>,
            onboarding_completed: bool,
        ) -> (NoteBuffer, Option<NoteFile>) {
            // Nombre único para la nota de atajos de NotNative
            const KEYBINDINGS_NOTE_NAME: &str = "NotNative_Atajos_de_Teclado";

            match notes_dir.find_note("bienvenida") {
                Ok(Some(note)) => match note.read() {
                    Ok(content) => {
                        println!("Nota 'bienvenida' cargada");

                        // Si el onboarding no está completo, crear nota de atajos
                        if !onboarding_completed {
                            create_keybindings_note(notes_dir, notes_config, KEYBINDINGS_NOTE_NAME);
                        }

                        (NoteBuffer::from_text(&content), Some(note))
                    }
                    Err(_) => (NoteBuffer::new(), None),
                },
                _ => {
                    // Solo crear la nota de bienvenida si es primera vez (ninguna otra nota existe)
                    match notes_dir.list_notes() {
                        Ok(notes) if notes.is_empty() => {
                            // Primera vez usando la app
                            let welcome_content = r#"# Bienvenido a NotNative

Esta es tu primera nota. NotNative guarda cada nota como un archivo .md independiente.

## Comandos básicos

- `i` → Modo INSERT (editar)
- `Esc` → Modo NORMAL
- `h/j/k/l` → Navegar (izquierda/abajo/arriba/derecha)
- `x` → Eliminar carácter
- `u` → Deshacer
- `Ctrl+S` → Guardar

Las notas se guardan automáticamente en: `~/.local/share/notnative/notes/`

## 📝 Quick Notes

Puedes abrir una ventana flotante de notas rápidas desde **cualquier aplicación** (incluso juegos fullscreen).

👉 **Lee la nota @NotNative_Atajos_de_Teclado para configurar los atajos de teclado globales.**
"#;
                            // Crear nota de bienvenida
                            let result = match notes_dir.create_note("bienvenida", welcome_content)
                            {
                                Ok(note) => {
                                    println!("Nota de bienvenida creada");
                                    (NoteBuffer::from_text(welcome_content), Some(note))
                                }
                                Err(_) => (NoteBuffer::new(), None),
                            };

                            // Crear nota de keybindings (primera vez)
                            create_keybindings_note(notes_dir, notes_config, KEYBINDINGS_NOTE_NAME);

                            result
                        }
                        _ => {
                            // Ya hay otras notas
                            // Si el onboarding no está completo, crear nota de atajos
                            if !onboarding_completed {
                                create_keybindings_note(
                                    notes_dir,
                                    notes_config,
                                    KEYBINDINGS_NOTE_NAME,
                                );
                            }

                            println!(
                                "Nota de bienvenida no existe y hay otras notas, iniciando vacío"
                            );
                            (NoteBuffer::new(), None)
                        }
                    }
                }
            }
        }

        // Crear popover de autocompletado de tags ANTES del modelo
        let completion_list_box = gtk::ListBox::new();
        completion_list_box.set_selection_mode(gtk::SelectionMode::None);
        completion_list_box.add_css_class("tag-suggestions");

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_has_frame(false);
        scrolled.set_child(Some(&completion_list_box));
        scrolled.set_max_content_height(150);
        scrolled.set_min_content_width(180);
        scrolled.set_propagate_natural_height(true);
        scrolled.set_propagate_natural_width(true);

        let completion_popover = gtk::Popover::new();
        completion_popover.set_parent(&text_view_actual);
        completion_popover.add_css_class("tag-completion");
        completion_popover.set_autohide(false);
        completion_popover.set_size_request(200, 160); // Tamaño fijo para evitar recalculos

        // CRÍTICO: Mostrar explícitamente el contenido antes de agregarlo
        scrolled.show();
        completion_list_box.show();

        completion_popover.set_child(Some(&scrolled));

        // Crear popover de autocompletado de menciones @ para backlinks
        let mention_list_box = gtk::ListBox::new();
        mention_list_box.set_selection_mode(gtk::SelectionMode::None);
        mention_list_box.add_css_class("mention-suggestions");
        mention_list_box.set_vexpand(true);
        mention_list_box.set_hexpand(true);

        let mention_scrolled = gtk::ScrolledWindow::new();
        mention_scrolled.set_has_frame(false);
        mention_scrolled.set_child(Some(&mention_list_box));
        mention_scrolled.set_min_content_height(250);
        mention_scrolled.set_min_content_width(300);
        mention_scrolled.set_max_content_height(400);
        mention_scrolled.set_propagate_natural_height(false);
        mention_scrolled.set_propagate_natural_width(false);
        mention_scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        mention_scrolled.set_vexpand(true);
        mention_scrolled.set_hexpand(true);

        let mention_popover = gtk::Popover::new();
        mention_popover.set_parent(&text_view_actual);
        mention_popover.add_css_class("mention-completion");
        mention_popover.set_autohide(false);
        mention_popover.set_size_request(300, 250);
        mention_popover.set_has_arrow(false);

        // CRÍTICO: Mostrar explícitamente el contenido antes de agregarlo
        mention_scrolled.show();
        mention_list_box.show();

        mention_popover.set_child(Some(&mention_scrolled));

        // Reproductor de música (se inicializará bajo demanda)
        let music_player: Rc<RefCell<Option<Rc<crate::music_player::MusicPlayer>>>> =
            Rc::new(RefCell::new(None));

        // Crear popover del reproductor de música
        let music_search_entry = gtk::SearchEntry::new();
        music_search_entry.set_placeholder_text(Some(&i18n.borrow().t("music_search_placeholder")));
        music_search_entry.set_hexpand(true);

        let music_results_list = gtk::ListBox::new();
        music_results_list.set_selection_mode(gtk::SelectionMode::None);
        music_results_list.add_css_class("music-results");

        let music_results_scroll = gtk::ScrolledWindow::new();
        music_results_scroll.set_child(Some(&music_results_list));
        music_results_scroll.set_min_content_height(200);
        music_results_scroll.set_max_content_height(300);
        music_results_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

        let music_now_playing_label = gtk::Label::new(Some(&i18n.borrow().t("no_music_playing")));
        music_now_playing_label.set_xalign(0.0);
        music_now_playing_label.set_wrap(false);
        music_now_playing_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        music_now_playing_label.set_max_width_chars(40);
        music_now_playing_label.set_width_chars(40);
        music_now_playing_label.add_css_class("music-title");

        // Tooltip para mostrar el título completo
        music_now_playing_label.set_has_tooltip(true);

        let music_state_label = gtk::Label::new(Some("●"));
        music_state_label.set_xalign(0.5);
        music_state_label.add_css_class("music-state-idle");

        let music_play_pause_btn = gtk::Button::new();
        music_play_pause_btn.set_icon_name("media-playback-start-symbolic");
        music_play_pause_btn.set_tooltip_text(Some(&i18n.borrow().t("music_play_pause")));
        music_play_pause_btn.add_css_class("flat");
        music_play_pause_btn.add_css_class("circular");

        let music_stop_btn = gtk::Button::new();
        music_stop_btn.set_icon_name("media-playback-stop-symbolic");
        music_stop_btn.set_tooltip_text(Some(&i18n.borrow().t("music_stop")));
        music_stop_btn.add_css_class("flat");
        music_stop_btn.add_css_class("circular");

        let music_back_btn = gtk::Button::new();
        music_back_btn.set_icon_name("media-seek-backward-symbolic");
        music_back_btn.set_tooltip_text(Some(&i18n.borrow().t("music_seek_back")));
        music_back_btn.add_css_class("flat");
        music_back_btn.add_css_class("circular");

        let music_forward_btn = gtk::Button::new();
        music_forward_btn.set_icon_name("media-seek-forward-symbolic");
        music_forward_btn.set_tooltip_text(Some(&i18n.borrow().t("music_seek_forward")));
        music_forward_btn.add_css_class("flat");
        music_forward_btn.add_css_class("circular");

        let music_vol_down_btn = gtk::Button::new();
        music_vol_down_btn.set_icon_name("audio-volume-low-symbolic");
        music_vol_down_btn.set_tooltip_text(Some(&i18n.borrow().t("music_volume_down")));
        music_vol_down_btn.add_css_class("flat");
        music_vol_down_btn.add_css_class("circular");

        let music_vol_up_btn = gtk::Button::new();
        music_vol_up_btn.set_icon_name("audio-volume-high-symbolic");
        music_vol_up_btn.set_tooltip_text(Some(&i18n.borrow().t("music_volume_up")));
        music_vol_up_btn.add_css_class("flat");
        music_vol_up_btn.add_css_class("circular");

        // Botones de playlist
        let music_prev_btn = gtk::Button::new();
        music_prev_btn.set_icon_name("media-skip-backward-symbolic");
        music_prev_btn.set_tooltip_text(Some(&i18n.borrow().t("music_previous_song")));
        music_prev_btn.add_css_class("flat");
        music_prev_btn.add_css_class("circular");

        let music_next_btn = gtk::Button::new();
        music_next_btn.set_icon_name("media-skip-forward-symbolic");
        music_next_btn.set_tooltip_text(Some(&i18n.borrow().t("music_next_song")));
        music_next_btn.add_css_class("flat");
        music_next_btn.add_css_class("circular");

        let music_repeat_btn = gtk::Button::new();
        music_repeat_btn.set_icon_name("media-playlist-repeat-symbolic");
        music_repeat_btn.set_tooltip_text(Some(&i18n.borrow().t("music_repeat_off")));
        music_repeat_btn.add_css_class("flat");
        music_repeat_btn.add_css_class("circular");

        let music_shuffle_btn = gtk::Button::new();
        music_shuffle_btn.set_icon_name("media-playlist-shuffle-symbolic");
        music_shuffle_btn.set_tooltip_text(Some(&i18n.borrow().t("music_shuffle_off")));
        music_shuffle_btn.add_css_class("flat");
        music_shuffle_btn.add_css_class("circular");

        // Caja de controles de reproducción básicos
        let music_playback_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        music_playback_box.set_halign(gtk::Align::Center);
        music_playback_box.append(&music_prev_btn);
        music_playback_box.append(&music_back_btn);
        music_playback_box.append(&music_play_pause_btn);
        music_playback_box.append(&music_forward_btn);
        music_playback_box.append(&music_next_btn);
        music_playback_box.append(&music_stop_btn);

        // Botón para abrir gestor de playlists (MenuButton)
        let music_playlist_btn = gtk::MenuButton::new();
        music_playlist_btn.set_icon_name("view-list-symbolic");
        music_playlist_btn.set_tooltip_text(Some(&i18n.borrow().t("music_manage_playlists")));
        music_playlist_btn.add_css_class("flat");
        music_playlist_btn.add_css_class("circular");

        // Caja de controles de volumen y modos
        let music_options_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        music_options_box.set_halign(gtk::Align::Center);
        music_options_box.append(&music_vol_down_btn);
        music_options_box.append(&music_vol_up_btn);
        music_options_box.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        music_options_box.append(&music_repeat_btn);
        music_options_box.append(&music_shuffle_btn);
        music_options_box.append(&gtk::Separator::new(gtk::Orientation::Vertical));
        music_options_box.append(&music_playlist_btn);

        let music_controls_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        music_controls_box.append(&music_playback_box);
        music_controls_box.append(&music_options_box);

        let music_status_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        music_status_box.set_margin_bottom(8);
        music_status_box.append(&music_state_label);
        music_status_box.append(&music_now_playing_label);

        let music_player_content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        music_player_content.set_margin_all(12);
        music_player_content.set_width_request(350);

        // Crear label dinámico para el título (se actualizará con traducciones)
        let music_player_title = gtk::Label::builder()
            .label(&format!("<b>{}</b>", i18n.borrow().t("music_player_title")))
            .use_markup(true)
            .xalign(0.0)
            .build();

        music_player_content.append(&music_player_title);
        music_player_content.append(&music_search_entry);
        music_player_content.append(&music_results_scroll);
        music_player_content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        music_player_content.append(&music_status_box);
        music_player_content.append(&music_controls_box);

        let music_player_popover = gtk::Popover::new();
        music_player_popover.set_child(Some(&music_player_content));
        music_player_popover.add_css_class("tags-popover");
        music_player_popover.set_autohide(true);
        music_player_popover.set_has_arrow(false);

        widgets
            .music_player_button
            .set_popover(Some(&music_player_popover));

        // ========== POPOVER DE GESTIÓN DE PLAYLISTS ==========

        // Lista de canciones en la cola actual
        let playlist_current_list = gtk::ListBox::new();
        playlist_current_list.set_selection_mode(gtk::SelectionMode::None);
        playlist_current_list.add_css_class("playlist-songs");

        let playlist_current_scroll = gtk::ScrolledWindow::new();
        playlist_current_scroll.set_child(Some(&playlist_current_list));
        playlist_current_scroll.set_min_content_height(150);
        playlist_current_scroll.set_max_content_height(250);
        playlist_current_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

        // Botones para gestionar la cola actual
        let playlist_new_btn = gtk::Button::builder().label("� Nueva").build();
        playlist_new_btn.add_css_class("flat");

        let playlist_save_btn = gtk::Button::builder().label("💾 Guardar").build();
        playlist_save_btn.add_css_class("flat");

        let playlist_clear_btn = gtk::Button::builder().label("🗑️ Limpiar").build();
        playlist_clear_btn.add_css_class("flat");

        let playlist_current_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        playlist_current_buttons.set_halign(gtk::Align::Center);
        playlist_current_buttons.append(&playlist_new_btn);
        playlist_current_buttons.append(&playlist_save_btn);
        playlist_current_buttons.append(&playlist_clear_btn);

        // Lista de playlists guardadas
        let playlist_saved_list = gtk::ListBox::new();
        playlist_saved_list.set_selection_mode(gtk::SelectionMode::None);
        playlist_saved_list.add_css_class("playlist-saved");

        let playlist_saved_scroll = gtk::ScrolledWindow::new();
        playlist_saved_scroll.set_child(Some(&playlist_saved_list));
        playlist_saved_scroll.set_min_content_height(100);
        playlist_saved_scroll.set_max_content_height(200);
        playlist_saved_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

        // Contenido del popover de playlists
        let playlist_manager_content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        playlist_manager_content.set_margin_all(12);
        playlist_manager_content.set_width_request(350);

        playlist_manager_content.append(
            &gtk::Label::builder()
                .label("<b>Cola de reproducción</b>")
                .use_markup(true)
                .xalign(0.0)
                .build(),
        );
        playlist_manager_content.append(&playlist_current_scroll);
        playlist_manager_content.append(&playlist_current_buttons);
        playlist_manager_content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        playlist_manager_content.append(
            &gtk::Label::builder()
                .label("<b>Playlists guardadas</b>")
                .use_markup(true)
                .xalign(0.0)
                .build(),
        );
        playlist_manager_content.append(&playlist_saved_scroll);

        let playlist_manager_popover = gtk::Popover::new();
        playlist_manager_popover.set_child(Some(&playlist_manager_content));
        playlist_manager_popover.add_css_class("tags-popover");
        playlist_manager_popover.set_autohide(true);
        playlist_manager_popover.set_has_arrow(false);

        music_playlist_btn.set_popover(Some(&playlist_manager_popover));

        // Conectar evento cuando se muestra el popover de playlists
        playlist_manager_popover.connect_show(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                // Actualizar listas cuando se abre el popover
                sender.input(AppMsg::TogglePlaylistView);
            }
        ));

        // Conectar eventos del reproductor
        music_play_pause_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::MusicTogglePlayPause);
            }
        ));

        music_stop_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::MusicStop);
            }
        ));

        music_back_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::MusicSeekBackward);
            }
        ));

        music_forward_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::MusicSeekForward);
            }
        ));

        music_vol_down_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::MusicVolumeDown);
            }
        ));

        music_vol_up_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::MusicVolumeUp);
            }
        ));

        // Conectar botones de playlist
        music_prev_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::MusicPreviousSong);
            }
        ));

        music_next_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::MusicNextSong);
            }
        ));

        music_repeat_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::MusicToggleRepeat);
            }
        ));

        music_shuffle_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::MusicToggleShuffle);
            }
        ));

        // Actualizar listas cuando se abre el popover
        playlist_manager_popover.connect_show(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::TogglePlaylistView);
            }
        ));

        // Cerrar popover principal cuando se cierra el de playlists para evitar que se quede atascado
        let music_player_popover_for_close = music_player_popover.clone();
        playlist_manager_popover.connect_closed(gtk::glib::clone!(move |_| {
            // Cerrar también el popover principal
            music_player_popover_for_close.popdown();
        }));

        // Conectar botón de nueva playlist
        playlist_new_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::MusicNewPlaylist);
            }
        ));

        // Conectar botón de guardar playlist
        let music_player_clone = music_player.clone();
        playlist_save_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            music_player_clone,
            move |_| {
                // Verificar si la playlist actual tiene nombre (y no es "Cola de reproducción")
                let should_ask_name = if let Some(player) = music_player_clone.borrow().as_ref() {
                    if let Some(playlist) = player.current_playlist() {
                        playlist.name == "Cola de reproducción" || playlist.name.is_empty()
                    } else {
                        true
                    }
                } else {
                    true
                };

                if should_ask_name {
                    // Mostrar diálogo para pedir nombre
                    let dialog = gtk::Window::builder()
                        .title("Guardar Playlist")
                        .modal(true)
                        .default_width(300)
                        .default_height(150)
                        .build();

                    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
                    content.set_margin_all(12);

                    content.append(
                        &gtk::Label::builder()
                            .label("Nombre de la playlist:")
                            .xalign(0.0)
                            .build(),
                    );

                    let entry = gtk::Entry::new();
                    entry.set_placeholder_text(Some("ej: Música relajante"));
                    content.append(&entry);

                    let buttons_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    buttons_box.set_halign(gtk::Align::End);

                    let cancel_btn = gtk::Button::builder().label("Cancelar").build();
                    let save_btn = gtk::Button::builder().label("Guardar").build();
                    save_btn.add_css_class("suggested-action");

                    buttons_box.append(&cancel_btn);
                    buttons_box.append(&save_btn);
                    content.append(&buttons_box);

                    dialog.set_child(Some(&content));

                    cancel_btn.connect_clicked(gtk::glib::clone!(
                        #[weak]
                        dialog,
                        move |_| {
                            dialog.close();
                        }
                    ));

                    save_btn.connect_clicked(gtk::glib::clone!(
                        #[weak]
                        dialog,
                        #[weak]
                        entry,
                        #[strong]
                        sender,
                        move |_| {
                            let name = entry.text().to_string();
                            if !name.is_empty() {
                                sender.input(AppMsg::MusicSavePlaylist(name));
                                dialog.close();
                            }
                        }
                    ));

                    dialog.present();
                } else {
                    // Ya tiene nombre, guardar directamente
                    if let Some(player) = music_player_clone.borrow().as_ref() {
                        if let Some(playlist) = player.current_playlist() {
                            println!(
                                "💾 Guardando playlist '{}' automáticamente...",
                                playlist.name
                            );
                            sender.input(AppMsg::MusicSavePlaylist(playlist.name.clone()));
                        }
                    }
                }
            }
        ));

        // Conectar botón de limpiar cola
        playlist_clear_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::MusicClearPlaylist);
            }
        ));

        // Conectar búsqueda de música
        music_search_entry.connect_search_changed(gtk::glib::clone!(
            #[strong]
            sender,
            move |entry| {
                let query = entry.text().to_string();
                if !query.is_empty() {
                    sender.input(AppMsg::MusicSearch(query));
                }
            }
        ));

        // ==================== RECORDATORIOS ====================

        // Inicializar sistema de recordatorios
        let reminder_db = crate::reminders::ReminderDatabase::new(
            rusqlite::Connection::open(notes_db.path().clone())
                .expect("No se pudo abrir BD para recordatorios"),
        );

        // Asegurar que existe la tabla
        if let Err(e) = reminder_db.ensure_schema() {
            eprintln!("⚠️ Error creando esquema de recordatorios: {}", e);
        }

        let reminder_db = std::sync::Arc::new(std::sync::Mutex::new(reminder_db));
        let i18n_for_notifier = std::sync::Arc::new(std::sync::Mutex::new(i18n.borrow().clone()));
        let reminder_notifier =
            std::sync::Arc::new(crate::reminders::ReminderNotifier::new(i18n_for_notifier));
        let reminder_scheduler = std::sync::Arc::new(crate::reminders::ReminderScheduler::new(
            reminder_db.clone(),
            reminder_notifier.clone(),
        ));
        let reminder_parser = crate::reminders::ReminderParser::new();

        // Iniciar scheduler
        reminder_scheduler.start();

        // Lista de recordatorios
        let reminders_list = gtk::ListBox::new();
        reminders_list.set_selection_mode(gtk::SelectionMode::None);
        reminders_list.add_css_class("reminders-list");

        let reminders_scroll = gtk::ScrolledWindow::new();
        reminders_scroll.set_child(Some(&reminders_list));
        reminders_scroll.set_min_content_height(200);
        reminders_scroll.set_max_content_height(400);
        reminders_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

        // Badge con contador de pendientes
        let reminders_pending_badge = gtk::Label::new(Some("0"));
        reminders_pending_badge.add_css_class("reminders-badge");
        reminders_pending_badge.set_visible(false);

        // Botones de acciones
        let reminders_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        reminders_header.set_margin_all(12);

        let reminders_title = gtk::Label::builder()
            .label(&format!("<b>{}</b>", i18n.borrow().t("reminders_title")))
            .use_markup(true)
            .xalign(0.0)
            .hexpand(true)
            .build();
        reminders_header.append(&reminders_title);

        let reminders_new_btn = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text(&i18n.borrow().t("reminders_new"))
            .build();
        reminders_new_btn.add_css_class("flat");
        reminders_new_btn.add_css_class("circular");
        reminders_new_btn.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ShowCreateReminderDialog);
            }
        ));
        reminders_header.append(&reminders_new_btn);

        // Contenido del popover
        let reminders_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        reminders_content.set_width_request(350);
        reminders_content.append(&reminders_header);
        reminders_content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        reminders_content.append(&reminders_scroll);

        let reminders_popover = gtk::Popover::new();
        reminders_popover.set_child(Some(&reminders_content));
        reminders_popover.add_css_class("tags-popover");
        reminders_popover.set_autohide(true);
        reminders_popover.set_has_arrow(false);

        widgets
            .reminders_button
            .set_popover(Some(&reminders_popover));

        // Conectar evento de apertura para refrescar
        reminders_popover.connect_show(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::RefreshReminders);
            }
        ));

        println!("✅ Sistema de recordatorios inicializado");

        // ==================== CHAT AI ====================

        // Contenedor principal del chat
        let chat_ai_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        chat_ai_container.set_vexpand(true);
        chat_ai_container.set_hexpand(true);
        chat_ai_container.add_css_class("chat-ai-root");

        // Header con información del modelo
        let chat_header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        chat_header.set_margin_all(16);
        chat_header.add_css_class("chat-ai-header");

        let chat_header_icon = gtk::Label::new(Some("🤖"));
        chat_header_icon.add_css_class("chat-header-icon");
        chat_header.append(&chat_header_icon);

        let chat_header_content = gtk::Box::new(gtk::Orientation::Vertical, 4);
        chat_header_content.add_css_class("chat-header-content");

        let chat_model_label = gtk::Label::new(Some(&i18n.borrow().t("chat_model_default")));
        chat_model_label.add_css_class("chat-model-label");
        chat_model_label.add_css_class("chat-header-title");
        chat_model_label.set_xalign(0.0);
        chat_header_content.append(&chat_model_label);

        let chat_header_subtitle = gtk::Label::new(Some(&i18n.borrow().t("chat_subtitle")));
        chat_header_subtitle.add_css_class("chat-header-subtitle");
        chat_header_subtitle.set_xalign(0.0);
        chat_header_content.append(&chat_header_subtitle);

        chat_header.append(&chat_header_content);

        let chat_header_right = gtk::Box::new(gtk::Orientation::Vertical, 6);
        chat_header_right.add_css_class("chat-header-right");
        chat_header_right.set_hexpand(true);
        chat_header_right.set_halign(gtk::Align::End);

        let chat_tokens_progress = gtk::ProgressBar::new();
        chat_tokens_progress.add_css_class("chat-token-progress");
        chat_tokens_progress.add_css_class("chat-tokens-progress");
        chat_tokens_progress.set_hexpand(false);
        chat_tokens_progress.set_valign(gtk::Align::Center);
        chat_tokens_progress.set_text(Some("Tokens: 0 / 4096"));
        chat_tokens_progress.set_show_text(true);
        chat_tokens_progress.set_width_request(220);
        chat_header_right.append(&chat_tokens_progress);

        chat_header.append(&chat_header_right);
        chat_ai_container.append(&chat_header);

        // Split view principal del chat
        let chat_split_view = gtk::Paned::new(gtk::Orientation::Horizontal);
        chat_split_view.set_position(250);
        chat_split_view.set_vexpand(true);
        chat_split_view.set_wide_handle(false);
        chat_split_view.add_css_class("chat-ai-split");

        // Panel izquierdo: Contexto (notas adjuntas) - MISMO DISEÑO QUE SIDEBAR NORMAL
        let context_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        context_box.add_css_class("sidebar");
        context_box.add_css_class("chat-context-panel");
        context_box.set_width_request(200);

        // Header con botones (igual que el sidebar normal)
        let context_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        context_header.set_margin_all(12);
        context_header.add_css_class("chat-context-header");

        let context_label = gtk::Label::builder()
            .label(&i18n.borrow().t("chat_context"))
            .xalign(0.0)
            .hexpand(true)
            .build();
        context_label.add_css_class("heading");
        context_label.add_css_class("chat-context-title");
        context_header.append(&context_label);
        context_box.append(&context_header);

        // Scroll con ListBox (igual que el sidebar normal)
        let context_scroll = gtk::ScrolledWindow::new();
        context_scroll.set_vexpand(true);
        context_scroll.set_hexpand(true);
        context_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        context_scroll.add_css_class("chat-context-scroll");

        let chat_context_list = gtk::ListBox::new();
        chat_context_list.add_css_class("navigation-sidebar");
        chat_context_list.add_css_class("chat-context-list");
        chat_context_list.set_selection_mode(gtk::SelectionMode::None);
        context_scroll.set_child(Some(&chat_context_list));
        context_box.append(&context_scroll);

        // Botones como iconos minimalistas en la parte baja
        let buttons_box = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        buttons_box.set_halign(gtk::Align::Center);
        buttons_box.set_margin_start(8);
        buttons_box.set_margin_end(8);
        buttons_box.set_margin_bottom(12);
        buttons_box.add_css_class("chat-context-actions");

        let chat_attach_button = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text(&i18n.borrow().t("chat_attach_note"))
            .build();
        chat_attach_button.set_can_focus(true);
        chat_attach_button.add_css_class("flat");
        chat_attach_button.add_css_class("circular");
        chat_attach_button.add_css_class("chat-context-action");
        chat_attach_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ShowAttachNoteDialog);
            }
        ));
        buttons_box.append(&chat_attach_button);

        let chat_clear_button = gtk::Button::builder()
            .icon_name("edit-clear-symbolic")
            .tooltip_text(&i18n.borrow().t("chat_clear_context"))
            .build();
        chat_clear_button.set_can_focus(true);
        chat_clear_button.add_css_class("flat");
        chat_clear_button.add_css_class("circular");
        chat_clear_button.add_css_class("chat-context-action");
        chat_clear_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ClearChatContext);
            }
        ));
        buttons_box.append(&chat_clear_button);

        let chat_history_button = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text(&i18n.borrow().t("chat_clear_history"))
            .build();
        chat_history_button.set_can_focus(true);
        chat_history_button.add_css_class("flat");
        chat_history_button.add_css_class("circular");
        chat_history_button.add_css_class("chat-context-action");
        chat_history_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ClearChatHistory);
            }
        ));
        buttons_box.append(&chat_history_button);

        context_box.append(&buttons_box);

        chat_split_view.set_start_child(Some(&context_box));

        // Panel derecho: Chat (historial + input)
        let chat_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
        chat_box.add_css_class("chat-area");
        chat_box.add_css_class("chat-main");
        chat_box.set_margin_all(0);

        // Indicador de modo activo
        let mode_indicator_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        mode_indicator_box.set_margin_start(12);
        mode_indicator_box.set_margin_end(12);
        mode_indicator_box.set_margin_top(8);
        mode_indicator_box.set_margin_bottom(4);
        mode_indicator_box.add_css_class("chat-mode-indicator");

        let mode_icon = gtk::Image::from_icon_name("emblem-system-symbolic");
        mode_icon.set_pixel_size(16);
        mode_indicator_box.append(&mode_icon);

        let chat_mode_label = {
            let i18n_borrow = i18n.borrow();
            gtk::Label::new(Some(&i18n_borrow.t("chat_mode_agent")))
        };
        chat_mode_label.set_halign(gtk::Align::Start);
        chat_mode_label.add_css_class("chat-mode-label");
        mode_indicator_box.append(&chat_mode_label);

        // Spacer para empujar el botón a la derecha
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        mode_indicator_box.append(&spacer);

        // Botón de Nueva Sesión
        let new_chat_button = {
            let i18n_borrow = i18n.borrow();
            gtk::Button::builder()
                .icon_name("document-new-symbolic")
                .tooltip_text(&i18n_borrow.t("chat_new_session")) // Asegurarse de agregar esta key o usar fallback
                .build()
        };
        new_chat_button.add_css_class("flat");
        new_chat_button.add_css_class("chat-action-button");
        new_chat_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::NewChatSession);
            }
        ));
        mode_indicator_box.append(&new_chat_button);

        chat_box.append(&mode_indicator_box);

        // Historial de mensajes
        let history_scroll = gtk::ScrolledWindow::new();
        history_scroll.set_vexpand(true);
        history_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        history_scroll.add_css_class("chat-history-scroll");

        let chat_history_list = gtk::ListBox::new();
        chat_history_list.add_css_class("chat-history-list");
        chat_history_list.set_selection_mode(gtk::SelectionMode::None);
        history_scroll.set_child(Some(&chat_history_list));
        chat_box.append(&history_scroll);

        // Input del usuario con diseño consistente tipo entry
        let input_area = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        input_area.set_margin_all(0);
        input_area.add_css_class("chat-input-container");
        input_area.add_css_class("chat-input-bar");

        // Box que simula el borde de entry (más fácil de estilizar que Frame)
        let input_wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
        input_wrapper.set_hexpand(true);
        input_wrapper.add_css_class("chat-input-wrapper");

        // ScrolledWindow interno
        let input_scroll = gtk::ScrolledWindow::new();
        input_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        input_scroll.set_hexpand(true);
        input_scroll.set_vexpand(false);
        input_scroll.set_min_content_height(80);
        input_scroll.set_max_content_height(200);
        input_scroll.set_overlay_scrolling(false);
        input_scroll.add_css_class("chat-input-scroll");

        let chat_input_view = gtk::TextView::new();
        let chat_input_buffer = chat_input_view.buffer();
        chat_input_view.set_wrap_mode(gtk::WrapMode::WordChar);
        chat_input_view.set_accepts_tab(false);
        chat_input_view.set_hexpand(true);
        chat_input_view.set_vexpand(false);
        chat_input_view.add_css_class("chat-input");

        // Crear popover para sugerencias de notas con @
        let chat_note_suggestions_list = gtk::ListBox::new();
        chat_note_suggestions_list.set_selection_mode(gtk::SelectionMode::Single); // Permitir selección para navegación
        chat_note_suggestions_list.add_css_class("suggestions-list");
        chat_note_suggestions_list.set_can_focus(false); // No capturar foco
        chat_note_suggestions_list.set_focusable(false);
        chat_note_suggestions_list.set_vexpand(true);
        chat_note_suggestions_list.set_hexpand(true);

        let suggestions_scroll = gtk::ScrolledWindow::new();
        suggestions_scroll.set_child(Some(&chat_note_suggestions_list));
        suggestions_scroll.set_max_content_height(300);
        suggestions_scroll.set_min_content_width(450);
        suggestions_scroll.set_min_content_height(200);
        suggestions_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        suggestions_scroll.set_can_focus(false); // No capturar foco
        suggestions_scroll.set_focusable(false);
        suggestions_scroll.set_vexpand(true);
        suggestions_scroll.set_hexpand(true);
        suggestions_scroll.set_propagate_natural_height(true);

        let chat_note_suggestions_popover = gtk::Popover::new();
        chat_note_suggestions_popover.set_parent(&chat_input_view);
        chat_note_suggestions_popover.add_css_class("mention-completion"); // Reutilizar estilo que funciona
        chat_note_suggestions_popover.set_size_request(450, 250); // Asegurar tamaño mínimo

        // CRÍTICO: Mostrar explícitamente el contenido antes de agregarlo
        suggestions_scroll.show();
        chat_note_suggestions_list.show();

        chat_note_suggestions_popover.set_child(Some(&suggestions_scroll));
        chat_note_suggestions_popover.set_autohide(false); // No autohide para mantener control del foco
        chat_note_suggestions_popover.set_has_arrow(false);
        chat_note_suggestions_popover.set_position(gtk::PositionType::Top); // Mostrar arriba del input
        chat_note_suggestions_popover.set_can_focus(false); // No robar el foco del input
        chat_note_suggestions_popover.set_focusable(false);

        // Agregar placeholder inicial
        let chat_placeholder = i18n.borrow().t("chat_input_placeholder");
        chat_input_buffer.set_text(&chat_placeholder);

        // Limpiar placeholder al hacer focus
        let focus_controller = gtk::EventControllerFocus::new();
        let placeholder_clone = chat_placeholder.clone();
        focus_controller.connect_enter(gtk::glib::clone!(
            #[strong]
            chat_input_buffer,
            move |_| {
                let start = chat_input_buffer.start_iter();
                let end = chat_input_buffer.end_iter();
                let text = chat_input_buffer.text(&start, &end, false).to_string();
                if text == placeholder_clone {
                    chat_input_buffer.set_text("");
                }
            }
        ));
        chat_input_view.add_controller(focus_controller);

        // Agregar controlador de click para dar foco al input
        let click_controller = gtk::GestureClick::new();
        click_controller.connect_released(gtk::glib::clone!(
            #[strong]
            chat_input_view,
            move |_gesture, _n, _x, _y| {
                chat_input_view.grab_focus();
            }
        ));
        chat_input_view.add_controller(click_controller);

        // Agregar controlador para Enter envía mensaje (Shift+Enter = nueva línea)
        let input_key_controller = gtk::EventControllerKey::new();
        let placeholder_for_enter = chat_placeholder.clone();
        input_key_controller.connect_key_pressed(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            chat_input_buffer,
            #[strong]
            chat_note_suggestions_popover,
            #[strong]
            chat_note_suggestions_list,
            move |_controller, keyval, _keycode, modifiers| {
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

                // Si el popover de sugerencias está visible, manejar navegación especial
                if chat_note_suggestions_popover.is_visible() {
                    match key_name.as_str() {
                        "Down" => {
                            let selected = chat_note_suggestions_list.selected_row();
                            let next_index = if let Some(row) = selected {
                                row.index() + 1
                            } else {
                                0
                            };

                            if let Some(next_row) =
                                chat_note_suggestions_list.row_at_index(next_index)
                            {
                                chat_note_suggestions_list.select_row(Some(&next_row));
                            }
                            return gtk::glib::Propagation::Stop;
                        }
                        "Up" => {
                            let selected = chat_note_suggestions_list.selected_row();
                            let prev_index = if let Some(row) = selected {
                                if row.index() > 0 { row.index() - 1 } else { 0 }
                            } else {
                                0
                            };

                            if let Some(prev_row) =
                                chat_note_suggestions_list.row_at_index(prev_index)
                            {
                                chat_note_suggestions_list.select_row(Some(&prev_row));
                            }
                            return gtk::glib::Propagation::Stop;
                        }
                        "Tab" | "Return" => {
                            // Tab o Enter: completar con la fila seleccionada o la primera
                            let row_to_activate = chat_note_suggestions_list
                                .selected_row()
                                .or_else(|| chat_note_suggestions_list.row_at_index(0));

                            if let Some(row) = row_to_activate {
                                // Recuperar el nombre de la nota guardado en el row
                                if let Some(note_name) = unsafe {
                                    row.data::<String>("note_name").map(|d| d.as_ref().clone())
                                } {
                                    sender.input(AppMsg::CompleteChatNote(note_name));
                                    return gtk::glib::Propagation::Stop;
                                }
                            }

                            // Si no se pudo completar, cerrar popover
                            chat_note_suggestions_popover.popdown();
                            return gtk::glib::Propagation::Stop;
                        }
                        "Escape" => {
                            // Cerrar popover
                            chat_note_suggestions_popover.popdown();
                            return gtk::glib::Propagation::Stop;
                        }
                        _ => {
                            // Cualquier otra tecla: dejar que se escriba en el input
                            return gtk::glib::Propagation::Proceed;
                        }
                    }
                }

                // ESC: Salir del modo chat
                if key_name == "Escape" {
                    sender.input(AppMsg::ExitChatMode);
                    return gtk::glib::Propagation::Stop;
                }

                // Enter sin modificadores: enviar mensaje
                if key_name == "Return" && !modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
                    let start = chat_input_buffer.start_iter();
                    let end = chat_input_buffer.end_iter();
                    let text = chat_input_buffer.text(&start, &end, false).to_string();

                    if !text.trim().is_empty() && text != placeholder_for_enter {
                        sender.input(AppMsg::SendChatMessage(text));
                        return gtk::glib::Propagation::Stop;
                    }
                    return gtk::glib::Propagation::Stop;
                }

                // Shift+Enter: nueva línea (comportamiento por defecto)
                gtk::glib::Propagation::Proceed
            }
        ));
        chat_input_view.add_controller(input_key_controller);

        input_scroll.set_child(Some(&chat_input_view));
        input_wrapper.append(&input_scroll);
        input_area.append(&input_wrapper);

        let chat_send_button = gtk::Button::builder()
            .label(&i18n.borrow().t("chat_send"))
            .icon_name("mail-send-symbolic")
            .build();
        chat_send_button.set_valign(gtk::Align::Center);
        chat_send_button.set_can_focus(true);
        chat_send_button.add_css_class("chat-send-button");
        chat_send_button.add_css_class("chat-action-primary");
        let placeholder_for_send = chat_placeholder.clone();
        chat_send_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            chat_input_buffer,
            move |_| {
                let start = chat_input_buffer.start_iter();
                let end = chat_input_buffer.end_iter();
                let text = chat_input_buffer.text(&start, &end, false).to_string();

                if !text.trim().is_empty() && text != placeholder_for_send {
                    sender.input(AppMsg::SendChatMessage(text));
                }
            }
        ));

        // Botón para alternar entre Modo Agente y Chat Normal
        let chat_mode_toggle = {
            let i18n_borrow = i18n.borrow();
            gtk::ToggleButton::builder()
                .icon_name("emblem-system-symbolic")
                .tooltip_text(&i18n_borrow.t("chat_toggle_mode_tooltip"))
                .active(true) // Por defecto: Modo Agente
                .build()
        };
        chat_mode_toggle.set_valign(gtk::Align::Center);
        chat_mode_toggle.add_css_class("chat-mode-toggle");
        chat_mode_toggle.connect_toggled(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ToggleChatMode);
            }
        ));
        input_area.append(&chat_mode_toggle);

        input_area.append(&chat_send_button);

        chat_box.append(&input_area);

        chat_split_view.set_end_child(Some(&chat_box));
        chat_ai_container.append(&chat_split_view);

        // Controlador de teclado para el modo Chat AI
        let chat_key_controller = gtk::EventControllerKey::new();
        chat_key_controller.connect_key_pressed(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            mode,
            #[strong]
            chat_input_view,
            #[strong]
            text_view_actual,
            move |_controller, keyval, _keycode, _modifiers| {
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

                // Solo procesar si no estamos en el input del chat
                if chat_input_view.has_focus() {
                    return gtk::glib::Propagation::Proceed;
                }

                // ESC: Salir del modo Chat AI y volver a Normal
                if key_name == "Escape" {
                    sender.input(AppMsg::ExitChatMode);
                    return gtk::glib::Propagation::Stop;
                }

                // i: Salir del modo Chat AI y entrar a Insert
                if key_name == "i" {
                    // Cambiar a Insert y luego salir del chat
                    *mode.borrow_mut() = crate::core::editor_mode::EditorMode::Insert;
                    sender.input(AppMsg::ExitChatMode);
                    return gtk::glib::Propagation::Stop;
                }

                gtk::glib::Propagation::Proceed
            }
        ));
        chat_ai_container.add_controller(chat_key_controller);

        // Agregar el chat al Stack
        widgets
            .content_stack
            .add_named(&chat_ai_container, Some("chat"));

        // Escaneo inicial: sincronizar BD con filesystem al arrancar
        println!("🔍 Escaneando directorio de notas para sincronizar BD...");
        let scan_start = std::time::Instant::now();
        let mut indexed_count = 0;
        let mut existing_paths = Vec::new();

        // Función recursiva para escanear carpetas
        fn scan_directory(
            path: &std::path::Path,
            notes_db: &crate::core::database::NotesDatabase,
            root: &std::path::Path,
            indexed_count: &mut usize,
            existing_paths: &mut Vec<String>,
        ) {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        let entry_path = entry.path();

                        if metadata.is_file() && entry_path.extension().map_or(false, |e| e == "md")
                        {
                            // Es un archivo .md, indexarlo
                            if let (Ok(content), Some(name)) = (
                                std::fs::read_to_string(&entry_path),
                                entry_path.file_stem().and_then(|s| s.to_str()),
                            ) {
                                // Detectar carpeta relativa al root
                                let folder = entry_path
                                    .parent()
                                    .and_then(|p| p.strip_prefix(root).ok())
                                    .filter(|p| !p.as_os_str().is_empty())
                                    .and_then(|p| p.to_str())
                                    .map(|s| s.to_string());

                                let note_path = entry_path.to_str().unwrap_or("");
                                existing_paths.push(note_path.to_string());

                                let _ = notes_db.index_note(
                                    name,
                                    note_path,
                                    &content,
                                    folder.as_deref(),
                                );
                                *indexed_count += 1;
                            }
                        } else if metadata.is_dir() {
                            // Es una carpeta, escanear recursivamente
                            scan_directory(
                                &entry_path,
                                notes_db,
                                root,
                                indexed_count,
                                existing_paths,
                            );
                        }
                    }
                }
            }
        }

        let notes_root = notes_dir.root().to_path_buf();
        scan_directory(
            &notes_root,
            &notes_db,
            &notes_root,
            &mut indexed_count,
            &mut existing_paths,
        );

        let scan_duration = scan_start.elapsed();
        println!(
            "✅ Escaneo completado: {} notas indexadas en {:?}",
            indexed_count, scan_duration
        );

        // Limpiar notas huérfanas de la BD
        match notes_db.cleanup_orphaned_notes(&existing_paths) {
            Ok(deleted) if deleted > 0 => {
                println!(
                    "🧹 Limpiadas {} notas huérfanas de la base de datos",
                    deleted
                );
            }
            Err(e) => {
                eprintln!("⚠️ Error al limpiar notas huérfanas: {}", e);
            }
            _ => {}
        }

        // Inicializar file watcher antes de crear el model
        let file_watcher = {
            let notes_path = notes_dir.root().to_path_buf();
            let watcher_db =
                std::sync::Arc::new(std::sync::Mutex::new(notes_db.clone_connection()));

            match crate::file_watcher::create_notes_watcher(
                notes_path,
                watcher_db,
                sender.input_sender().clone(),
            ) {
                Ok(watcher) => {
                    println!("✅ File watcher activado");
                    Some(watcher)
                }
                Err(e) => {
                    eprintln!("⚠️ Error activando file watcher: {}", e);
                    None
                }
            }
        };

        let mut model = MainApp {
            theme,
            buffer: initial_buffer,
            mode: mode.clone(),
            command_parser: CommandParser::new(),
            cursor_position: 0,
            text_buffer: text_buffer.clone(),
            mode_label: widgets.mode_label.clone(),
            stats_label: widgets.stats_label.clone(),
            window_title: widgets.window_title.clone(),
            notes_dir,
            notes_db,
            notes_config: notes_config.clone(),
            current_note,
            has_unsaved_changes: false,
            markdown_enabled: true, // Ahora con parser robusto usando offsets de pulldown-cmark
            bit8_mode: false,
            text_view: text_view_actual.clone(),
            preview_webview: preview_webview.clone(),
            editor_stack: editor_stack.clone(),
            editor_scroll: editor_scroll.clone(),
            preview_scroll: preview_scroll.clone(),
            preview_scroll_percent: Rc::new(RefCell::new(0.0)),
            split_view: widgets.split_view.clone(),
            notes_list: widgets.notes_list.clone(),
            sidebar_visible: false,
            expanded_folders: std::collections::HashSet::new(),
            is_populating_list: Rc::new(RefCell::new(false)),
            is_syncing_to_gtk: Rc::new(RefCell::new(false)),
            context_menu: context_menu.clone(),
            context_item_name: Rc::new(RefCell::new(String::new())),
            context_is_folder: Rc::new(RefCell::new(false)),
            renaming_item: Rc::new(RefCell::new(None)),
            main_window: widgets.main_window.clone(),
            link_spans: Rc::new(RefCell::new(Vec::new())),
            heading_anchors: Rc::new(RefCell::new(Vec::new())),
            tag_spans: Rc::new(RefCell::new(Vec::new())),
            note_mention_spans: Rc::new(RefCell::new(Vec::new())),
            youtube_video_spans: Rc::new(RefCell::new(Vec::new())),
            tags_menu_button: widgets.tags_menu_button.clone(),
            tags_list_box: widgets.tags_list_box.clone(),
            todos_menu_button: widgets.todos_menu_button.clone(),
            todos_list_box: widgets.todos_list_box.clone(),
            tag_completion_popup: completion_popover.clone(),
            tag_completion_list: completion_list_box.clone(),
            current_tag_prefix: Rc::new(RefCell::new(None)),
            just_completed_tag: Rc::new(RefCell::new(false)),
            note_mention_popup: mention_popover.clone(),
            note_mention_list: mention_list_box.clone(),
            current_mention_prefix: Rc::new(RefCell::new(None)),
            just_completed_mention: Rc::new(RefCell::new(false)),
            search_toggle_button: widgets.search_toggle_button.clone(),
            floating_search_bar: widgets.floating_search_bar.clone(),
            floating_search_entry: widgets.floating_search_entry.clone(),
            floating_search_mode_label: widgets.floating_search_mode_label.clone(),
            floating_search_results: widgets.floating_search_results.clone(),
            floating_search_results_list: widgets.floating_search_results_list.clone(),
            floating_search_rows: Rc::new(RefCell::new(Vec::new())),
            floating_search_visible: false,
            floating_search_in_current_note: Rc::new(RefCell::new(false)),
            in_note_search_matches: Rc::new(RefCell::new(Vec::new())),
            in_note_search_current_index: Rc::new(RefCell::new(0)),
            semantic_search_enabled: false,
            semantic_search_timeout_id: Rc::new(RefCell::new(None)),
            traditional_search_timeout_id: Rc::new(RefCell::new(None)),
            semantic_search_answer_box: {
                // PRIMERO crear el box
                let answer_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
                answer_box.set_margin_start(12);
                answer_box.set_margin_end(12);
                answer_box.set_margin_top(8);
                answer_box.set_margin_bottom(8);
                answer_box.set_visible(false);
                answer_box.set_vexpand(true);
                answer_box.set_hexpand(true);
                answer_box.add_css_class("semantic-answer-box");
                answer_box
            },
            semantic_search_answer_row: {
                // DESPUÉS crear el row CON el box como hijo
                let row = gtk::ListBoxRow::builder()
                    .selectable(false)
                    .activatable(false)
                    .visible(false)
                    .build();

                // Insertar en el ListBox
                widgets.floating_search_results_list.prepend(&row);
                row
            },
            semantic_search_answer_label: {
                let label = gtk::Label::new(None);
                label.set_wrap(true);
                label.set_wrap_mode(gtk::pango::WrapMode::Word);
                label.set_selectable(true);
                label.set_xalign(0.0);
                label.set_yalign(0.0);
                label.set_use_markup(true);
                label.set_vexpand(false);
                label.set_hexpand(true);
                label.add_css_class("semantic-answer-label");

                // Conectar el evento de activación de enlaces
                let sender_clone = sender.clone();
                label.connect_activate_link(move |_label, uri| {
                    // Si el link tiene el esquema note://, cargar la nota
                    if let Some(note_name) = uri.strip_prefix("note://") {
                        sender_clone.input(AppMsg::LoadNote {
                            name: note_name.to_string(),
                            highlight_text: None,
                        });
                        return gtk::glib::Propagation::Stop;
                    }
                    gtk::glib::Propagation::Proceed
                });

                label
            },
            semantic_search_answer_visible: Rc::new(RefCell::new(false)),
            i18n,
            sidebar_toggle_button: widgets.sidebar_toggle_button.clone(),
            sidebar_notes_label: widgets.sidebar_notes_label.clone(),
            new_note_button: widgets.new_note_button.clone(),
            settings_button: widgets.settings_button.clone(),
            image_widgets: Rc::new(RefCell::new(Vec::new())),
            todo_widgets: Rc::new(RefCell::new(Vec::new())),
            video_widgets: Rc::new(RefCell::new(Vec::new())),
            table_widgets: Rc::new(RefCell::new(Vec::new())),
            reminder_widgets: Rc::new(RefCell::new(Vec::new())),
            app_sender: Rc::new(RefCell::new(None)),
            youtube_server: {
                let server = Rc::new(crate::youtube_server::YouTubeEmbedServer::new(8787));
                // Iniciar el servidor en un thread separado
                if let Err(e) = server.start() {
                    eprintln!("Error iniciando servidor YouTube: {}", e);
                }
                server
            },
            music_player,
            music_player_button: widgets.music_player_button.clone(),
            music_player_popover,
            music_search_entry,
            music_results_list,
            music_now_playing_label,
            music_state_label,
            music_play_pause_btn,
            playlist_current_list,
            playlist_saved_list,
            chat_session: Rc::new(RefCell::new(None)),
            chat_session_id: Rc::new(RefCell::new(None)),
            content_stack: widgets.content_stack.clone(),
            chat_ai_container,
            chat_split_view,
            chat_context_list,
            chat_history_scroll: history_scroll.clone(),
            chat_history_list,
            chat_input_view,
            chat_input_buffer,
            chat_send_button,
            chat_clear_button,
            chat_attach_button,
            chat_model_label,
            chat_tokens_progress,
            chat_note_suggestions_popover,
            chat_note_suggestions_list,
            chat_current_note_prefix: Rc::new(RefCell::new(None)),
            chat_just_completed_note: Rc::new(RefCell::new(false)),
            chat_streaming_label: Rc::new(RefCell::new(None)),
            chat_streaming_text: Rc::new(RefCell::new(String::new())),
            chat_thinking_container: Rc::new(RefCell::new(None)),
            mcp_executor,
            mcp_registry,
            mcp_last_update_check: Rc::new(RefCell::new(0)),
            window_visible: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            file_watcher,
            cached_rendered_text: Rc::new(RefCell::new(None)),
            cached_source_text: Rc::new(RefCell::new(None)),
            router_agent: Rc::new(RefCell::new(None)),
            chat_agent_mode: Rc::new(RefCell::new(true)), // Por defecto: Modo Agente activado
            chat_mode_label,
            notification_revealer: widgets.notification_revealer.clone(),
            notification_label: widgets.notification_label.clone(),
            reminder_db,
            reminder_scheduler,
            reminder_notifier,
            reminder_parser,
            reminders_button: widgets.reminders_button.clone(),
            reminders_popover,
            reminders_list,
            reminders_pending_badge,
            note_memory: Rc::new(RefCell::new(None)),
            quick_note_window: Rc::new(RefCell::new(None)),
        };

        // Guardar el sender en el modelo
        *model.app_sender.borrow_mut() = Some(sender.clone());

        // Configurar handler para mensajes JS→Rust desde el WebView de preview
        {
            if let Some(content_manager) = preview_webview.user_content_manager() {
                let sender_clone = sender.clone();
                content_manager.connect_script_message_received(
                    Some("notnative"),
                    move |_manager, js_result| {
                        // Parsear el mensaje JSON del JavaScript
                        // js_result es un javascriptcore::Value, usamos to_str() para convertir a GString
                        let message_str = js_result.to_str();
                        if let Ok(message) = serde_json::from_str::<serde_json::Value>(&message_str)
                        {
                            let action = message["action"].as_str().unwrap_or("");
                            let args = &message["args"];

                            match action {
                                "todo-toggle" => {
                                    // args: [line_number, is_checked]
                                    if let (Some(line), Some(checked)) = (
                                        args.get(0).and_then(|v| v.as_i64()),
                                        args.get(1).and_then(|v| v.as_bool()),
                                    ) {
                                        sender_clone.input(AppMsg::ToggleTodoLine {
                                            line: line as usize,
                                            checked,
                                        });
                                    }
                                }
                                "open-note" => {
                                    // args: [note_name]
                                    if let Some(note_name) = args.get(0).and_then(|v| v.as_str()) {
                                        sender_clone.input(AppMsg::LoadNote {
                                            name: note_name.to_string(),
                                            highlight_text: None,
                                        });
                                    }
                                }
                                "search-tag" => {
                                    // args: [tag_name]
                                    if let Some(tag_name) = args.get(0).and_then(|v| v.as_str()) {
                                        sender_clone
                                            .input(AppMsg::SaveAndSearchTag(tag_name.to_string()));
                                    }
                                }
                                "line-click" => {
                                    // args: [line_number]
                                    if let Some(line) = args.get(0).and_then(|v| v.as_i64()) {
                                        sender_clone.input(AppMsg::SwitchToInsertAtLine {
                                            line: line as usize,
                                        });
                                    }
                                }
                                _ => {
                                    println!("WebView: mensaje desconocido: {}", action);
                                }
                            }
                        }
                    },
                );
            }
        }

        // Configurar el widget de respuesta semántica
        {
            // Asegurar que el row tenga el box como hijo ANTES de insertarlo
            model
                .floating_search_results_list
                .remove(&model.semantic_search_answer_row);
            model
                .semantic_search_answer_row
                .set_child(Some(&model.semantic_search_answer_box));
            model
                .floating_search_results_list
                .prepend(&model.semantic_search_answer_row);

            let title_label = gtk::Label::new(Some("🧠 Respuesta del Asistente"));
            title_label.add_css_class("semantic-answer-title");
            title_label.set_xalign(0.0);
            model.semantic_search_answer_box.append(&title_label);
            model
                .semantic_search_answer_box
                .append(&model.semantic_search_answer_label);
        }

        // Configurar el sender en el reminder_notifier
        model.reminder_notifier.set_app_sender(sender.clone());

        // Inicializar RouterAgent para el sistema multi-agente
        // Crear cliente de IA para el router (usa misma configuración que chat)
        let api_key = notes_config
            .borrow()
            .get_ai_config()
            .api_key
            .clone()
            .unwrap_or_else(|| std::env::var("OPENAI_API_KEY").unwrap_or_default());

        if !api_key.is_empty() {
            // Crear modelo de configuración temporal para el router
            let (provider_str, model_str) = {
                let config = notes_config.borrow();
                let ai_config = config.get_ai_config();
                (ai_config.provider.clone(), ai_config.model.clone())
            };

            let provider = match provider_str.to_lowercase().as_str() {
                "anthropic" => crate::ai_chat::AIProvider::Anthropic,
                "ollama" => crate::ai_chat::AIProvider::Ollama,
                "custom" => crate::ai_chat::AIProvider::Custom,
                _ => crate::ai_chat::AIProvider::OpenAI,
            };

            let router_config = crate::ai_chat::AIModelConfig {
                provider,
                model: model_str,
                temperature: 0.3, // Temperatura baja para clasificación precisa
                max_tokens: 4000,
            };

            match crate::ai_client::create_client(&router_config, &api_key) {
                Ok(ai_client) => {
                    // Crear RouterAgent con el cliente de IA (ya envuelto en Box<dyn AIClient>)
                    // Necesitamos convertir Box<dyn AIClient> a Arc<dyn AIClient>
                    // La forma correcta es crear un nuevo Arc desde el Box
                    let router = crate::ai::RouterAgent::new(std::sync::Arc::from(ai_client));
                    *model.router_agent.borrow_mut() = Some(router);
                    println!("✅ RouterAgent inicializado con 5 agentes especializados");
                }
                Err(e) => {
                    eprintln!("⚠️ No se pudo inicializar RouterAgent: {}", e);
                    eprintln!("   El chat seguirá funcionando con el sistema anterior");
                }
            }
        } else {
            println!("⚠️ No hay API key configurada, RouterAgent deshabilitado");
        }

        // Inicializar NoteMemory para búsqueda semántica (RIG integrado)
        println!("🔍🔍🔍 INICIO BLOQUE NOTEMEMORY 🔍🔍🔍");
        {
            let embedding_config = notes_config.borrow().get_embedding_config().clone();
            println!(
                "🔍 DEBUG NoteMemory: embeddings_enabled={}, api_key_len={}",
                embedding_config.enabled,
                api_key.len()
            );

            if embedding_config.enabled && !api_key.is_empty() {
                println!("🔍 DEBUG: Condiciones cumplidas, verificando router...");
                if let Some(router) = model.router_agent.borrow().as_ref() {
                    println!("🔍 DEBUG: Router disponible, extrayendo cliente RIG...");
                    use rig::client::EmbeddingsClient;

                    // Extraer el cliente RIG del AIClient
                    let ai_client_any = router.get_llm();
                    if let Some(rig_client) = ai_client_any
                        .as_any()
                        .downcast_ref::<crate::ai::rig_adapter::RigClient>()
                    {
                        let embedding_model = match &rig_client.backend {
                            crate::ai::rig_adapter::RigClientBackend::OpenAI(oa_client) => {
                                eprintln!("🔍 Usando backend OpenAI para embeddings");
                                Some(oa_client.embedding_model(&embedding_config.model))
                            }
                            crate::ai::rig_adapter::RigClientBackend::OpenRouter(_) => {
                                eprintln!(
                                    "🔍 Detectado backend OpenRouter - creando cliente compatible"
                                );
                                eprintln!("   Modelo de embeddings: {}", &embedding_config.model);
                                // Crear cliente OpenAI con URL de OpenRouter para embeddings
                                let or_client = crate::ai::rig_adapter::RigClient::create_openrouter_embedding_client(&api_key);
                                Some(or_client.embedding_model(&embedding_config.model))
                            }
                        };

                        if let Some(emb_model) = embedding_model {
                            let db_path_str = db_path.to_str().unwrap_or("notes.db").to_string();

                            // Inicializar sincrónicamente con block_on
                            let rt =
                                tokio::runtime::Runtime::new().expect("No se pudo crear runtime");
                            match rt.block_on(crate::ai::memory::NoteMemory::new(
                                &db_path_str,
                                emb_model,
                            )) {
                                Ok(memory) => {
                                    *model.note_memory.borrow_mut() = Some(Arc::new(memory));

                                    // Actualizar MCPToolExecutor con la referencia al NoteMemory compartido
                                    model
                                        .mcp_executor
                                        .borrow_mut()
                                        .set_note_memory(model.note_memory.clone());

                                    println!("✅ NoteMemory inicializado para búsqueda semántica");
                                }
                                Err(e) => {
                                    eprintln!("⚠️ Error inicializando NoteMemory: {}", e);
                                    eprintln!(
                                        "   La búsqueda semántica usará el sistema tradicional"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Iniciar monitoreo de cambios MCP cada 2 segundos
        let sender_clone = sender.clone();
        glib::timeout_add_seconds_local(2, move || {
            sender_clone.input(AppMsg::CheckMCPUpdates);
            glib::ControlFlow::Continue
        });

        // Crear acciones para el menú contextual
        let rename_action = gtk::gio::SimpleAction::new("rename", None);
        rename_action.connect_activate(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong(rename_to = item_name)]
            model.context_item_name,
            #[strong(rename_to = is_folder)]
            model.context_is_folder,
            move |_, _| {
                sender.input(AppMsg::RenameItem(
                    item_name.borrow().clone(),
                    *is_folder.borrow(),
                ));
            }
        ));

        let delete_action = gtk::gio::SimpleAction::new("delete", None);
        delete_action.connect_activate(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong(rename_to = item_name)]
            model.context_item_name,
            #[strong(rename_to = is_folder)]
            model.context_is_folder,
            move |_, _| {
                sender.input(AppMsg::DeleteItem(
                    item_name.borrow().clone(),
                    *is_folder.borrow(),
                ));
            }
        ));

        let open_folder_action = gtk::gio::SimpleAction::new("open_folder", None);
        open_folder_action.connect_activate(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong(rename_to = item_name)]
            model.context_item_name,
            #[strong(rename_to = is_folder)]
            model.context_is_folder,
            move |_, _| {
                sender.input(AppMsg::OpenInFileManager(
                    item_name.borrow().clone(),
                    *is_folder.borrow(),
                ));
            }
        ));

        // Acción para cambiar icono
        let change_icon_action = gtk::gio::SimpleAction::new("change_icon", None);
        change_icon_action.connect_activate(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong(rename_to = item_name)]
            model.context_item_name,
            #[strong(rename_to = is_folder)]
            model.context_is_folder,
            move |_, _| {
                sender.input(AppMsg::ShowIconPicker {
                    name: item_name.borrow().clone(),
                    is_folder: *is_folder.borrow(),
                });
            }
        ));

        let action_group = gtk::gio::SimpleActionGroup::new();
        action_group.add_action(&rename_action);
        action_group.add_action(&delete_action);
        action_group.add_action(&open_folder_action);
        action_group.add_action(&change_icon_action);
        context_menu.insert_action_group("item", Some(&action_group));

        // Crear tags de estilo para markdown
        model.create_text_tags();

        // Crear popover del settings button con textos traducidos
        model.create_settings_popover(&sender);

        // Aplicar traducciones iniciales a todos los widgets
        model.apply_initial_translations();

        // Sincronizar contenido inicial con la vista
        // Configurar TextView según el modo inicial (Normal)
        text_view_actual.set_editable(false);
        text_view_actual.set_cursor_visible(true); // Cursor visible para navegación
        println!("🔧 Modo inicial configurado: Normal (editable=false, cursor_visible=true)");

        model.sync_to_view();
        model.update_status_bar(&sender);

        // Configurar autocompletado de notas en chat con @
        model.chat_input_buffer.connect_changed(gtk::glib::clone!(
            #[strong(rename_to = chat_current_note_prefix)]
            model.chat_current_note_prefix,
            #[strong(rename_to = chat_just_completed_note)]
            model.chat_just_completed_note,
            #[strong]
            sender,
            move |buffer| {
                // Si acabamos de completar, saltar esta iteración y resetear flag
                if *chat_just_completed_note.borrow() {
                    *chat_just_completed_note.borrow_mut() = false;
                    return;
                }

                let cursor_pos = buffer.cursor_position();
                let iter = buffer.iter_at_offset(cursor_pos);

                // Buscar @ antes del cursor
                let mut start_iter = iter;
                let mut found_at = false;
                let mut search_text = String::new();

                while start_iter.backward_char() {
                    let ch = start_iter.char();
                    if ch == '@' {
                        found_at = true;
                        break;
                    } else if ch.is_whitespace() || ch == '\n' {
                        break;
                    } else {
                        search_text.insert(0, ch);
                    }
                }

                if found_at {
                    // Guardar el prefijo actual
                    *chat_current_note_prefix.borrow_mut() = Some(search_text.clone());

                    // Enviar mensaje para mostrar sugerencias
                    sender.input(AppMsg::ShowChatNoteSuggestions(search_text));
                } else {
                    *chat_current_note_prefix.borrow_mut() = None;
                    sender.input(AppMsg::HideChatNoteSuggestions);
                }
            }
        ));

        // Actualizar popovers si hay una nota cargada
        if model.current_note.is_some() {
            model.refresh_tags_display_with_sender(&sender);
            model.refresh_todos_summary();
        }

        // Configurar autoguardado cada 5 segundos
        gtk::glib::timeout_add_seconds_local(
            5,
            gtk::glib::clone!(
                #[strong]
                sender,
                move || {
                    sender.input(AppMsg::AutoSave);
                    gtk::glib::ControlFlow::Continue
                }
            ),
        );

        // Configurar watcher para cambios de tema
        Self::setup_theme_watcher(sender.clone());

        let action_group = gtk::gio::SimpleActionGroup::new();
        let toggle_action = gtk::gio::SimpleAction::new("toggle-theme", None);
        toggle_action.connect_activate(gtk::glib::clone!(
            #[strong]
            sender,
            move |_, _| {
                sender.input(AppMsg::ToggleTheme);
            }
        ));
        action_group.add_action(&toggle_action);
        widgets
            .main_window
            .insert_action_group("app", Some(&action_group));

        let shortcuts = gtk::ShortcutController::new();
        shortcuts.set_scope(gtk::ShortcutScope::Local);
        if let (Some(trigger), Some(action)) = (
            gtk::ShortcutTrigger::parse_string("<Primary>d"),
            gtk::ShortcutAction::parse_string("activate app.toggle-theme"),
        ) {
            let shortcut = gtk::Shortcut::new(Some(trigger), Some(action));
            shortcuts.add_shortcut(shortcut);
        }
        widgets.main_window.add_controller(shortcuts);

        // Conectar señal de cierre para minimizar a bandeja en lugar de cerrar
        widgets.main_window.connect_close_request(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::SaveCurrentNote);
                sender.input(AppMsg::MinimizeToTray);
                gtk::glib::Propagation::Stop // Prevenir el cierre
            }
        ));

        // Conectar eventos de teclado al TextView
        let key_controller = gtk::EventControllerKey::new();
        key_controller.connect_key_pressed(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            mode,
            move |_controller, keyval, _keycode, modifiers| {
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

                // PRIORIDAD MÁXIMA: Ctrl+F siempre funciona, sin importar el modo
                if key_name == "f" && modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                    // Cerrar sidebar y abrir barra flotante de búsqueda global
                    sender.input(AppMsg::CloseSidebarAndOpenSearch);
                    return gtk::glib::Propagation::Stop;
                }

                // Alt+F: Búsqueda dentro de la nota actual
                if key_name == "f" && modifiers.contains(gtk::gdk::ModifierType::ALT_MASK) {
                    sender.input(AppMsg::ToggleFloatingSearchInNote);
                    return gtk::glib::Propagation::Stop;
                }

                let current_mode = *mode.borrow();

                let key_mods = KeyModifiers {
                    ctrl: modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK),
                    alt: modifiers.contains(gtk::gdk::ModifierType::ALT_MASK),
                    shift: modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK),
                };

                // En modo Insert, interceptar teclas especiales (Escape, Tab)
                // Dejar que GTK maneje el resto para permitir composición de acentos
                if current_mode == EditorMode::Insert {
                    if key_mods.ctrl {
                        sender.input(AppMsg::KeyPress {
                            key: key_name,
                            modifiers: key_mods,
                        });
                        gtk::glib::Propagation::Stop
                    } else {
                        match key_name.as_str() {
                            "Escape" | "Tab" => {
                                sender.input(AppMsg::KeyPress {
                                    key: key_name,
                                    modifiers: key_mods,
                                });
                                gtk::glib::Propagation::Stop
                            }
                            _ => {
                                // Dejar que GTK maneje la tecla (para acentos, etc.)
                                gtk::glib::Propagation::Proceed
                            }
                        }
                    }
                } else {
                    // En modo Normal y otros, manejar todas las teclas nosotros
                    sender.input(AppMsg::KeyPress {
                        key: key_name,
                        modifiers: key_mods,
                    });
                    gtk::glib::Propagation::Stop
                }
            }
        ));
        text_view_actual.add_controller(key_controller);

        // Añadir key controller al WebView de preview para que los keybindings funcionen en modo Normal
        let webview_key_controller = gtk::EventControllerKey::new();
        let mode_for_webview = model.mode.clone();
        let webview_for_scroll = preview_webview.clone();
        webview_key_controller.connect_key_pressed(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            mode_for_webview,
            #[strong]
            webview_for_scroll,
            move |_controller, keyval, _keycode, modifiers| {
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

                // Ctrl+F siempre funciona
                if key_name == "f" && modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                    sender.input(AppMsg::CloseSidebarAndOpenSearch);
                    return gtk::glib::Propagation::Stop;
                }

                // Alt+F: Búsqueda dentro de la nota actual
                if key_name == "f" && modifiers.contains(gtk::gdk::ModifierType::ALT_MASK) {
                    sender.input(AppMsg::ToggleFloatingSearchInNote);
                    return gtk::glib::Propagation::Stop;
                }

                let current_mode = *mode_for_webview.borrow();

                // En modo Normal, manejar scroll con flechas/j/k
                if current_mode == EditorMode::Normal {
                    match key_name.as_str() {
                        "Down" | "j" => {
                            // Scroll hacia abajo
                            webview_for_scroll.evaluate_javascript(
                                "window.scrollBy(0, 60);",
                                None,
                                None,
                                None::<&gtk::gio::Cancellable>,
                                |_| {},
                            );
                            return gtk::glib::Propagation::Stop;
                        }
                        "Up" | "k" => {
                            // Scroll hacia arriba
                            webview_for_scroll.evaluate_javascript(
                                "window.scrollBy(0, -60);",
                                None,
                                None,
                                None::<&gtk::gio::Cancellable>,
                                |_| {},
                            );
                            return gtk::glib::Propagation::Stop;
                        }
                        "Page_Down" => {
                            webview_for_scroll.evaluate_javascript(
                                "window.scrollBy(0, window.innerHeight * 0.8);",
                                None,
                                None,
                                None::<&gtk::gio::Cancellable>,
                                |_| {},
                            );
                            return gtk::glib::Propagation::Stop;
                        }
                        "Page_Up" => {
                            webview_for_scroll.evaluate_javascript(
                                "window.scrollBy(0, -window.innerHeight * 0.8);",
                                None,
                                None,
                                None::<&gtk::gio::Cancellable>,
                                |_| {},
                            );
                            return gtk::glib::Propagation::Stop;
                        }
                        "Home" | "g" if !modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK) => {
                            webview_for_scroll.evaluate_javascript(
                                "window.scrollTo(0, 0);",
                                None,
                                None,
                                None::<&gtk::gio::Cancellable>,
                                |_| {},
                            );
                            return gtk::glib::Propagation::Stop;
                        }
                        "End" | "G" => {
                            webview_for_scroll.evaluate_javascript(
                                "window.scrollTo(0, document.body.scrollHeight);",
                                None,
                                None,
                                None::<&gtk::gio::Cancellable>,
                                |_| {},
                            );
                            return gtk::glib::Propagation::Stop;
                        }
                        _ => {}
                    }
                }

                let key_mods = KeyModifiers {
                    ctrl: modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK),
                    alt: modifiers.contains(gtk::gdk::ModifierType::ALT_MASK),
                    shift: modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK),
                };

                // En modo Normal, procesar otros keybindings
                if current_mode == EditorMode::Normal {
                    sender.input(AppMsg::KeyPress {
                        key: key_name,
                        modifiers: key_mods,
                    });
                    return gtk::glib::Propagation::Stop;
                }

                gtk::glib::Propagation::Proceed
            }
        ));
        preview_webview.add_controller(webview_key_controller);

        // Conectar señales de inserción y eliminación del TextBuffer para mantener nuestro NoteBuffer sincronizado
        let is_syncing_to_gtk_insert = model.is_syncing_to_gtk.clone();
        model.text_buffer.connect_insert_text(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            is_syncing_to_gtk_insert,
            move |_buffer, location, text| {
                if *is_syncing_to_gtk_insert.borrow() {
                    return;
                }

                let offset = location.offset() as usize;
                sender.input(AppMsg::GtkInsertText {
                    offset,
                    text: text.to_string(),
                });
            }
        ));

        let is_syncing_to_gtk_delete = model.is_syncing_to_gtk.clone();
        model.text_buffer.connect_delete_range(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            is_syncing_to_gtk_delete,
            move |_buffer, start, end| {
                if *is_syncing_to_gtk_delete.borrow() {
                    return;
                }

                let start_offset = start.offset() as usize;
                let end_offset = end.offset() as usize;
                sender.input(AppMsg::GtkDeleteRange {
                    start: start_offset,
                    end: end_offset,
                });
            }
        ));

        let link_spans = model.link_spans.clone();
        let click_text_view = text_view_actual.clone();
        // Conectar eventos de clic para actualizar posición del cursor o abrir enlaces/tags
        let click_controller = gtk::GestureClick::new();
        let tag_spans_for_click = model.tag_spans.clone();
        let note_mention_spans_for_click = model.note_mention_spans.clone();
        click_controller.connect_released(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            text_buffer,
            #[strong(rename_to = text_view)]
            click_text_view,
            #[strong]
            mode,
            #[strong]
            link_spans,
            #[strong]
            tag_spans_for_click,
            #[strong]
            note_mention_spans_for_click,
            move |gesture, _n_press, x, y| {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let current_mode = *mode.borrow();
                    if current_mode == EditorMode::Normal {
                        // Convertir coordenadas de ventana a buffer
                        let (buffer_x, buffer_y) = text_view.window_to_buffer_coords(
                            gtk::TextWindowType::Widget,
                            x as i32,
                            y as i32,
                        );

                        // Obtener el iter en la posición exacta (devuelve None si no hay texto)
                        if let Some((iter, _trailing)) =
                            text_view.iter_at_position(buffer_x, buffer_y)
                        {
                            let offset = iter.offset();

                            // Verificar si es un tag
                            if let Some(tag_span) = tag_spans_for_click
                                .borrow()
                                .iter()
                                .find(|span| offset >= span.start && offset < span.end)
                            {
                                gesture.set_state(gtk::EventSequenceState::Claimed);
                                // Guardar nota actual y buscar tag
                                sender.input(AppMsg::SaveAndSearchTag(tag_span.tag.clone()));
                                return;
                            }

                            // Verificar si es una mención @ de nota
                            if let Some(mention_span) = note_mention_spans_for_click
                                .borrow()
                                .iter()
                                .find(|span| offset >= span.start && offset < span.end)
                            {
                                gesture.set_state(gtk::EventSequenceState::Claimed);
                                // Guardar nota actual y cargar la nota mencionada
                                sender.input(AppMsg::SaveCurrentNote);
                                sender.input(AppMsg::LoadNote {
                                    name: mention_span.note_name.clone(),
                                    highlight_text: None,
                                });
                                return;
                            }

                            // Verificar si es un link
                            if let Some(link) = link_spans
                                .borrow()
                                .iter()
                                .find(|span| offset >= span.start && offset < span.end)
                            {
                                gesture.set_state(gtk::EventSequenceState::Claimed);

                                // Verificar si es un anchor link (comienza con #)
                                if link.url.starts_with('#') {
                                    // Es un enlace interno, hacer scroll al heading
                                    let anchor_id = link.url[1..].to_string(); // Remover el #
                                    sender.input(AppMsg::ScrollToAnchor(anchor_id));
                                } else {
                                    // Es un enlace externo, abrirlo
                                    if let Err(err) = gtk::gio::AppInfo::launch_default_for_uri(
                                        &link.url,
                                        None::<&gtk::gio::AppLaunchContext>,
                                    ) {
                                        eprintln!("Error al abrir enlace {}: {}", link.url, err);
                                    }
                                }
                                return;
                            }
                        }
                    }

                    // Obtener la posición del cursor después del clic
                    let cursor_mark = text_buffer.get_insert();
                    let cursor_iter = text_buffer.iter_at_mark(&cursor_mark);
                    let cursor_pos = cursor_iter.offset() as usize;

                    // Notificar al modelo para actualizar su cursor_position
                    sender.input(AppMsg::UpdateCursorPosition(cursor_pos));
                }))
                .map_err(|e| eprintln!("Panic capturado en click_controller: {:?}", e));
            }
        ));
        text_view_actual.add_controller(click_controller);

        // Controller para cerrar sidebar cuando se hace click en el área del editor
        // Lo ponemos en el editor_stack para capturar clicks tanto en modo Normal (WebView) como Insert (TextView)
        let editor_stack_for_click = model.editor_stack.clone();
        let sidebar_click_controller = gtk::GestureClick::new();
        sidebar_click_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        sidebar_click_controller.connect_pressed(gtk::glib::clone!(
            #[strong]
            sender,
            move |gesture, _n_press, _x, _y| {
                sender.input(AppMsg::CloseSidebar);
                // No consumir el evento para que los widgets hijos lo reciban también
                gesture.set_state(gtk::EventSequenceState::None);
            }
        ));
        editor_stack_for_click.add_controller(sidebar_click_controller);

        // Agregar controlador de movimiento del mouse para cambiar cursor sobre links y tags
        let motion_controller = gtk::EventControllerMotion::new();
        let motion_text_view = text_view_actual.clone();
        let tag_spans_for_motion = model.tag_spans.clone();
        let note_mention_spans_for_motion = model.note_mention_spans.clone();
        motion_controller.connect_motion(gtk::glib::clone!(
            #[strong(rename_to = text_view)]
            motion_text_view,
            #[strong]
            mode,
            #[strong]
            link_spans,
            #[strong]
            tag_spans_for_motion,
            #[strong]
            note_mention_spans_for_motion,
            move |_controller, x, y| {
                let current_mode = *mode.borrow();
                if current_mode == EditorMode::Normal {
                    // Convertir coordenadas de ventana a buffer
                    let (buffer_x, buffer_y) = text_view.window_to_buffer_coords(
                        gtk::TextWindowType::Widget,
                        x as i32,
                        y as i32,
                    );

                    // Verificar si hay texto en esa posición
                    if let Some((iter, _trailing)) = text_view.iter_at_position(buffer_x, buffer_y)
                    {
                        let offset = iter.offset();

                        let is_over_tag = tag_spans_for_motion
                            .borrow()
                            .iter()
                            .any(|span| offset >= span.start && offset < span.end);

                        let is_over_mention = note_mention_spans_for_motion
                            .borrow()
                            .iter()
                            .any(|span| offset >= span.start && offset < span.end);

                        let is_over_link = link_spans
                            .borrow()
                            .iter()
                            .any(|span| offset >= span.start && offset < span.end);

                        if is_over_link || is_over_tag || is_over_mention {
                            text_view.set_cursor_from_name(Some("pointer"));
                        } else {
                            text_view.set_cursor_from_name(Some("text"));
                        }
                    } else {
                        // No hay texto en esa posición
                        text_view.set_cursor_from_name(Some("text"));
                    }
                } else {
                    text_view.set_cursor_from_name(Some("text"));
                }
            }
        ));
        text_view_actual.add_controller(motion_controller);

        // Configurar DropTarget para detectar cuando se arrastra contenido
        let drop_target = gtk::DropTarget::new(gtk::glib::Type::STRING, gtk::gdk::DragAction::COPY);
        drop_target.connect_drop(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            mode,
            move |_target, value, _x, _y| {
                // Solo permitir drop en modo Insert
                let current_mode = *mode.borrow();
                if current_mode != EditorMode::Insert {
                    return false;
                }

                if let Ok(text) = value.get::<String>() {
                    // Procesar el texto arrastrado (puede ser URL de imagen)
                    sender.input(AppMsg::ProcessPastedText(text));
                    true
                } else {
                    false
                }
            }
        ));
        text_view_actual.add_controller(drop_target);

        // Poblar la lista de notas
        model.populate_notes_list(&sender);
        *model.is_populating_list.borrow_mut() = false;

        // Si hay una nota cargada inicialmente, expandir su carpeta y seleccionarla
        if let Some(ref note) = model.current_note {
            // Extraer solo el nombre base sin la carpeta
            let full_name = note.name();
            let note_name = full_name.split('/').last().unwrap_or(full_name).to_string();

            // Detectar la carpeta de la nota
            let note_path = note.path();
            if let Some(parent) = note_path.parent() {
                if let Ok(relative_folder) = parent.strip_prefix(model.notes_dir.root()) {
                    if !relative_folder.as_os_str().is_empty() {
                        // La nota está en una carpeta, expandirla
                        if let Some(folder_str) = relative_folder.to_str() {
                            // Expandir la carpeta y TODOS sus padres
                            let parts: Vec<&str> = folder_str.split('/').collect();
                            for i in 1..=parts.len() {
                                let sub_path = parts[..i].join("/");
                                model.expanded_folders.insert(sub_path);
                            }

                            // Repoblar para mostrar la carpeta expandida
                            model.populate_notes_list(&sender);
                            *model.is_populating_list.borrow_mut() = false;

                            println!(
                                "📂 Carpeta '{}' (y padres) expandida al inicio para mostrar nota '{}'",
                                folder_str, note_name
                            );

                            // Re-seleccionar la nota después de un delay más largo
                            let notes_list = model.notes_list.clone();
                            let note_name_clone = note_name.clone();
                            let folder_str_clone = folder_str.to_string();
                            gtk::glib::timeout_add_local_once(
                                std::time::Duration::from_millis(150),
                                move || {
                                    println!(
                                        "🔍 Buscando nota '{}' en carpeta '{}' para seleccionar...",
                                        note_name_clone, folder_str_clone
                                    );
                                    // Buscar y seleccionar la nota
                                    let mut child = notes_list.first_child();
                                    let mut found = false;
                                    let mut current_folder: Option<String> = None;

                                    while let Some(widget) = child {
                                        if let Ok(list_row) =
                                            widget.clone().downcast::<gtk::ListBoxRow>()
                                        {
                                            // Verificar si es una carpeta para trackear en qué carpeta estamos
                                            let is_folder = unsafe {
                                                list_row
                                                    .data::<bool>("is_folder")
                                                    .map(|data| *data.as_ref())
                                                    .unwrap_or(false)
                                            };

                                            if is_folder {
                                                // Actualizar la carpeta actual
                                                current_folder = unsafe {
                                                    list_row
                                                        .data::<String>("folder_name")
                                                        .map(|data| data.as_ref().clone())
                                                };
                                            } else if list_row.is_selectable() {
                                                // Intentar obtener el nombre desde set_data primero
                                                let note_name_from_data = unsafe {
                                                    list_row
                                                        .data::<String>("note_name")
                                                        .map(|data| data.as_ref().clone())
                                                };

                                                let name_matches =
                                                    if let Some(name) = note_name_from_data {
                                                        name == note_name_clone
                                                    } else if let Some(child_w) = list_row.child() {
                                                        if let Ok(box_widget) =
                                                            child_w.downcast::<gtk::Box>()
                                                        {
                                                            if let Some(label_widget) = box_widget
                                                                .first_child()
                                                                .and_then(|w| w.next_sibling())
                                                            {
                                                                if let Ok(label) = label_widget
                                                                    .downcast::<gtk::Label>(
                                                                ) {
                                                                    label.text() == note_name_clone
                                                                } else {
                                                                    false
                                                                }
                                                            } else {
                                                                false
                                                            }
                                                        } else {
                                                            false
                                                        }
                                                    } else {
                                                        false
                                                    };

                                                // Verificar que tanto el nombre como la carpeta coincidan
                                                if name_matches
                                                    && current_folder.as_ref()
                                                        == Some(&folder_str_clone)
                                                {
                                                    notes_list.select_row(Some(&list_row));
                                                    found = true;
                                                    println!(
                                                        "✅ Nota '{}' en carpeta '{}' seleccionada",
                                                        note_name_clone, folder_str_clone
                                                    );
                                                    break;
                                                }
                                            }
                                        }
                                        child = widget.next_sibling();
                                    }

                                    if !found {
                                        println!(
                                            "⚠️ No se encontró la nota '{}' en carpeta '{}' en el sidebar",
                                            note_name_clone, folder_str_clone
                                        );
                                    }
                                },
                            );
                        }
                    }
                }
            }
        }

        // Conectar evento de cambio de selección en el ListBox
        // Deshabilitado para permitir drag-and-drop. La carga se hace con click en folder_click
        /*
        let is_populating_for_select = model.is_populating_list.clone();
        let notes_list_for_focus = model.notes_list.clone();
        widgets.notes_list.connect_row_selected(
            gtk::glib::clone!(#[strong] sender, #[strong] notes_list_for_focus, #[strong] is_populating_for_select , move |_list_box, row| {
                // No cargar notas si se está repoblando la lista
                if *is_populating_for_select.borrow() {
                    return;
                }

                if let Some(row) = row {
                    notes_list_for_focus.grab_focus();

                    // Verificar si es una carpeta
                    let is_folder = unsafe {
                        row.data::<bool>("is_folder")
                            .map(|data| *data.as_ref())
                            .unwrap_or(false)
                    };

                    // Si es una carpeta, no cargar nota
                    if is_folder {
                        return;
                    }

                    // Primero intentar obtener el nombre de set_data (resultados de búsqueda)
                    let note_name = unsafe {
                        row.data::<String>("note_name")
                            .map(|data| data.as_ref().clone())
                    };

                    if let Some(name) = note_name {
                        sender.input(AppMsg::LoadNote { name: name, highlight_text: None });
                        return;
                    }

                    // Si no está en set_data, obtener desde el label (lista normal)
                    if let Some(child) = row.child() {
                        if let Ok(box_widget) = child.downcast::<gtk::Box>() {
                            // El label es el segundo hijo (después del icono)
                            if let Some(label_widget) = box_widget.first_child().and_then(|w| w.next_sibling()) {
                                if let Ok(label) = label_widget.downcast::<gtk::Label>() {
                                    let note_name = label.text().to_string();
                                    sender.input(AppMsg::LoadNote { name: note_name, highlight_text: None });
                                }
                            }
                        }
                    }
                }
            })
        );
        */

        // Conectar activación de fila (Enter o doble click)
        widgets.notes_list.connect_row_activated(gtk::glib::clone!(
            #[strong]
            sender,
            move |_list_box, row| {
                if !row.is_activatable() {
                    return;
                }

                // Verificar si es una carpeta
                let is_folder = unsafe {
                    row.data::<bool>("is_folder")
                        .map(|data| *data.as_ref())
                        .unwrap_or(false)
                };

                if is_folder {
                    // Si es una carpeta, toggle su estado
                    if let Some(folder_name) = unsafe {
                        row.data::<String>("folder_name")
                            .map(|d| d.as_ref().clone())
                    } {
                        sender.input(AppMsg::ToggleFolder(folder_name));
                    }
                    return;
                }

                // Intentar obtener el nombre de la nota de set_data (resultados de búsqueda)
                let note_name = unsafe {
                    row.data::<String>("note_name")
                        .map(|data| data.as_ref().clone())
                };

                // Obtener snippet si existe (para resaltar)
                let snippet = unsafe {
                    row.data::<String>("snippet")
                        .map(|data| data.as_ref().clone())
                };

                println!("[DEBUG row_activated] note_name obtenido: {:?}", note_name);

                if let Some(name) = note_name {
                    println!("[DEBUG row_activated] Cargando nota: '{}' con snippet: {:?}", name, snippet.as_ref().map(|s| &s[..s.len().min(50)]));
                    sender.input(AppMsg::LoadNote {
                        name,
                        highlight_text: snippet, // Pasar el snippet para resaltar
                    });
                    return;
                }

                // Si no está en set_data, intentar obtenerlo del label (lista normal)
                // IMPORTANTE: Solo para lista normal, NO para resultados de búsqueda
                if let Some(child) = row.child() {
                    if let Ok(box_widget) = child.downcast::<gtk::Box>() {
                        // Verificar si es resultado de búsqueda (tiene Box vertical con múltiples labels)
                        // o lista normal (tiene icono + label)
                        let first_child = box_widget.first_child();

                        // En lista normal: primer hijo es Image (icono), segundo es Label (nombre)
                        // En búsqueda: primer hijo es Box (name_box), luego Label (snippet)
                        if let Some(first) = first_child {
                            // Si el primer hijo es un Image, es lista normal
                            if first.type_().name() == "GtkImage" {
                                if let Some(label_widget) = first.next_sibling() {
                                    if let Ok(label) = label_widget.downcast::<gtk::Label>() {
                                        let note_name = label.text().to_string();
                                        println!("[DEBUG row_activated] Cargando nota desde label (lista normal): '{}'", note_name);
                                        sender.input(AppMsg::LoadNote { name: note_name, highlight_text: None });
                                    }
                                }
                            } else {
                                println!("[DEBUG row_activated] Estructura no reconocida, ignorando click");
                            }
                        }
                    }
                }
            }
        ));

        // Conectar click en carpetas para expandir/colapsar y cargar notas
        let folder_click = gtk::GestureClick::new();
        folder_click.connect_released(gtk::glib::clone!(
            #[strong(rename_to = notes_list)]
            widgets.notes_list,
            #[strong]
            sender,
            move |gesture, _n_press, _x, y| {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);

                    // Dar foco al notes_list para que la navegación con teclado funcione después del click
                    notes_list.grab_focus();

                    // Obtener la fila bajo el click
                    if let Some(row) = notes_list.row_at_y(y as i32) {
                        // Verificar si es una carpeta
                        let is_folder = unsafe {
                            row.data::<bool>("is_folder")
                                .map(|data| *data.as_ref())
                                .unwrap_or(false)
                        };

                        if is_folder {
                            if let Some(folder_name) = unsafe {
                                row.data::<String>("folder_name")
                                    .map(|d| d.as_ref().clone())
                            } {
                                sender.input(AppMsg::ToggleFolder(folder_name));
                            }
                        } else {
                            // Es una nota, cargarla
                            // Primero intentar obtener el nombre de set_data (resultados de búsqueda)
                            let note_name = unsafe {
                                row.data::<String>("note_name")
                                    .map(|data| data.as_ref().clone())
                            };

                            println!("[DEBUG gesture_click] note_name obtenido: {:?}", note_name);

                            if let Some(name) = note_name {
                                println!("[DEBUG gesture_click] Cargando nota: '{}'", name);
                                sender.input(AppMsg::LoadNote { name, highlight_text: None });
                                return;
                            }

                            // Si no está en set_data, obtener desde el label (lista normal)
                            // IMPORTANTE: Solo para lista normal, NO para resultados de búsqueda
                            if let Some(child) = row.child() {
                                if let Ok(box_widget) = child.downcast::<gtk::Box>() {
                                    let first_child = box_widget.first_child();

                                    // En lista normal: primer hijo es Image (icono), segundo es Label (nombre)
                                    // En búsqueda: primer hijo es Box (name_box), luego Label (snippet)
                                    if let Some(first) = first_child {
                                        // Si el primer hijo es un Image, es lista normal
                                        if first.type_().name() == "GtkImage" {
                                            if let Some(label_widget) = first.next_sibling() {
                                                if let Ok(label) = label_widget.downcast::<gtk::Label>() {
                                                    let note_name = label.text().to_string();
                                                    println!("[DEBUG gesture_click] Cargando nota desde label (lista normal): '{}'", note_name);
                                                    sender.input(AppMsg::LoadNote { name: note_name, highlight_text: None });
                                                }
                                            }
                                        } else {
                                            println!("[DEBUG gesture_click] Estructura no reconocida, ignorando click");
                                        }
                                    }
                                }
                            }
                        }
                    }
                }))
                .map_err(|e| eprintln!("Panic capturado en folder_click: {:?}", e));
            }
        ));
        widgets.notes_list.add_controller(folder_click);

        // Agregar DropTarget al notes_list para manejar drops en la raíz
        let root_drop_target = gtk::DropTarget::new(glib::Type::STRING, gtk::gdk::DragAction::MOVE);
        root_drop_target.connect_drop(gtk::glib::clone!(
            #[strong]
            sender,
            move |_target, value, _x, _y| {
                if let Ok(data_str) = value.get::<String>() {
                    // Parsear el dato arrastrado
                    if let Some((drag_type, drag_name)) = data_str.split_once(':') {
                        match drag_type {
                            "note" => {
                                // Arrastrar nota al fondo -> mover a raíz
                                sender.input(AppMsg::MoveNoteToFolder {
                                    note_name: drag_name.to_string(),
                                    folder_name: None, // None significa raíz
                                });
                                return true;
                            }
                            "folder" => {
                                // Arrastrar carpeta al fondo -> mover a raíz
                                sender.input(AppMsg::MoveFolder {
                                    folder_name: drag_name.to_string(),
                                    target_folder: None, // None significa raíz
                                });
                                return true;
                            }
                            _ => {}
                        }
                    }
                }
                false
            }
        ));
        widgets.notes_list.add_controller(root_drop_target);

        // Agregar manejador de teclas para el notes_list
        let notes_list_for_keys = model.notes_list.clone();
        let list_key_controller = gtk::EventControllerKey::new();
        list_key_controller.connect_key_pressed(gtk::glib::clone!(
            #[strong]
            notes_list_for_keys,
            #[strong]
            sender,
            move |_controller, keyval, _keycode, _modifiers| {
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

                match key_name.as_str() {
                    "Escape" => {
                        // Cerrar sidebar y devolver foco al editor (ToggleSidebar maneja el foco)
                        sender.input(AppMsg::ToggleSidebar);
                        gtk::glib::Propagation::Stop
                    }
                    "Return" => {
                        if let Some(row) = notes_list_for_keys.selected_row() {
                            row.activate();
                            gtk::glib::Propagation::Stop
                        } else {
                            gtk::glib::Propagation::Proceed
                        }
                    }
                    "Up" | "Down" => {
                        // Manejar la navegación manualmente
                        let is_down = key_name.as_str() == "Down";

                        if let Some(selected_row) = notes_list_for_keys.selected_row() {
                            let current_index = selected_row.index();
                            let target_index = if is_down {
                                current_index + 1
                            } else {
                                current_index.saturating_sub(1)
                            };

                            // Intentar seleccionar la siguiente/anterior fila
                            if let Some(target_row) = notes_list_for_keys.row_at_index(target_index)
                            {
                                notes_list_for_keys.select_row(Some(&target_row));

                                // Cargar la nota después de un delay
                                let sender_clone = sender.clone();
                                gtk::glib::timeout_add_local_once(
                                    std::time::Duration::from_millis(50),
                                    move || {
                                        // Verificar si es una carpeta
                                        let is_folder = unsafe {
                                            target_row
                                                .data::<bool>("is_folder")
                                                .map(|data| *data.as_ref())
                                                .unwrap_or(false)
                                        };

                                        if !is_folder {
                                            // Obtener nombre de la nota
                                            let note_name = unsafe {
                                                target_row
                                                    .data::<String>("note_name")
                                                    .map(|data| data.as_ref().clone())
                                            };

                                            if let Some(name) = note_name {
                                                sender_clone
                                                    .input(AppMsg::LoadNoteFromSidebar { name });
                                            } else {
                                                // Fallback: obtener del label
                                                if let Some(child) = target_row.child() {
                                                    if let Ok(box_widget) =
                                                        child.downcast::<gtk::Box>()
                                                    {
                                                        if let Some(label_widget) = box_widget
                                                            .first_child()
                                                            .and_then(|w| w.next_sibling())
                                                        {
                                                            if let Ok(label) = label_widget
                                                                .downcast::<gtk::Label>(
                                                            ) {
                                                                let note_name =
                                                                    label.text().to_string();
                                                                sender_clone.input(
                                                                    AppMsg::LoadNoteFromSidebar {
                                                                        name: note_name,
                                                                    },
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    },
                                );
                            }
                        } else if let Some(first_row) = notes_list_for_keys.row_at_index(0) {
                            // Si no hay selección, seleccionar el primer elemento
                            notes_list_for_keys.select_row(Some(&first_row));
                        }

                        // Detener propagación para mantener el foco en el sidebar
                        gtk::glib::Propagation::Stop
                    }
                    _ => gtk::glib::Propagation::Proceed,
                }
            }
        ));
        widgets.notes_list.add_controller(list_key_controller);

        // Agregar click derecho para menú contextual
        let right_click = gtk::GestureClick::new();
        right_click.set_button(3); // Botón derecho
        right_click.connect_released(gtk::glib::clone!(
            #[strong(rename_to = notes_list)]
            widgets.notes_list,
            #[strong]
            sender,
            move |_, _n_press, x, y| {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Obtener la fila bajo el click
                    if let Some(row) = notes_list.row_at_y(y as i32) {
                        // Intentar obtener datos almacenados en el row
                        let is_folder: bool =
                            unsafe { row.data("is_folder").map(|p| *p.as_ptr()).unwrap_or(false) };

                        let item_name = if is_folder {
                            // Para carpetas, usar el nombre completo almacenado
                            unsafe {
                                row.data::<String>("folder_name")
                                    .map(|p| (*p.as_ptr()).clone())
                                    .unwrap_or_default()
                            }
                        } else {
                            // Para notas, usar el nombre almacenado
                            unsafe {
                                row.data::<String>("note_name")
                                    .map(|p| (*p.as_ptr()).clone())
                                    .unwrap_or_default()
                            }
                        };

                        if !item_name.is_empty() {
                            sender.input(AppMsg::ShowContextMenu(x, y, item_name, is_folder));
                        }
                    }
                }))
                .map_err(|e| eprintln!("Panic capturado en right_click: {:?}", e));
            }
        ));
        widgets.notes_list.add_controller(right_click);

        // Agregar hover para cargar notas al pasar el ratón
        let motion_controller = gtk::EventControllerMotion::new();
        motion_controller.connect_motion(gtk::glib::clone!(
            #[strong(rename_to = notes_list)]
            widgets.notes_list,
            move |_controller, _x, y| {
                // Solo seleccionar visualmente la fila, NO cargar la nota
                // La carga se hará con click o navegación con teclado
                if let Some(row) = notes_list.row_at_y(y as i32) {
                    if row.is_selectable() {
                        notes_list.select_row(Some(&row));
                    }
                }
            }
        ));
        widgets.notes_list.add_controller(motion_controller);

        // Agregar detector de salida del mouse del sidebar para dar foco al editor
        let leave_controller = gtk::EventControllerMotion::new();
        let text_view_for_leave = model.text_view.clone();
        leave_controller.connect_leave(gtk::glib::clone!(
            #[strong]
            text_view_for_leave,
            move |_controller| {
                // Cuando el mouse sale del sidebar, dar foco al editor
                // Esto permite usar teclas de navegación (como → para cerrar sidebar)
                // sin necesidad de hacer click primero
                text_view_for_leave.grab_focus();
            }
        ));
        widgets.notes_list.add_controller(leave_controller);

        // Agregar control de teclado al ListBox para navegación con j/k
        let notes_key_controller = gtk::EventControllerKey::new();
        notes_key_controller.connect_key_pressed(gtk::glib::clone!(
            #[strong(rename_to = notes_list)]
            widgets.notes_list,
            #[strong]
            sender,
            move |_controller, keyval, _keycode, _modifiers| {
                if !notes_list.has_focus() {
                    return gtk::glib::Propagation::Proceed;
                }
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

                match key_name.as_str() {
                    "j" | "Down" => {
                        // Mover a la siguiente nota
                        if let Some(selected_row) = notes_list.selected_row() {
                            let index = selected_row.index();
                            if let Some(next_row) = notes_list.row_at_index(index + 1) {
                                notes_list.select_row(Some(&next_row));

                                // Cargar la nota seleccionada
                                // Verificar si es una carpeta
                                let is_folder = unsafe {
                                    next_row
                                        .data::<bool>("is_folder")
                                        .map(|data| *data.as_ref())
                                        .unwrap_or(false)
                                };

                                // Si no es una carpeta, cargar la nota
                                if !is_folder {
                                    // Intentar obtener el nombre de set_data (resultados de búsqueda)
                                    let note_name = unsafe {
                                        next_row
                                            .data::<String>("note_name")
                                            .map(|data| data.as_ref().clone())
                                    };

                                    if let Some(name) = note_name {
                                        sender.input(AppMsg::LoadNoteFromSidebar { name });
                                    } else {
                                        // Si no está en set_data, obtener desde el label (lista normal)
                                        if let Some(child) = next_row.child() {
                                            if let Ok(box_widget) = child.downcast::<gtk::Box>() {
                                                if let Some(label_widget) = box_widget
                                                    .first_child()
                                                    .and_then(|w| w.next_sibling())
                                                {
                                                    if let Ok(label) =
                                                        label_widget.downcast::<gtk::Label>()
                                                    {
                                                        let note_name = label.text().to_string();
                                                        sender.input(AppMsg::LoadNoteFromSidebar {
                                                            name: note_name,
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        return gtk::glib::Propagation::Stop;
                    }
                    "k" | "Up" => {
                        // Mover a la nota anterior
                        if let Some(selected_row) = notes_list.selected_row() {
                            let index = selected_row.index();
                            if index > 0 {
                                if let Some(prev_row) = notes_list.row_at_index(index - 1) {
                                    notes_list.select_row(Some(&prev_row));

                                    // Cargar la nota seleccionada
                                    // Verificar si es una carpeta
                                    let is_folder = unsafe {
                                        prev_row
                                            .data::<bool>("is_folder")
                                            .map(|data| *data.as_ref())
                                            .unwrap_or(false)
                                    };

                                    // Si no es una carpeta, cargar la nota
                                    if !is_folder {
                                        // Intentar obtener el nombre de set_data (resultados de búsqueda)
                                        let note_name = unsafe {
                                            prev_row
                                                .data::<String>("note_name")
                                                .map(|data| data.as_ref().clone())
                                        };

                                        if let Some(name) = note_name {
                                            sender.input(AppMsg::LoadNoteFromSidebar { name });
                                        } else {
                                            // Si no está en set_data, obtener desde el label (lista normal)
                                            if let Some(child) = prev_row.child() {
                                                if let Ok(box_widget) = child.downcast::<gtk::Box>()
                                                {
                                                    if let Some(label_widget) = box_widget
                                                        .first_child()
                                                        .and_then(|w| w.next_sibling())
                                                    {
                                                        if let Ok(label) =
                                                            label_widget.downcast::<gtk::Label>()
                                                        {
                                                            let note_name =
                                                                label.text().to_string();
                                                            sender.input(
                                                                AppMsg::LoadNoteFromSidebar {
                                                                    name: note_name,
                                                                },
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        return gtk::glib::Propagation::Stop;
                    }
                    "l" | "Right" | "Escape" => {
                        // Cerrar sidebar y volver al editor
                        sender.input(AppMsg::ToggleSidebar);
                        return gtk::glib::Propagation::Stop;
                    }
                    _ => {}
                }

                gtk::glib::Propagation::Proceed
            }
        ));
        widgets.notes_list.add_controller(notes_key_controller);

        // Dar foco inicial al TextView para que detecte teclas inmediatamente
        // Usar un delay más largo para asegurar que la UI esté completamente renderizada
        let text_view_for_initial_focus = text_view_actual.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
            text_view_for_initial_focus.grab_focus();
        });

        // Handler global de clicks en la ventana principal para restaurar foco
        let text_view_for_click = text_view_actual.clone();
        let click_controller = gtk::GestureClick::new();
        click_controller.connect_pressed(move |_gesture, _n_press, _x, _y| {
            // Restaurar foco al TextView cuando se haga click en cualquier lugar
            // (excepto si el click fue en un widget interactivo, GTK lo maneja)
            gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(10), {
                let tv = text_view_for_click.clone();
                move || {
                    tv.grab_focus();
                }
            });
        });
        widgets.main_window.add_controller(click_controller);

        // Handler para la barra flotante de búsqueda
        let floating_entry_clone = model.floating_search_entry.clone();
        let sender_for_floating = sender.clone();
        let timeout_id_ref = model.semantic_search_timeout_id.clone();

        floating_entry_clone.connect_search_changed(move |entry| {
            let query = entry.text().to_string();
            sender_for_floating.input(AppMsg::FloatingSearchNotes(query));
        });

        // Handler de teclas para la barra flotante (Esc, flechas, Enter)
        let floating_key_controller = gtk::EventControllerKey::new();
        let sender_for_floating_key = sender.clone();
        let floating_results_for_nav = model.floating_search_results_list.clone();
        let floating_scroll_for_nav = model.floating_search_results.clone();
        let floating_rows_for_nav = model.floating_search_rows.clone();
        let floating_in_current_note = model.floating_search_in_current_note.clone();

        floating_key_controller.connect_key_pressed(
            move |_controller, keyval, _keycode, modifiers| {
                let shift_pressed = modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK);
                let in_current_note = *floating_in_current_note.borrow();

                match keyval {
                    // Cambiar modo de búsqueda con Control
                    gtk::gdk::Key::Control_L | gtk::gdk::Key::Control_R => {
                        sender_for_floating_key.input(AppMsg::ToggleSemanticSearchWithNotification);
                        return gtk::glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::Escape => {
                        sender_for_floating_key.input(AppMsg::ToggleFloatingSearch);
                        return gtk::glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::Down => {
                        // Mover foco a la lista de resultados (al seleccionado o al primero)
                        if let Some(selected_row) = floating_results_for_nav.selected_row() {
                            selected_row.grab_focus();
                        } else if let Some(first_row) =
                            floating_rows_for_nav.borrow().first().cloned()
                        {
                            floating_results_for_nav.select_row(Some(&first_row));
                            first_row.grab_focus();
                        }
                        return gtk::glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::Up => {
                        // Navegar al resultado anterior
                        if let Some(selected_row) = floating_results_for_nav.selected_row() {
                            let index = selected_row.index();
                            if index > 0 {
                                let prev_index = index - 1;
                                let prev_row = {
                                    let rows = floating_rows_for_nav.borrow();
                                    rows.get(prev_index as usize).cloned()
                                };
                                if let Some(prev_row) = prev_row {
                                    floating_results_for_nav.select_row(Some(&prev_row));
                                    // IMPORTANTE: Dar foco a la fila para que GTK maneje el scroll correctamente
                                    prev_row.grab_focus();

                                    let scroll = floating_scroll_for_nav.clone();
                                    let prev_index_usize = prev_index as usize;
                                    gtk::glib::timeout_add_local_once(
                                        std::time::Duration::from_millis(10),
                                        move || {
                                            let adjustment = scroll.vadjustment();
                                            let current_value = adjustment.value();

                                            let estimated_row_height = 48.0;
                                            let target_start =
                                                prev_index_usize as f64 * estimated_row_height;

                                            if target_start < current_value {
                                                adjustment.set_value(target_start.max(0.0));
                                            }
                                        },
                                    );
                                }
                            }
                        } else if let Some(first_row) =
                            floating_rows_for_nav.borrow().first().cloned()
                        {
                            // Si no hay selección, seleccionar y dar foco al primer resultado
                            floating_results_for_nav.select_row(Some(&first_row));
                            first_row.grab_focus();
                        }
                        return gtk::glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter => {
                        // Shift+Enter: ir a coincidencia anterior (solo en modo búsqueda en nota)
                        if shift_pressed {
                            sender_for_floating_key.input(AppMsg::InNoteSearchPrev);
                            return gtk::glib::Propagation::Stop;
                        }

                        // Enter normal en modo búsqueda en nota: ir a siguiente coincidencia
                        if in_current_note {
                            sender_for_floating_key.input(AppMsg::InNoteSearchNext);
                            return gtk::glib::Propagation::Stop;
                        }

                        // Enter normal en modo global: cargar la nota seleccionada
                        let row_to_load = floating_results_for_nav
                            .selected_row()
                            .or_else(|| floating_rows_for_nav.borrow().first().cloned());

                        if let Some(row) = row_to_load {
                            let note_name = unsafe {
                                row.data::<String>("note_name")
                                    .map(|data| data.as_ref().clone())
                            };

                            if let Some(name) = note_name {
                                sender_for_floating_key
                                    .input(AppMsg::LoadNoteFromFloatingSearch(name));
                            }
                        }
                        return gtk::glib::Propagation::Stop;
                    }
                    _ => {}
                }
                gtk::glib::Propagation::Proceed
            },
        );
        model
            .floating_search_entry
            .add_controller(floating_key_controller);

        // Handler de activación (Enter) para el SearchEntry
        // Esto captura Enter cuando el SearchEntry lo procesa internamente
        let sender_for_activate = sender.clone();
        let floating_in_current_note_for_activate = model.floating_search_in_current_note.clone();
        model.floating_search_entry.connect_activate(move |_entry| {
            let in_current_note = *floating_in_current_note_for_activate.borrow();
            if in_current_note {
                sender_for_activate.input(AppMsg::InNoteSearchNext);
            }
        });

        // Handler de activación de resultados en la barra flotante
        let floating_results_clone = model.floating_search_results_list.clone();
        let sender_for_results = sender.clone();
        floating_results_clone.connect_row_activated(move |_list, row| {
            // Obtener el nombre de la nota desde los datos de la fila
            let note_name = unsafe {
                row.data::<String>("note_name")
                    .map(|data| data.as_ref().clone())
            };

            if let Some(name) = note_name {
                sender_for_results.input(AppMsg::LoadNoteFromFloatingSearch(name));
            }
        });

        // Handler de teclado para la lista de resultados flotantes
        let floating_list_key_controller = gtk::EventControllerKey::new();
        let sender_for_list_keys = sender.clone();
        let floating_entry_for_focus = model.floating_search_entry.clone();
        let floating_results_for_enter = model.floating_search_results_list.clone();
        let floating_scroll_for_list = model.floating_search_results.clone();

        floating_list_key_controller.connect_key_pressed(
            move |_controller, keyval, _keycode, _modifiers| {
                match keyval {
                    gtk::gdk::Key::Escape => {
                        sender_for_list_keys.input(AppMsg::ToggleFloatingSearch);
                        return gtk::glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::Up => {
                        if let Some(row) = floating_results_for_enter.selected_row() {
                            if let Some(prev) = row.prev_sibling() {
                                if let Some(prev_row) = prev.downcast_ref::<gtk::ListBoxRow>() {
                                    floating_results_for_enter.select_row(Some(prev_row));
                                    prev_row.grab_focus();

                                    // Scroll manual para asegurar visibilidad
                                    let scroll = floating_scroll_for_list.clone();
                                    let row_y = prev_row
                                        .compute_bounds(&floating_results_for_enter)
                                        .map(|r| r.y())
                                        .unwrap_or(0.0)
                                        as f64;

                                    gtk::glib::timeout_add_local_once(
                                        std::time::Duration::from_millis(10),
                                        move || {
                                            let adjustment = scroll.vadjustment();
                                            if row_y < adjustment.value() {
                                                adjustment.set_value(row_y);
                                            }
                                        },
                                    );
                                }
                            } else {
                                // Si estamos en el primer elemento, volver al input
                                floating_entry_for_focus.grab_focus();
                            }
                        }
                        return gtk::glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::Down => {
                        if let Some(row) = floating_results_for_enter.selected_row() {
                            if let Some(next) = row.next_sibling() {
                                if let Some(next_row) = next.downcast_ref::<gtk::ListBoxRow>() {
                                    floating_results_for_enter.select_row(Some(next_row));
                                    next_row.grab_focus();

                                    // Scroll manual
                                    let scroll = floating_scroll_for_list.clone();
                                    let row_height = next_row.height() as f64;
                                    let row_y = next_row
                                        .compute_bounds(&floating_results_for_enter)
                                        .map(|r| r.y())
                                        .unwrap_or(0.0)
                                        as f64;

                                    gtk::glib::timeout_add_local_once(
                                        std::time::Duration::from_millis(10),
                                        move || {
                                            let adjustment = scroll.vadjustment();
                                            let page_size = adjustment.page_size();
                                            if row_y + row_height > adjustment.value() + page_size {
                                                adjustment
                                                    .set_value(row_y + row_height - page_size);
                                            }
                                        },
                                    );
                                }
                            }
                        }
                        return gtk::glib::Propagation::Stop;
                    }
                    gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter => {
                        // Cargar la nota seleccionada
                        if let Some(selected_row) = floating_results_for_enter.selected_row() {
                            let note_name = unsafe {
                                selected_row
                                    .data::<String>("note_name")
                                    .map(|data| data.as_ref().clone())
                            };

                            if let Some(name) = note_name {
                                sender_for_list_keys
                                    .input(AppMsg::LoadNoteFromFloatingSearch(name));
                            }
                        }
                        return gtk::glib::Propagation::Stop;
                    }
                    _ => {
                        // Cualquier otra tecla devuelve el foco al entry para seguir escribiendo
                        if keyval.to_unicode().is_some() {
                            floating_entry_for_focus.grab_focus();
                            return gtk::glib::Propagation::Proceed;
                        }
                    }
                }
                gtk::glib::Propagation::Proceed
            },
        );
        model
            .floating_search_results_list
            .add_controller(floating_list_key_controller);

        // Timer para verificar si debe reproducir la siguiente canción (cada 2 segundos)
        let sender_clone = sender.clone();
        gtk::glib::timeout_add_seconds_local(2, move || {
            sender_clone.input(AppMsg::MusicCheckNextSong);
            gtk::glib::ControlFlow::Continue
        });

        // Crear system tray icon (pasar i18n para traducciones y estado de visibilidad)
        crate::system_tray::create_system_tray(
            sender.clone(),
            model.i18n.clone(),
            model.window_visible.clone(),
        );

        // Click en el indicador de modo para cambiar entre modos
        let mode_click = gtk::GestureClick::new();
        let mode_label_for_click = model.mode_label.clone();
        let text_view_for_mode_click = text_view_actual.clone();
        let buffer_for_mode_click = text_buffer.clone();
        mode_click.connect_released(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            mode,
            #[strong(rename_to = mode_label)]
            mode_label_for_click,
            move |_gesture, _n_press, _x, _y| {
                let current_mode = *mode.borrow();

                // Ciclar entre modos: Normal -> Insert -> Chat AI -> Normal
                let new_mode = match current_mode {
                    EditorMode::Normal => EditorMode::Insert,
                    EditorMode::Insert => EditorMode::ChatAI,
                    EditorMode::ChatAI => EditorMode::Normal,
                    EditorMode::Visual => EditorMode::Normal,
                    EditorMode::Command => EditorMode::Normal,
                };

                // Usar el mensaje apropiado para cambiar de modo
                match new_mode {
                    EditorMode::ChatAI => {
                        sender.input(AppMsg::EnterChatMode);
                    }
                    EditorMode::Normal | EditorMode::Insert => {
                        // Si estamos saliendo del modo Chat AI, primero salir
                        if current_mode == EditorMode::ChatAI {
                            sender.input(AppMsg::ExitChatMode);
                        }
                        // Usar ProcessAction para cambiar el modo correctamente
                        sender.input(AppMsg::ProcessAction(EditorAction::ChangeMode(new_mode)));
                    }
                    _ => {
                        sender.input(AppMsg::ProcessAction(EditorAction::ChangeMode(new_mode)));
                    }
                }
            }
        ));
        model.mode_label.add_controller(mode_click);

        // Verificar si debe iniciar en segundo plano
        let start_in_background = model.notes_config.borrow().get_start_in_background();
        if start_in_background {
            widgets.main_window.set_visible(false);
            model
                .window_visible
                .store(false, std::sync::atomic::Ordering::Relaxed);
            println!("Iniciando en segundo plano (minimizado)");
        }

        // Sincronizar estado de autostart (asegurar que el archivo .desktop exista si está habilitado)
        if let Err(e) = Self::manage_autostart(start_in_background) {
            eprintln!("Error sincronizando autostart al inicio: {}", e);
        }

        // Actualizar tooltips según el idioma actual al inicio
        {
            let i18n = model.i18n.borrow();
            model
                .sidebar_toggle_button
                .set_tooltip_text(Some(&i18n.t("show_hide_notes")));
            model
                .search_toggle_button
                .set_tooltip_text(Some(&i18n.t("search_notes")));
            model
                .new_note_button
                .set_tooltip_text(Some(&i18n.t("new_note")));
            model
                .settings_button
                .set_tooltip_text(Some(&i18n.t("settings")));
            model
                .tags_menu_button
                .set_tooltip_text(Some(&i18n.t("tags_note")));
            model
                .todos_menu_button
                .set_tooltip_text(Some(&i18n.t("todos_note")));
            model
                .music_player_button
                .set_tooltip_text(Some(&i18n.t("music_player")));
            model
                .reminders_button
                .set_tooltip_text(Some(&i18n.t("reminder_tooltip")));
            model.sidebar_notes_label.set_label(&i18n.t("notes"));
            model
                .floating_search_entry
                .set_placeholder_text(Some(&i18n.t("search_placeholder")));
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: AppMsg, sender: ComponentSender<Self>) {
        match message {
            AppMsg::ToggleTheme => {
                self.theme = match self.theme {
                    ThemePreference::FollowSystem => ThemePreference::Dark,
                    ThemePreference::Light => ThemePreference::Dark,
                    ThemePreference::Dark => ThemePreference::Light,
                };
                self.refresh_style_manager();
            }
            AppMsg::SetTheme(theme) => {
                self.theme = theme;
                self.refresh_style_manager();
            }
            AppMsg::RefreshTheme => {
                // Recrear los tags de texto para adaptar colores al nuevo tema
                self.create_text_tags();

                // Re-aplicar estilos markdown si está habilitado
                if self.markdown_enabled {
                    self.sync_to_view();
                }

                println!("Tema actualizado dinámicamente");
            }
            AppMsg::Toggle8BitMode => {
                self.bit8_mode = !self.bit8_mode;
                self.apply_8bit_font();
            }
            AppMsg::ToggleSidebar => {
                let mode = *self.mode.borrow();

                // En modo Chat AI, toggle el sidebar de contexto
                if mode == EditorMode::ChatAI {
                    let current_pos = self.chat_split_view.position();
                    let target_position = if current_pos > 0 { 0 } else { 250 };
                    self.chat_split_view.set_position(target_position);

                    if target_position == 0 {
                        // Dar foco al input del chat
                        let chat_input = self.chat_input_view.clone();
                        gtk::glib::timeout_add_local_once(
                            std::time::Duration::from_millis(160),
                            move || {
                                chat_input.grab_focus();
                            },
                        );
                    }
                } else {
                    // En modo Normal, toggle el sidebar principal
                    self.sidebar_visible = !self.sidebar_visible;
                    let target_position = if self.sidebar_visible { 250 } else { 0 };
                    self.animate_sidebar(target_position);

                    // Si estamos cerrando el sidebar, devolver foco al widget correcto según el modo
                    if !self.sidebar_visible {
                        let current_mode = *self.mode.borrow();
                        let markdown_enabled = self.markdown_enabled;
                        let text_view = self.text_view.clone();
                        let preview_webview = self.preview_webview.clone();

                        gtk::glib::timeout_add_local_once(
                            std::time::Duration::from_millis(160),
                            move || {
                                if current_mode == EditorMode::Normal && markdown_enabled {
                                    preview_webview.grab_focus();
                                } else {
                                    text_view.grab_focus();
                                }
                            },
                        );
                    }
                }
            }

            AppMsg::CloseSidebar => {
                // Cerrar sidebar si está abierto (en modo Normal o Insert)
                let mode = *self.mode.borrow();
                if mode != EditorMode::ChatAI && self.sidebar_visible {
                    self.sidebar_visible = false;
                    self.animate_sidebar(0);
                }
            }

            AppMsg::CloseSidebarAndOpenSearch => {
                // Cerrar sidebar si está abierto (en modo Normal)
                let mode = *self.mode.borrow();
                if mode != EditorMode::ChatAI && self.sidebar_visible {
                    self.sidebar_visible = false;
                    self.animate_sidebar(0);
                }

                // Abrir barra flotante de búsqueda
                if !self.floating_search_visible {
                    sender.input(AppMsg::ToggleFloatingSearch);
                }
            }

            AppMsg::OpenSidebarAndFocus => {
                // Abrir sidebar si está cerrado
                if !self.sidebar_visible {
                    self.sidebar_visible = true;
                    self.animate_sidebar(250);
                }

                // Determinar la nota actualmente cargada y su carpeta para re-seleccionarla al abrir
                let (current_note_name, current_folder) = if let Some(ref note) = self.current_note
                {
                    let full_name = note.name();
                    let base_name = full_name.split('/').last().unwrap_or(full_name).to_string();

                    // Detectar la carpeta de la nota
                    let folder = note
                        .path()
                        .parent()
                        .and_then(|p| p.strip_prefix(self.notes_dir.root()).ok())
                        .filter(|p| !p.as_os_str().is_empty())
                        .and_then(|p| p.to_str())
                        .map(|s| s.to_string());

                    (Some(base_name), folder)
                } else {
                    (None, None)
                };

                // Dar foco al ListBox después de un pequeño delay para que termine la animación
                let notes_list = self.notes_list.clone();
                gtk::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(160),
                    move || {
                        notes_list.grab_focus();

                        // Si hay una nota actualmente abierta, intentar seleccionarla
                        if let Some(note_name) = current_note_name.clone() {
                            // Buscar la fila con esa nota en la carpeta correcta
                            let mut child = notes_list.first_child();
                            let mut found = false;
                            let mut current_folder_in_list: Option<String> = None;

                            while let Some(widget) = child {
                                if let Ok(list_row) = widget.clone().downcast::<gtk::ListBoxRow>() {
                                    // Verificar si es una carpeta para trackear en qué carpeta estamos
                                    let is_folder = unsafe {
                                        list_row
                                            .data::<bool>("is_folder")
                                            .map(|data| *data.as_ref())
                                            .unwrap_or(false)
                                    };

                                    if is_folder {
                                        // Actualizar la carpeta actual
                                        current_folder_in_list = unsafe {
                                            list_row
                                                .data::<String>("folder_name")
                                                .map(|data| data.as_ref().clone())
                                        };
                                    } else if list_row.is_selectable() {
                                        // Intentar obtener el nombre desde set_data primero
                                        let note_name_from_data = unsafe {
                                            list_row
                                                .data::<String>("note_name")
                                                .map(|data| data.as_ref().clone())
                                        };

                                        let name_matches = if let Some(name) = note_name_from_data {
                                            name == note_name
                                        } else if let Some(child_w) = list_row.child() {
                                            if let Ok(box_widget) = child_w.downcast::<gtk::Box>() {
                                                if let Some(label_widget) = box_widget
                                                    .first_child()
                                                    .and_then(|w| w.next_sibling())
                                                {
                                                    if let Ok(label) =
                                                        label_widget.downcast::<gtk::Label>()
                                                    {
                                                        label.text() == note_name
                                                    } else {
                                                        false
                                                    }
                                                } else {
                                                    false
                                                }
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        };

                                        // Verificar que tanto el nombre como la carpeta coincidan
                                        if name_matches && current_folder_in_list == current_folder
                                        {
                                            notes_list.select_row(Some(&list_row));
                                            found = true;
                                            break;
                                        }
                                    }
                                }
                                child = widget.next_sibling();
                            }

                            // Si no se encontró y no hay nada seleccionado, intentar sin verificar carpeta (fallback)
                            if !found && notes_list.selected_row().is_none() {
                                let mut child = notes_list.first_child();
                                while let Some(widget) = child {
                                    if let Ok(list_row) =
                                        widget.clone().downcast::<gtk::ListBoxRow>()
                                    {
                                        if list_row.is_selectable() {
                                            if let Some(child_w) = list_row.child() {
                                                if let Ok(box_widget) =
                                                    child_w.downcast::<gtk::Box>()
                                                {
                                                    if let Some(label_widget) = box_widget
                                                        .first_child()
                                                        .and_then(|w| w.next_sibling())
                                                    {
                                                        if let Ok(label) =
                                                            label_widget.downcast::<gtk::Label>()
                                                        {
                                                            if label.text() == note_name {
                                                                notes_list
                                                                    .select_row(Some(&list_row));
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    child = widget.next_sibling();
                                }
                            }
                        }

                        // Si no hay nada seleccionado (no había nota abierta), seleccionar primero
                        if notes_list.selected_row().is_none() {
                            if let Some(first_row) = notes_list.row_at_index(0) {
                                notes_list.select_row(Some(&first_row));
                            }
                        }
                    },
                );
            }
            AppMsg::KeyPress { key, modifiers } => {
                let current_mode = *self.mode.borrow();

                // PRIORIDAD 1: Si ESC y la barra flotante está abierta, cerrarla
                if key == "Escape" && self.floating_search_visible {
                    self.floating_search_visible = false;
                    self.floating_search_bar.set_visible(false);

                    // Limpiar resultados (preservando semantic_search_answer_row)
                    let answer_row_ptr = self.semantic_search_answer_row.as_ptr();
                    let mut child = self.floating_search_results_list.first_child();
                    while let Some(widget) = child {
                        let next = widget.next_sibling();
                        // No eliminar el semantic_search_answer_row
                        if widget.as_ptr() != answer_row_ptr as *mut _ {
                            self.floating_search_results_list.remove(&widget);
                        }
                        child = next;
                    }
                    // Ocultar el answer_row pero mantenerlo en el árbol
                    self.semantic_search_answer_row.set_visible(false);

                    // Devolver foco al editor correcto según el modo
                    if current_mode == EditorMode::Normal && self.markdown_enabled {
                        self.preview_webview.grab_focus();
                    } else {
                        self.text_view.grab_focus();
                    }
                    return;
                }

                // DEBUG: Mostrar estado cuando se presiona Tab
                if key == "Tab" && current_mode == EditorMode::Insert {
                    println!("DEBUG: Tab presionado en Insert mode");
                    println!(
                        "DEBUG: current_tag_prefix = {:?}",
                        *self.current_tag_prefix.borrow()
                    );
                    println!(
                        "DEBUG: current_mention_prefix = {:?}",
                        *self.current_mention_prefix.borrow()
                    );
                }

                // Interceptar Tab en modo INSERT para autocompletado de tags
                if current_mode == EditorMode::Insert
                    && key == "Tab"
                    && self.current_tag_prefix.borrow().is_some()
                {
                    // Buscar sugerencias de tags
                    if let Ok(all_tags) = self.notes_db.get_tags() {
                        let prefix = self.current_tag_prefix.borrow().clone().unwrap();
                        let matches: Vec<_> = all_tags
                            .iter()
                            .filter(|t| t.name.starts_with(&prefix.to_lowercase()))
                            .collect();

                        if let Some(first_match) = matches.first() {
                            // Completar con el primer match
                            println!("DEBUG: Completando tag con: {}", first_match.name);
                            sender.input(AppMsg::CompleteTag(first_match.name.clone()));
                            return;
                        }
                    }
                }

                // Interceptar Tab en modo INSERT para autocompletado de menciones @
                if current_mode == EditorMode::Insert
                    && key == "Tab"
                    && self.current_mention_prefix.borrow().is_some()
                {
                    println!("DEBUG: Intentando autocompletar mención");
                    // Buscar sugerencias de notas
                    if let Ok(notes) = self.notes_dir.list_notes() {
                        let prefix = self.current_mention_prefix.borrow().clone().unwrap();
                        println!("DEBUG: Prefix de mención: {}", prefix);
                        let matches: Vec<_> = notes
                            .iter()
                            .filter(|note| {
                                let note_name = note.name().to_lowercase();
                                let base_name = if let Some(idx) = note_name.rfind('/') {
                                    &note_name[idx + 1..]
                                } else {
                                    &note_name
                                };
                                base_name.contains(&prefix)
                            })
                            .collect();

                        println!("DEBUG: Encontradas {} coincidencias", matches.len());
                        if let Some(first_match) = matches.first() {
                            // Completar con el primer match (sin .md)
                            let note_name = first_match.name().trim_end_matches(".md");
                            println!("DEBUG: Completando mención con: {}", note_name);
                            sender.input(AppMsg::CompleteMention(note_name.to_string()));
                            return;
                        }
                    }
                }

                // Cerrar popover de autocompletado con Escape o al salir de modo INSERT
                if key == "Escape"
                    || (current_mode != EditorMode::Insert
                        && self.tag_completion_popup.is_visible())
                {
                    self.tag_completion_popup.popdown();
                    *self.current_tag_prefix.borrow_mut() = None;
                }

                // Cerrar popover de menciones con Escape
                if key == "Escape"
                    || (current_mode != EditorMode::Insert && self.note_mention_popup.is_visible())
                {
                    self.note_mention_popup.popdown();
                    *self.current_mention_prefix.borrow_mut() = None;
                }

                // Atajo global: Ctrl+Shift+A para entrar al Chat AI desde cualquier modo
                if modifiers.ctrl && modifiers.shift && key == "a" {
                    sender.input(AppMsg::EnterChatMode);
                    return;
                }

                let action = match current_mode {
                    EditorMode::Normal => self.command_parser.parse_normal_mode(&key, modifiers),
                    EditorMode::Insert => self.command_parser.parse_insert_mode(&key, modifiers),
                    EditorMode::Command => {
                        // En modo comando, acumular input hasta Enter
                        // Por ahora, simplificamos
                        EditorAction::None
                    }
                    EditorMode::Visual => EditorAction::None,
                    EditorMode::ChatAI => {
                        // En modo Chat AI, Escape sale del modo
                        if key == "Escape" {
                            sender.input(AppMsg::ExitChatMode);
                            return;
                        }
                        // Enter envía el mensaje
                        if key == "Return" || key == "Enter" {
                            if !modifiers.shift {
                                let start = self.chat_input_buffer.start_iter();
                                let end = self.chat_input_buffer.end_iter();
                                let text =
                                    self.chat_input_buffer.text(&start, &end, false).to_string();
                                if !text.trim().is_empty() {
                                    sender.input(AppMsg::SendChatMessage(text));
                                }
                                return;
                            }
                        }
                        EditorAction::None
                    }
                };

                if action != EditorAction::None {
                    sender.input(AppMsg::ProcessAction(action));
                }
            }
            AppMsg::ProcessAction(action) => {
                self.execute_action(action, &sender);
            }
            AppMsg::SaveCurrentNote => {
                self.save_current_note(true);
                // Escanear recordatorios solo cuando se guarda manualmente (Ctrl+S)
                sender.input(AppMsg::ParseRemindersInNote);
            }
            AppMsg::AutoSave => {
                // Solo guardar si hay cambios sin guardar
                if self.has_unsaved_changes {
                    self.save_current_note(false);
                    // NO escanear recordatorios en autoguardado para evitar duplicados
                    println!("Autoguardado ejecutado");
                }
            }
            AppMsg::LoadNote {
                name,
                highlight_text,
            } => {
                // Guardar nota actual antes de cambiar (con embeddings)
                // Solo si hay una nota actual O si hay cambios sin guardar (scratchpad)
                if self.current_note.is_some() || self.has_unsaved_changes {
                    self.save_current_note(true);
                }

                // Limpiar nombre de nota por si viene sucio desde la IA (hallucinación de contexto)
                // Ej: "System: === Folder/Note ===" -> "Folder/Note"
                let clean_name = name
                    .trim()
                    .trim_start_matches("System:")
                    .trim()
                    .trim_start_matches("===")
                    .trim_end_matches("===")
                    .trim()
                    .to_string();

                if let Err(e) = self.load_note(&clean_name) {
                    eprintln!(
                        "Error cargando nota '{}' (original: '{}'): {}",
                        clean_name, name, e
                    );
                } else {
                    // Invalidar cache al cargar nueva nota
                    *self.cached_source_text.borrow_mut() = None;
                    *self.cached_rendered_text.borrow_mut() = None;

                    // Asegurar que estamos viendo el editor (por si venimos del chat)
                    self.content_stack.set_visible_child_name("editor");

                    // Si estamos en modo ChatAI, cambiar a Normal para poder editar/ver la nota
                    if *self.mode.borrow() == EditorMode::ChatAI {
                        *self.mode.borrow_mut() = EditorMode::Normal;
                    }

                    // Sincronizar vista y actualizar UI
                    self.sync_to_view();
                    self.update_status_bar(&sender);
                    self.refresh_tags_display_with_sender(&sender);
                    self.refresh_todos_summary();
                    self.window_title.set_label(&clean_name);
                    self.has_unsaved_changes = false;

                    // Si hay texto para resaltar, hacerlo
                    if let Some(text_to_highlight) = highlight_text {
                        self.highlight_and_scroll_to_text(&text_to_highlight);
                    }

                    // IMPORTANTE: Solo devolver el foco al editor si el sidebar no está abierto
                    // o si el sidebar no tiene el foco actualmente (para permitir navegación con teclado)
                    if !self.sidebar_visible || !self.notes_list.has_focus() {
                        let current_mode = *self.mode.borrow();
                        if current_mode == EditorMode::Normal && self.markdown_enabled {
                            self.preview_webview.grab_focus();
                        } else {
                            self.text_view.grab_focus();
                        }
                    }
                }
            }
            AppMsg::LoadNoteFromSidebar { name } => {
                // Cargar nota desde el sidebar SIN cambiar el foco
                // (permite navegación continua con flechas)
                if self.current_note.is_some() || self.has_unsaved_changes {
                    self.save_current_note(true);
                }

                let clean_name = name
                    .trim()
                    .trim_start_matches("System:")
                    .trim()
                    .trim_start_matches("===")
                    .trim_end_matches("===")
                    .trim()
                    .to_string();

                if let Err(e) = self.load_note(&clean_name) {
                    eprintln!("Error cargando nota '{}': {}", clean_name, e);
                } else {
                    *self.cached_source_text.borrow_mut() = None;
                    *self.cached_rendered_text.borrow_mut() = None;
                    self.content_stack.set_visible_child_name("editor");

                    if *self.mode.borrow() == EditorMode::ChatAI {
                        *self.mode.borrow_mut() = EditorMode::Normal;
                    }

                    // Usar sync_to_view_no_focus para NO robar el foco del sidebar
                    self.sync_to_view_no_focus();
                    self.update_status_bar(&sender);
                    self.refresh_tags_display_with_sender(&sender);
                    self.refresh_todos_summary();
                    self.window_title.set_label(&clean_name);
                    self.has_unsaved_changes = false;

                    // Forzar que el foco vuelva al sidebar
                    let notes_list = self.notes_list.clone();
                    gtk::glib::idle_add_local_once(move || {
                        notes_list.grab_focus();
                    });
                }
            }
            AppMsg::CreateNewNote(name) => {
                // Limpiar nombre de nota por si viene sucio desde la IA
                let clean_name = name
                    .trim()
                    .trim_start_matches("System:")
                    .trim()
                    .trim_start_matches("===")
                    .trim_end_matches("===")
                    .trim()
                    .to_string();

                let is_folder_only = clean_name.ends_with('/');

                if let Err(e) = self.create_new_note(&clean_name) {
                    eprintln!(
                        "Error creando '{}' (original: '{}'): {}",
                        clean_name, name, e
                    );
                } else if is_folder_only {
                    // Solo se creó una carpeta, refrescar sidebar
                    self.populate_notes_list(&sender);
                    *self.is_populating_list.borrow_mut() = false;
                    println!("📂 Carpeta creada y sidebar actualizado");
                } else {
                    // Se creó una nota, hacer el proceso completo
                    self.sync_to_view();
                    self.update_status_bar(&sender);
                    self.refresh_tags_display_with_sender(&sender);
                    self.refresh_todos_summary();
                    self.window_title.set_label(&clean_name);

                    // Refrescar lista de notas en el sidebar
                    self.populate_notes_list(&sender);
                    *self.is_populating_list.borrow_mut() = false;

                    // Cambiar a modo Insert para empezar a escribir
                    *self.mode.borrow_mut() = EditorMode::Insert;
                }
            }
            AppMsg::UpdateCursorPosition(pos) => {
                // Actualizar la posición del cursor cuando el usuario hace clic
                // En modo Normal, GTK muestra texto limpio, entonces pos es posición display
                // Necesitamos mapear a posición buffer, PERO solo si markdown está habilitado
                let current_mode = *self.mode.borrow();
                if current_mode == EditorMode::Normal && self.markdown_enabled {
                    // En modo Normal, el texto mostrado es "limpio"
                    // Necesitamos encontrar la posición correspondiente en el buffer original
                    let buffer_text = self.buffer.to_string();
                    self.cursor_position = self.map_display_pos_to_buffer(&buffer_text, pos);
                } else {
                    self.cursor_position = pos;
                }
            }

            AppMsg::ScrollToAnchor(anchor_id) => {
                // Buscar el heading con el ID especificado
                if let Some(anchor) = self
                    .heading_anchors
                    .borrow()
                    .iter()
                    .find(|a| a.id == anchor_id)
                    .cloned()
                {
                    // Crear un iterador en la posición del heading
                    let mut iter = self.text_buffer.start_iter();
                    iter.set_offset(anchor.line_offset);

                    // Hacer scroll a esa posición
                    self.text_view
                        .scroll_to_iter(&mut iter, 0.0, true, 0.0, 0.1);

                    // Opcionalmente, mostrar una notificación
                    println!("📍 Navegando a: {}", anchor.text);
                } else {
                    eprintln!("⚠️ No se encontró el heading con ID: #{}", anchor_id);
                }
            }

            AppMsg::ShowCreateNoteDialog => {
                self.show_create_note_dialog(&sender);
            }

            AppMsg::ToggleFolder(folder_name) => {
                // Activar flag durante la repoblación
                *self.is_populating_list.borrow_mut() = true;

                // Toggle el estado de la carpeta
                let was_expanded = self.expanded_folders.contains(&folder_name);
                let just_expanded = !was_expanded;

                if was_expanded {
                    self.expanded_folders.remove(&folder_name);
                } else {
                    self.expanded_folders.insert(folder_name.clone());
                }

                // Refrescar la lista para mostrar/ocultar las notas
                self.populate_notes_list(&sender);

                // Re-seleccionar después de refrescar
                let notes_list = self.notes_list.clone();
                let folder_name_clone = folder_name.clone();
                let is_populating_clone = self.is_populating_list.clone();

                gtk::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(50),
                    move || {
                        // Si la carpeta se acaba de expandir, contar las notas visibles
                        let mut notes_in_folder = Vec::new();
                        let mut folder_row_opt = None;

                        if just_expanded {
                            let mut child = notes_list.first_child();
                            let mut in_target_folder = false;

                            while let Some(widget) = child {
                                if let Ok(row) = widget.clone().downcast::<gtk::ListBoxRow>() {
                                    let is_folder = unsafe {
                                        row.data::<bool>("is_folder")
                                            .map(|data| *data.as_ref())
                                            .unwrap_or(false)
                                    };

                                    if is_folder {
                                        if let Some(row_folder) = unsafe {
                                            row.data::<String>("folder_name")
                                                .map(|d| d.as_ref().clone())
                                        } {
                                            if row_folder == folder_name_clone {
                                                in_target_folder = true;
                                                folder_row_opt = Some(row.clone());
                                            } else if in_target_folder {
                                                // Llegamos a otra carpeta, terminamos
                                                break;
                                            }
                                        }
                                    } else if in_target_folder {
                                        // Es una nota dentro de nuestra carpeta
                                        if let Some(child_box) = row.child() {
                                            if let Ok(box_widget) = child_box.downcast::<gtk::Box>()
                                            {
                                                if let Some(label_widget) = box_widget
                                                    .first_child()
                                                    .and_then(|w| w.next_sibling())
                                                {
                                                    if let Ok(label) =
                                                        label_widget.downcast::<gtk::Label>()
                                                    {
                                                        notes_in_folder.push((
                                                            label.text().to_string(),
                                                            row.clone(),
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                child = widget.next_sibling();
                            }
                        }

                        // Si hay exactamente una nota, seleccionarla
                        if notes_in_folder.len() == 1 {
                            let (_, row) = &notes_in_folder[0];
                            notes_list.select_row(Some(row));

                            // Hacer scroll a la fila seleccionada
                            if let Some(scrolled) = notes_list
                                .parent()
                                .and_then(|p| p.parent())
                                .and_then(|p| p.downcast::<gtk::ScrolledWindow>().ok())
                            {
                                let adjustment = scrolled.vadjustment();
                                let row_y = row.allocation().y() as f64;
                                adjustment.set_value(row_y);
                            }
                        } else if let Some(folder_row) = folder_row_opt {
                            // Si no es carpeta con una sola nota, re-seleccionar la carpeta
                            notes_list.select_row(Some(&folder_row));

                            // Hacer scroll a la fila seleccionada
                            if let Some(scrolled) = notes_list
                                .parent()
                                .and_then(|p| p.parent())
                                .and_then(|p| p.downcast::<gtk::ScrolledWindow>().ok())
                            {
                                let adjustment = scrolled.vadjustment();
                                let row_y = folder_row.allocation().y() as f64;
                                adjustment.set_value(row_y);
                            }
                        }

                        // Restaurar el foco a la lista para permitir navegación con teclado
                        notes_list.grab_focus();

                        // Desactivar flag después de re-seleccionar
                        *is_populating_clone.borrow_mut() = false;
                    },
                );
            }

            AppMsg::ShowContextMenu(x, y, item_name, is_folder) => {
                *self.context_item_name.borrow_mut() = item_name;
                *self.context_is_folder.borrow_mut() = is_folder;

                // Recrear el menú con las traducciones actuales
                let i18n = self.i18n.borrow();
                let menu = gtk::gio::Menu::new();

                // Agregar opción de "Abrir en explorador"
                menu.append(
                    Some(&i18n.t("open_in_file_manager")),
                    Some("item.open_folder"),
                );
                menu.append(Some(&i18n.t("change_icon")), Some("item.change_icon"));
                menu.append(Some(&i18n.t("rename")), Some("item.rename"));
                menu.append(Some(&i18n.t("delete")), Some("item.delete"));
                self.context_menu.set_menu_model(Some(&menu));

                // Establecer parent solo cuando se va a mostrar
                self.context_menu.set_parent(&self.notes_list);

                let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                self.context_menu.set_pointing_to(Some(&rect));
                self.context_menu.popup();
            }

            AppMsg::DeleteItem(item_name, is_folder) => {
                self.context_menu.popdown();
                self.context_menu.unparent();

                if is_folder {
                    println!("Eliminar carpeta: {}", item_name);

                    // Construir la ruta completa de la carpeta
                    let folder_path = self.notes_dir.root().join(&item_name);

                    if folder_path.exists() && folder_path.is_dir() {
                        // 1. Eliminar notas de la base de datos PRIMERO (incluyendo embeddings)
                        if let Err(e) = self.notes_db.delete_notes_in_folder(&item_name) {
                            eprintln!("Error al eliminar notas de la carpeta en BD: {}", e);
                        }

                        // 2. Mover carpeta a la papelera
                        let trash_path = self.notes_dir.trash_path();
                        if !trash_path.exists() {
                            let _ = std::fs::create_dir_all(&trash_path);
                        }

                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        let safe_name = item_name.replace('/', "_");
                        let trash_folder_name = format!("{}_{}", safe_name, timestamp);
                        let dest_path = trash_path.join(trash_folder_name);

                        if let Err(e) = std::fs::rename(&folder_path, &dest_path) {
                            eprintln!("Error al mover carpeta a papelera: {}", e);
                            // Fallback: intentar eliminar si no se puede mover
                            if let Err(e) = std::fs::remove_dir_all(&folder_path) {
                                eprintln!("Error al eliminar carpeta: {}", e);
                            }
                        } else {
                            println!("Carpeta movida a papelera: {}", item_name);

                            // Si la nota actual estaba en esta carpeta, limpiar el editor
                            if let Some(current) = &self.current_note {
                                if current.name().starts_with(&format!("{}/", item_name)) {
                                    self.current_note = None;
                                    self.buffer = NoteBuffer::new();
                                    self.sync_to_view();
                                    self.window_title.set_label("NotNative");
                                    self.has_unsaved_changes = false;
                                }
                            }

                            // Refrescar sidebar
                            self.populate_notes_list(&sender);
                            *self.is_populating_list.borrow_mut() = false;
                        }
                    }
                } else {
                    println!("Eliminar nota: {}", item_name);
                    if let Ok(Some(note)) = self.notes_dir.find_note(&item_name) {
                        // Mover a papelera en lugar de eliminar permanentemente
                        if let Err(e) = note.trash(&self.notes_dir) {
                            eprintln!("Error al mover nota a papelera: {}", e);
                        } else {
                            // Eliminar de la base de datos (ya no está accesible en la UI)
                            if let Err(e) = self.notes_db.delete_note(&item_name) {
                                eprintln!("Error al eliminar nota del índice: {}", e);
                            } else {
                                println!("Nota eliminada del índice y movida a papelera");
                            }

                            // Si era la nota actual, limpiar el editor
                            if let Some(current) = &self.current_note {
                                if current.name() == item_name {
                                    self.current_note = None;
                                    self.buffer = NoteBuffer::new();
                                    self.sync_to_view();
                                    self.window_title.set_label("NotNative");
                                    self.has_unsaved_changes = false;
                                }
                            }
                            // Refrescar sidebar
                            self.populate_notes_list(&sender);
                            *self.is_populating_list.borrow_mut() = false;
                        }
                    }
                }
            }

            AppMsg::RenameItem(item_name, is_folder) => {
                self.context_menu.popdown();
                self.context_menu.unparent();

                // Activar modo de renombrado
                *self.renaming_item.borrow_mut() = Some((item_name, is_folder));

                // Repoblar la lista para mostrar el Entry editable
                self.populate_notes_list(&sender);
            }

            AppMsg::OpenInFileManager(item_name, is_folder) => {
                self.context_menu.popdown();
                self.context_menu.unparent();

                let path = if is_folder {
                    // Para carpetas, abrir la carpeta directamente
                    self.notes_dir.root().join(&item_name)
                } else {
                    // Para notas, abrir el directorio que contiene la nota
                    if let Ok(Some(note)) = self.notes_dir.find_note(&item_name) {
                        if let Some(parent) = note.path().parent() {
                            parent.to_path_buf()
                        } else {
                            self.notes_dir.root().to_path_buf()
                        }
                    } else {
                        self.notes_dir.root().to_path_buf()
                    }
                };

                // Abrir el explorador de archivos del sistema
                if let Err(e) = std::process::Command::new("xdg-open").arg(&path).spawn() {
                    eprintln!("Error al abrir explorador de archivos: {}", e);
                }
            }

            AppMsg::RefreshSidebar => {
                self.populate_notes_list(&sender);
                *self.is_populating_list.borrow_mut() = false;
            }

            AppMsg::ExpandFolder(folder) => {
                // Expandir carpeta si no está expandida
                if !self.expanded_folders.contains(&folder) {
                    self.expanded_folders.insert(folder.clone());
                    println!("📂 Carpeta expandida automáticamente: {}", folder);
                    // Refrescar sidebar para mostrar el contenido
                    self.populate_notes_list(&sender);
                    *self.is_populating_list.borrow_mut() = false;
                }
            }

            AppMsg::MinimizeToTray => {
                println!("📱 Minimizando a bandeja del sistema...");
                // Guardar cambios antes de minimizar
                sender.input(AppMsg::SaveCurrentNote);
                self.main_window.set_visible(false);
                // Actualizar estado para el system tray
                self.window_visible
                    .store(false, std::sync::atomic::Ordering::Relaxed);
            }

            AppMsg::ShowWindow => {
                println!("📱 Mostrando ventana desde bandeja...");

                // En Wayland/Hyprland, necesitamos esta secuencia específica:
                // 1. Primero hacer visible
                self.main_window.set_visible(true);

                // 2. Actualizar estado para el system tray
                self.window_visible
                    .store(true, std::sync::atomic::Ordering::Relaxed);

                // 3. Forzar update del display
                self.main_window.queue_draw();

                // 4. Present con foco
                self.main_window.present();
                self.main_window
                    .present_with_time((gtk::glib::monotonic_time() / 1000) as u32);

                // 5. Forzar activación
                if let Some(surface) = self.main_window.surface() {
                    surface.queue_render();
                }

                // 6. Re-seleccionar la nota actual en el sidebar si existe
                if let Some(ref note) = self.current_note {
                    // Extraer el nombre base y la carpeta
                    let full_name = note.name();
                    let note_name = full_name.split('/').last().unwrap_or(full_name).to_string();

                    // Detectar la carpeta de la nota
                    let note_folder = note
                        .path()
                        .parent()
                        .and_then(|p| p.strip_prefix(self.notes_dir.root()).ok())
                        .filter(|p| !p.as_os_str().is_empty())
                        .and_then(|p| p.to_str())
                        .map(|s| s.to_string());

                    let notes_list = self.notes_list.clone();

                    println!(
                        "🔍 Programando re-selección de nota: {} en carpeta: {:?}",
                        note_name, note_folder
                    );

                    gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(100),
                        move || {
                            println!(
                                "🔎 Buscando nota '{}' en carpeta {:?} para seleccionar...",
                                note_name, note_folder
                            );
                            // Buscar y seleccionar la nota
                            let mut child = notes_list.first_child();
                            let mut found = false;
                            let mut count = 0;
                            let mut current_folder: Option<String> = None;

                            while let Some(widget) = child {
                                count += 1;
                                if let Ok(list_row) = widget.clone().downcast::<gtk::ListBoxRow>() {
                                    // Verificar si es una carpeta para trackear en qué carpeta estamos
                                    let is_folder = unsafe {
                                        list_row
                                            .data::<bool>("is_folder")
                                            .map(|data| *data.as_ref())
                                            .unwrap_or(false)
                                    };

                                    if is_folder {
                                        // Actualizar la carpeta actual
                                        current_folder = unsafe {
                                            list_row
                                                .data::<String>("folder_name")
                                                .map(|data| data.as_ref().clone())
                                        };
                                    } else if list_row.is_selectable() {
                                        // Intentar obtener el nombre desde set_data primero
                                        let note_name_from_data = unsafe {
                                            list_row
                                                .data::<String>("note_name")
                                                .map(|data| data.as_ref().clone())
                                        };

                                        let name_matches = if let Some(name) = note_name_from_data {
                                            name == note_name
                                        } else if let Some(child_w) = list_row.child() {
                                            if let Ok(box_widget) = child_w.downcast::<gtk::Box>() {
                                                if let Some(label_widget) = box_widget
                                                    .first_child()
                                                    .and_then(|w| w.next_sibling())
                                                {
                                                    if let Ok(label) =
                                                        label_widget.downcast::<gtk::Label>()
                                                    {
                                                        label.text() == note_name
                                                    } else {
                                                        false
                                                    }
                                                } else {
                                                    false
                                                }
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        };

                                        // Verificar que tanto el nombre como la carpeta coincidan
                                        if name_matches && current_folder == note_folder {
                                            notes_list.select_row(Some(&list_row));
                                            found = true;
                                            println!(
                                                "✅ Nota '{}' en carpeta {:?} seleccionada en sidebar",
                                                note_name, current_folder
                                            );
                                            break;
                                        }
                                    }
                                }
                                child = widget.next_sibling();
                            }

                            println!("📊 Total de filas revisadas: {}", count);
                            if !found {
                                println!(
                                    "⚠️ No se encontró la nota '{}' en carpeta {:?} en el sidebar",
                                    note_name, note_folder
                                );
                            }
                        },
                    );
                } else {
                    println!("⚠️ No hay nota actual para seleccionar");
                }

                // 7. Dar foco al editor
                gtk::glib::idle_add_local_once(gtk::glib::clone!(
                    #[strong(rename_to = text_view)]
                    self.text_view,
                    move || {
                        text_view.grab_focus();
                    }
                ));

                println!("✅ Ventana mostrada y activada");
            }

            AppMsg::QuitApp => {
                println!("👋 Cerrando aplicación completamente...");
                sender.input(AppMsg::SaveCurrentNote);

                // Limpiar archivos temporales
                let _ = std::fs::remove_file("/tmp/notnative.lock");
                let _ = std::fs::remove_file("/tmp/notnative.control");

                std::process::exit(0);
            }

            AppMsg::ToggleQuickNote => {
                println!("📝 Toggle Quick Note...");

                // Crear ventana si no existe
                if self.quick_note_window.borrow().is_none() {
                    let qn_window = crate::quick_note::QuickNoteWindow::new(
                        &self.main_window,
                        self.notes_dir.clone(),
                        self.i18n.clone(),
                    );
                    *self.quick_note_window.borrow_mut() = Some(qn_window);
                }

                // Toggle visibilidad
                if let Some(ref qn) = *self.quick_note_window.borrow() {
                    qn.toggle();
                }
            }

            AppMsg::NewQuickNote => {
                println!("📝 Nueva Quick Note...");

                // Crear ventana si no existe
                if self.quick_note_window.borrow().is_none() {
                    let qn_window = crate::quick_note::QuickNoteWindow::new(
                        &self.main_window,
                        self.notes_dir.clone(),
                        self.i18n.clone(),
                    );
                    *self.quick_note_window.borrow_mut() = Some(qn_window);
                }

                // Crear y abrir nueva nota
                if let Some(ref qn) = *self.quick_note_window.borrow() {
                    qn.new_note();
                }
            }

            AppMsg::NewChatSession => {
                println!("✨ Iniciando nueva sesión de chat...");

                // Limpiar sesión en memoria
                *self.chat_session.borrow_mut() = None;
                *self.chat_session_id.borrow_mut() = None;

                // Limpiar UI
                while let Some(child) = self.chat_history_list.first_child() {
                    self.chat_history_list.remove(&child);
                }

                // Configuración
                let ai_config = self.notes_config.borrow().get_ai_config().clone();
                let model_config = crate::ai_chat::AIModelConfig {
                    provider: match ai_config.provider.as_str() {
                        "anthropic" => crate::ai_chat::AIProvider::Anthropic,
                        "ollama" => crate::ai_chat::AIProvider::Ollama,
                        _ => crate::ai_chat::AIProvider::OpenAI,
                    },
                    model: ai_config.model.clone(),
                    max_tokens: ai_config.max_tokens as usize,
                    temperature: ai_config.temperature,
                };

                // Actualizar label del modelo
                self.chat_model_label.set_text(&format!(
                    "{} - {} (temp: {:.1})",
                    ai_config.provider, ai_config.model, ai_config.temperature
                ));

                // Crear nueva sesión vacía
                let session = crate::ai_chat::ChatSession::new(model_config);
                *self.chat_session.borrow_mut() = Some(session);

                // Dar foco al input
                self.chat_input_view.grab_focus();
            }

            AppMsg::ToggleChatMode => {
                let i18n = self.i18n.borrow();
                let current_mode = *self.chat_agent_mode.borrow();
                let new_mode = !current_mode;
                *self.chat_agent_mode.borrow_mut() = new_mode;

                let mode_name = if new_mode {
                    "Agente (con tools)"
                } else {
                    "Chat Normal (sin tools)"
                };
                println!("🔄 Modo de chat cambiado a: {}", mode_name);

                // Actualizar label visible del modo
                let mode_label_text = if new_mode {
                    i18n.t("chat_mode_agent")
                } else {
                    i18n.t("chat_mode_normal")
                };
                self.chat_mode_label.set_text(&mode_label_text);

                // Solo eliminar mensajes System, mantener User/Assistant para contexto
                if let Some(session) = self.chat_session.borrow_mut().as_mut() {
                    session
                        .messages
                        .retain(|msg| msg.role != crate::ai_chat::MessageRole::System);
                    println!("🧹 System prompts eliminados, historial de conversación mantenido");
                }

                // NO limpiar UI - mantener mensajes visibles

                // Mostrar notificación al usuario
                let notification_text = if new_mode {
                    self.i18n.borrow().t("agent_mode_activated")
                } else {
                    self.i18n.borrow().t("chat_mode_activated")
                };

                self.show_notification(&notification_text);
            }

            AppMsg::CheckMCPUpdates => {
                // Verificar si hay archivo de señal de cambios MCP
                let signal_path = std::env::temp_dir().join("notnative_mcp_update.signal");
                if let Ok(content) = std::fs::read_to_string(&signal_path) {
                    if let Ok(timestamp) = content.trim().parse::<u64>() {
                        let last_check = *self.mcp_last_update_check.borrow();
                        if timestamp > last_check {
                            println!("🔄 Detectados cambios desde MCP, actualizando sidebar...");
                            *self.mcp_last_update_check.borrow_mut() = timestamp;

                            // Recargar la nota actual si hay una abierta y no tiene cambios sin guardar
                            if let Some(ref note) = self.current_note {
                                if !self.has_unsaved_changes {
                                    if let Ok(content) = note.read() {
                                        println!(
                                            "🔄 Recargando nota actual desde disco: {}",
                                            note.name()
                                        );

                                        // Recargar contenido en el buffer
                                        self.buffer = crate::core::NoteBuffer::from_text(&content);
                                        self.cursor_position = 0;

                                        // sync_to_view se encarga de todo: renderizar markdown si está en Normal,
                                        // mostrar texto crudo si está en Insert, y posicionar el cursor
                                        self.sync_to_view();

                                        // Actualizar UI
                                        sender.input(AppMsg::RefreshTags);
                                    }
                                } else {
                                    println!(
                                        "⚠️ Nota actual tiene cambios sin guardar, no se recarga automáticamente"
                                    );
                                }
                            }

                            sender.input(AppMsg::RefreshSidebar);
                        }
                    }
                }
            }

            AppMsg::IndexNoteEmbeddings { path, content } => {
                if self.notes_config.borrow().get_embeddings_enabled() {
                    println!("🔄 Indexando embeddings para: {}", path);
                    let path_buf = std::path::PathBuf::from(path);
                    self.index_note_embeddings_async(&path_buf, &content);
                } else {
                    println!(
                        "⏭️ Embeddings deshabilitados, saltando indexación de: {}",
                        path
                    );
                }
            }

            AppMsg::GtkInsertText { offset, text } => {
                println!(
                    "GtkInsertText en offset {} (modo {:?})",
                    offset,
                    *self.mode.borrow()
                );
                self.buffer.insert(offset, &text);
                self.cursor_position = offset + text.chars().count();
                self.has_unsaved_changes = true;

                // Actualizar barra de estado y UI relacionada
                self.update_status_bar(&sender);
                sender.input(AppMsg::RefreshTags);
                sender.input(AppMsg::CheckTagCompletion);
                println!("DEBUG: Enviando CheckNoteMention desde GtkInsertText");
                sender.input(AppMsg::CheckNoteMention);
            }

            AppMsg::GtkDeleteRange { start, end } => {
                println!(
                    "GtkDeleteRange {}..{} (modo {:?})",
                    start,
                    end,
                    *self.mode.borrow()
                );
                if start < end {
                    self.buffer.delete(start..end);
                    self.cursor_position = start;
                    self.has_unsaved_changes = true;

                    self.update_status_bar(&sender);
                    sender.input(AppMsg::RefreshTags);
                }
            }

            AppMsg::AddTag(tag) => {
                if let Some(ref note) = self.current_note {
                    let content = self.buffer.to_string();

                    // Actualizar frontmatter
                    use crate::core::frontmatter::Frontmatter;
                    let (mut frontmatter, body) = Frontmatter::parse_or_empty(&content);

                    // Añadir tag si no existe ya
                    if !frontmatter.tags.contains(&tag) {
                        frontmatter.tags.push(tag.clone());

                        // Actualizar contenido con nuevo frontmatter
                        let new_content = match frontmatter.serialize() {
                            Ok(fm_str) => format!("{}\n{}", fm_str, body),
                            Err(_) => content.clone(),
                        };

                        self.buffer = NoteBuffer::from_text(&new_content);
                        self.sync_to_view();

                        // Guardar y actualizar base de datos
                        self.has_unsaved_changes = true;
                        sender.input(AppMsg::SaveCurrentNote);

                        // Actualizar visualización de tags
                        sender.input(AppMsg::RefreshTags);
                    }
                }
            }

            AppMsg::RemoveTag(tag) => {
                if let Some(ref note) = self.current_note {
                    let content = self.buffer.to_string();

                    // Actualizar frontmatter
                    use crate::core::frontmatter::Frontmatter;
                    let (mut frontmatter, body) = Frontmatter::parse_or_empty(&content);

                    // Remover tag
                    frontmatter.tags.retain(|t| t != &tag);

                    // Actualizar contenido
                    let new_content = match frontmatter.serialize() {
                        Ok(fm_str) => format!("{}\n{}", fm_str, body),
                        Err(_) => content.clone(),
                    };

                    self.buffer = NoteBuffer::from_text(&new_content);
                    self.sync_to_view();

                    // Guardar y actualizar base de datos
                    self.has_unsaved_changes = true;
                    sender.input(AppMsg::SaveCurrentNote);

                    // Actualizar visualización de tags
                    sender.input(AppMsg::RefreshTags);
                }
            }

            AppMsg::RefreshTags => {
                self.refresh_tags_display_with_sender(&sender);
                self.refresh_todos_summary();
            }

            AppMsg::CheckTagCompletion => {
                // Si acabamos de completar un tag, ignorar esta comprobación
                if *self.just_completed_tag.borrow() {
                    return;
                }

                // Solo en modo INSERT
                if *self.mode.borrow() != EditorMode::Insert {
                    return;
                }

                // Obtener texto alrededor del cursor
                let cursor_mark = self.text_buffer.get_insert();
                let cursor_iter = self.text_buffer.iter_at_mark(&cursor_mark);

                // Obtener línea actual
                let mut line_start = cursor_iter.clone();
                line_start.set_line_offset(0);
                let line_text = self.text_buffer.text(&line_start, &cursor_iter, false);

                // Buscar si hay un # seguido de texto antes del cursor
                if let Some(tag_start) = line_text.rfind('#') {
                    let after_hash = &line_text[tag_start + 1..];

                    // Verificar que no es un heading (# seguido de espacio)
                    if !after_hash.starts_with(' ') && !after_hash.is_empty() {
                        // Es un tag potencial
                        *self.current_tag_prefix.borrow_mut() = Some(after_hash.to_string());

                        // Mostrar popup con sugerencias
                        self.show_tag_suggestions(&after_hash.to_lowercase(), &sender);
                    } else {
                        *self.current_tag_prefix.borrow_mut() = None;
                        self.tag_completion_popup.popdown();
                    }
                } else {
                    *self.current_tag_prefix.borrow_mut() = None;
                    self.tag_completion_popup.popdown();
                }
            }

            AppMsg::CompleteTag(tag) => {
                // Obtener el prefix y liberar el borrow inmediatamente
                let prefix_opt = self.current_tag_prefix.borrow().clone();

                if let Some(prefix) = prefix_opt {
                    // Limpiar estado ANTES de modificar el buffer
                    *self.current_tag_prefix.borrow_mut() = None;
                    self.tag_completion_popup.popdown();

                    // Activar bandera para evitar que se reabra el popover
                    *self.just_completed_tag.borrow_mut() = true;

                    let cursor_mark = self.text_buffer.get_insert();
                    let cursor_iter = self.text_buffer.iter_at_mark(&cursor_mark);

                    // Buscar inicio del tag
                    let mut start_iter = cursor_iter.clone();
                    start_iter.backward_chars(prefix.len() as i32);
                    start_iter.backward_char(); // El #

                    // Reemplazar con el tag completo
                    let mut delete_end = cursor_iter.clone();
                    self.text_buffer.delete(&mut start_iter, &mut delete_end);
                    self.text_buffer
                        .insert(&mut start_iter, &format!("#{}", tag));

                    // Asegurar que el cursor queda al final del tag recién insertado
                    let caret_iter = start_iter.clone();
                    self.text_buffer.place_cursor(&caret_iter);
                    self.text_view.grab_focus();

                    // Resetear la bandera después de un breve delay para que todos los eventos se procesen
                    let flag = self.just_completed_tag.clone();
                    gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(50),
                        move || {
                            *flag.borrow_mut() = false;
                        },
                    );
                }
            }

            AppMsg::CheckNoteMention => {
                // Verificar si hay un @ seguido de texto para autocompletar notas
                println!("DEBUG: CheckNoteMention llamado");

                if *self.just_completed_mention.borrow() {
                    println!("DEBUG: Saliendo porque just_completed_mention es true");
                    return; // Evitar reabrir inmediatamente después de completar
                }

                // Solo en modo INSERT
                if *self.mode.borrow() != EditorMode::Insert {
                    println!("DEBUG: No estoy en modo Insert, saliendo");
                    return;
                }

                // Obtener texto alrededor del cursor
                let cursor_mark = self.text_buffer.get_insert();
                let cursor_iter = self.text_buffer.iter_at_mark(&cursor_mark);

                // Obtener línea actual
                let mut line_start = cursor_iter.clone();
                line_start.set_line_offset(0);
                let line_text = self.text_buffer.text(&line_start, &cursor_iter, false);

                println!("DEBUG: Texto de línea hasta cursor: '{}'", line_text);

                // Buscar si hay un @ seguido de texto antes del cursor
                if let Some(mention_start) = line_text.rfind('@') {
                    let after_at = &line_text[mention_start + 1..];
                    println!(
                        "DEBUG: Encontrado @ en posición {}, después: '{}'",
                        mention_start, after_at
                    );

                    // Debe tener al menos un carácter después de @
                    if !after_at.is_empty() && !after_at.contains(' ') {
                        // Es una mención potencial
                        println!("DEBUG: Mostrando sugerencias para: '{}'", after_at);
                        *self.current_mention_prefix.borrow_mut() = Some(after_at.to_string());

                        // Mostrar popup con sugerencias de notas
                        self.show_note_mention_suggestions(&after_at.to_lowercase(), &sender);
                    } else {
                        println!("DEBUG: after_at está vacío o contiene espacio");
                        *self.current_mention_prefix.borrow_mut() = None;
                        self.note_mention_popup.popdown();
                    }
                } else {
                    println!("DEBUG: No se encontró @ en la línea");
                    *self.current_mention_prefix.borrow_mut() = None;
                    self.note_mention_popup.popdown();
                }
            }

            AppMsg::CompleteMention(note_name) => {
                println!("DEBUG: CompleteMention llamado para nota: {}", note_name);

                // Obtener el prefix y liberar el borrow inmediatamente
                let prefix_opt = self.current_mention_prefix.borrow().clone();

                if let Some(prefix) = prefix_opt {
                    println!("DEBUG: Prefix guardado: '{}'", prefix);

                    // Limpiar estado ANTES de modificar el buffer
                    *self.current_mention_prefix.borrow_mut() = None;
                    self.note_mention_popup.popdown();

                    // Activar bandera para evitar que se reabra el popover
                    *self.just_completed_mention.borrow_mut() = true;

                    let cursor_mark = self.text_buffer.get_insert();
                    let cursor_iter = self.text_buffer.iter_at_mark(&cursor_mark);

                    // Obtener la línea actual para buscar el @
                    let mut line_start = cursor_iter.clone();
                    line_start.set_line_offset(0);
                    let mut line_end = cursor_iter.clone();
                    if !line_end.ends_line() {
                        line_end.forward_to_line_end();
                    }
                    let line_text = self.text_buffer.text(&line_start, &line_end, false);

                    println!("DEBUG: Línea completa: '{}'", line_text);

                    // Buscar @ seguido del prefix en la línea
                    let search_pattern = format!("@{}", prefix);
                    if let Some(mention_pos) = line_text.find(&search_pattern) {
                        println!(
                            "DEBUG: Encontrado '{}' en posición {}",
                            search_pattern, mention_pos
                        );

                        // Posicionar al inicio de la mención
                        let mut start_iter = line_start.clone();
                        start_iter.forward_chars(mention_pos as i32);

                        // Posicionar al final de la mención actual
                        let mut end_iter = start_iter.clone();
                        end_iter.forward_chars((prefix.len() + 1) as i32); // +1 por el @

                        // Borrar la mención parcial
                        self.text_buffer.delete(&mut start_iter, &mut end_iter);

                        // Insertar la mención completa
                        self.text_buffer
                            .insert(&mut start_iter, &format!("@{}", note_name));

                        println!("DEBUG: Mención completada: @{}", note_name);

                        // Colocar cursor al final de la mención
                        self.text_buffer.place_cursor(&start_iter);
                        self.text_view.grab_focus();
                    } else {
                        println!("DEBUG: No se encontró '{}' en la línea", search_pattern);
                    }

                    // Resetear la bandera después de un breve delay
                    let flag = self.just_completed_mention.clone();
                    gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(50),
                        move || {
                            *flag.borrow_mut() = false;
                        },
                    );
                }
            }

            AppMsg::ShowChatNoteSuggestions(prefix) => {
                self.show_chat_note_suggestions(&prefix, &sender);
            }

            AppMsg::HideChatNoteSuggestions => {
                // Limpiar sugerencias
                while let Some(child) = self.chat_note_suggestions_list.first_child() {
                    self.chat_note_suggestions_list.remove(&child);
                }
                self.chat_note_suggestions_popover.popdown();
            }

            AppMsg::CompleteChatNote(note_name) => {
                // Obtener el prefix y liberar el borrow inmediatamente
                let prefix_opt = self.chat_current_note_prefix.borrow().clone();

                if let Some(prefix) = prefix_opt {
                    // Limpiar estado ANTES de modificar el buffer
                    *self.chat_current_note_prefix.borrow_mut() = None;
                    self.chat_note_suggestions_popover.popdown();

                    // Activar bandera para evitar que se reabra el popover
                    *self.chat_just_completed_note.borrow_mut() = true;

                    let cursor_pos = self.chat_input_buffer.cursor_position();
                    let mut cursor_iter = self.chat_input_buffer.iter_at_offset(cursor_pos);

                    // Buscar @ hacia atrás
                    let mut at_iter = cursor_iter;
                    while at_iter.backward_char() {
                        if at_iter.char() == '@' {
                            break;
                        }
                    }

                    // Borrar desde @ hasta cursor
                    self.chat_input_buffer
                        .delete(&mut at_iter, &mut cursor_iter);

                    // Insertar @notename
                    self.chat_input_buffer
                        .insert(&mut at_iter, &format!("@{} ", note_name));

                    // Colocar cursor y devolver foco
                    self.chat_input_buffer.place_cursor(&at_iter);
                    self.chat_input_view.grab_focus();

                    // Resetear la bandera después de un breve delay
                    let flag = self.chat_just_completed_note.clone();
                    gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(50),
                        move || {
                            *flag.borrow_mut() = false;
                        },
                    );
                }
            }

            AppMsg::SaveAndSearchTag(tag) => {
                // Primero guardar la nota actual para indexar los tags nuevos
                self.save_current_note(true);

                // Abrir barra flotante y buscar el tag
                sender.input(AppMsg::ToggleFloatingSearch);

                // Delay para asegurar que la barra esté visible antes de buscar
                let sender_clone = sender.clone();
                let tag_clone = tag.clone();
                gtk::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(100),
                    move || {
                        sender_clone.input(AppMsg::FloatingSearchNotes(format!("#{}", tag_clone)));
                    },
                );
            }

            AppMsg::SearchNotes(query) => {
                // Este mensaje ahora solo abre la barra flotante con el query
                if !query.trim().is_empty() {
                    sender.input(AppMsg::ToggleFloatingSearch);

                    // Delay para asegurar que la barra esté visible
                    let sender_clone = sender.clone();
                    let query_clone = query.clone();
                    gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(100),
                        move || {
                            sender_clone.input(AppMsg::FloatingSearchNotes(query_clone));
                        },
                    );
                }
            }

            AppMsg::ToggleSemanticSearch(enabled) => {
                self.semantic_search_enabled = enabled;
                println!(
                    "[DEBUG] Búsqueda semántica: {}",
                    if enabled { "ACTIVADA" } else { "DESACTIVADA" }
                );

                // Si hay una búsqueda activa en barra flotante, re-ejecutarla
                let floating_query = self.floating_search_entry.text().to_string();
                if !floating_query.is_empty() {
                    self.perform_floating_search(&floating_query, &sender);
                }
            }

            AppMsg::ToggleSemanticSearchWithNotification => {
                // Toggle del estado
                self.semantic_search_enabled = !self.semantic_search_enabled;

                // Actualizar el label del modo en la barra flotante
                let mode_markup = if self.semantic_search_enabled {
                    "<small>🧠 Semántica</small>"
                } else {
                    "<small>🔍 Normal</small>"
                };
                self.floating_search_mode_label.set_markup(mode_markup);

                // Mostrar notificación del modo activo
                let mode_text = if self.semantic_search_enabled {
                    "Búsqueda Semántica activada
🧠 Buscar por significado y contexto"
                } else {
                    "Búsqueda Normal activada
🔍 Buscar por palabras exactas"
                };
                self.show_notification(mode_text);

                println!(
                    "[DEBUG] Búsqueda semántica: {}",
                    if self.semantic_search_enabled {
                        "ACTIVADA"
                    } else {
                        "DESACTIVADA"
                    }
                );

                // Si hay una búsqueda activa, re-ejecutarla con el nuevo modo
                let floating_query = self.floating_search_entry.text().to_string();
                if !floating_query.is_empty() {
                    self.perform_floating_search(&floating_query, &sender);
                }
            }

            AppMsg::ToggleFloatingSearch => {
                self.floating_search_visible = !self.floating_search_visible;
                *self.floating_search_in_current_note.borrow_mut() = false; // Búsqueda global
                self.floating_search_bar
                    .set_visible(self.floating_search_visible);

                if self.floating_search_visible {
                    self.floating_search_rows.borrow_mut().clear();
                    // IMPORTANTE: Mostrar la lista de resultados en modo global
                    self.floating_search_results.set_visible(true);

                    // Actualizar el indicador de modo
                    let mode_markup = if self.semantic_search_enabled {
                        "<small>🧠 Semántica</small>"
                    } else {
                        "<small>🔍 Normal</small>"
                    };
                    self.floating_search_mode_label.set_markup(mode_markup);

                    // Actualizar placeholder
                    self.floating_search_entry.set_placeholder_text(Some(
                        "Buscar en todas las notas... (Ctrl: cambiar modo)",
                    ));

                    // Limpiar búsqueda anterior y dar foco
                    self.floating_search_entry.set_text("");

                    // Dar foco después de un pequeño delay para asegurar que la animación termine
                    let entry_clone = self.floating_search_entry.clone();
                    gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(50),
                        move || {
                            entry_clone.grab_focus();
                        },
                    );
                } else {
                    self.floating_search_rows.borrow_mut().clear();
                    // Limpiar resultados al cerrar (preservando semantic_search_answer_row)
                    let answer_row_ptr = self.semantic_search_answer_row.as_ptr();
                    let mut child = self.floating_search_results_list.first_child();
                    while let Some(widget) = child {
                        let next = widget.next_sibling();
                        // No eliminar el semantic_search_answer_row
                        if widget.as_ptr() != answer_row_ptr as *mut _ {
                            self.floating_search_results_list.remove(&widget);
                        }
                        child = next;
                    }
                    // Ocultar el answer_row pero mantenerlo en el árbol
                    self.semantic_search_answer_row.set_visible(false);

                    // Devolver foco al editor correcto según el modo
                    let current_mode = *self.mode.borrow();
                    if current_mode == EditorMode::Normal && self.markdown_enabled {
                        self.preview_webview.grab_focus();
                    } else {
                        self.text_view.grab_focus();
                    }
                }
            }

            AppMsg::ToggleFloatingSearchInNote => {
                // Solo abrir si hay una nota activa
                if self.current_note.is_none() {
                    self.show_notification(
                        "⚠️ No hay nota activa\nAbre una nota para buscar en ella",
                    );
                    return;
                }

                // Abrir buscador simple sin resultados (solo input para buscar y saltar)
                self.floating_search_visible = !self.floating_search_visible;
                *self.floating_search_in_current_note.borrow_mut() = true;

                if self.floating_search_visible {
                    // Cambiar a modo Insert para buscar en el texto markdown puro
                    if *self.mode.borrow() == EditorMode::Normal {
                        sender.input(AppMsg::ProcessAction(EditorAction::ChangeMode(
                            EditorMode::Insert,
                        )));
                    }

                    self.floating_search_rows.borrow_mut().clear();
                    // Ocultar la lista de resultados en este modo
                    self.floating_search_results.set_visible(false);
                    self.floating_search_bar.set_visible(true);

                    let note_name = self
                        .current_note
                        .as_ref()
                        .map(|n| n.name())
                        .unwrap_or("esta nota");
                    self.floating_search_entry
                        .set_placeholder_text(Some(&format!(
                            "Buscar en '{}'... (Esc para cerrar)",
                            note_name
                        )));

                    self.floating_search_entry.set_text("");

                    let entry_clone = self.floating_search_entry.clone();
                    gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(50),
                        move || {
                            entry_clone.grab_focus();
                        },
                    );
                } else {
                    self.floating_search_rows.borrow_mut().clear();
                    // Limpiar coincidencias de búsqueda en nota
                    self.in_note_search_matches.borrow_mut().clear();
                    *self.in_note_search_current_index.borrow_mut() = 0;
                    // Limpiar resaltados
                    self.text_buffer.remove_tag_by_name(
                        "search-highlight",
                        &self.text_buffer.start_iter(),
                        &self.text_buffer.end_iter(),
                    );
                    self.text_buffer.remove_tag_by_name(
                        "search-highlight-current",
                        &self.text_buffer.start_iter(),
                        &self.text_buffer.end_iter(),
                    );
                    // Al cerrar, restaurar visibilidad de resultados para modo global
                    self.floating_search_bar.set_visible(false);
                    self.floating_search_results.set_visible(true);

                    // Devolver foco al editor correcto según el modo
                    let current_mode = *self.mode.borrow();
                    if current_mode == EditorMode::Normal && self.markdown_enabled {
                        self.preview_webview.grab_focus();
                    } else {
                        self.text_view.grab_focus();
                    }
                }
            }

            AppMsg::InNoteSearchNext => {
                self.go_to_next_match();
            }

            AppMsg::InNoteSearchPrev => {
                self.go_to_prev_match();
            }

            AppMsg::FloatingSearchNotes(query) => {
                if !query.is_empty() {
                    // Si estamos en modo búsqueda en nota actual, resaltar y hacer scroll
                    if *self.floating_search_in_current_note.borrow() {
                        // Resaltar y hacer scroll al texto mientras se escribe
                        self.highlight_and_scroll_to_text(&query);
                        return;
                    }

                    // Búsqueda global: si está en modo semántico, usar debounce
                    if self.semantic_search_enabled {
                        // Cancelar timeout anterior si existe y crear uno nuevo
                        // Esto asegura que solo busca cuando DEJAS de escribir
                        if let Some(id) = self.semantic_search_timeout_id.borrow_mut().take() {
                            id.remove();
                        }

                        // Crear nuevo timeout de 2500ms (2.5 segundos)
                        // Tiempo suficiente para escribir frases completas sin que se ejecute prematuramente
                        let sender_clone = sender.clone();
                        let query_clone = query.clone();
                        let entry_clone = self.floating_search_entry.clone();
                        let timeout_id_ref = self.semantic_search_timeout_id.clone();

                        let id = gtk::glib::timeout_add_local_once(
                            std::time::Duration::from_millis(2500),
                            move || {
                                let current_text = entry_clone.text();
                                if current_text.as_str() != query_clone {
                                    entry_clone.set_text(&query_clone);
                                }
                                sender_clone.input(AppMsg::PerformFloatingSearch(query_clone));
                                timeout_id_ref.borrow_mut().take();
                            },
                        );
                        *self.semantic_search_timeout_id.borrow_mut() = Some(id);
                    } else {
                        // Modo normal: búsqueda con debounce corto (150ms)
                        // Cancelar timeout anterior si existe
                        if let Some(id) = self.traditional_search_timeout_id.borrow_mut().take() {
                            id.remove();
                        }

                        let sender_clone = sender.clone();
                        let query_clone = query.clone();
                        let timeout_id_ref = self.traditional_search_timeout_id.clone();

                        let id = gtk::glib::timeout_add_local_once(
                            std::time::Duration::from_millis(150),
                            move || {
                                sender_clone.input(AppMsg::PerformFloatingSearch(query_clone));
                                timeout_id_ref.borrow_mut().take();
                            },
                        );
                        *self.traditional_search_timeout_id.borrow_mut() = Some(id);
                    }
                } else {
                    // Limpiar resultados si el query está vacío
                    // Cancelar timeouts pendientes si existen
                    if let Some(id) = self.semantic_search_timeout_id.borrow_mut().take() {
                        id.remove();
                    }
                    if let Some(id) = self.traditional_search_timeout_id.borrow_mut().take() {
                        id.remove();
                    }

                    self.floating_search_rows.borrow_mut().clear();
                    // Limpiar resultados (preservando semantic_search_answer_row)
                    let answer_row_ptr = self.semantic_search_answer_row.as_ptr();
                    let mut child = self.floating_search_results_list.first_child();
                    while let Some(widget) = child {
                        let next = widget.next_sibling();
                        if widget.as_ptr() != answer_row_ptr as *mut _ {
                            self.floating_search_results_list.remove(&widget);
                        }
                        child = next;
                    }
                    self.semantic_search_answer_row.set_visible(false);
                }
            }

            AppMsg::PerformFloatingSearch(query) => {
                // Mostrar indicador de "Buscando..." mientras se ejecuta la búsqueda
                self.floating_search_rows.borrow_mut().clear();
                // Limpiar resultados (preservando semantic_search_answer_row)
                let answer_row_ptr = self.semantic_search_answer_row.as_ptr();
                let mut child = self.floating_search_results_list.first_child();
                while let Some(widget) = child {
                    let next = widget.next_sibling();
                    if widget.as_ptr() != answer_row_ptr as *mut _ {
                        self.floating_search_results_list.remove(&widget);
                    }
                    child = next;
                }
                self.semantic_search_answer_row.set_visible(false);

                let searching_label = gtk::Label::builder()
                    .label("🔍 Buscando...")
                    .margin_top(24)
                    .margin_bottom(24)
                    .margin_start(24)
                    .margin_end(24)
                    .justify(gtk::Justification::Center)
                    .build();

                let row = gtk::ListBoxRow::builder()
                    .selectable(false)
                    .activatable(false)
                    .child(&searching_label)
                    .build();

                self.floating_search_results_list.append(&row);

                // Ejecutar la búsqueda después de un pequeño delay para que se renderice el "Buscando..."
                let sender_clone = sender.clone();
                let query_clone = query.clone();
                gtk::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(50),
                    move || {
                        sender_clone.input(AppMsg::ExecuteFloatingSearch(query_clone));
                    },
                );
            }

            AppMsg::ExecuteFloatingSearch(query) => {
                // Ejecutar la búsqueda real
                self.perform_floating_search(&query, &sender);
            }

            AppMsg::LoadNoteFromFloatingSearch(name) => {
                // Si estamos en modo búsqueda dentro de la nota, ir a siguiente coincidencia
                if *self.floating_search_in_current_note.borrow() {
                    // El Enter en modo búsqueda en nota va a la siguiente coincidencia
                    // (El Shift+Enter se maneja por separado con InNoteSearchPrev)
                    self.go_to_next_match();
                } else {
                    // Búsqueda global: cerrar buscador y cargar la nota
                    self.floating_search_visible = false;
                    self.floating_search_bar.set_visible(false);

                    sender.input(AppMsg::LoadNote {
                        name,
                        highlight_text: Some(self.floating_search_entry.text().to_string()),
                    });
                }
            }

            AppMsg::ShowPreferences => {
                self.show_preferences_dialog(&sender);
            }

            AppMsg::ShowKeyboardShortcuts => {
                self.show_keyboard_shortcuts();
            }

            AppMsg::ShowAboutDialog => {
                self.show_about_dialog();
            }

            AppMsg::ShowMCPServerInfo => {
                self.show_mcp_server_info_dialog();
            }

            AppMsg::SetStartInBackground(enabled) => {
                self.notes_config
                    .borrow_mut()
                    .set_start_in_background(enabled);
                if let Err(e) = self.notes_config.borrow().save(NotesConfig::default_path()) {
                    eprintln!(
                        "Error guardando configuración de inicio en segundo plano: {}",
                        e
                    );
                }

                // Gestionar archivo de autostart en Linux
                if let Err(e) = Self::manage_autostart(enabled) {
                    eprintln!("Error gestionando autostart: {}", e);
                }

                println!("Inicio en segundo plano configurado a: {}", enabled);
            }

            AppMsg::ChangeLanguage(new_language) => {
                // Actualizar idioma en I18n
                self.i18n.borrow_mut().set_language(new_language);

                // Guardar preferencia en configuración
                self.notes_config
                    .borrow_mut()
                    .set_language(Some(new_language.code().to_string()));
                if let Err(e) = self.notes_config.borrow().save(NotesConfig::default_path()) {
                    eprintln!("Error guardando configuración de idioma: {}", e);
                }

                println!("Idioma cambiado a: {:?}", new_language);

                // Actualizar todos los textos de la UI
                self.update_ui_language(&sender);
            }

            AppMsg::ReloadConfig => {
                // Recargar configuración desde disco
                if let Ok(config) = NotesConfig::load(NotesConfig::default_path()) {
                    *self.notes_config.borrow_mut() = config.clone();
                    println!("✅ Configuración recargada desde disco");

                    // Actualizar el label del modelo de chat AI si está en ese modo
                    let current_mode = *self.mode.borrow();
                    if current_mode == EditorMode::ChatAI {
                        let ai_config = config.get_ai_config();
                        self.chat_model_label.set_text(&format!(
                            "{} / {} (T: {})",
                            ai_config.provider, ai_config.model, ai_config.temperature
                        ));
                        println!(
                            "✅ Configuración de AI actualizada: {} / {}",
                            ai_config.provider, ai_config.model
                        );

                        // Reinicializar la sesión de chat con la nueva configuración
                        // Convertir string provider a AIProvider enum
                        let provider = match ai_config.provider.as_str() {
                            "openai" => crate::ai_chat::AIProvider::OpenAI,
                            "anthropic" => crate::ai_chat::AIProvider::Anthropic,
                            "ollama" => crate::ai_chat::AIProvider::Ollama,
                            _ => crate::ai_chat::AIProvider::Custom,
                        };

                        let model_config = crate::ai_chat::AIModelConfig {
                            provider,
                            model: ai_config.model.clone(),
                            max_tokens: ai_config.max_tokens as usize,
                            temperature: ai_config.temperature,
                        };

                        // Crear nueva sesión de chat
                        let new_session = crate::ai_chat::ChatSession::new(model_config);
                        *self.chat_session.borrow_mut() = Some(new_session);
                        println!("✅ Sesión de chat reinicializada con nuevo modelo");
                    }

                    // Para embeddings, no es necesario reinicializar nada aquí
                    // ya que el cliente se crea bajo demanda en cada búsqueda
                    let embedding_config = config.get_embedding_config();
                    println!(
                        "ℹ️  Configuración de embeddings actualizada: {} / {} (habilitado: {})",
                        embedding_config.provider, embedding_config.model, embedding_config.enabled
                    );
                } else {
                    eprintln!("❌ Error recargando configuración");
                }
            }

            AppMsg::InsertImage => {
                self.show_insert_image_dialog(&sender);
            }

            AppMsg::InsertImageFromPath(path) => {
                self.insert_image_from_path(&path, &sender);
            }

            AppMsg::ProcessPastedText(text) => {
                self.process_pasted_text(&text, &sender);
            }
            AppMsg::ToggleTodo {
                line_number,
                new_state,
            } => {
                // Actualizar el estado del TODO en el buffer interno
                self.update_todo_in_buffer(line_number, new_state);

                // Guardar automáticamente el cambio para que persista
                self.save_current_note(true);

                // Actualizar resumen de TODOs
                self.refresh_todos_summary();

                // Actualizar barra de estado
                self.update_status_bar(&sender);
            }
            // WebView preview handlers
            AppMsg::ToggleTodoLine { line, checked } => {
                // Toggle TODO checkbox desde el WebView de preview
                // Buscar la línea en el buffer y cambiar su estado
                self.toggle_todo_at_line(line, checked);

                // Guardar automáticamente
                self.save_current_note(true);

                // Re-renderizar el preview
                self.render_preview_html();

                // Actualizar resumen de TODOs
                self.refresh_todos_summary();
            }
            AppMsg::SwitchToInsertAtLine { line } => {
                // Cambiar a modo Insert y posicionar cursor en la línea especificada
                *self.mode.borrow_mut() = EditorMode::Insert;

                // Calcular la posición del cursor para esa línea
                let buffer_text = self.buffer.to_string();
                let mut char_offset = 0;
                for (i, line_text) in buffer_text.lines().enumerate() {
                    if i + 1 >= line {
                        break;
                    }
                    char_offset += line_text.chars().count() + 1; // +1 for newline
                }

                self.cursor_position = char_offset;

                // Actualizar vista a modo Insert (TextView)
                self.editor_stack.set_visible_child_name("editor");
                self.sync_to_view();

                // Dar foco al TextView
                let text_view = self.text_view.clone();
                gtk::glib::idle_add_local_once(move || {
                    text_view.grab_focus();
                });

                // Actualizar indicador de modo
                self.update_status_bar(&sender);
            }
            AppMsg::AskTranscribeYouTube { url, video_id } => {
                self.show_transcribe_dialog(url, video_id, &sender);
            }
            AppMsg::InsertYouTubeLink(video_id) => {
                self.insert_youtube_link(&video_id, &sender);
            }
            AppMsg::InsertYouTubeWithTranscript { video_id } => {
                self.insert_youtube_with_transcript(&video_id, &sender);
            }
            AppMsg::UpdateTranscript {
                video_id,
                transcript,
            } => {
                self.update_transcript(&video_id, &transcript, &sender);
            }
            AppMsg::MoveNoteToFolder {
                note_name,
                folder_name,
            } => {
                self.move_note_to_folder(&note_name, folder_name.as_deref(), &sender);
            }
            AppMsg::ReorderNotes {
                source_name,
                target_name,
            } => {
                self.reorder_notes(&source_name, &target_name, &sender);
            }
            AppMsg::MoveFolder {
                folder_name,
                target_folder,
            } => {
                self.move_folder(&folder_name, target_folder.as_deref(), &sender);
            }

            // Manejadores del reproductor de música
            AppMsg::ToggleMusicPlayer => {
                // El popover se abre/cierra automáticamente con el MenuButton
            }

            AppMsg::MusicSearch(query) => {
                println!("🔍 Buscando música: {}", query);
                let music_player_ref = self.music_player.clone();
                let sender_clone = sender.clone();
                let results_list = self.music_results_list.clone();
                let audio_sink = self
                    .notes_config
                    .borrow()
                    .get_audio_output_sink()
                    .map(|s| s.to_string());

                // Limpiar resultados anteriores
                while let Some(child) = results_list.first_child() {
                    results_list.remove(&child);
                }

                // Mostrar indicador de carga
                let loading_label = gtk::Label::new(Some("🔄 Buscando..."));
                loading_label.set_xalign(0.0);
                loading_label.set_margin_all(8);
                results_list.append(&loading_label);

                // Spawn tarea asíncrona para buscar
                gtk::glib::spawn_future_local(async move {
                    // Inicializar player bajo demanda
                    let player = {
                        let mut player_opt = music_player_ref.borrow_mut();
                        if player_opt.is_none() {
                            match crate::music_player::MusicPlayer::new(audio_sink.as_deref()) {
                                Ok(p) => {
                                    *player_opt = Some(Rc::new(p));
                                }
                                Err(e) => {
                                    // Mostrar error en UI
                                    while let Some(child) = results_list.first_child() {
                                        results_list.remove(&child);
                                    }
                                    let error_label = gtk::Label::new(Some(&format!(
                                        "❌ Error al inicializar reproductor: {}",
                                        e
                                    )));
                                    error_label.set_xalign(0.0);
                                    error_label.set_margin_all(8);
                                    error_label.add_css_class("error");
                                    results_list.append(&error_label);
                                    return;
                                }
                            }
                        }
                        player_opt.as_ref().unwrap().clone()
                    };

                    match player.search(&query).await {
                        Ok(results) => {
                            // Limpiar indicador de carga
                            while let Some(child) = results_list.first_child() {
                                results_list.remove(&child);
                            }

                            if results.is_empty() {
                                let no_results =
                                    gtk::Label::new(Some("❌ No se encontraron resultados"));
                                no_results.set_xalign(0.0);
                                no_results.set_margin_all(8);
                                results_list.append(&no_results);
                            } else {
                                println!("✅ {} canciones encontradas", results.len());

                                // Mostrar cada resultado como un botón clickeable
                                for song in results {
                                    let song_clone = song.clone();
                                    let song_clone2 = song.clone();
                                    let sender_clone2 = sender_clone.clone();
                                    let sender_clone3 = sender_clone.clone();

                                    let labels_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
                                    labels_box.set_hexpand(true);

                                    let title_label = gtk::Label::new(Some(&song.title));
                                    title_label.set_xalign(0.0);
                                    title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                                    title_label.add_css_class("music-title");

                                    let info_label = gtk::Label::new(Some(&format!(
                                        "{} {}",
                                        song.artist_names(),
                                        song.duration
                                            .as_ref()
                                            .map(|d| format!("• {}", d))
                                            .unwrap_or_default()
                                    )));
                                    info_label.set_xalign(0.0);
                                    info_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                                    info_label.add_css_class("dim-label");

                                    labels_box.append(&title_label);
                                    labels_box.append(&info_label);

                                    // Botón para agregar a playlist (independiente, no dentro del botón principal)
                                    let add_to_playlist_btn = gtk::Button::new();
                                    add_to_playlist_btn.set_icon_name("list-add-symbolic");
                                    add_to_playlist_btn
                                        .set_tooltip_text(Some("Agregar a playlist"));
                                    add_to_playlist_btn.add_css_class("flat");
                                    add_to_playlist_btn.add_css_class("circular");
                                    add_to_playlist_btn.connect_clicked(move |_| {
                                        sender_clone3
                                            .input(AppMsg::MusicAddToPlaylist(song_clone2.clone()));
                                    });

                                    // Botón principal para reproducir (solo con las etiquetas)
                                    let play_button = gtk::Button::new();
                                    play_button.set_child(Some(&labels_box));
                                    play_button.add_css_class("flat");
                                    play_button.set_hexpand(true);
                                    play_button.connect_clicked(move |_| {
                                        sender_clone2.input(AppMsg::MusicPlay(song_clone.clone()));
                                    });

                                    // Fila con botón de reproducir y botón de agregar
                                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                                    row.set_margin_all(8);
                                    row.append(&play_button);
                                    row.append(&add_to_playlist_btn);

                                    results_list.append(&row);
                                }
                            }
                        }
                        Err(e) => {
                            // Limpiar indicador de carga
                            while let Some(child) = results_list.first_child() {
                                results_list.remove(&child);
                            }

                            let error_label = gtk::Label::new(Some(&format!("❌ Error: {}", e)));
                            error_label.set_xalign(0.0);
                            error_label.set_margin_all(8);
                            error_label.set_wrap(true);
                            results_list.append(&error_label);
                            eprintln!("Error buscando música: {}", e);
                        }
                    }
                });
            }

            AppMsg::MusicPlay(song) => {
                println!("🎵 Reproduciendo: {} - {}", song.title, song.artist_names());
                let music_player_ref = self.music_player.clone();
                let sender_clone = sender.clone();

                // Actualizar UI - mostrar "Cargando"
                let full_title = format!("⏳ {} - {}", song.title, song.artist_names());
                self.music_now_playing_label.set_text(&full_title);
                self.music_now_playing_label
                    .set_tooltip_text(Some(&full_title));
                self.music_state_label.remove_css_class("music-state-idle");
                self.music_state_label
                    .remove_css_class("music-state-paused");
                self.music_state_label
                    .remove_css_class("music-state-playing");
                self.music_state_label.add_css_class("music-state-loading");

                // Cambiar icono del botón a "cargando"
                self.music_player_button
                    .set_icon_name("content-loading-symbolic");

                // Spawn tarea asíncrona para reproducir
                gtk::glib::spawn_future_local(async move {
                    // Clonar el player antes del await para evitar mantener el RefCell prestado
                    let player_opt = music_player_ref.borrow().as_ref().map(Rc::clone);
                    if let Some(player) = player_opt {
                        if let Err(e) = player.play(song.clone()).await {
                            eprintln!("❌ Error reproduciendo música: {}", e);
                        } else {
                            println!("✅ Reproducción iniciada correctamente");
                        }
                        // Actualizar estado después de reproducir
                        sender_clone.input(AppMsg::MusicUpdateState);
                    }
                });
            }

            AppMsg::MusicTogglePlayPause => {
                println!("⏯️  Toggle play/pause");
                if let Some(player) = self.music_player.borrow().as_ref() {
                    if let Err(e) = player.toggle_play_pause() {
                        eprintln!("Error al pausar/reanudar: {}", e);
                    } else {
                        println!("✅ Toggle exitoso");
                    }
                }
                sender.input(AppMsg::MusicUpdateState);
            }

            AppMsg::MusicStop => {
                if let Some(player) = self.music_player.borrow().as_ref() {
                    if let Err(e) = player.stop() {
                        eprintln!("Error al detener: {}", e);
                    }
                }
                let no_music_text = "No hay música reproduciéndose";
                self.music_now_playing_label.set_text(no_music_text);
                self.music_now_playing_label
                    .set_tooltip_text(Some(no_music_text));
                self.music_state_label
                    .remove_css_class("music-state-playing");
                self.music_state_label
                    .remove_css_class("music-state-paused");
                self.music_state_label.add_css_class("music-state-idle");
                self.music_player_button
                    .set_icon_name("media-playback-start-symbolic");
            }

            AppMsg::MusicSeekForward => {
                if let Some(player) = self.music_player.borrow().as_ref() {
                    if let Err(e) = player.seek_forward(5.0) {
                        eprintln!("Error al avanzar: {}", e);
                    }
                }
            }

            AppMsg::MusicSeekBackward => {
                if let Some(player) = self.music_player.borrow().as_ref() {
                    if let Err(e) = player.seek_backward(5.0) {
                        eprintln!("Error al retroceder: {}", e);
                    }
                }
            }

            AppMsg::MusicVolumeUp => {
                if let Some(player) = self.music_player.borrow().as_ref() {
                    if let Err(e) = player.volume_up() {
                        eprintln!("Error al subir volumen: {}", e);
                    }
                }
            }

            AppMsg::MusicVolumeDown => {
                if let Some(player) = self.music_player.borrow().as_ref() {
                    if let Err(e) = player.volume_down() {
                        eprintln!("Error al bajar volumen: {}", e);
                    }
                }
            }

            AppMsg::MusicUpdateState => {
                use crate::music_player::PlayerState;

                let (state, current_song) = {
                    let player_ref = self.music_player.borrow();
                    if let Some(player) = player_ref.as_ref() {
                        (player.state(), player.current_song())
                    } else {
                        (PlayerState::Idle, None)
                    }
                };

                // Actualizar label con la canción actual
                if let Some(song) = current_song {
                    let full_title = if state == PlayerState::Loading {
                        format!("⏳ {} - {}", song.title, song.artist_names())
                    } else {
                        format!("🎵 {} - {}", song.title, song.artist_names())
                    };
                    self.music_now_playing_label.set_text(&full_title);
                    self.music_now_playing_label
                        .set_tooltip_text(Some(&full_title));
                } else if state == PlayerState::Idle {
                    self.music_now_playing_label
                        .set_text("No hay música reproduciéndose");
                    self.music_now_playing_label
                        .set_tooltip_text(Some("No hay música reproduciéndose"));
                }

                println!("🔄 Actualizando estado UI: {:?}", state);
                match state {
                    PlayerState::Idle => {
                        self.music_state_label
                            .remove_css_class("music-state-playing");
                        self.music_state_label
                            .remove_css_class("music-state-paused");
                        self.music_state_label.add_css_class("music-state-idle");
                        // Botón interno del reproductor
                        self.music_play_pause_btn
                            .set_icon_name("media-playback-start-symbolic");
                    }
                    PlayerState::Playing => {
                        self.music_state_label.remove_css_class("music-state-idle");
                        self.music_state_label
                            .remove_css_class("music-state-paused");
                        self.music_state_label.add_css_class("music-state-playing");
                        // Cuando está reproduciendo, mostrar icono de PAUSA en el botón interno
                        self.music_play_pause_btn
                            .set_icon_name("media-playback-pause-symbolic");
                    }
                    PlayerState::Paused => {
                        self.music_state_label.remove_css_class("music-state-idle");
                        self.music_state_label
                            .remove_css_class("music-state-playing");
                        self.music_state_label.add_css_class("music-state-paused");
                        // Cuando está pausado, mostrar icono de PLAY en el botón interno
                        self.music_play_pause_btn
                            .set_icon_name("media-playback-start-symbolic");
                    }
                    PlayerState::Loading => {
                        self.music_state_label.remove_css_class("music-state-idle");
                        self.music_state_label.add_css_class("music-state-loading");
                        // Mostrar icono de carga en el botón interno
                        self.music_play_pause_btn
                            .set_icon_name("content-loading-symbolic");
                    }
                    PlayerState::Error => {
                        self.music_state_label
                            .remove_css_class("music-state-playing");
                        self.music_state_label.add_css_class("music-state-error");
                        // Mostrar icono de error en el botón interno
                        self.music_play_pause_btn
                            .set_icon_name("dialog-error-symbolic");
                    }
                }
            }

            AppMsg::MusicAddToPlaylist(song) => {
                // Inicializar player bajo demanda si no existe
                let player = {
                    let mut player_opt = self.music_player.borrow_mut();
                    if player_opt.is_none() {
                        let audio_sink = self
                            .notes_config
                            .borrow()
                            .get_audio_output_sink()
                            .map(|s| s.to_string());
                        match crate::music_player::MusicPlayer::new(audio_sink.as_deref()) {
                            Ok(p) => {
                                *player_opt = Some(Rc::new(p));
                            }
                            Err(e) => {
                                eprintln!("❌ Error al inicializar reproductor: {}", e);
                                return;
                            }
                        }
                    }
                    player_opt.as_ref().unwrap().clone()
                };

                player.add_to_playlist(song.clone());
                println!("✅ Canción agregada a la playlist: {}", song.title);
            }

            AppMsg::MusicRemoveFromPlaylist(index) => {
                if let Some(player) = self.music_player.borrow().as_ref() {
                    if let Some(removed) = player.remove_from_playlist(index) {
                        println!("✅ Canción eliminada: {}", removed.title);
                        // Refrescar vista
                        sender.input(AppMsg::TogglePlaylistView);
                    }
                }
            }

            AppMsg::MusicClearPlaylist => {
                if let Some(player) = self.music_player.borrow().as_ref() {
                    player.clear_playlist();
                    println!("✅ Playlist limpiada");
                    // Refrescar vista
                    sender.input(AppMsg::TogglePlaylistView);
                }
            }

            AppMsg::MusicNewPlaylist => {
                // Inicializar player si no existe
                if self.music_player.borrow().is_none() {
                    println!("🎵 Inicializando reproductor de música...");
                    use crate::music_player::MusicPlayer;
                    let player = MusicPlayer::new(None).expect("Failed to initialize music player");
                    *self.music_player.borrow_mut() = Some(Rc::new(player));
                }

                if let Some(player) = self.music_player.borrow().as_ref() {
                    // Crear nueva playlist vacía con nombre temporal
                    use crate::music_player::Playlist;
                    let new_playlist = Playlist::new("Cola de reproducción".to_string());
                    player.load_playlist(new_playlist);
                    println!("📝 Nueva playlist creada");
                    // Refrescar vista
                    sender.input(AppMsg::TogglePlaylistView);
                }
            }

            AppMsg::MusicNextSong => {
                let music_player_ref = self.music_player.clone();
                let sender_clone = sender.clone();
                gtk::glib::spawn_future_local(async move {
                    let player_opt = music_player_ref.borrow().as_ref().map(Rc::clone);
                    if let Some(player) = player_opt {
                        match player.play_next().await {
                            Ok(_) => {
                                println!("✅ Reproduciendo siguiente canción");
                                sender_clone.input(AppMsg::MusicUpdateState);
                            }
                            Err(e) => {
                                eprintln!("❌ Error al reproducir siguiente: {}", e);
                            }
                        }
                    }
                });
            }

            AppMsg::MusicPreviousSong => {
                let music_player_ref = self.music_player.clone();
                let sender_clone = sender.clone();
                gtk::glib::spawn_future_local(async move {
                    let player_opt = music_player_ref.borrow().as_ref().map(Rc::clone);
                    if let Some(player) = player_opt {
                        match player.play_previous().await {
                            Ok(_) => {
                                println!("✅ Reproduciendo canción anterior");
                                sender_clone.input(AppMsg::MusicUpdateState);
                            }
                            Err(e) => {
                                eprintln!("❌ Error al reproducir anterior: {}", e);
                            }
                        }
                    }
                });
            }

            AppMsg::MusicPlayFromPlaylist(index) => {
                // Cerrar popover principal al reproducir desde playlist
                self.music_player_popover.popdown();

                let music_player_ref = self.music_player.clone();
                let sender_clone = sender.clone();
                gtk::glib::spawn_future_local(async move {
                    let player_opt = music_player_ref.borrow().as_ref().map(Rc::clone);
                    if let Some(player) = player_opt {
                        match player.play_from_playlist(index).await {
                            Ok(_) => {
                                println!("✅ Reproduciendo canción de playlist");
                                sender_clone.input(AppMsg::MusicUpdateState);
                            }
                            Err(e) => {
                                eprintln!("❌ Error al reproducir de playlist: {}", e);
                            }
                        }
                    }
                });
            }

            AppMsg::MusicToggleRepeat => {
                use crate::music_player::RepeatMode;
                if let Some(player) = self.music_player.borrow().as_ref() {
                    let current = player.repeat_mode();
                    let next = match current {
                        RepeatMode::Off => RepeatMode::All,
                        RepeatMode::All => RepeatMode::One,
                        RepeatMode::One => RepeatMode::Off,
                    };
                    player.set_repeat_mode(next);
                    println!("🔁 Modo repetición: {:?}", next);
                }
            }

            AppMsg::MusicToggleShuffle => {
                if let Some(player) = self.music_player.borrow().as_ref() {
                    player.toggle_shuffle();
                    let is_shuffle = player.is_shuffle();
                    println!("🔀 Shuffle: {}", if is_shuffle { "ON" } else { "OFF" });
                }
            }

            AppMsg::MusicSavePlaylist(name) => {
                if let Some(player) = self.music_player.borrow().as_ref() {
                    match player.save_current_playlist(Some(name.clone())) {
                        Ok(_) => {
                            println!("✅ Playlist '{}' guardada", name);
                            // Refrescar vista de playlists guardadas
                            sender.input(AppMsg::TogglePlaylistView);
                        }
                        Err(e) => eprintln!("❌ Error guardando playlist: {}", e),
                    }
                }
            }

            AppMsg::MusicLoadPlaylist(name) => {
                use crate::music_player::Playlist;
                match Playlist::load(&name) {
                    Ok(playlist) => {
                        let song_count = playlist.len();
                        println!(
                            "✅ Playlist '{}' cargada con {} canciones",
                            name, song_count
                        );

                        // Inicializar player si no existe
                        if self.music_player.borrow().is_none() {
                            println!("🎵 Inicializando reproductor de música...");
                            use crate::music_player::MusicPlayer;
                            let player =
                                MusicPlayer::new(None).expect("Failed to initialize music player");
                            *self.music_player.borrow_mut() = Some(Rc::new(player));
                        }

                        if let Some(player) = self.music_player.borrow().as_ref() {
                            player.load_playlist(playlist);

                            // Debug: verificar que se cargó
                            if let Some(loaded_pl) = player.current_playlist() {
                                println!(
                                    "🔍 Playlist cargada verificada: {} canciones",
                                    loaded_pl.len()
                                );
                                for (i, song) in loaded_pl.songs.iter().enumerate() {
                                    println!(
                                        "  {}. {} - {}",
                                        i + 1,
                                        song.title,
                                        song.artist_names()
                                    );
                                }
                            }

                            // Refrescar vista
                            sender.input(AppMsg::TogglePlaylistView);
                        }
                    }
                    Err(e) => eprintln!("❌ Error cargando playlist: {}", e),
                }
            }

            AppMsg::MusicDeletePlaylist(name) => {
                use crate::music_player::Playlist;
                match Playlist::delete(&name) {
                    Ok(_) => {
                        println!("✅ Playlist '{}' eliminada", name);
                        // Refrescar vista de playlists guardadas
                        sender.input(AppMsg::TogglePlaylistView);
                    }
                    Err(e) => eprintln!("❌ Error eliminando playlist: {}", e),
                }
            }

            AppMsg::MusicCheckNextSong => {
                // Verificar si debe reproducir la siguiente canción
                // Usar catch_unwind para prevenir panics del reproductor
                if let Some(player) = self.music_player.borrow().as_ref() {
                    // Intentar verificar el estado del reproductor de forma segura
                    if let Ok(should_play) =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            player.check_should_play_next()
                        }))
                    {
                        if should_play {
                            sender.input(AppMsg::MusicNextSong);
                        }
                    }
                    // Si hay un panic, simplemente lo ignoramos y continuamos
                }
            }

            AppMsg::TogglePlaylistView => {
                println!("🔄 Actualizando vista de playlist...");

                // Actualizar lista de canciones en la cola actual
                while let Some(child) = self.playlist_current_list.first_child() {
                    self.playlist_current_list.remove(&child);
                }

                if let Some(player) = self.music_player.borrow().as_ref() {
                    if let Some(playlist) = player.current_playlist() {
                        println!(
                            "📋 Playlist encontrada con {} canciones",
                            playlist.songs.len()
                        );
                        if playlist.songs.is_empty() {
                            let empty_label = gtk::Label::new(Some("Cola vacía"));
                            empty_label.add_css_class("dim-label");
                            empty_label.set_margin_all(8);
                            self.playlist_current_list.append(&empty_label);
                        } else {
                            for (idx, song) in playlist.songs.iter().enumerate() {
                                let song_clone = song.clone();
                                let sender_clone = sender.clone();

                                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                                row.set_margin_all(4);

                                // Indicador de canción actual
                                let indicator = if idx == playlist.current_index {
                                    "▶ "
                                } else {
                                    &format!("{}. ", idx + 1)
                                };

                                let labels_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
                                labels_box.set_hexpand(true);

                                let title_label =
                                    gtk::Label::new(Some(&format!("{}{}", indicator, song.title)));
                                title_label.set_xalign(0.0);
                                title_label.set_ellipsize(gtk::pango::EllipsizeMode::End);

                                let artist_label = gtk::Label::new(Some(&song.artist_names()));
                                artist_label.set_xalign(0.0);
                                artist_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                                artist_label.add_css_class("dim-label");

                                labels_box.append(&title_label);
                                labels_box.append(&artist_label);

                                // Botón para reproducir esta canción
                                let play_btn = gtk::Button::new();
                                play_btn.set_icon_name("media-playback-start-symbolic");
                                play_btn.set_tooltip_text(Some("Reproducir"));
                                play_btn.add_css_class("flat");
                                play_btn.add_css_class("circular");
                                play_btn.connect_clicked(move |_| {
                                    sender_clone.input(AppMsg::MusicPlayFromPlaylist(idx));
                                });

                                // Botón para eliminar de la cola
                                let remove_btn = gtk::Button::new();
                                remove_btn.set_icon_name("list-remove-symbolic");
                                remove_btn.set_tooltip_text(Some("Eliminar"));
                                remove_btn.add_css_class("flat");
                                remove_btn.add_css_class("circular");

                                let sender_clone2 = sender.clone();
                                remove_btn.connect_clicked(move |_| {
                                    sender_clone2.input(AppMsg::MusicRemoveFromPlaylist(idx));
                                });

                                row.append(&labels_box);
                                row.append(&play_btn);
                                row.append(&remove_btn);

                                self.playlist_current_list.append(&row);
                            }
                        }
                    } else {
                        println!("⚠️  No hay playlist cargada en el player");
                        let empty_label = gtk::Label::new(Some("No hay playlist cargada"));
                        empty_label.add_css_class("dim-label");
                        empty_label.set_margin_all(8);
                        self.playlist_current_list.append(&empty_label);
                    }
                } else {
                    println!("⚠️  No hay music player inicializado");
                }

                // Actualizar lista de playlists guardadas
                while let Some(child) = self.playlist_saved_list.first_child() {
                    self.playlist_saved_list.remove(&child);
                }

                use crate::music_player::Playlist;
                match Playlist::list_saved() {
                    Ok(playlists) => {
                        if playlists.is_empty() {
                            let empty_label = gtk::Label::new(Some("No hay playlists guardadas"));
                            empty_label.add_css_class("dim-label");
                            empty_label.set_margin_all(8);
                            self.playlist_saved_list.append(&empty_label);
                        } else {
                            for playlist_name in playlists {
                                let name_clone = playlist_name.clone();
                                let name_clone2 = playlist_name.clone();
                                let sender_clone = sender.clone();
                                let sender_clone2 = sender.clone();

                                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                                row.set_margin_all(4);

                                let label = gtk::Label::new(Some(&playlist_name));
                                label.set_xalign(0.0);
                                label.set_hexpand(true);

                                // Botón para cargar playlist
                                let load_btn = gtk::Button::new();
                                load_btn.set_icon_name("media-playback-start-symbolic");
                                load_btn.set_tooltip_text(Some("Cargar playlist"));
                                load_btn.add_css_class("flat");
                                load_btn.add_css_class("circular");
                                load_btn.connect_clicked(move |_| {
                                    sender_clone
                                        .input(AppMsg::MusicLoadPlaylist(name_clone.clone()));
                                });

                                // Botón para eliminar playlist
                                let delete_btn = gtk::Button::new();
                                delete_btn.set_icon_name("user-trash-symbolic");
                                delete_btn.set_tooltip_text(Some("Eliminar playlist"));
                                delete_btn.add_css_class("flat");
                                delete_btn.add_css_class("circular");
                                delete_btn.connect_clicked(move |_| {
                                    sender_clone2
                                        .input(AppMsg::MusicDeletePlaylist(name_clone2.clone()));
                                });

                                row.append(&label);
                                row.append(&load_btn);
                                row.append(&delete_btn);

                                self.playlist_saved_list.append(&row);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error listando playlists: {}", e);
                        let error_label = gtk::Label::new(Some(&format!("Error: {}", e)));
                        error_label.add_css_class("dim-label");
                        error_label.set_margin_all(8);
                        self.playlist_saved_list.append(&error_label);
                    }
                }
            }

            // ==================== CHAT AI HANDLERS ====================
            AppMsg::EnterChatMode => {
                println!("🤖 Entrando al modo Chat AI...");

                // Cambiar modo
                *self.mode.borrow_mut() = EditorMode::ChatAI;
                self.update_status_bar(&sender);

                // Ocultar sidebar principal
                self.split_view.set_position(0);

                // Mostrar sidebar de contexto en el chat
                self.chat_split_view.set_position(250);

                // Cambiar a la página del chat en el Stack
                self.content_stack.set_visible_child_name("chat");

                // Limpiar historial visual
                while let Some(child) = self.chat_history_list.first_child() {
                    self.chat_history_list.remove(&child);
                }

                // Crear o cargar sesión con configuración actualizada
                let ai_config = self.notes_config.borrow().get_ai_config().clone();
                let model_config = crate::ai_chat::AIModelConfig {
                    provider: match ai_config.provider.as_str() {
                        "anthropic" => crate::ai_chat::AIProvider::Anthropic,
                        "ollama" => crate::ai_chat::AIProvider::Ollama,
                        _ => crate::ai_chat::AIProvider::OpenAI,
                    },
                    model: ai_config.model.clone(),
                    max_tokens: ai_config.max_tokens as usize,
                    temperature: ai_config.temperature,
                };

                // Actualizar label del modelo
                self.chat_model_label.set_text(&format!(
                    "{} - {} (temp: {:.1})",
                    ai_config.provider, ai_config.model, ai_config.temperature
                ));

                // Verificar si ya tenemos una sesión activa en memoria para reanudarla
                let has_active_session = self.chat_session.borrow().is_some();

                if has_active_session {
                    println!("♻️ Reanudando sesión de chat activa en memoria");

                    // Actualizar configuración del modelo en la sesión existente
                    if let Some(session) = self.chat_session.borrow_mut().as_mut() {
                        session.model_config = model_config.clone();

                        // Renderizar mensajes existentes
                        for msg in &session.messages {
                            if msg.role == crate::ai_chat::MessageRole::Assistant
                                && self.is_search_result(&msg.content)
                            {
                                self.append_search_results_widget(&msg.content, &sender);
                            } else {
                                self.append_chat_message(
                                    msg.role,
                                    &msg.content,
                                    Some(sender.clone()),
                                );
                            }
                        }
                    }
                } else {
                    // Intentar cargar la última sesión si save_history está activado
                    if ai_config.save_history {
                        if let Ok(Some(session_id)) = self.notes_db.get_latest_chat_session() {
                            println!("📂 Cargando sesión #{}", session_id);
                            *self.chat_session_id.borrow_mut() = Some(session_id);

                            // Cargar mensajes de la sesión
                            if let Ok(messages) = self.notes_db.get_chat_messages(session_id) {
                                let mut session =
                                    crate::ai_chat::ChatSession::new(model_config.clone());

                                for (role_str, content, _timestamp) in messages {
                                    let role = match role_str.as_str() {
                                        "user" => crate::ai_chat::MessageRole::User,
                                        "assistant" => crate::ai_chat::MessageRole::Assistant,
                                        _ => crate::ai_chat::MessageRole::System,
                                    };

                                    session.add_message(role.clone(), content.clone());

                                    // Detectar si es resultado de búsqueda y renderizar apropiadamente
                                    if role == crate::ai_chat::MessageRole::Assistant
                                        && self.is_search_result(&content)
                                    {
                                        self.append_search_results_widget(&content, &sender);
                                    } else {
                                        self.append_chat_message(
                                            role,
                                            &content,
                                            Some(sender.clone()),
                                        );
                                    }
                                }

                                // Cargar notas del contexto
                                if let Ok(notes_meta) =
                                    self.notes_db.get_chat_context_notes(session_id)
                                {
                                    for note_meta in notes_meta {
                                        if let Ok(Some(note_file)) =
                                            self.notes_dir.find_note(&note_meta.name)
                                        {
                                            session.attach_note(note_file);
                                        }
                                    }
                                }

                                *self.chat_session.borrow_mut() = Some(session);
                            }
                        } else {
                            // Crear nueva sesión en BD
                            if let Ok(session_id) = self.notes_db.create_chat_session(
                                &ai_config.model,
                                &ai_config.provider,
                                ai_config.temperature,
                                ai_config.max_tokens,
                            ) {
                                println!("✨ Nueva sesión creada: #{}", session_id);
                                *self.chat_session_id.borrow_mut() = Some(session_id);
                                *self.chat_session.borrow_mut() =
                                    Some(crate::ai_chat::ChatSession::new(model_config.clone()));
                            }
                        }
                    } else {
                        // Si save_history está desactivado, crear sesión en memoria
                        *self.chat_session.borrow_mut() =
                            Some(crate::ai_chat::ChatSession::new(model_config.clone()));
                        *self.chat_session_id.borrow_mut() = None;
                    }
                }

                // Adjuntar nota actual al contexto (si no está ya)
                if let Some(note) = &self.current_note {
                    {
                        if let Some(session) = self.chat_session.borrow_mut().as_mut() {
                            session.attach_note(note.clone());

                            // Guardar en BD si corresponde
                            if let (Some(session_id), Some(note_id)) =
                                (*self.chat_session_id.borrow(), self.get_current_note_id())
                            {
                                let _ = self.notes_db.attach_note_to_chat(session_id, note_id);
                            }
                        }
                    } // ← Libera borrow_mut aquí
                }

                self.refresh_context_list();
                sender.input(AppMsg::UpdateChatTokenCount);

                // Dar foco al input con un pequeño delay para asegurar que el widget esté renderizado
                let input_clone = self.chat_input_view.clone();
                gtk::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(100),
                    move || {
                        input_clone.grab_focus();
                    },
                );
            }

            AppMsg::ExitChatMode => {
                println!("👋 Saliendo del modo Chat AI...");

                *self.mode.borrow_mut() = EditorMode::Normal;
                self.update_status_bar(&sender);

                // Restaurar sidebar principal si estaba visible
                if self.sidebar_visible {
                    self.split_view.set_position(250);
                }

                // Ocultar sidebar de contexto del chat
                self.chat_split_view.set_position(0);

                // Recargar la nota actual por si hubo cambios mientras estábamos en el chat
                if let Some(ref note) = self.current_note {
                    if !self.has_unsaved_changes {
                        if let Ok(content) = note.read() {
                            println!(
                                "🔄 Recargando nota actual al salir del chat: {}",
                                note.name()
                            );

                            // Recargar contenido en el buffer
                            self.buffer = crate::core::NoteBuffer::from_text(&content);
                            self.cursor_position = 0;

                            // Sincronizar a la vista para renderizar markdown correctamente
                            self.sync_to_view();

                            // Actualizar UI
                            sender.input(AppMsg::RefreshTags);
                        }
                    } else {
                        println!(
                            "⚠️ Nota tiene cambios sin guardar, no se recarga al salir del chat"
                        );
                    }
                }

                // Cambiar a la página del editor en el Stack
                self.content_stack.set_visible_child_name("editor");

                // Re-seleccionar la nota actual en el sidebar si existe
                if let Some(ref note) = self.current_note {
                    let full_name = note.name();
                    let note_name = full_name.split('/').last().unwrap_or(full_name).to_string();

                    // Detectar la carpeta de la nota
                    let note_folder = note
                        .path()
                        .parent()
                        .and_then(|p| p.strip_prefix(self.notes_dir.root()).ok())
                        .filter(|p| !p.as_os_str().is_empty())
                        .and_then(|p| p.to_str())
                        .map(|s| s.to_string());

                    let notes_list = self.notes_list.clone();
                    let text_view = self.text_view.clone();

                    gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(100),
                        move || {
                            // Re-seleccionar la nota en el sidebar
                            let mut child = notes_list.first_child();
                            let mut found = false;
                            let mut current_folder: Option<String> = None;

                            while let Some(widget) = child {
                                if let Ok(list_row) = widget.clone().downcast::<gtk::ListBoxRow>() {
                                    let is_folder = unsafe {
                                        list_row
                                            .data::<bool>("is_folder")
                                            .map(|data| *data.as_ref())
                                            .unwrap_or(false)
                                    };

                                    if is_folder {
                                        current_folder = unsafe {
                                            list_row
                                                .data::<String>("folder_name")
                                                .map(|data| data.as_ref().clone())
                                        };
                                    } else if list_row.is_selectable() {
                                        let note_name_from_data = unsafe {
                                            list_row
                                                .data::<String>("note_name")
                                                .map(|data| data.as_ref().clone())
                                        };

                                        let name_matches = if let Some(name) = note_name_from_data {
                                            name == note_name
                                        } else if let Some(child_w) = list_row.child() {
                                            if let Ok(box_widget) = child_w.downcast::<gtk::Box>() {
                                                if let Some(label_widget) = box_widget
                                                    .first_child()
                                                    .and_then(|w| w.next_sibling())
                                                {
                                                    if let Ok(label) =
                                                        label_widget.downcast::<gtk::Label>()
                                                    {
                                                        label.text() == note_name
                                                    } else {
                                                        false
                                                    }
                                                } else {
                                                    false
                                                }
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        };

                                        if name_matches && current_folder == note_folder {
                                            notes_list.select_row(Some(&list_row));
                                            found = true;
                                            break;
                                        }
                                    }
                                }
                                child = widget.next_sibling();
                            }

                            if !found {
                                println!(
                                    "⚠️ No se encontró la nota '{}' en carpeta {:?} en el sidebar al salir del chat",
                                    note_name, note_folder
                                );
                            }

                            // Dar foco al editor
                            text_view.grab_focus();
                        },
                    );
                } else {
                    // Si no hay nota, solo dar foco al editor
                    let text_view = self.text_view.clone();
                    gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(100),
                        move || {
                            text_view.grab_focus();
                        },
                    );
                }
            }

            AppMsg::SendChatMessage(message) => {
                println!(
                    "💬 Enviando mensaje: {}",
                    message.chars().take(50).collect::<String>()
                );

                // Parsear menciones de notas @nota y adjuntarlas al contexto
                let note_mentions = self.extract_note_mentions(&message);
                if !note_mentions.is_empty() {
                    println!("📎 Notas mencionadas: {:?}", note_mentions);

                    // Adjuntar cada nota mencionada
                    for note_name in &note_mentions {
                        if let Ok(Some(note_file)) = self.notes_dir.find_note(note_name) {
                            if let Some(session) = self.chat_session.borrow_mut().as_mut() {
                                session.attach_note(note_file.clone());

                                // Guardar en BD si corresponde
                                if let (Some(session_id), Some(note_id)) =
                                    (*self.chat_session_id.borrow(), self.get_note_id(&note_file))
                                {
                                    let _ = self.notes_db.attach_note_to_chat(session_id, note_id);
                                }
                            }
                        } else {
                            println!("⚠️ Nota no encontrada: {}", note_name);
                        }
                    }

                    // Actualizar lista de contexto
                    self.refresh_context_list();
                    sender.input(AppMsg::UpdateChatTokenCount);
                }

                if let Some(session) = self.chat_session.borrow_mut().as_mut() {
                    // Agregar mensaje del usuario
                    session.add_message(crate::ai_chat::MessageRole::User, message.clone());

                    // Guardar mensaje en BD si hay sesión activa
                    if let Some(session_id) = *self.chat_session_id.borrow() {
                        let _ = self
                            .notes_db
                            .save_chat_message(session_id, "user", &message);
                    }

                    // Mostrar en UI
                    self.append_chat_message(
                        crate::ai_chat::MessageRole::User,
                        &message,
                        Some(sender.clone()),
                    );

                    // Limpiar input
                    self.chat_input_buffer.set_text("");

                    // Verificar si hay RouterAgent disponible y si el modo agente está activo
                    let has_router = self.router_agent.borrow().is_some();
                    let agent_mode = *self.chat_agent_mode.borrow();

                    if has_router && agent_mode {
                        // ============ MODO AGENTE: RouterAgent con ReAct y tools ============
                        println!("🤖 Usando RouterAgent (sistema multi-agente)");

                        // Mostrar indicador de análisis
                        let analyzing_text = self.i18n.borrow().t("analyzing_task");
                        self.append_chat_typing_indicator(&analyzing_text);

                        let router_agent = self.router_agent.clone();
                        let sender_clone = sender.clone();
                        let message_clone = message.clone();
                        let chat_session_id = *self.chat_session_id.borrow();
                        let notes_db = self.notes_db.clone_connection();
                        let mcp_executor = self.mcp_executor.clone();

                        // Clonar los mensajes del historial para pasarlos al router
                        let chat_messages = session.messages.clone();

                        // Clonar las notas adjuntas para construir el contexto
                        let attached_notes = session.attached_notes.clone();

                        gtk::glib::spawn_future_local(async move {
                            // Construir contexto desde la sesión (notas adjuntas)
                            let mut context = String::new();
                            for note in &attached_notes {
                                if let Ok(content) = note.read() {
                                    context.push_str(&format!(
                                        "=== {} ===\n{}\n\n",
                                        note.name(),
                                        content
                                    ));
                                }
                            }

                            if !context.is_empty() {
                                println!(
                                    "📋 Contexto construido: {} notas, {} caracteres",
                                    attached_notes.len(),
                                    context.len()
                                );
                            }

                            // Crear callback para enviar los pasos del ReAct a la UI en tiempo real
                            let sender_for_steps = sender_clone.clone();
                            let step_callback =
                                move |step: &crate::ai::executors::react::ReActStep| {
                                    match step {
                                        crate::ai::executors::react::ReActStep::Thought(text) => {
                                            sender_for_steps
                                                .input(AppMsg::ShowAgentThought(text.clone()));
                                        }
                                        crate::ai::executors::react::ReActStep::Action(
                                            tool_call,
                                        ) => {
                                            let action_text = format!("{:?}", tool_call);
                                            sender_for_steps
                                                .input(AppMsg::ShowAgentAction(action_text));
                                        }
                                        crate::ai::executors::react::ReActStep::Observation(
                                            text,
                                        ) => {
                                            sender_for_steps
                                                .input(AppMsg::ShowAgentObservation(text.clone()));
                                        }
                                        crate::ai::executors::react::ReActStep::Answer(_) => {
                                            // Answer se maneja aparte después del resultado
                                        }
                                    }
                                };

                            // Ejecutar router (clasifica intent y delega al agente apropiado)
                            // Clonar router y executor para evitar mantener RefCell prestado durante await
                            let router_opt = router_agent.borrow().as_ref().cloned();
                            let executor = mcp_executor.borrow().clone();

                            match router_opt {
                                Some(router) => {
                                    match router
                                        .route_and_execute(
                                            &chat_messages,
                                            &context,
                                            &executor,
                                            step_callback,
                                        )
                                        .await
                                    {
                                        Ok(response) => {
                                            // Respuesta exitosa del agente
                                            sender_clone.input(AppMsg::ReceiveChatResponse(
                                                response.clone(),
                                            ));

                                            // NOTA: No guardar aquí en BD - se guarda en ReceiveChatResponse
                                            // para evitar duplicados

                                            // Actualizar sidebar con delay para asegurar que el filesystem se actualizó
                                            let sender_for_refresh = sender_clone.clone();
                                            gtk::glib::timeout_add_local_once(
                                                std::time::Duration::from_millis(200),
                                                move || {
                                                    sender_for_refresh
                                                        .input(AppMsg::RefreshSidebar);
                                                },
                                            );
                                        }
                                        Err(e) => {
                                            // Error en el router
                                            let error_msg = format!("❌ Error: {}", e);
                                            sender_clone
                                                .input(AppMsg::ReceiveChatResponse(error_msg));
                                        }
                                    }
                                }
                                None => {
                                    // Router no disponible (no debería pasar)
                                    sender_clone.input(AppMsg::ReceiveChatResponse(
                                        "❌ Error: Router no disponible".to_string(),
                                    ));
                                }
                            }
                        });
                    } else {
                        // ============ MODO CHAT NORMAL: Sin tools, conversación directa ============
                        println!("💬 Usando Chat Normal (sin herramientas) con STREAMING");

                        // Obtener API key de la configuración
                        let api_key = self
                            .notes_config
                            .borrow()
                            .get_ai_config()
                            .api_key
                            .clone()
                            .unwrap_or_else(|| std::env::var("OPENAI_API_KEY").unwrap_or_default());

                        if api_key.is_empty() {
                            sender.input(AppMsg::ReceiveChatResponse(
                                "❌ Error: No se ha configurado la API Key. \
                                 Ve a Ajustes > AI Assistant para configurarla."
                                    .to_string(),
                            ));
                            return;
                        }

                        // Chat normal sin tools pero CON STREAMING
                        let session_clone = session.clone();
                        let sender_clone = sender.clone();

                        // Iniciar el mensaje de streaming
                        sender.input(AppMsg::StartChatStream);

                        gtk::glib::spawn_future_local(async move {
                            match crate::ai_client::create_client(
                                &session_clone.model_config,
                                &api_key,
                            ) {
                                Ok(client) => {
                                    // Construir contexto desde notas adjuntas
                                    let mut context_parts = Vec::new();
                                    for note in &session_clone.attached_notes {
                                        if let Ok(content) = note.read() {
                                            context_parts.push(format!(
                                                "=== {} ===\n{}",
                                                note.name(),
                                                content
                                            ));
                                        }
                                    }
                                    let context = if context_parts.is_empty() {
                                        String::new()
                                    } else {
                                        format!(
                                            "Notas disponibles para consulta:\n\n{}",
                                            context_parts.join("\n\n")
                                        )
                                    };

                                    // Crear mensajes con system prompt diferente para chat normal
                                    let mut chat_messages = Vec::new();

                                    // System prompt para chat normal que menciona el contexto si existe
                                    let system_prompt = if !context.is_empty() {
                                        format!(
                                            "Eres un asistente conversacional amigable y útil. Responde de manera natural y directa a las preguntas del usuario.\n\n\
                                            Tienes acceso al siguiente contexto de notas para consulta:\n\n{}",
                                            context
                                        )
                                    } else {
                                        "Eres un asistente conversacional amigable y útil. Responde de manera natural y directa a las preguntas del usuario.".to_string()
                                    };

                                    chat_messages.push(crate::ai_chat::ChatMessage {
                                        role: crate::ai_chat::MessageRole::System,
                                        content: system_prompt,
                                        timestamp: chrono::Utc::now(),
                                        context_notes: Vec::new(),
                                    });

                                    // Agregar mensajes del historial (excepto el system prompt original)
                                    for msg in &session_clone.messages {
                                        if msg.role != crate::ai_chat::MessageRole::System {
                                            chat_messages.push(msg.clone());
                                        }
                                    }

                                    // Usar streaming!
                                    match client.send_message_streaming(&chat_messages, "").await {
                                        Ok(mut rx) => {
                                            // Recibir chunks y enviarlos a la UI
                                            while let Some(chunk) = rx.recv().await {
                                                sender_clone.input(AppMsg::ReceiveChatChunk(chunk));
                                            }
                                            // Finalizar streaming
                                            sender_clone.input(AppMsg::EndChatStream);
                                        }
                                        Err(e) => {
                                            sender_clone.input(AppMsg::ReceiveChatResponse(
                                                format!("❌ Error: {}", e),
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    sender_clone.input(AppMsg::ReceiveChatResponse(format!(
                                        "❌ Error creando cliente: {}",
                                        e
                                    )));
                                }
                            }
                        });
                    }
                }
            }

            AppMsg::ReceiveChatResponse(response) => {
                println!("🤖 Respuesta recibida: {} caracteres", response.len());

                // Retirar el indicador inmediatamente para evitar que quede colgado
                self.remove_chat_typing_indicator();

                // Limpiar contenedor de pensamiento del agente si existe
                *self.chat_thinking_container.borrow_mut() = None;

                // Agregar a la sesión
                if let Some(session) = self.chat_session.borrow_mut().as_mut() {
                    session.add_message(crate::ai_chat::MessageRole::Assistant, response.clone());
                }

                // Guardar respuesta en BD si hay sesión activa
                if let Some(session_id) = *self.chat_session_id.borrow() {
                    let _ = self
                        .notes_db
                        .save_chat_message(session_id, "assistant", &response);
                }

                // Mostrar en UI SOLO si NO es un resultado de búsqueda
                // (los resultados de búsqueda ya se mostraron como widget)
                if !self.is_search_result(&response) {
                    self.append_chat_message(
                        crate::ai_chat::MessageRole::Assistant,
                        &response,
                        Some(sender.clone()),
                    );
                } else {
                    println!("🔍 Resultado de búsqueda ya mostrado como widget, no duplicar");
                }

                sender.input(AppMsg::UpdateChatTokenCount);
            }

            AppMsg::StartChatStream => {
                // Eliminar indicador de "Pensando..." si existe
                self.remove_chat_typing_indicator();

                // Limpiar texto acumulado
                *self.chat_streaming_text.borrow_mut() = String::new();

                // Crear el mensaje visual con un label vacío que iremos actualizando
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                row.set_margin_top(6);
                row.set_margin_bottom(6);
                row.set_hexpand(true);
                row.set_halign(gtk::Align::Start);
                row.add_css_class("chat-row");
                row.add_css_class("chat-row-assistant");

                let avatar = gtk::Label::new(Some("🤖"));
                avatar.add_css_class("chat-avatar");
                avatar.add_css_class("chat-avatar-assistant");
                avatar.set_valign(gtk::Align::Start);
                row.append(&avatar);

                let bubble = gtk::Box::new(gtk::Orientation::Vertical, 4);
                bubble.add_css_class("chat-bubble");
                bubble.add_css_class("chat-bubble-assistant");

                let label = gtk::Label::new(Some(""));
                label.set_xalign(0.0);
                label.set_yalign(0.0);
                label.set_wrap(true);
                label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                label.set_selectable(true);
                label.set_use_markup(false);
                label.add_css_class("chat-message");
                label.add_css_class("chat-message-streaming");
                bubble.append(&label);

                row.append(&bubble);
                self.chat_history_list.append(&row);

                // Guardar referencia al label para ir actualizándolo
                *self.chat_streaming_label.borrow_mut() = Some(label);

                self.schedule_chat_scroll();
            }

            AppMsg::ReceiveChatChunk(chunk) => {
                // Agregar chunk al texto acumulado
                let mut streaming_text = self.chat_streaming_text.borrow_mut();
                streaming_text.push_str(&chunk);

                // Actualizar el label si existe
                if let Some(label) = self.chat_streaming_label.borrow().as_ref() {
                    label.set_text(&streaming_text);
                    self.schedule_chat_scroll();
                }
            }

            AppMsg::EndChatStream => {
                // Obtener texto final
                let final_text = self.chat_streaming_text.borrow().clone();

                // Limpiar referencia al label
                *self.chat_streaming_label.borrow_mut() = None;

                // Eliminar la fila de streaming para reemplazarla con el mensaje formateado
                if let Some(last_child) = self.chat_history_list.last_child() {
                    self.chat_history_list.remove(&last_child);
                }

                // Agregar mensaje final formateado correctamente (con soporte para links)
                self.append_chat_message(
                    crate::ai_chat::MessageRole::Assistant,
                    &final_text,
                    Some(sender.clone()),
                );

                // Agregar a la sesión
                if let Some(session) = self.chat_session.borrow_mut().as_mut() {
                    session.add_message(crate::ai_chat::MessageRole::Assistant, final_text.clone());
                }

                // Guardar respuesta en BD si hay sesión activa
                if let Some(session_id) = *self.chat_session_id.borrow() {
                    let _ = self
                        .notes_db
                        .save_chat_message(session_id, "assistant", &final_text);
                }

                sender.input(AppMsg::UpdateChatTokenCount);
            }

            AppMsg::ShowAgentThought(thought) => {
                // Crear o actualizar el contenedor de "thinking steps"
                self.ensure_thinking_container();

                if let Some(container) = self.chat_thinking_container.borrow().as_ref() {
                    let i18n = self.i18n.borrow();

                    // Crear box para el pensamiento
                    let thought_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    thought_box.set_margin_all(4);
                    thought_box.add_css_class("agent-thought");

                    let icon_text = i18n.t("chat_agent_thinking");
                    let icon = gtk::Label::new(Some(&icon_text));
                    icon.set_margin_end(4);
                    thought_box.append(&icon);

                    let text = gtk::Label::new(Some(&thought));
                    text.set_xalign(0.0);
                    text.set_wrap(true);
                    text.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                    text.set_selectable(true);
                    text.add_css_class("agent-thought-text");
                    thought_box.append(&text);

                    container.append(&thought_box);
                    self.schedule_chat_scroll();
                }
            }

            AppMsg::ShowAgentAction(action) => {
                self.ensure_thinking_container();

                if let Some(container) = self.chat_thinking_container.borrow().as_ref() {
                    let i18n = self.i18n.borrow();

                    let action_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
                    action_box.set_margin_all(8);
                    action_box.add_css_class("agent-action");

                    // Header con icono
                    let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let icon_text = i18n.t("chat_agent_action");
                    let icon =
                        gtk::Label::new(Some(&icon_text.chars().take(2).collect::<String>()));
                    icon.set_margin_end(4);
                    header_box.append(&icon);

                    let header_text = icon_text.chars().skip(3).collect::<String>();
                    let header_label = gtk::Label::new(Some(&header_text));
                    header_label.set_xalign(0.0);
                    header_label.add_css_class("agent-step-header");
                    header_box.append(&header_label);
                    action_box.append(&header_box);

                    // Parsear y formatear el action mejor
                    let formatted_action = Self::format_action_text(&action);

                    let text = gtk::Label::new(Some(&formatted_action));
                    text.set_xalign(0.0);
                    text.set_wrap(true);
                    text.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                    text.set_selectable(true);
                    text.add_css_class("agent-action-text");
                    text.set_margin_start(28); // Indent para alinear con el header
                    action_box.append(&text);

                    container.append(&action_box);
                    self.schedule_chat_scroll();
                }
            }

            AppMsg::ShowAgentObservation(observation) => {
                self.ensure_thinking_container();

                if let Some(container) = self.chat_thinking_container.borrow().as_ref() {
                    let i18n = self.i18n.borrow();

                    let obs_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
                    obs_box.set_margin_all(8);
                    obs_box.add_css_class("agent-observation");

                    // Header con icono
                    let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let icon_text = i18n.t("chat_agent_observation");
                    let icon =
                        gtk::Label::new(Some(&icon_text.chars().take(2).collect::<String>()));
                    icon.set_margin_end(4);
                    header_box.append(&icon);

                    let header_text = icon_text.chars().skip(3).collect::<String>();
                    let header_label = gtk::Label::new(Some(&header_text));
                    header_label.set_xalign(0.0);
                    header_label.add_css_class("agent-step-header");
                    header_box.append(&header_label);
                    obs_box.append(&header_box);

                    // Formatear la observación
                    let formatted_obs = Self::format_observation_text(&observation);

                    // Truncar observaciones muy largas
                    let display_text = if formatted_obs.len() > 500 {
                        format!("{}... (truncado)", &formatted_obs[..500])
                    } else {
                        formatted_obs
                    };

                    let text = gtk::Label::new(Some(&display_text));
                    text.set_xalign(0.0);
                    text.set_wrap(true);
                    text.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                    text.set_selectable(true);
                    text.add_css_class("agent-observation-text");
                    text.set_margin_start(28); // Indent para alinear con el header
                    obs_box.append(&text);

                    container.append(&obs_box);
                    self.schedule_chat_scroll();
                }
            }

            AppMsg::UpdateChatStatus(status_text) => {
                self.append_chat_typing_indicator(&status_text);
            }

            AppMsg::ShowAttachNoteDialog => {
                let i18n = self.i18n.borrow();

                // Crear diálogo con lista de notas
                let dialog = gtk::Dialog::builder()
                    .transient_for(&self.main_window)
                    .modal(true)
                    .title(&i18n.t("chat_attach_note_dialog_title"))
                    .width_request(400)
                    .height_request(500)
                    .build();

                dialog.add_button(&i18n.t("cancel"), gtk::ResponseType::Cancel);
                dialog.add_button(&i18n.t("chat_attach_button"), gtk::ResponseType::Accept);

                // Crear lista scrollable
                let scrolled = gtk::ScrolledWindow::builder()
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .vscrollbar_policy(gtk::PolicyType::Automatic)
                    .vexpand(true)
                    .build();

                let list_box = gtk::ListBox::new();
                list_box.set_selection_mode(gtk::SelectionMode::Single);
                list_box.add_css_class("boxed-list");

                // Agregar todas las notas
                if let Ok(notes) = self.notes_dir.list_notes() {
                    for note in notes {
                        let note_name = note.name().to_string();

                        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                        row.set_margin_all(8);

                        let icon = gtk::Label::new(Some("📄"));
                        row.append(&icon);

                        let label = gtk::Label::new(Some(&note_name));
                        label.set_xalign(0.0);
                        label.set_hexpand(true);
                        row.append(&label);

                        let list_row = gtk::ListBoxRow::new();
                        list_row.set_child(Some(&row));
                        list_row.set_property("tooltip-text", Some(&note_name));

                        list_box.append(&list_row);
                    }
                }

                scrolled.set_child(Some(&list_box));
                dialog.content_area().append(&scrolled);

                let sender_clone = sender.clone();
                dialog.connect_response(move |dialog, response| {
                    if response == gtk::ResponseType::Accept {
                        if let Some(row) = list_box.selected_row() {
                            // Extraer el nombre de la nota del Label (segundo hijo del Box)
                            if let Some(child) = row.child() {
                                if let Ok(box_widget) = child.downcast::<gtk::Box>() {
                                    // Saltar el icono (primer hijo) y obtener el label (segundo hijo)
                                    if let Some(icon) = box_widget.first_child() {
                                        if let Some(label_widget) = icon.next_sibling() {
                                            if let Ok(label) = label_widget.downcast::<gtk::Label>()
                                            {
                                                let note_name = label.text().to_string();
                                                println!(
                                                    "📎 Intentando adjuntar nota: {}",
                                                    note_name
                                                );
                                                sender_clone
                                                    .input(AppMsg::AttachNoteToContext(note_name));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    dialog.close();
                });

                dialog.present();
            }

            AppMsg::AttachNoteToContext(note_name) => {
                println!("📎 AttachNoteToContext recibido para: {}", note_name);

                if let Ok(Some(note)) = self.notes_dir.find_note(&note_name) {
                    {
                        if let Some(session) = self.chat_session.borrow_mut().as_mut() {
                            session.attach_note(note);
                            println!("✅ Nota '{}' adjuntada al contexto", note_name);
                        } else {
                            println!("⚠️ No hay sesión de chat activa");
                        }
                    } // ← Libera borrow_mut aquí
                    self.refresh_context_list();
                    sender.input(AppMsg::UpdateChatTokenCount);
                } else {
                    println!("❌ No se pudo encontrar la nota: {}", note_name);
                }
            }

            AppMsg::DetachNoteFromContext(note_name) => {
                {
                    if let Some(session) = self.chat_session.borrow_mut().as_mut() {
                        session.detach_note(&note_name);
                        println!("📎 Nota '{}' removida del contexto", note_name);
                    }
                } // ← Libera borrow_mut aquí
                self.refresh_context_list();
                sender.input(AppMsg::UpdateChatTokenCount);
            }

            AppMsg::ClearChatContext => {
                {
                    if let Some(session) = self.chat_session.borrow_mut().as_mut() {
                        session.clear_context();
                        println!("🧹 Contexto limpiado");
                    }
                } // ← Libera borrow_mut aquí
                self.refresh_context_list();
                sender.input(AppMsg::UpdateChatTokenCount);
            }

            AppMsg::ClearChatHistory => {
                let i18n = self.i18n.borrow();

                // Mostrar diálogo de confirmación
                let dialog = gtk::MessageDialog::builder()
                    .transient_for(&self.main_window)
                    .modal(true)
                    .message_type(gtk::MessageType::Warning)
                    .buttons(gtk::ButtonsType::YesNo)
                    .text(&i18n.t("chat_clear_history_confirm_title"))
                    .secondary_text(&i18n.t("chat_clear_history_confirm_message"))
                    .build();

                let sender_clone = sender.clone();
                dialog.connect_response(move |dialog, response| {
                    if response == gtk::ResponseType::Yes {
                        sender_clone.input(AppMsg::ConfirmClearChatHistory);
                    }
                    dialog.close();
                });

                dialog.present();
            }

            AppMsg::ConfirmClearChatHistory => {
                // Borrar de la base de datos
                if let Err(e) = self.notes_db.clear_all_chat_history() {
                    eprintln!("Error borrando historial: {}", e);
                } else {
                    println!("🗑️ Historial borrado completamente de la base de datos");
                }

                // Limpiar solo los mensajes de la sesión actual, pero mantener el contexto
                *self.chat_session_id.borrow_mut() = None;

                // Si hay sesión activa, limpiar solo el historial pero mantener el contexto
                if let Some(session) = self.chat_session.borrow_mut().as_mut() {
                    session.clear_history();
                    println!("🧹 Historial de mensajes limpiado, contexto mantenido");
                }

                // IMPORTANTE: Reiniciar el RouterAgent para limpiar su contexto interno
                // El RouterAgent mantiene su propio estado que debe resetearse
                println!("🔄 Reiniciando RouterAgent para limpiar contexto...");
                let api_key = self
                    .notes_config
                    .borrow()
                    .get_ai_config()
                    .api_key
                    .clone()
                    .unwrap_or_else(|| std::env::var("OPENAI_API_KEY").unwrap_or_default());

                if !api_key.is_empty() {
                    let (provider_str, model_str) = {
                        let config = self.notes_config.borrow();
                        let ai_config = config.get_ai_config();
                        (ai_config.provider.clone(), ai_config.model.clone())
                    };

                    let provider = match provider_str.to_lowercase().as_str() {
                        "anthropic" => crate::ai_chat::AIProvider::Anthropic,
                        "ollama" => crate::ai_chat::AIProvider::Ollama,
                        "custom" => crate::ai_chat::AIProvider::Custom,
                        _ => crate::ai_chat::AIProvider::OpenAI,
                    };

                    let router_config = crate::ai_chat::AIModelConfig {
                        provider,
                        model: model_str,
                        temperature: 0.3,
                        max_tokens: 4000,
                    };

                    match crate::ai_client::create_client(&router_config, &api_key) {
                        Ok(ai_client) => {
                            let router =
                                crate::ai::RouterAgent::new(std::sync::Arc::from(ai_client));
                            *self.router_agent.borrow_mut() = Some(router);
                            println!("✅ RouterAgent reiniciado sin contexto anterior");
                        }
                        Err(e) => {
                            eprintln!("⚠️ Error reiniciando RouterAgent: {}", e);
                        }
                    }
                } else {
                    // Si no hay sesión, crear una nueva con la configuración actual
                    let ai_config = self.notes_config.borrow().get_ai_config().clone();
                    let model_config = crate::ai_chat::AIModelConfig {
                        provider: match ai_config.provider.as_str() {
                            "anthropic" => crate::ai_chat::AIProvider::Anthropic,
                            "ollama" => crate::ai_chat::AIProvider::Ollama,
                            _ => crate::ai_chat::AIProvider::OpenAI,
                        },
                        model: ai_config.model.clone(),
                        max_tokens: ai_config.max_tokens as usize,
                        temperature: ai_config.temperature,
                    };
                    let new_session = crate::ai_chat::ChatSession::new(model_config);
                    *self.chat_session.borrow_mut() = Some(new_session);
                    println!("✨ Nueva sesión de chat creada");
                }

                // Limpiar UI del historial
                while let Some(child) = self.chat_history_list.first_child() {
                    self.chat_history_list.remove(&child);
                }

                // Mantener la lista de contexto
                self.refresh_context_list();
                sender.input(AppMsg::UpdateChatTokenCount);

                let i18n = self.i18n.borrow();

                // Mostrar confirmación
                let info_dialog = gtk::MessageDialog::builder()
                    .transient_for(&self.main_window)
                    .modal(true)
                    .message_type(gtk::MessageType::Info)
                    .buttons(gtk::ButtonsType::Ok)
                    .text(&i18n.t("chat_history_cleared"))
                    .secondary_text(&i18n.t("chat_history_cleared_message"))
                    .build();

                info_dialog.connect_response(|dialog, _| {
                    dialog.close();
                });

                info_dialog.present();
            }

            AppMsg::UpdateChatTokenCount => {
                if let Some(session) = self.chat_session.borrow().as_ref() {
                    let current = session.total_context_tokens();
                    let max = session.model_config.max_tokens;
                    let percentage = (current as f64 / max as f64).min(1.0);

                    self.chat_tokens_progress.set_fraction(percentage);
                    self.chat_tokens_progress
                        .set_text(Some(&format!("Tokens: {} / {}", current, max)));

                    // Cambiar color según uso
                    if percentage > 0.9 {
                        self.chat_tokens_progress.add_css_class("chat-token-danger");
                        self.chat_tokens_progress
                            .remove_css_class("chat-token-warning");
                    } else if percentage > 0.7 {
                        self.chat_tokens_progress
                            .add_css_class("chat-token-warning");
                        self.chat_tokens_progress
                            .remove_css_class("chat-token-danger");
                    } else {
                        self.chat_tokens_progress
                            .remove_css_class("chat-token-danger");
                        self.chat_tokens_progress
                            .remove_css_class("chat-token-warning");
                    }
                }
            }

            AppMsg::CopyText(text) => {
                if let Some(display) = gtk::gdk::Display::default() {
                    display.clipboard().set_text(&text);
                    println!("Texto copiado al portapapeles");
                }
            }

            AppMsg::CreateNoteFromContent(content) => {
                // Generar un nombre único basado en timestamp
                let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
                let name = format!("AI_Note_{}", timestamp);

                // Crear la nota
                match self.notes_dir.create_note(&name, &content) {
                    Ok(note) => {
                        // Indexar en DB
                        let folder_for_db = self.notes_dir.relative_folder(note.path());
                        let path_str = note.path().to_string_lossy().to_string();
                        let _ = self.notes_db.index_note(
                            &name,
                            &path_str,
                            &content,
                            folder_for_db.as_deref(),
                        );

                        // Cargar la nota
                        sender.input(AppMsg::LoadNote {
                            name: name.clone(),
                            highlight_text: None,
                        });

                        // Cambiar a modo normal si estamos en chat
                        if *self.mode.borrow() == EditorMode::ChatAI {
                            sender.input(AppMsg::ToggleChatMode);
                        }

                        println!("Nota creada: {}", name);
                    }
                    Err(e) => {
                        eprintln!("Error creando nota desde chat: {}", e);
                    }
                }
            }

            // ==================== RECORDATORIOS ====================
            AppMsg::ToggleRemindersPopover => {
                // El toggle se maneja automáticamente por el botón con popover
            }

            AppMsg::RefreshReminders => {
                // Limpiar lista actual
                while let Some(child) = self.reminders_list.first_child() {
                    self.reminders_list.remove(&child);
                }

                // Obtener recordatorios de la base de datos
                if let Ok(db) = self.reminder_db.lock() {
                    match db.list_reminders(None) {
                        Ok(reminders) => {
                            let i18n = self.i18n.borrow();

                            if reminders.is_empty() {
                                let empty_label = gtk::Label::new(Some(&i18n.t("reminders_empty")));
                                empty_label.add_css_class("dim-label");
                                empty_label.set_margin_all(24);
                                self.reminders_list.append(&empty_label);
                            } else {
                                for reminder in reminders {
                                    let row = self.create_reminder_row(&reminder, sender.clone());
                                    self.reminders_list.append(&row);
                                }
                            }

                            // Actualizar badge
                            if let Ok(count) = db.count_pending() {
                                self.update_reminder_badge(count);
                            }
                        }
                        Err(e) => {
                            eprintln!("Error cargando recordatorios: {}", e);
                        }
                    }
                }
            }

            AppMsg::ShowCreateReminderDialog => {
                // Aquí irá el diálogo de creación de recordatorios
                println!("TODO: Implementar diálogo de creación");
            }

            AppMsg::CreateReminder {
                title,
                description,
                due_date,
                priority,
                repeat_pattern,
            } => {
                if let Ok(db) = self.reminder_db.lock() {
                    match db.create_reminder(
                        None, // note_id: vincular con nota actual si se desea
                        &title,
                        description.as_deref(),
                        due_date,
                        priority,
                        repeat_pattern,
                    ) {
                        Ok(_) => {
                            println!("✅ Recordatorio creado: {}", title);
                            sender.input(AppMsg::RefreshReminders);
                        }
                        Err(e) => {
                            eprintln!("❌ Error creando recordatorio: {}", e);
                        }
                    }
                }
            }

            AppMsg::CompleteReminder(id) => {
                if let Ok(db) = self.reminder_db.lock() {
                    use crate::reminders::ReminderStatus;
                    match db.update_status(id, ReminderStatus::Completed) {
                        Ok(_) => {
                            println!("✅ Recordatorio {} completado", id);
                            sender.input(AppMsg::RefreshReminders);
                        }
                        Err(e) => {
                            eprintln!("❌ Error completando recordatorio: {}", e);
                        }
                    }
                }
            }

            AppMsg::SnoozeReminder { id, minutes } => {
                if let Ok(db) = self.reminder_db.lock() {
                    use chrono::Duration;
                    let duration = Duration::minutes(minutes as i64);
                    let snooze_until = chrono::Utc::now() + duration;
                    match db.snooze_reminder(id, snooze_until) {
                        Ok(_) => {
                            println!("⏰ Recordatorio {} pospuesto {} minutos", id, minutes);
                            sender.input(AppMsg::RefreshReminders);
                        }
                        Err(e) => {
                            eprintln!("❌ Error posponiendo recordatorio: {}", e);
                        }
                    }
                }
            }

            AppMsg::DeleteReminder(id) => {
                if let Ok(db) = self.reminder_db.lock() {
                    match db.delete_reminder(id) {
                        Ok(_) => {
                            println!("🗑️ Recordatorio {} eliminado", id);
                            sender.input(AppMsg::RefreshReminders);
                        }
                        Err(e) => {
                            eprintln!("❌ Error eliminando recordatorio: {}", e);
                        }
                    }
                }
            }

            AppMsg::PerformSemanticSearchWithAI { query, results } => {
                // Mostrar indicador de carga
                *self.semantic_search_answer_visible.borrow_mut() = true;
                self.semantic_search_answer_box.set_visible(true);
                self.semantic_search_answer_row.set_visible(true);
                self.semantic_search_answer_label
                    .set_markup("<i>🔄 Analizando resultados con IA...</i>");

                // Obtener el router agent y extraer el cliente de IA
                if let Some(router) = self.router_agent.borrow().as_ref() {
                    let ai_client = router.get_llm();
                    let mcp_executor = self.mcp_executor.clone();
                    let sender_clone = sender.clone();
                    let query_clone = query.clone();

                    println!(
                        "🤖 Iniciando búsqueda semántica con agente RIG para: '{}'",
                        query
                    );

                    // Ejecutar en un thread para no bloquear la UI
                    gtk::glib::spawn_future_local(async move {
                        // Crear un prompt simple que hará que el agente use semantic_search
                        let prompt = format!(
                            "Actúa como un asistente que ya conoce el contenido de las notas del usuario. Tras analizar el tema '{}' debes responder:
1. Comienza tu respuesta exactamente con la frase 'Después de revisar tus notas,'.
2. Resume los hallazgos relevantes en un tono cercano y conversacional.
3. Siempre indica en qué notas encontraste la información usando enlaces con el formato [[Nombre de la Nota]] para que sean clicables.
4. Si hay varias notas, preséntalas como una pequeña lista o párrafos separados, pero mantén la narrativa.
5. Evita repetir resultados crudos de la búsqueda; céntrate en explicar qué información útil hay en cada nota.

Para ello debes usar la herramienta semantic_search para encontrar las notas adecuadas y leer las más relevantes antes de responder.",
                            query_clone
                        );

                        let messages = vec![crate::ai_chat::ChatMessage::new(
                            crate::ai_chat::MessageRole::User,
                            prompt,
                            vec![],
                        )];

                        println!("📝 Llamando a RigExecutor con el prompt");

                        let mcp_instance = mcp_executor.borrow().clone();
                        match crate::ai::executors::rig_executor::RigExecutor::run(
                            ai_client,
                            &messages,
                            "",
                            &mcp_instance,
                        )
                        .await
                        {
                            Ok(response) => {
                                println!(
                                    "✅ Respuesta recibida del agente: {} caracteres",
                                    response.len()
                                );
                                println!("🚀 Enviando mensaje ShowSemanticSearchAnswer...");
                                sender_clone.input(AppMsg::ShowSemanticSearchAnswer(response));
                                println!("✅ Mensaje enviado");
                            }
                            Err(e) => {
                                eprintln!("❌ Error en agente RIG: {}", e);
                                sender_clone.input(AppMsg::ShowSemanticSearchAnswer(format!(
                                    "❌ Error al analizar resultados: {}",
                                    e
                                )));
                            }
                        }
                    });
                } else {
                    // Si no hay agente disponible, mostrar lista simple
                    let mut response = format!("Encontré {} notas relevantes:\n\n", results.len());
                    for (idx, result) in results.iter().take(5).enumerate() {
                        response.push_str(&format!(
                            "{}. [[{}]] (Relevancia: {:.2})\n",
                            idx + 1,
                            result.note_name,
                            result.similarity.unwrap_or(result.relevance)
                        ));
                    }
                    sender.input(AppMsg::ShowSemanticSearchAnswer(response));
                }
            }

            AppMsg::ShowSemanticSearchAnswer(answer) => {
                println!(
                    "📦 ShowSemanticSearchAnswer recibido: {} caracteres",
                    answer.len()
                );

                // Reinsertar (o mover) el row al inicio para garantizar que esté presente
                if self.semantic_search_answer_row.parent().is_some() {
                    self.floating_search_results_list
                        .remove(&self.semantic_search_answer_row);
                }
                self.floating_search_results_list
                    .prepend(&self.semantic_search_answer_row);
                println!("📦 answer_row repositionado al inicio del list");

                // Limpiar la lista de búsqueda (incluyendo mensaje de carga)
                // pero mantener el answer_box
                let answer_box_ptr = self.semantic_search_answer_box.as_ptr();
                let answer_row_ptr =
                    self.semantic_search_answer_row.as_ptr() as *mut gtk::ffi::GtkWidget;
                println!("📦 answer_box ptr: {:?}", answer_box_ptr);
                println!("📦 answer_row ptr: {:?}", answer_row_ptr);
                println!(
                    "📦 answer_row parent presente: {}",
                    self.semantic_search_answer_row.parent().is_some()
                );

                let mut child = self.floating_search_results_list.first_child();
                let mut removed_count = 0;
                while let Some(widget) = child {
                    let next = widget.next_sibling();
                    let widget_ptr = widget.as_ptr();
                    println!("📦 Evaluando widget ptr: {:?}", widget_ptr);

                    // No eliminar el row que contiene el answer_box
                    if widget_ptr != answer_row_ptr {
                        println!("📦 Eliminando widget (no es answer_row)");
                        self.floating_search_results_list.remove(&widget);
                        removed_count += 1;
                    } else {
                        println!("📦 Preservando answer_row/box");
                    }
                    child = next;
                }

                println!("📦 Widgets eliminados: {}", removed_count);

                // Convertir [[Nombre]] a enlaces clickeables
                let markup = self.convert_note_links_to_markup(&answer);
                println!("📦 Markup generado: {} caracteres", markup.len());

                self.semantic_search_answer_label.set_markup(&markup);

                // Debug: verificar estado del contenedor padre
                println!(
                    "📦 floating_search_results visible: {}, allocated: {}x{}",
                    self.floating_search_results.is_visible(),
                    self.floating_search_results.allocated_width(),
                    self.floating_search_results.allocated_height()
                );
                println!(
                    "📦 floating_search_results_list visible: {}, allocated: {}x{}",
                    self.floating_search_results_list.is_visible(),
                    self.floating_search_results_list.allocated_width(),
                    self.floating_search_results_list.allocated_height()
                );

                // Debug: verificar tamaños asignados antes de mostrar
                println!(
                    "📦 answer_box allocated width: {}, height: {}",
                    self.semantic_search_answer_box.allocated_width(),
                    self.semantic_search_answer_box.allocated_height()
                );
                println!(
                    "📦 answer_row allocated width: {}, height: {}",
                    self.semantic_search_answer_row.allocated_width(),
                    self.semantic_search_answer_row.allocated_height()
                );
                println!(
                    "📦 answer_label allocated width: {}, height: {}",
                    self.semantic_search_answer_label.allocated_width(),
                    self.semantic_search_answer_label.allocated_height()
                );

                *self.semantic_search_answer_visible.borrow_mut() = true;

                // CRÍTICO: Primero mostrar el row, luego el box
                self.semantic_search_answer_row.set_visible(true);
                self.semantic_search_answer_box.set_visible(true);

                // Forzar que el row y box tengan altura mínima
                self.semantic_search_answer_row.set_height_request(120);
                self.semantic_search_answer_box.set_height_request(100);

                println!("📦 DESPUÉS DE set_height_request:");
                println!(
                    "📦 answer_row allocated width: {}, height: {}",
                    self.semantic_search_answer_row.allocated_width(),
                    self.semantic_search_answer_row.allocated_height()
                );

                // Forzar actualización TOTAL del layout desde el padre
                self.floating_search_results.queue_allocate();
                self.floating_search_results_list.queue_allocate();
                self.semantic_search_answer_row.queue_allocate();
                self.semantic_search_answer_box.queue_allocate();

                // Verificar tamaños DESPUÉS del próximo ciclo de eventos
                let row_clone = self.semantic_search_answer_row.clone();
                let box_clone = self.semantic_search_answer_box.clone();
                gtk::glib::idle_add_local_once(move || {
                    println!("📦 [IDLE] DESPUÉS DEL LAYOUT:");
                    println!(
                        "📦 [IDLE] answer_row allocated: {}x{}",
                        row_clone.allocated_width(),
                        row_clone.allocated_height()
                    );
                    println!(
                        "📦 [IDLE] answer_box allocated: {}x{}",
                        box_clone.allocated_width(),
                        box_clone.allocated_height()
                    );
                });

                println!(
                    "📦 answer_box visible: {}",
                    self.semantic_search_answer_box.is_visible()
                );
                println!(
                    "📦 answer_row visible: {}",
                    self.semantic_search_answer_row.is_visible()
                );
                println!(
                    "📦 answer_label text length: {}",
                    self.semantic_search_answer_label.text().len()
                );
                println!(
                    "📦 answer_box opacity: {}",
                    self.semantic_search_answer_box.opacity()
                );
                println!(
                    "📦 answer_row opacity: {}",
                    self.semantic_search_answer_row.opacity()
                );

                // Debugging visual
                if let Some(parent) = self.semantic_search_answer_box.parent() {
                    println!("📦 answer_box tiene padre: {:?}", parent.type_());
                }

                // Intentar mostrar el floating search si no está visible
                if !self.floating_search_results.is_visible() {
                    println!("⚠️ floating_search_results no está visible, haciéndolo visible");
                    self.floating_search_results.set_visible(true);
                }
            }

            AppMsg::ReloadCurrentNoteIfMatching { path } => {
                // Si hay cambios sin guardar en el buffer, NO recargar desde disco
                // Esto protege contra condiciones de carrera donde el usuario sigue escribiendo
                // mientras se procesa un autoguardado anterior.
                if self.has_unsaved_changes {
                    return;
                }

                if let Some(current) = &self.current_note {
                    if current.path().to_str().unwrap_or("") == path {
                        // Verificar si el contenido realmente cambió en disco
                        // Esto evita recargas innecesarias que resetean el cursor (ej: autoguardado)
                        if let Ok(disk_content) = std::fs::read_to_string(path) {
                            let current_content = self.buffer.to_string();
                            if disk_content == current_content {
                                // Contenido idéntico, ignorar recarga
                                return;
                            }
                        }

                        // Recargar nota SIN guardar la actual (evitar sobrescribir cambios externos)
                        // y SIN cambiar de modo/vista si estamos en Chat
                        let name = current.name().to_string();
                        let old_cursor = self.cursor_position;

                        if let Err(e) = self.load_note(&name) {
                            eprintln!("Error recargando nota '{}': {}", name, e);
                        } else {
                            // Restaurar cursor (limitado al nuevo tamaño)
                            self.cursor_position = old_cursor.min(self.buffer.len_chars());

                            // Invalidar cache
                            *self.cached_source_text.borrow_mut() = None;
                            *self.cached_rendered_text.borrow_mut() = None;

                            // Sincronizar vista y actualizar UI
                            self.sync_to_view();
                            self.update_status_bar(&sender);
                            self.refresh_tags_display_with_sender(&sender);
                            self.refresh_todos_summary();
                            // No cambiamos window_title porque es la misma nota
                            self.has_unsaved_changes = false;

                            // NOTA: No cambiamos el modo ni la vista (content_stack)
                            // para no interrumpir al usuario si está en el chat
                        }

                        // Forzar parseo de recordatorios
                        sender.input(AppMsg::ParseRemindersInNote);
                    }
                }
            }

            AppMsg::ParseRemindersInNote => {
                // Obtener el contenido de la nota actual
                if let Some(note) = &self.current_note {
                    // Obtener note_id de la base de datos
                    let note_name = note.name();
                    let note_path = note.path().to_str().unwrap_or("");

                    // Intentar obtener por nombre primero, luego por path
                    let note_id = self
                        .notes_db
                        .get_note(note_name)
                        .ok()
                        .flatten()
                        .map(|metadata| metadata.id)
                        .or_else(|| {
                            // Fallback: intentar buscar por path
                            self.notes_db
                                .get_note_by_path(note_path)
                                .ok()
                                .flatten()
                                .map(|metadata| metadata.id)
                        });

                    if note_id.is_none() {
                        println!(
                            "⚠️ WARNING: No se pudo encontrar ID para la nota '{}' (path: '{}')",
                            note_name, note_path
                        );
                        // Intentar re-indexar la nota si no existe
                        if let Ok(content) = note.read() {
                            let folder = self.notes_dir.relative_folder(note.path());
                            println!("🔄 Intentando re-indexar nota perdida...");
                            if let Ok(new_id) = self.notes_db.index_note(
                                note_name,
                                note_path,
                                &content,
                                folder.as_deref(),
                            ) {
                                println!("✅ Nota re-indexada con ID: {}", new_id);
                                // Forzar una recarga de la UI para asegurar consistencia
                                // sender.input(AppMsg::RefreshSidebar);
                            }
                        }
                    }

                    // Acceder al contenido correcto según el modo
                    let content = if *self.mode.borrow() == EditorMode::Insert {
                        // En modo Insert, el buffer de GTK tiene el texto real
                        let start_iter = self.text_buffer.start_iter();
                        let end_iter = self.text_buffer.end_iter();
                        self.text_buffer
                            .text(&start_iter, &end_iter, false)
                            .to_string()
                    } else {
                        // En modo Normal, el buffer de GTK tiene widgets, usar el buffer interno
                        self.buffer.to_string()
                    };

                    let language = self.i18n.borrow().current_language();

                    // Parsear recordatorios del texto
                    let parsed_reminders =
                        self.reminder_parser.extract_reminders(&content, language);

                    if parsed_reminders.is_empty() {
                        // No hay recordatorios en el texto, eliminar los existentes de esta nota
                        if let (Some(nid), Ok(db)) = (note_id, self.reminder_db.lock()) {
                            if let Ok(existing) = db.list_reminders_by_note(nid) {
                                for reminder in existing {
                                    let _ = db.delete_reminder(reminder.id);
                                }
                            }
                        }
                    } else {
                        // Hay recordatorios en el texto
                        if let Ok(db) = self.reminder_db.lock() {
                            // Obtener recordatorios existentes de esta nota
                            let existing_reminders = if let Some(nid) = note_id {
                                let reminders = db.list_reminders_by_note(nid).unwrap_or_default();
                                println!(
                                    "🔍 DEBUG: Note ID: {}, Existing reminders count: {}",
                                    nid,
                                    reminders.len()
                                );
                                reminders
                            } else {
                                println!("🔍 DEBUG: Note ID is None!");
                                Vec::new()
                            };

                            let mut created_count = 0;
                            let mut updated_count = 0;

                            for parsed in &parsed_reminders {
                                println!(
                                    "🔍 DEBUG: Parsed reminder: Title='{}', Date={}",
                                    parsed.title, parsed.due_date
                                );

                                // Buscar si ya existe un recordatorio similar (mismo título y fecha)
                                let exists = existing_reminders.iter().any(|existing| {
                                    let title_match = existing.title == parsed.title;
                                    let date_match = (existing.due_date.timestamp() - parsed.due_date.timestamp()).abs() < 60; // Margen de 1 minuto

                                    println!("    Compare with DB: Title='{}', Date={} -> TitleMatch: {}, DateMatch: {} (Diff: {}s)",
                                        existing.title, existing.due_date, title_match, date_match,
                                        existing.due_date.timestamp() - parsed.due_date.timestamp());

                                    title_match && date_match
                                });

                                if !exists {
                                    // Crear nuevo recordatorio
                                    match db.create_reminder(
                                        note_id,
                                        &parsed.title,
                                        None,
                                        parsed.due_date,
                                        parsed.priority,
                                        parsed.repeat_pattern,
                                    ) {
                                        Ok(_) => created_count += 1,
                                        Err(e) => eprintln!("❌ Error creando recordatorio: {}", e),
                                    }
                                } else {
                                    updated_count += 1;
                                }
                            }

                            // Eliminar recordatorios que ya no están en el texto
                            for existing in existing_reminders {
                                let still_exists = parsed_reminders.iter().any(|parsed| {
                                    existing.title == parsed.title
                                        && existing.due_date.timestamp()
                                            == parsed.due_date.timestamp()
                                });

                                if !still_exists {
                                    let _ = db.delete_reminder(existing.id);
                                }
                            }

                            if created_count > 0 {
                                println!("✅ {} recordatorios nuevos creados", created_count);
                            }
                            if updated_count > 0 {
                                println!("ℹ️ {} recordatorios ya existían", updated_count);
                            }

                            // Actualizar UI
                            sender.input(AppMsg::RefreshReminders);
                        }

                        sender.input(AppMsg::RefreshReminders);
                    }
                }
            }

            AppMsg::EditReminder(_id) => {
                // TODO: Implementar diálogo de edición
                println!("TODO: Implementar diálogo de edición de recordatorio");
            }

            AppMsg::UpdateReminder {
                id,
                title,
                description,
                due_date,
                priority,
                repeat_pattern,
            } => {
                if let Ok(db) = self.reminder_db.lock() {
                    // Por ahora solo actualizar campos si están presentes
                    // TODO: Implementar update completo en database.rs
                    println!("TODO: Implementar actualización de recordatorio {}", id);
                    let _ = (title, description, due_date, priority, repeat_pattern); // Evitar warnings
                }
            }

            AppMsg::ShowNotification(text) => {
                // Mostrar notificación toast
                self.notification_label.set_text(&text);
                self.notification_revealer.set_reveal_child(true);

                // Auto-ocultar después de 3 segundos
                let revealer = self.notification_revealer.clone();
                gtk::glib::timeout_add_seconds_local_once(3, move || {
                    revealer.set_reveal_child(false);
                });
            }

            AppMsg::ShowIconPicker { name, is_folder } => {
                // Mostrar popover con selector de iconos
                self.show_icon_picker_popover(&name, is_folder, &sender);
            }

            AppMsg::SetNoteIcon {
                note_name,
                icon,
                color,
            } => {
                // Establecer icono de nota en la BD
                if let Err(e) = self.notes_db.set_note_icon(&note_name, icon.as_deref()) {
                    eprintln!("Error estableciendo icono de nota: {}", e);
                } else {
                    // Si hay color, establecerlo también
                    if let Some(ref c) = color {
                        let _ = self
                            .notes_db
                            .set_note_icon_color(&note_name, Some(c.as_str()));
                    } else if icon.is_none() {
                        // Si se quita el icono, también quitar el color
                        let _ = self.notes_db.set_note_icon_color(&note_name, None);
                    }
                    println!(
                        "✅ Icono de nota '{}' actualizado a: {:?} (color: {:?})",
                        note_name, icon, color
                    );
                    sender.input(AppMsg::RefreshSidebar);
                }
            }

            AppMsg::SetFolderIcon {
                folder_path,
                icon,
                color,
            } => {
                // Establecer icono de carpeta en la BD
                if let Err(e) = self.notes_db.set_folder_icon(&folder_path, icon.as_deref()) {
                    eprintln!("Error estableciendo icono de carpeta: {}", e);
                } else {
                    // Si hay color, establecerlo también
                    if let Some(ref c) = color {
                        let _ = self
                            .notes_db
                            .set_folder_icon_color(&folder_path, Some(c.as_str()));
                    } else if icon.is_none() {
                        // Si se quita el icono, también quitar el color
                        let _ = self.notes_db.set_folder_icon_color(&folder_path, None);
                    }
                    println!(
                        "✅ Icono de carpeta '{}' actualizado a: {:?} (color: {:?})",
                        folder_path, icon, color
                    );
                    sender.input(AppMsg::RefreshSidebar);
                }
            }
        }
    }
}

impl MainApp {
    /// Renderiza el contenido del chat soportando tablas y markdown
    fn render_chat_content(
        &self,
        content: &str,
        container: &gtk::Box,
        sender: Option<ComponentSender<Self>>,
    ) {
        let mut current_text = String::new();
        let mut in_table = false;
        let mut table_lines = Vec::new();
        let mut in_code_block = false;

        for line in content.lines() {
            // Detectar bloques de código para no romper tablas dentro de ellos (aunque es raro)
            if line.trim_start().starts_with("```") {
                in_code_block = !in_code_block;
            }

            if !in_code_block && line.trim().starts_with('|') && line.trim().ends_with('|') {
                if !in_table {
                    // Inicio de posible tabla
                    // Flushear texto anterior
                    if !current_text.trim().is_empty() {
                        let label = self.create_markdown_label(&current_text, sender.clone());
                        container.append(&label);
                        current_text.clear();
                    }
                    in_table = true;
                }
                table_lines.push(line);
            } else {
                if in_table {
                    // Fin de tabla
                    self.render_table(&table_lines, container);
                    table_lines.clear();
                    in_table = false;
                }
                current_text.push_str(line);
                current_text.push('\n');
            }
        }

        // Flushear lo que quede
        if in_table {
            self.render_table(&table_lines, container);
        } else if !current_text.trim().is_empty() {
            let label = self.create_markdown_label(&current_text, sender.clone());
            container.append(&label);
        }
    }

    fn render_table(&self, table_lines: &[&str], container: &gtk::Box) {
        if table_lines.len() < 2 {
            return;
        } // Necesitamos al menos header y separador

        let grid = gtk::Grid::new();
        grid.add_css_class("markdown-table");
        grid.set_column_spacing(0);
        grid.set_row_spacing(0);
        grid.set_vexpand(false); // Importante: no expandir verticalmente
        grid.set_hexpand(true); // NUEVO: Expandir horizontalmente
        grid.set_valign(gtk::Align::Start); // Alinear arriba
        grid.set_halign(gtk::Align::Fill); // NUEVO: Llenar todo el ancho disponible
        grid.set_column_homogeneous(true); // NUEVO: Columnas de igual ancho

        // Parsear filas
        let mut row_idx = 0;
        for line in table_lines {
            // Saltar línea separadora (contiene --- y |)
            if line.contains("---") && line.contains('|') {
                continue;
            }

            // Split por | y trim
            // El formato es | cell | cell |, así que el primer y último elemento suelen ser vacíos
            let cells: Vec<&str> = line.trim().split('|').collect();

            // Filtrar celdas vacías del inicio/fin si existen
            let valid_cells: Vec<&str> = cells
                .iter()
                .enumerate()
                .filter(|(i, s)| {
                    !(*i == 0 && s.trim().is_empty()
                        || *i == cells.len() - 1 && s.trim().is_empty())
                })
                .map(|(_, s)| s.trim())
                .collect();

            for (col_idx, cell_text) in valid_cells.iter().enumerate() {
                let label = gtk::Label::new(None);
                // Procesar markdown dentro de la celda también
                let markup = Self::markdown_to_pango(cell_text);
                label.set_markup(&markup);

                label.set_wrap(true);
                label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                label.set_xalign(0.0);
                label.set_hexpand(true);
                label.set_margin_start(4);
                label.set_margin_end(4);
                label.set_valign(gtk::Align::Start); // Alinear texto arriba en celdas multilínea

                let cell_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                cell_box.add_css_class("table-cell");
                cell_box.set_valign(gtk::Align::Fill); // Estirar caja para llenar celda
                cell_box.set_hexpand(true); // NUEVO: Expandir horizontalmente
                cell_box.set_halign(gtk::Align::Fill); // NUEVO: Llenar todo el ancho

                if row_idx == 0 {
                    cell_box.add_css_class("table-header");
                    label.add_css_class("table-header-label");
                }
                cell_box.append(&label);

                grid.attach(&cell_box, col_idx as i32, row_idx, 1, 1);
            }
            row_idx += 1;
        }

        // Envolver en ScrolledWindow por si es muy ancha
        // Usamos PolicyType::Automatic en ambos ejes para que:
        // 1. Si es pequeña, propagate_natural_height la ajusta al tamaño exacto.
        // 2. Si es grande (> 400px), aparece scroll y se limita la altura.
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic); // CAMBIADO: Never en horizontal
        scrolled.set_child(Some(&grid));
        scrolled.add_css_class("table-container");

        scrolled.set_propagate_natural_height(true);
        scrolled.set_propagate_natural_width(false); // CAMBIADO: No propagar ancho natural

        // Limitar altura máxima para tablas gigantes
        scrolled.set_max_content_height(400);

        scrolled.set_vexpand(false);
        scrolled.set_hexpand(true); // NUEVO: Expandir horizontalmente
        scrolled.set_valign(gtk::Align::Start);
        scrolled.set_halign(gtk::Align::Fill); // NUEVO: Llenar todo el ancho
        scrolled.set_has_frame(false);

        container.append(&scrolled);
    }

    fn create_markdown_label(
        &self,
        content: &str,
        sender: Option<ComponentSender<Self>>,
    ) -> gtk::Label {
        let label = gtk::Label::new(None);
        let markup = Self::markdown_to_pango(content);
        label.set_markup(&markup);
        label.set_wrap(true);
        label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        label.set_selectable(true);
        label.set_xalign(0.0);
        label.add_css_class("chat-message");
        label.add_css_class("chat-message-assistant");

        // Conectar señal de link click
        if let Some(sender) = sender {
            label.connect_activate_link(move |_, uri| {
                println!("🔗 Link clickeado en chat: {}", uri);
                if uri.starts_with("http") {
                    let _ = open::that(uri);
                } else {
                    sender.input(AppMsg::LoadNote {
                        name: uri.to_string(),
                        highlight_text: None,
                    });
                }
                glib::Propagation::Stop
            });
        }

        label
    }
    /// Gestiona la entrada de autostart en Linux
    fn manage_autostart(enable: bool) -> std::io::Result<()> {
        let home_dir = std::env::var("HOME").map_err(std::io::Error::other)?;
        let autostart_dir = std::path::PathBuf::from(format!("{}/.config/autostart", home_dir));
        let desktop_file_path = autostart_dir.join("notnative.desktop");

        if enable {
            // Asegurar que el directorio existe
            if !autostart_dir.exists() {
                std::fs::create_dir_all(&autostart_dir)?;
            }

            // Obtener ruta del ejecutable actual
            let current_exe = std::env::current_exe()?;
            let exe_path = current_exe.to_string_lossy();

            // Contenido del archivo .desktop
            let content = format!(
                "[Desktop Entry]\n\
                Name=NotNative\n\
                Comment=Note-taking application with Vim-like keybindings\n\
                Exec={}\n\
                Icon=notnative\n\
                Terminal=false\n\
                Type=Application\n\
                Categories=Office;TextEditor;Utility;\n\
                Keywords=notes;markdown;vim;editor;\n\
                StartupNotify=true\n\
                X-GNOME-Autostart-enabled=true\n",
                exe_path
            );

            std::fs::write(&desktop_file_path, content)?;
            println!("✅ Autostart habilitado: {:?}", desktop_file_path);
        } else if desktop_file_path.exists() {
            std::fs::remove_file(&desktop_file_path)?;
            println!("✅ Autostart deshabilitado: {:?}", desktop_file_path);
        }

        Ok(())
    }

    /// Búsqueda difusa simple: verifica si los caracteres del patrón aparecen en orden
    fn fuzzy_match(text: &str, pattern: &str) -> bool {
        let mut pattern_chars = pattern.chars();
        let mut current_pattern_char = match pattern_chars.next() {
            Some(ch) => ch,
            None => return true, // patrón vacío coincide con todo
        };

        for text_char in text.chars() {
            if text_char == current_pattern_char {
                current_pattern_char = match pattern_chars.next() {
                    Some(ch) => ch,
                    None => return true, // todos los caracteres del patrón encontrados
                };
            }
        }

        false // no se encontraron todos los caracteres del patrón
    }

    /// Extrae menciones de notas del formato @notaX del mensaje
    fn extract_note_mentions(&self, message: &str) -> Vec<String> {
        let mut mentions = Vec::new();
        let mut current_idx = 0;

        while let Some(off) = message[current_idx..].find('@') {
            let start_idx = current_idx + off + 1; // Skip '@'
            let remainder = &message[start_idx..];

            // 1. Extraer candidato "greedy" basado en terminadores
            let mut end_idx = 0;
            let mut chars = remainder.char_indices().peekable();
            let mut last_char_was_space = false;

            while let Some((idx, ch)) = chars.next() {
                // Terminadores duros
                if ch == '\n'
                    || ch == ','
                    || ch == '.'
                    || ch == ';'
                    || ch == ':'
                    || ch == '?'
                    || ch == '!'
                {
                    break;
                }

                // Detectar doble espacio como terminador
                if ch == ' ' {
                    if last_char_was_space {
                        // El espacio anterior ya se contó, pero este segundo termina la mención
                        // Retrocedemos end_idx para excluir el primer espacio también (trim lo hará igual)
                        break;
                    }
                    last_char_was_space = true;
                } else {
                    last_char_was_space = false;
                }

                end_idx = idx + ch.len_utf8();
            }

            let greedy_candidate = remainder[..end_idx].trim();

            if greedy_candidate.is_empty() {
                current_idx = start_idx;
                continue;
            }

            // 2. Reducir desde la derecha para encontrar la nota válida más larga
            // Esto soluciona el problema de capturar texto posterior como parte del nombre
            let mut found_note = None;
            let mut check = greedy_candidate;

            loop {
                // Verificar si 'check' es una nota válida
                if let Ok(Some(_)) = self.notes_dir.find_note(check) {
                    found_note = Some(check.to_string());
                    break;
                }

                // Reducir palabra por palabra
                if let Some(last_space) = check.rfind(' ') {
                    check = &check[..last_space];
                } else {
                    break;
                }
            }

            if let Some(note) = found_note {
                mentions.push(note.clone());
                // Avanzar índice pasado la nota encontrada
                current_idx = start_idx + note.len();
            } else {
                // Fallback: usar el candidato greedy si no se encuentra ninguna nota válida
                // (comportamiento original para notas que no existen)
                mentions.push(greedy_candidate.to_string());
                current_idx = start_idx + greedy_candidate.len();
            }
        }

        mentions
    }

    /// Obtiene el ID de una nota específica
    fn get_note_id(&self, note: &crate::core::NoteFile) -> Option<i64> {
        if let Ok(Some(metadata)) = self.notes_db.get_note(note.name()) {
            return Some(metadata.id);
        }
        None
    }

    fn get_current_note_id(&self) -> Option<i64> {
        if let Some(note) = &self.current_note {
            if let Ok(Some(metadata)) = self.notes_db.get_note(note.name()) {
                return Some(metadata.id);
            }
        }
        None
    }
    fn setup_theme_watcher(sender: ComponentSender<Self>) {
        use notify::{Event, RecursiveMode, Watcher};
        use std::sync::mpsc::channel;
        use std::time::Duration;

        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
        let theme_symlink = format!("{}/.config/omarchy/current", home_dir);

        std::thread::spawn(move || {
            let (tx, rx) = channel();
            let mut watcher =
                match notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
                    if let Ok(_event) = res {
                        let _ = tx.send(());
                    }
                }) {
                    Ok(w) => w,
                    Err(_) => return,
                };

            if watcher
                .watch(
                    std::path::Path::new(&theme_symlink),
                    RecursiveMode::Recursive,
                )
                .is_err()
            {
                return;
            }

            loop {
                if rx.recv_timeout(Duration::from_secs(1)).is_ok() {
                    std::thread::sleep(Duration::from_millis(500)); // Debounce

                    // Recargar CSS
                    let (combined_css, _) = Self::load_theme_css();

                    gtk::glib::MainContext::default().invoke(move || {
                        if let Some(display) = gtk::gdk::Display::default() {
                            let new_provider = gtk::CssProvider::new();
                            new_provider.load_from_data(&combined_css);
                            gtk::style_context_add_provider_for_display(
                                &display,
                                &new_provider,
                                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                            );
                        }
                    });

                    // Notificar a la app para actualizar colores de TextTags
                    sender.input(AppMsg::RefreshTheme);
                }
            }
        });
    }

    fn load_theme_css() -> (String, bool) {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
        let theme_dir = format!("{}/.config/omarchy/current/theme", home_dir);

        let css_files = vec![
            format!("{}/walker.css", theme_dir),
            format!("{}/waybar.css", theme_dir),
            format!("{}/swayosd.css", theme_dir),
        ];

        // Primero, cargamos y extraemos las variables de color de Omarchy
        let mut omarchy_css = String::new();
        let mut theme_loaded = false;

        for css_file in &css_files {
            if let Ok(content) = std::fs::read_to_string(css_file) {
                omarchy_css.push_str(&content);
                omarchy_css.push('\n');
                theme_loaded = true;
            }
        }

        // Cargar el CSS de la aplicación
        // Prioridad: 1) Desarrollo local, 2) Sistema instalado
        println!("🔍 DEBUG: Intentando cargar CSS...");
        let app_css = std::fs::read_to_string("assets/style.css")
            .inspect(|_| println!("✅ CSS cargado desde: assets/style.css"))
            .ok()
            .or_else(|| {
                std::fs::read_to_string("/usr/share/notnative-app/assets/style.css")
                    .inspect(|_| {
                        println!("✅ CSS cargado desde: /usr/share/notnative-app/assets/style.css")
                    })
                    .ok()
            })
            .or_else(|| {
                std::fs::read_to_string("/usr/share/notnative/assets/style.css")
                    .inspect(|_| {
                        println!(
                            "✅ CSS cargado desde: /usr/share/notnative/assets/style.css (fallback)"
                        )
                    })
                    .ok()
            })
            .or_else(|| {
                // Rutas de desarrollo
                if let Ok(exe_path) = std::env::current_exe() {
                    let css_path = exe_path
                        .parent()
                        .and_then(|p| p.parent())
                        .and_then(|p| p.parent())
                        .map(|p| p.join("assets/style.css"));

                    if let Some(ref path) = css_path {
                        println!("🔍 DEBUG: Intentando ruta exe: {:?}", path);
                        if let Ok(content) = std::fs::read_to_string(path) {
                            println!("✅ CSS cargado desde ruta exe: {:?}", path);
                            return Some(content);
                        }
                    }
                }
                None
            })
            .or_else(|| {
                println!("🔍 DEBUG: Intentando assets/style.css");
                std::fs::read_to_string("assets/style.css")
                    .inspect(|_| println!("✅ CSS cargado desde: assets/style.css"))
                    .ok()
            })
            .or_else(|| {
                println!("🔍 DEBUG: Intentando ./notnative-app/assets/style.css");
                std::fs::read_to_string("./notnative-app/assets/style.css")
                    .inspect(|_| println!("✅ CSS cargado desde: ./notnative-app/assets/style.css"))
                    .ok()
            });

        // Combinamos los CSS: primero las variables de Omarchy, luego el CSS de la app
        let mut combined_css = String::new();

        // Agregar las variables de Omarchy al principio
        if theme_loaded {
            combined_css.push_str("/* Variables de color de Omarchy */\n");
            combined_css.push_str(&omarchy_css);
            combined_css.push('\n');
        }

        // Agregar el CSS de la aplicación
        if let Some(app_css_content) = app_css {
            combined_css.push_str(&app_css_content);
        }

        (combined_css, theme_loaded)
    }

    fn execute_action(&mut self, action: EditorAction, sender: &ComponentSender<Self>) {
        // Verificar si hay una selección activa
        let selection_bounds = self.text_buffer.selection_bounds();
        let has_selection = selection_bounds.is_some();

        match action {
            EditorAction::ChangeMode(new_mode) => {
                // Si cambiamos a ChatAI, usar el mensaje apropiado
                if new_mode == EditorMode::ChatAI {
                    sender.input(AppMsg::EnterChatMode);
                } else {
                    let old_mode = *self.mode.borrow();

                    // Sincronización de cursor ANTES de cambiar el modo
                    if old_mode == EditorMode::Insert && new_mode == EditorMode::Normal {
                        // Salir de Insert: Capturar posición visual de GTK y actualizar posición lógica
                        let iter = self
                            .text_buffer
                            .iter_at_mark(&self.text_buffer.get_insert());
                        let display_pos = iter.offset() as usize;
                        let buffer_text = self.buffer.to_string();
                        self.cursor_position =
                            self.map_display_pos_to_buffer(&buffer_text, display_pos);

                        sender.input(AppMsg::ParseRemindersInNote);
                    } else if old_mode == EditorMode::Normal && new_mode == EditorMode::Insert {
                        // Entrar a Insert: Mover cursor visual de GTK a la posición lógica actual
                        let buffer_text = self.buffer.to_string();
                        let display_pos =
                            self.map_buffer_pos_to_display(&buffer_text, self.cursor_position);

                        let mut iter = self.text_buffer.start_iter();
                        iter.set_offset(display_pos as i32);
                        self.text_buffer.place_cursor(&iter);

                        // Scroll to cursor to ensure visibility
                        let mark = self.text_buffer.create_mark(None, &iter, false);
                        self.text_view.scroll_to_mark(&mark, 0.0, false, 0.0, 0.0);
                        self.text_buffer.delete_mark(&mark);
                    }

                    *self.mode.borrow_mut() = new_mode;
                    println!("Cambiado a modo: {:?}", new_mode);

                    // Actualizar configuración del TextView según el nuevo modo
                    match new_mode {
                        EditorMode::Normal => {
                            self.text_view.set_editable(false);
                            self.text_view.set_cursor_visible(true); // Cursor visible para ver navegación
                            self.text_view.grab_focus();
                        }
                        EditorMode::Insert => {
                            self.text_view.set_editable(true);
                            self.text_view.set_cursor_visible(true);
                            self.text_view.grab_focus();

                            // Cerrar sidebar automáticamente al entrar en modo Insert
                            if self.sidebar_visible {
                                self.sidebar_visible = false;
                                self.animate_sidebar(0);
                            }
                        }
                        _ => {}
                    }
                }
            }
            EditorAction::InsertChar(ch) => {
                // Si hay selección, primero borrarla
                if has_selection {
                    self.delete_selection();
                }
                self.buffer.insert(self.cursor_position, &ch.to_string());
                self.cursor_position += 1;
                self.has_unsaved_changes = true;
            }
            EditorAction::InsertNewline => {
                // Si hay selección, primero borrarla
                if has_selection {
                    self.delete_selection();
                }
                self.buffer.insert(self.cursor_position, "\n");
                self.cursor_position += 1;
                self.has_unsaved_changes = true;
            }
            EditorAction::DeleteCharBefore => {
                if has_selection {
                    // Borrar selección
                    self.delete_selection();
                } else if self.cursor_position > 0 {
                    // Borrar un carácter antes del cursor
                    self.buffer
                        .delete(self.cursor_position - 1..self.cursor_position);
                    self.cursor_position -= 1;
                    self.has_unsaved_changes = true;
                }
            }
            EditorAction::DeleteCharAfter => {
                if has_selection {
                    // Borrar selección
                    self.delete_selection();
                } else if self.cursor_position < self.buffer.len_chars() {
                    // Borrar un carácter después del cursor
                    self.buffer
                        .delete(self.cursor_position..self.cursor_position + 1);
                    self.has_unsaved_changes = true;
                }
            }
            EditorAction::DeleteSelection => {
                if has_selection {
                    self.delete_selection();
                }
            }
            EditorAction::MoveCursorLeft => {
                let current_mode = *self.mode.borrow();
                if current_mode == EditorMode::Normal && self.markdown_enabled {
                    // En modo Normal, mover visualmente
                    let buffer_text = self.buffer.to_string();
                    let current_display_pos =
                        self.map_buffer_pos_to_display(&buffer_text, self.cursor_position);

                    if current_display_pos > 0 {
                        let mut new_display_pos = current_display_pos - 1;
                        let mut new_cursor_pos =
                            self.map_display_pos_to_buffer(&buffer_text, new_display_pos);

                        // Si el cursor no se movió (estamos en un widget atómico), seguir moviendo
                        while new_cursor_pos == self.cursor_position && new_display_pos > 0 {
                            new_display_pos -= 1;
                            new_cursor_pos =
                                self.map_display_pos_to_buffer(&buffer_text, new_display_pos);
                        }

                        self.cursor_position = new_cursor_pos;
                    }
                } else if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            EditorAction::MoveCursorRight => {
                let current_mode = *self.mode.borrow();
                if current_mode == EditorMode::Normal && self.markdown_enabled {
                    // En modo Normal, mover visualmente
                    let buffer_text = self.buffer.to_string();
                    let current_display_pos =
                        self.map_buffer_pos_to_display(&buffer_text, self.cursor_position);
                    let clean_text = self.render_clean_markdown(&buffer_text);
                    let max_display_pos = clean_text.chars().count();

                    if current_display_pos < max_display_pos {
                        let mut new_display_pos = current_display_pos + 1;
                        let mut new_cursor_pos =
                            self.map_display_pos_to_buffer(&buffer_text, new_display_pos);

                        // Si el cursor no se movió (estamos en un widget atómico), seguir moviendo
                        while new_cursor_pos == self.cursor_position
                            && new_display_pos < max_display_pos
                        {
                            new_display_pos += 1;
                            new_cursor_pos =
                                self.map_display_pos_to_buffer(&buffer_text, new_display_pos);
                        }

                        self.cursor_position = new_cursor_pos;
                    }
                } else if self.cursor_position < self.buffer.len_chars() {
                    self.cursor_position += 1;
                }
            }
            EditorAction::MoveCursorUp => {
                let current_mode = *self.mode.borrow();
                if current_mode == EditorMode::Normal && self.markdown_enabled {
                    // Delegar el movimiento visual a GTK para que sea natural (respete wrapping, etc)
                    self.text_view
                        .emit_move_cursor(gtk::MovementStep::DisplayLines, -1, false);

                    // Sincronizar nuestra posición lógica con la nueva posición visual
                    // Usamos get_insert() para obtener el mark del cursor actual
                    let iter = self
                        .text_buffer
                        .iter_at_mark(&self.text_buffer.get_insert());
                    let new_display_pos = iter.offset() as usize;
                    let buffer_text = self.buffer.to_string();
                    self.cursor_position =
                        self.map_display_pos_to_buffer(&buffer_text, new_display_pos);
                } else {
                    // Obtener la línea actual y columna
                    let line = self.buffer.rope().char_to_line(self.cursor_position);
                    if line > 0 {
                        // Ir a la línea anterior
                        let prev_line = line - 1;
                        let line_start = self.buffer.rope().line_to_char(prev_line);
                        let line_end = if prev_line < self.buffer.len_lines() - 1 {
                            self.buffer
                                .rope()
                                .line_to_char(prev_line + 1)
                                .saturating_sub(1)
                        } else {
                            self.buffer.len_chars()
                        };

                        // Intentar mantener la columna, pero no exceder el largo de la línea
                        let current_line_start = self.buffer.rope().line_to_char(line);
                        let col_in_line = self.cursor_position - current_line_start;
                        let prev_line_len = line_end - line_start;

                        self.cursor_position = line_start + col_in_line.min(prev_line_len);
                    }
                }
            }
            EditorAction::MoveCursorDown => {
                let current_mode = *self.mode.borrow();
                if current_mode == EditorMode::Normal && self.markdown_enabled {
                    // Delegar el movimiento visual a GTK para que sea natural (respete wrapping, etc)
                    self.text_view
                        .emit_move_cursor(gtk::MovementStep::DisplayLines, 1, false);

                    // Sincronizar nuestra posición lógica con la nueva posición visual
                    let iter = self
                        .text_buffer
                        .iter_at_mark(&self.text_buffer.get_insert());
                    let new_display_pos = iter.offset() as usize;
                    let buffer_text = self.buffer.to_string();
                    self.cursor_position =
                        self.map_display_pos_to_buffer(&buffer_text, new_display_pos);
                } else {
                    // Obtener la línea actual y columna
                    let line = self.buffer.rope().char_to_line(self.cursor_position);
                    if line < self.buffer.len_lines() - 1 {
                        // Ir a la línea siguiente
                        let next_line = line + 1;
                        let line_start = self.buffer.rope().line_to_char(next_line);
                        let line_end = if next_line < self.buffer.len_lines() - 1 {
                            self.buffer
                                .rope()
                                .line_to_char(next_line + 1)
                                .saturating_sub(1)
                        } else {
                            self.buffer.len_chars()
                        };

                        // Intentar mantener la columna, pero no exceder el largo de la línea
                        let current_line_start = self.buffer.rope().line_to_char(line);
                        let col_in_line = self.cursor_position - current_line_start;
                        let next_line_len = line_end - line_start;

                        self.cursor_position = line_start + col_in_line.min(next_line_len);
                    }
                }
            }
            EditorAction::MoveCursorLineStart => {
                let line = self.buffer.rope().char_to_line(self.cursor_position);
                self.cursor_position = self.buffer.rope().line_to_char(line);
            }
            EditorAction::MoveCursorLineEnd => {
                let line = self.buffer.rope().char_to_line(self.cursor_position);
                let line_start = self.buffer.rope().line_to_char(line);
                let line_end = if line < self.buffer.len_lines() - 1 {
                    self.buffer.rope().line_to_char(line + 1).saturating_sub(1)
                } else {
                    self.buffer.len_chars()
                };
                self.cursor_position = line_end;
            }
            EditorAction::MoveCursorDocStart => {
                self.cursor_position = 0;
            }
            EditorAction::MoveCursorDocEnd => {
                self.cursor_position = self.buffer.len_chars();
            }
            EditorAction::Undo => {
                if self.buffer.undo() {
                    println!(
                        "Undo ejecutado. Puede rehacer ahora: {}",
                        self.buffer.can_redo()
                    );
                    self.has_unsaved_changes = true;
                }
            }
            EditorAction::Redo => {
                println!(
                    "Intentando rehacer. Puede rehacer: {}",
                    self.buffer.can_redo()
                );
                if self.buffer.redo() {
                    println!("Redo exitoso");
                    self.has_unsaved_changes = true;
                } else {
                    println!("Redo falló - no hay nada para rehacer");
                }
            }
            EditorAction::Copy => {
                // Copiar al portapapeles usando GTK
                if let Some(display) = gtk::gdk::Display::default() {
                    let clipboard = display.clipboard();

                    // Obtener texto seleccionado del text_buffer
                    if self.text_buffer.has_selection() {
                        let (start, end) = self.text_buffer.selection_bounds().unwrap();
                        let text = self.text_buffer.text(&start, &end, false);
                        clipboard.set_text(&text);
                    }
                }
            }
            EditorAction::Cut => {
                // Cortar al portapapeles usando GTK
                if let Some(display) = gtk::gdk::Display::default() {
                    let clipboard = display.clipboard();

                    // Obtener texto seleccionado y eliminarlo
                    if self.text_buffer.has_selection() {
                        let (start, end) = self.text_buffer.selection_bounds().unwrap();
                        let text = self.text_buffer.text(&start, &end, false);
                        clipboard.set_text(&text);

                        // Eliminar el texto seleccionado del buffer
                        let start_offset = start.offset() as usize;
                        let end_offset = end.offset() as usize;
                        self.buffer.delete(start_offset..end_offset);
                        self.has_unsaved_changes = true;
                    }
                }
            }
            EditorAction::Paste => {
                // Pegar desde el portapapeles (texto o imagen)
                if let Some(display) = gtk::gdk::Display::default() {
                    let clipboard = display.clipboard();
                    let clipboard_for_text = clipboard.clone();
                    let clipboard_for_fallback = clipboard.clone();

                    // Primero intentar leer una imagen
                    let sender_clone = sender.clone();
                    let text_buffer = self.text_buffer.clone();
                    let text_buffer_fallback = self.text_buffer.clone();
                    let buffer = self.buffer.clone();
                    let buffer_fallback = self.buffer.clone();
                    let cursor_pos = self.cursor_position;

                    clipboard.read_texture_async(None::<&gtk::gio::Cancellable>, move |result| {
                        if let Ok(Some(texture)) = result {
                            // Hay una imagen en el portapapeles
                            // Guardarla como archivo temporal y luego insertarla
                            if let Err(e) = Self::save_texture_and_insert(&texture, &sender_clone) {
                                eprintln!("Error guardando imagen del portapapeles: {}", e);

                                // Si falla, intentar pegar como texto
                                let sender_for_fallback = sender_clone.clone();
                                clipboard_for_fallback.read_text_async(
                                    None::<&gtk::gio::Cancellable>,
                                    move |result| {
                                        if let Ok(Some(text)) = result {
                                            sender_for_fallback
                                                .input(AppMsg::ProcessPastedText(text.to_string()));
                                        }
                                    },
                                );
                            }
                        } else {
                            // No hay imagen, intentar pegar texto
                            let sender_for_text = sender_clone.clone();
                            clipboard_for_text.read_text_async(
                                None::<&gtk::gio::Cancellable>,
                                move |result| {
                                    if let Ok(Some(text)) = result {
                                        // Procesar el texto (puede ser URL de imagen)
                                        sender_for_text
                                            .input(AppMsg::ProcessPastedText(text.to_string()));
                                    }
                                },
                            );
                        }
                    });
                    self.has_unsaved_changes = true;
                }
            }
            EditorAction::Save => {
                sender.input(AppMsg::SaveCurrentNote);
            }
            EditorAction::OpenSidebar => {
                sender.input(AppMsg::OpenSidebarAndFocus);
            }
            EditorAction::CloseSidebar => {
                // Solo cerrar si el sidebar está abierto
                if self.sidebar_visible {
                    sender.input(AppMsg::ToggleSidebar);
                }
            }
            EditorAction::CreateNote => {
                sender.input(AppMsg::ShowCreateNoteDialog);
            }
            EditorAction::InsertTable => {
                // Si hay selección, primero borrarla
                if has_selection {
                    self.delete_selection();
                }

                let table_template = "| Columna 1 | Columna 2 |\n|---|---|\n| Dato 1 | Dato 2 |\n";
                self.buffer.insert(self.cursor_position, table_template);
                self.cursor_position += table_template.chars().count();
                self.has_unsaved_changes = true;
            }
            EditorAction::InsertImage => {
                sender.input(AppMsg::InsertImage);
            }
            _ => {
                println!("Acción no implementada: {:?}", action);
            }
        }

        // Sincronizar el buffer con GTK TextView
        self.sync_to_view();

        // Actualizar barra de estado
        self.update_status_bar(sender);
    }

    /// Actualiza el estado de un TODO en el buffer interno
    fn update_todo_in_buffer(&mut self, line_pos: usize, new_state: bool) {
        let text = self.buffer.to_string();
        let chars: Vec<char> = text.chars().collect();

        // Verificar que la posición es válida
        if line_pos >= chars.len() {
            return;
        }

        // Verificar que hay un TODO en esa posición
        if line_pos + 4 >= chars.len() {
            return;
        }

        if chars[line_pos] == '-'
            && chars[line_pos + 1] == ' '
            && chars[line_pos + 2] == '['
            && chars[line_pos + 4] == ']'
        {
            let current_char = chars[line_pos + 3];
            let should_be_checked = new_state;
            let is_currently_checked = current_char == 'x' || current_char == 'X';

            // Solo actualizar si el estado cambió
            if should_be_checked != is_currently_checked {
                let new_char = if should_be_checked { "x" } else { " " };

                // Reemplazar el carácter en la posición correcta
                self.buffer.delete(line_pos + 3..line_pos + 4);
                self.buffer.insert(line_pos + 3, new_char);

                // Marcar como no guardado
                self.has_unsaved_changes = true;
            }
        }
    }

    /// Toggle TODO checkbox en una línea específica (desde WebView)
    /// El `checkbox_num` es el número secuencial del checkbox (1-indexado), no el número de línea
    fn toggle_todo_at_line(&mut self, checkbox_num: usize, checked: bool) {
        let text = self.buffer.to_string();
        let lines: Vec<&str> = text.lines().collect();

        if checkbox_num == 0 {
            return;
        }

        // Buscar el N-ésimo checkbox en el documento
        let mut checkbox_count = 0;
        let mut char_offset = 0;

        for line in lines.iter() {
            // Buscar patrones de TODO en esta línea
            let todo_patterns = ["- [ ]", "- [x]", "- [X]"];
            for pattern in &todo_patterns {
                if let Some(todo_pos) = line.find(pattern) {
                    checkbox_count += 1;
                    if checkbox_count == checkbox_num {
                        let buffer_pos = char_offset + todo_pos;
                        self.update_todo_in_buffer(buffer_pos, checked);
                        return;
                    }
                    break; // Solo un checkbox por línea
                }
            }
            char_offset += line.chars().count() + 1; // +1 for newline
        }
    }

    /// Renderiza el contenido actual como HTML y lo carga en el WebView de preview
    fn render_preview_html(&self) {
        let buffer_text = self.buffer.to_string();

        // Determinar el tema basado en la preferencia
        let preview_theme = match self.theme {
            ThemePreference::Light => PreviewTheme::Light,
            ThemePreference::Dark | ThemePreference::FollowSystem => PreviewTheme::Dark,
        };

        // Generar HTML con base_path para resolver imágenes locales
        let notes_base_path = self.notes_dir.root().to_path_buf();
        let renderer = HtmlRenderer::with_base_path(preview_theme, notes_base_path);
        let html = renderer.render(&buffer_text);

        // Cargar en el WebView
        use webkit6::prelude::WebViewExt;
        self.preview_webview.load_html(&html, None);
    }

    fn sync_to_view(&self) {
        self.sync_to_view_internal(true);
    }

    fn sync_to_view_no_focus(&self) {
        self.sync_to_view_internal(false);
    }

    fn sync_to_view_internal(&self, grab_focus: bool) {
        // Activar flag para evitar que los handlers GTK nos sincronicen de vuelta
        *self.is_syncing_to_gtk.borrow_mut() = true;
        println!("sync_to_view activado. Flag is_syncing_to_gtk = true");

        let buffer_text = self.buffer.to_string();
        let current_mode = *self.mode.borrow();

        // En modo Normal con markdown habilitado, usar WebView para preview HTML
        if current_mode == EditorMode::Normal && self.markdown_enabled {
            // Verificar si el texto fuente cambió
            let cached_source = self.cached_source_text.borrow();
            let text_changed = cached_source.as_ref() != Some(&buffer_text);
            drop(cached_source);

            if text_changed {
                // Actualizar cache
                *self.cached_source_text.borrow_mut() = Some(buffer_text.clone());

                // Renderizar HTML y cargar en WebView
                self.render_preview_html();

                println!(
                    "📍 sync_to_view: Modo Normal (WebView), buffer.len={}",
                    self.buffer.len_chars()
                );
            }

            // Asegurar que el WebView (preview) está visible
            self.editor_stack.set_visible_child_name("preview");

            // Solo dar foco si se solicita
            if grab_focus {
                let webview = self.preview_webview.clone();
                gtk::glib::idle_add_local_once(move || {
                    webview.grab_focus();
                });
            }
        } else {
            // En modo Insert o sin markdown, usar TextView tradicional
            // Limpiar el cache
            *self.cached_source_text.borrow_mut() = None;
            *self.cached_rendered_text.borrow_mut() = None;

            // Asegurar que el TextView (editor) está visible
            self.editor_stack.set_visible_child_name("editor");

            // Solo dar foco si se solicita
            if grab_focus {
                let text_view = self.text_view.clone();
                gtk::glib::idle_add_local_once(move || {
                    text_view.grab_focus();
                });
            }

            let cursor_offset = self.cursor_position.min(self.buffer.len_chars());

            println!(
                "📍 sync_to_view: Modo {:?} (TextView), cursor_offset={}, buffer.len={}",
                current_mode,
                cursor_offset,
                self.buffer.len_chars()
            );

            // Bloquear señales GTK durante la actualización
            self.text_buffer.begin_user_action();

            // Reemplazar todo el contenido
            let start_iter = self.text_buffer.start_iter();
            let end_iter = self.text_buffer.end_iter();
            self.text_buffer
                .delete(&mut start_iter.clone(), &mut end_iter.clone());
            self.text_buffer
                .insert(&mut self.text_buffer.start_iter(), &buffer_text);

            // Restaurar cursor
            let safe_cursor_offset = cursor_offset.min(buffer_text.chars().count());
            let mut iter = self.text_buffer.start_iter();
            iter.set_offset(safe_cursor_offset as i32);
            self.text_buffer.place_cursor(&iter);

            self.text_buffer.end_user_action();

            // Hacer scroll para mantener el cursor visible
            let text_view = self.text_view.clone();
            for delay_ms in [10, 50, 150] {
                let text_view_clone = text_view.clone();
                gtk::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(delay_ms),
                    move || {
                        let buffer = text_view_clone.buffer();
                        let insert_mark = buffer.get_insert();
                        text_view_clone.scroll_mark_onscreen(&insert_mark);
                    },
                );
            }

            // Mostrar cursor en TextView
            self.text_view.set_cursor_visible(true);
        }

        // Reiniciar el flag al terminar toda la sincronización
        *self.is_syncing_to_gtk.borrow_mut() = false;
        println!("sync_to_view completado. Reiniciando flag is_syncing_to_gtk");
    }

    /// Resalta texto en el editor y hace scroll hasta él
    /// Encuentra TODAS las coincidencias y permite navegar entre ellas
    fn highlight_and_scroll_to_text(&self, search_text: &str) {
        if search_text.is_empty() {
            // Limpiar coincidencias si el texto está vacío
            self.in_note_search_matches.borrow_mut().clear();
            *self.in_note_search_current_index.borrow_mut() = 0;
            self.text_buffer.remove_tag_by_name(
                "search-highlight",
                &self.text_buffer.start_iter(),
                &self.text_buffer.end_iter(),
            );
            self.text_buffer.remove_tag_by_name(
                "search-highlight-current",
                &self.text_buffer.start_iter(),
                &self.text_buffer.end_iter(),
            );
            return;
        }

        let buffer = &self.text_buffer;

        // En modo Normal, el buffer contiene texto renderizado (sin markdown).
        // Usamos el texto del buffer para buscar y resaltar.
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        let text_str = text.as_str();

        // Normalizar texto sin acentos para búsqueda
        let text_normalized = Self::remove_accents(&text_str.to_lowercase());
        let search_normalized = Self::remove_accents(&search_text.to_lowercase());

        // Encontrar TODAS las coincidencias (sin distinguir acentos)
        let mut matches: Vec<(i32, i32)> = Vec::new();
        let mut start_pos = 0;

        while let Some(pos) = text_normalized[start_pos..].find(&search_normalized) {
            let absolute_pos = start_pos + pos;
            // Convertir posición de bytes a offset de caracteres en texto normalizado
            let char_start_normalized = text_normalized[..absolute_pos].chars().count();
            // Como ambos textos tienen el mismo número de caracteres (solo cambian los acentos),
            // la posición en caracteres es la misma
            let char_start = char_start_normalized as i32;
            let char_end = char_start + search_normalized.chars().count() as i32;
            matches.push((char_start, char_end));
            start_pos = absolute_pos + search_normalized.len();
        }

        println!(
            "🔍 Búsqueda en nota: '{}' encontró {} coincidencias en buffer de {} chars",
            search_text,
            matches.len(),
            text_str.chars().count()
        );

        // Actualizar el indicador de modo con el conteo de coincidencias
        if !matches.is_empty() {
            let current_idx = *self.in_note_search_current_index.borrow();
            let display_idx = if current_idx < matches.len() {
                current_idx + 1
            } else {
                1
            };
            self.floating_search_mode_label.set_markup(&format!(
                "<small>{}/{} (Enter: sig, Shift+Enter: ant)</small>",
                display_idx,
                matches.len()
            ));
        } else {
            self.floating_search_mode_label
                .set_markup("<small>Sin coincidencias</small>");
        }

        // Guardar las coincidencias
        let previous_matches = self.in_note_search_matches.borrow().clone();
        *self.in_note_search_matches.borrow_mut() = matches.clone();

        // Si el texto de búsqueda cambió, reiniciar el índice
        if previous_matches != matches {
            *self.in_note_search_current_index.borrow_mut() = 0;
        }

        if matches.is_empty() {
            // Limpiar resaltados si no hay coincidencias
            buffer.remove_tag_by_name("search-highlight", &buffer.start_iter(), &buffer.end_iter());
            buffer.remove_tag_by_name(
                "search-highlight-current",
                &buffer.start_iter(),
                &buffer.end_iter(),
            );
            return;
        }

        // Crear o obtener tags de resaltado
        let tag_table = buffer.tag_table();

        // Tag para todas las coincidencias (resaltado suave)
        let tag_all = if let Some(existing_tag) = tag_table.lookup("search-highlight") {
            existing_tag
        } else {
            let new_tag = gtk::TextTag::new(Some("search-highlight"));
            new_tag.set_background(Some("#4a4a00")); // Amarillo oscuro para coincidencias no actuales
            tag_table.add(&new_tag);
            new_tag
        };

        // Tag para la coincidencia actual (resaltado brillante)
        let tag_current = if let Some(existing_tag) = tag_table.lookup("search-highlight-current") {
            existing_tag
        } else {
            let new_tag = gtk::TextTag::new(Some("search-highlight-current"));
            new_tag.set_background(Some("#ffd700")); // Amarillo dorado brillante
            new_tag.set_foreground(Some("#000000")); // Texto negro
            tag_table.add(&new_tag);
            new_tag
        };

        // Limpiar resaltados previos
        buffer.remove_tag_by_name("search-highlight", &buffer.start_iter(), &buffer.end_iter());
        buffer.remove_tag_by_name(
            "search-highlight-current",
            &buffer.start_iter(),
            &buffer.end_iter(),
        );

        // Aplicar resaltado a todas las coincidencias
        for (start, end) in &matches {
            let mut start_iter = buffer.start_iter();
            let mut end_iter = buffer.start_iter();
            start_iter.set_offset(*start);
            end_iter.set_offset(*end);
            buffer.apply_tag(&tag_all, &start_iter, &end_iter);
        }

        // Resaltar la coincidencia actual con color más brillante
        self.highlight_current_match();
    }

    /// Resalta la coincidencia actual y hace scroll hasta ella
    fn highlight_current_match(&self) {
        let matches = self.in_note_search_matches.borrow();
        let current_index = *self.in_note_search_current_index.borrow();

        if matches.is_empty() || current_index >= matches.len() {
            return;
        }

        let (start, end) = matches[current_index];
        let buffer = &self.text_buffer;

        // Actualizar indicador de posición
        self.floating_search_mode_label.set_markup(&format!(
            "<small>{}/{} (Enter: sig, Shift+Enter: ant)</small>",
            current_index + 1,
            matches.len()
        ));

        // Quitar resaltado actual anterior
        buffer.remove_tag_by_name(
            "search-highlight-current",
            &buffer.start_iter(),
            &buffer.end_iter(),
        );

        // Obtener tag de resaltado actual
        let tag_table = buffer.tag_table();
        if let Some(tag_current) = tag_table.lookup("search-highlight-current") {
            let mut start_iter = buffer.start_iter();
            let mut end_iter = buffer.start_iter();
            start_iter.set_offset(start);
            end_iter.set_offset(end);
            buffer.apply_tag(&tag_current, &start_iter, &end_iter);

            // Mover cursor y hacer scroll
            buffer.place_cursor(&start_iter);

            let text_view = self.text_view.clone();
            gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
                let buffer = text_view.buffer();
                let insert_mark = buffer.get_insert();
                text_view.scroll_to_mark(&insert_mark, 0.1, true, 0.0, 0.4);
            });
        }
    }

    /// Elimina acentos de un texto para búsqueda sin distinción de acentos
    fn remove_accents(text: &str) -> String {
        text.chars()
            .map(|c| match c {
                'á' | 'à' | 'ä' | 'â' | 'ã' => 'a',
                'é' | 'è' | 'ë' | 'ê' => 'e',
                'í' | 'ì' | 'ï' | 'î' => 'i',
                'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
                'ú' | 'ù' | 'ü' | 'û' => 'u',
                'ñ' => 'n',
                'ç' => 'c',
                'Á' | 'À' | 'Ä' | 'Â' | 'Ã' => 'a',
                'É' | 'È' | 'Ë' | 'Ê' => 'e',
                'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
                'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' => 'o',
                'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
                'Ñ' => 'n',
                'Ç' => 'c',
                _ => c,
            })
            .collect()
    }

    /// Ir a la siguiente coincidencia en búsqueda dentro de nota
    fn go_to_next_match(&self) {
        let matches_len = self.in_note_search_matches.borrow().len();
        if matches_len == 0 {
            return;
        }

        let mut current_index = self.in_note_search_current_index.borrow_mut();
        *current_index = (*current_index + 1) % matches_len;
        drop(current_index);

        self.highlight_current_match();
    }

    /// Ir a la anterior coincidencia en búsqueda dentro de nota
    fn go_to_prev_match(&self) {
        let matches_len = self.in_note_search_matches.borrow().len();
        if matches_len == 0 {
            return;
        }

        let mut current_index = self.in_note_search_current_index.borrow_mut();
        if *current_index == 0 {
            *current_index = matches_len - 1;
        } else {
            *current_index -= 1;
        }
        drop(current_index);

        self.highlight_current_match();
    }

    /// Hace scroll para que el cursor sea visible (NO USAR - scroll se hace en sync_to_view)
    #[allow(dead_code)]
    fn scroll_to_cursor(&self) {
        let text_view_clone = self.text_view.clone();
        // Usar timeout en lugar de idle para dar más tiempo a GTK a procesar los cambios
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(10), move || {
            let cursor_mark = text_view_clone.buffer().get_insert();
            text_view_clone.scroll_mark_onscreen(&cursor_mark);
        });
    }

    /// Renderiza el texto markdown sin los símbolos de formato
    fn render_clean_markdown(&self, text: &str) -> String {
        self.render_clean_markdown_internal(text, None, None).0
    }

    /// Mapea una posición del buffer original al texto limpio (sin símbolos markdown)
    fn map_buffer_pos_to_display(&self, original_text: &str, buffer_pos: usize) -> usize {
        self.render_clean_markdown_internal(original_text, Some(buffer_pos), None)
            .1
    }

    /// Mapea una posición del texto limpio (display) al buffer original
    fn map_display_pos_to_buffer(&self, original_text: &str, display_pos: usize) -> usize {
        self.render_clean_markdown_internal(original_text, None, Some(display_pos))
            .1
    }

    fn render_clean_markdown_internal(
        &self,
        text: &str,
        stop_at_original_pos: Option<usize>,
        stop_at_display_pos: Option<usize>,
    ) -> (String, usize) {
        let mut result = String::new();
        let mut display_char_count = 0;
        let mut chars = text.chars().peekable();
        let mut original_idx = 0;
        let mut in_code_block = false;
        let mut at_line_start = true;
        let mut indent_spaces = 0;

        // Detectar si estamos en modo mapeo (calculando posiciones) o renderizado (generando texto)
        // Si estamos mapeando, los widgets (Tasks, Videos, etc.) ocupan 1 caracter (el anchor).
        // Si estamos renderizando, ocupan el texto completo del marcador.
        let is_mapping = stop_at_original_pos.is_some() || stop_at_display_pos.is_some();

        loop {
            // Check stops
            if let Some(stop) = stop_at_original_pos {
                if original_idx == stop {
                    return (result.clone(), display_char_count);
                }
            }
            if let Some(stop) = stop_at_display_pos {
                if display_char_count == stop {
                    return (result.clone(), original_idx);
                }
            }

            let ch = match chars.next() {
                Some(c) => c,
                None => break,
            };
            original_idx += 1;

            match ch {
                // Code blocks: ```
                '`' if chars.peek() == Some(&'`') => {
                    let mut backtick_count = 1;
                    while chars.peek() == Some(&'`') {
                        if let Some(stop) = stop_at_original_pos {
                            if original_idx == stop {
                                return (result.clone(), display_char_count);
                            }
                        }
                        chars.next();
                        original_idx += 1;
                        backtick_count += 1;
                    }

                    if backtick_count >= 3 {
                        in_code_block = !in_code_block;
                        while let Some(&next_ch) = chars.peek() {
                            if let Some(stop) = stop_at_original_pos {
                                if original_idx == stop {
                                    return (result.clone(), display_char_count);
                                }
                            }
                            chars.next();
                            original_idx += 1;
                            if next_ch == '\n' {
                                at_line_start = true;
                                break;
                            }
                        }
                        continue;
                    } else if backtick_count == 1 {
                        at_line_start = false;
                        continue;
                    }
                }

                // Encabezados
                '#' if !in_code_block && at_line_start => {
                    let mut hash_count = 1;
                    while chars.peek() == Some(&'#') {
                        if let Some(stop) = stop_at_original_pos {
                            if original_idx == stop {
                                return (result.clone(), display_char_count);
                            }
                        }
                        chars.next();
                        original_idx += 1;
                        hash_count += 1;
                    }

                    if chars.peek() == Some(&' ') {
                        if let Some(stop) = stop_at_original_pos {
                            if original_idx == stop {
                                return (result.clone(), display_char_count);
                            }
                        }
                        chars.next();
                        original_idx += 1;
                        at_line_start = false;
                    } else {
                        for _ in 0..hash_count {
                            result.push('#');
                            display_char_count += 1;
                        }
                        at_line_start = false;
                    }
                }

                // Listas y TODOs
                '-' if !in_code_block && at_line_start => {
                    let mut lookahead = Vec::new();
                    let mut temp_chars = chars.clone();
                    for _ in 0..6 {
                        if let Some(c) = temp_chars.next() {
                            lookahead.push(c);
                        } else {
                            break;
                        }
                    }

                    if lookahead.len() >= 5
                        && lookahead[0] == ' '
                        && lookahead[1] == '['
                        && lookahead[2] == ' '
                        && lookahead[3] == ']'
                        && lookahead[4] == ' '
                    {
                        let start_idx = original_idx; // After -
                        if indent_spaces > 0 {
                            let tree_indicator = "└─ ";
                            let chars_to_remove = 2.min(result.len());
                            // Ajustar display_char_count
                            let chars_removed_count = 2.min(display_char_count); // Aproximado
                            display_char_count -= chars_removed_count;

                            result.truncate(result.len() - chars_to_remove);
                            result.push_str(tree_indicator);
                            display_char_count += tree_indicator.chars().count();
                        }
                        for _ in 0..5 {
                            if let Some(stop) = stop_at_original_pos {
                                if original_idx == stop {
                                    return (result.clone(), display_char_count);
                                }
                            }
                            chars.next();
                            original_idx += 1;
                        }
                        let widget_text = "[TODO:unchecked] ";
                        let widget_len = if is_mapping {
                            1
                        } else {
                            widget_text.chars().count()
                        };

                        if let Some(stop) = stop_at_display_pos {
                            if stop >= display_char_count && stop < display_char_count + widget_len
                            {
                                // Si estamos al principio del widget, devolver el inicio
                                if stop == display_char_count {
                                    return (result, start_idx - 1); // -1 for -
                                }
                                // Si estamos en cualquier otro lugar dentro del widget, devolver el final
                                return (result, original_idx);
                            }
                        }

                        result.push_str(widget_text);
                        display_char_count += widget_len;
                        at_line_start = false;
                    } else if lookahead.len() >= 5
                        && lookahead[0] == ' '
                        && lookahead[1] == '['
                        && (lookahead[2] == 'x' || lookahead[2] == 'X')
                        && lookahead[3] == ']'
                        && lookahead[4] == ' '
                    {
                        let start_idx = original_idx; // After -
                        if indent_spaces > 0 {
                            let tree_indicator = "└─ ";
                            let chars_to_remove = 2.min(result.len());
                            let chars_removed_count = 2.min(display_char_count);
                            display_char_count -= chars_removed_count;

                            result.truncate(result.len() - chars_to_remove);
                            result.push_str(tree_indicator);
                            display_char_count += tree_indicator.chars().count();
                        }
                        for _ in 0..5 {
                            if let Some(stop) = stop_at_original_pos {
                                if original_idx == stop {
                                    return (result.clone(), display_char_count);
                                }
                            }
                            chars.next();
                            original_idx += 1;
                        }
                        let widget_text = "[TODO:checked] ";
                        let widget_len = if is_mapping {
                            1
                        } else {
                            widget_text.chars().count()
                        };

                        if let Some(stop) = stop_at_display_pos {
                            if stop >= display_char_count && stop < display_char_count + widget_len
                            {
                                // Si estamos al principio del widget, devolver el inicio
                                if stop == display_char_count {
                                    return (result, start_idx - 1); // -1 for -
                                }
                                // Si estamos en cualquier otro lugar dentro del widget, devolver el final
                                return (result, original_idx);
                            }
                        }

                        result.push_str(widget_text);
                        display_char_count += widget_len;
                        at_line_start = false;
                    } else if lookahead.len() >= 1 && lookahead[0] == ' ' {
                        if let Some(stop) = stop_at_original_pos {
                            if original_idx == stop {
                                return (result.clone(), display_char_count);
                            }
                        }
                        chars.next();
                        original_idx += 1;
                        result.push('•');
                        result.push(' ');
                        display_char_count += 2;
                        at_line_start = false;
                    } else {
                        result.push(ch);
                        display_char_count += 1;
                        at_line_start = false;
                    }
                }

                // Blockquotes
                '>' if !in_code_block && at_line_start => {
                    if chars.peek() == Some(&' ') {
                        if let Some(stop) = stop_at_original_pos {
                            if original_idx == stop {
                                return (result.clone(), display_char_count);
                            }
                        }
                        chars.next();
                        original_idx += 1;
                    }
                    at_line_start = false;
                }

                // Links e Imágenes
                '!' if !in_code_block && chars.peek() == Some(&'[') => {
                    if let Some(stop) = stop_at_original_pos {
                        if original_idx == stop {
                            return (result.clone(), display_char_count);
                        }
                    }
                    chars.next(); // [
                    original_idx += 1;
                    let start_idx = original_idx; // Start of alt text

                    while let Some(&next_ch) = chars.peek() {
                        if let Some(stop) = stop_at_original_pos {
                            if original_idx == stop {
                                return (result.clone(), display_char_count);
                            }
                        }
                        chars.next();
                        original_idx += 1;
                        if next_ch == ']' {
                            break;
                        }
                    }

                    if chars.peek() == Some(&'(') {
                        if let Some(stop) = stop_at_original_pos {
                            if original_idx == stop {
                                return (result.clone(), display_char_count);
                            }
                        }
                        chars.next(); // (
                        original_idx += 1;
                        let mut img_src = String::new();
                        while let Some(&next_ch) = chars.peek() {
                            if let Some(stop) = stop_at_original_pos {
                                if original_idx == stop {
                                    return (result.clone(), display_char_count);
                                }
                            }
                            chars.next();
                            original_idx += 1;
                            if next_ch == ')' {
                                break;
                            }
                            img_src.push(next_ch);
                        }
                        let marker = format!("[IMG:{}]", img_src);
                        let marker_len = if is_mapping {
                            1
                        } else {
                            marker.chars().count()
                        };

                        if let Some(stop) = stop_at_display_pos {
                            if stop >= display_char_count && stop < display_char_count + marker_len
                            {
                                return (result, start_idx - 2); // -2 for ![
                            }
                        }

                        result.push_str(&marker);
                        display_char_count += marker_len;
                    } else {
                        if let Some(stop) = stop_at_display_pos {
                            if stop == display_char_count {
                                return (result, start_idx - 2);
                            } // ![
                            if stop == display_char_count + 1 {
                                return (result, start_idx - 1);
                            } // [
                        }
                        result.push_str("![");
                        display_char_count += 2;
                    }
                }

                // Links
                '[' if !in_code_block => {
                    let mut link_text = String::new();
                    let mut found_close = false;
                    let link_start_idx = original_idx; // Start of link text in buffer

                    // Capture if we hit the stop pos inside the link text
                    let mut hit_in_link_text = None;

                    while let Some(&next_ch) = chars.peek() {
                        if let Some(stop) = stop_at_original_pos {
                            if original_idx == stop {
                                // Don't return yet!
                                hit_in_link_text = Some(link_text.chars().count());
                            }
                        }
                        chars.next();
                        original_idx += 1;
                        if next_ch == ']' {
                            found_close = true;
                            break;
                        }
                        link_text.push(next_ch);
                    }

                    // Check if we hit stop at the closing bracket ']'
                    if let Some(stop) = stop_at_original_pos {
                        if original_idx == stop && hit_in_link_text.is_none() {
                            // We are at ']'
                            hit_in_link_text = Some(link_text.chars().count());
                        }
                    }

                    if found_close && chars.peek() == Some(&'(') {
                        // Check if we hit stop at '('
                        if let Some(stop) = stop_at_original_pos {
                            if original_idx == stop {
                                // Map to end of link text
                                return (
                                    result.clone(),
                                    display_char_count + link_text.chars().count(),
                                );
                            }
                        }

                        chars.next(); // (
                        original_idx += 1;
                        let mut url = String::new();

                        // Check inside URL
                        let mut hit_in_url = false;

                        while let Some(&next_ch) = chars.peek() {
                            if let Some(stop) = stop_at_original_pos {
                                if original_idx == stop {
                                    hit_in_url = true;
                                }
                            }
                            chars.next();
                            original_idx += 1;
                            if next_ch == ')' {
                                break;
                            }
                            url.push(next_ch);
                        }

                        // Check closing paren ')'
                        if let Some(stop) = stop_at_original_pos {
                            if original_idx == stop {
                                hit_in_url = true;
                            }
                        }

                        if let Some(video_id) = Self::extract_youtube_video_id(&url) {
                            let marker = format!("[VIDEO:{}]", video_id);
                            // En GTK se insertan: \n (1) + \n (1) + Anchor (1) = 3 chars
                            // La estructura real parece ser \n\n[Anchor] debido a la lógica de inserción
                            let marker_len = if is_mapping {
                                3
                            } else {
                                marker.chars().count()
                            };

                            if let Some(stop) = stop_at_display_pos {
                                if stop >= display_char_count
                                    && stop < display_char_count + marker_len
                                {
                                    // If clicking on video marker (or its surrounding newlines), return start of link structure
                                    return (result, link_start_idx - 1); // -1 for [
                                }
                            }

                            // If we hit inside the link text or URL, map to start of video marker
                            if hit_in_link_text.is_some() || hit_in_url {
                                return (result.clone(), display_char_count);
                            }

                            if is_mapping {
                                result.push_str("\n\n");
                            }
                            result.push_str(&marker);
                            display_char_count += marker_len;
                        } else {
                            let len = link_text.chars().count();
                            if let Some(stop) = stop_at_display_pos {
                                if stop >= display_char_count && stop < display_char_count + len {
                                    let offset = stop - display_char_count;
                                    return (result, link_start_idx + offset);
                                }
                            }

                            // If we hit inside the link text, return exact position
                            if let Some(offset) = hit_in_link_text {
                                return (result.clone(), display_char_count + offset);
                            }

                            // If we hit inside the URL, map to end of link text
                            if hit_in_url {
                                return (result.clone(), display_char_count + len);
                            }

                            result.push_str(&link_text);
                            display_char_count += len;
                        }
                    } else {
                        // Not a valid link, restore [
                        if let Some(stop) = stop_at_display_pos {
                            if stop == display_char_count {
                                return (result, link_start_idx - 1); // Position of [
                            }
                        }

                        // If we hit stop at '['
                        if let Some(stop) = stop_at_original_pos {
                            if stop == link_start_idx - 1 {
                                return (result.clone(), display_char_count);
                            }
                        }

                        result.push('[');
                        display_char_count += 1;

                        let len = link_text.chars().count();
                        if let Some(stop) = stop_at_display_pos {
                            if stop >= display_char_count && stop <= display_char_count + len {
                                let offset = stop - display_char_count;
                                return (result, link_start_idx + offset);
                            }
                        }

                        // If we hit inside the link text (which is now just text)
                        if let Some(offset) = hit_in_link_text {
                            return (result.clone(), display_char_count + offset);
                        }

                        result.push_str(&link_text);
                        display_char_count += len;

                        if found_close {
                            if let Some(stop) = stop_at_display_pos {
                                if stop == display_char_count {
                                    return (result, original_idx - 1); // Position of ]
                                }
                            }

                            // If we hit stop at ']'
                            if let Some(stop) = stop_at_original_pos {
                                if stop == original_idx - 1 {
                                    return (result.clone(), display_char_count);
                                }
                            }

                            result.push(']');
                            display_char_count += 1;
                        }
                    }
                }

                // Recordatorios
                '!' if !in_code_block && chars.peek() == Some(&'!') => {
                    if let Some(stop) = stop_at_original_pos {
                        if original_idx == stop {
                            return (result.clone(), display_char_count);
                        }
                    }
                    chars.next(); // !
                    original_idx += 1;
                    let start_idx = original_idx; // After !!

                    let lookahead: String = chars.clone().take(9).collect();
                    if lookahead.starts_with("RECORDAR(") || lookahead.starts_with("REMIND(") {
                        let keyword_len = if lookahead.starts_with("RECORDAR(") {
                            8
                        } else {
                            6
                        };
                        for _ in 0..keyword_len {
                            if let Some(stop) = stop_at_original_pos {
                                if original_idx == stop {
                                    return (result.clone(), display_char_count);
                                }
                            }
                            chars.next();
                            original_idx += 1;
                        }
                        if chars.peek() == Some(&'(') {
                            if let Some(stop) = stop_at_original_pos {
                                if original_idx == stop {
                                    return (result.clone(), display_char_count);
                                }
                            }
                            chars.next();
                            original_idx += 1;
                        }
                        let mut reminder_content = String::new();
                        let mut paren_count = 1;
                        while let Some(&next_ch) = chars.peek() {
                            if let Some(stop) = stop_at_original_pos {
                                if original_idx == stop {
                                    return (result.clone(), display_char_count);
                                }
                            }
                            chars.next();
                            original_idx += 1;
                            if next_ch == '(' {
                                paren_count += 1;
                                reminder_content.push(next_ch);
                            } else if next_ch == ')' {
                                paren_count -= 1;
                                if paren_count == 0 {
                                    break;
                                }
                                reminder_content.push(next_ch);
                            } else {
                                reminder_content.push(next_ch);
                            }
                        }
                        let (params, reminder_text) =
                            if let Some(last_comma) = reminder_content.rfind(',') {
                                let params = &reminder_content[..last_comma];
                                let text = &reminder_content[last_comma + 1..];
                                (params.trim().to_string(), text.trim().to_string())
                            } else {
                                // Si no hay coma, es formato V1 (texto fuera) o inválido.
                                // El usuario ha solicitado desactivar el soporte para texto fuera.
                                // Por lo tanto, si no hay coma, no generamos widget y dejamos el texto original.

                                // Restaurar el texto original en el buffer visual
                                let keyword = if lookahead.starts_with("RECORDAR(") {
                                    "RECORDAR"
                                } else {
                                    "REMIND"
                                };

                                result.push_str("!!");
                                result.push_str(keyword);
                                result.push('(');
                                result.push_str(&reminder_content);
                                result.push(')');

                                display_char_count +=
                                    2 + keyword.len() + 1 + reminder_content.len() + 1;

                                continue;
                            };
                        let marker = format!("[REMINDER:{}|{}]", params, reminder_text);
                        // En GTK se inserta: Anchor (1) + " " (1) = 2 chars
                        // La estructura real es "[Anchor] "
                        let marker_len = if is_mapping {
                            2
                        } else {
                            marker.chars().count()
                        };

                        if let Some(stop) = stop_at_display_pos {
                            if stop >= display_char_count && stop < display_char_count + marker_len
                            {
                                return (result, start_idx - 2); // -2 for !!
                            }
                        }

                        result.push_str(&marker);
                        if is_mapping {
                            result.push(' ');
                        }
                        display_char_count += marker_len;
                    } else {
                        if let Some(stop) = stop_at_display_pos {
                            if stop == display_char_count {
                                return (result, start_idx - 2);
                            } // !!
                            if stop == display_char_count + 1 {
                                return (result, start_idx - 1);
                            } // !
                        }
                        result.push_str("!!");
                        display_char_count += 2;
                        at_line_start = false;
                    }
                }

                // Negrita
                '*' if !in_code_block && chars.peek() == Some(&'*') => {
                    if let Some(stop) = stop_at_original_pos {
                        if original_idx == stop {
                            return (result.clone(), display_char_count);
                        }
                    }
                    chars.next();
                    original_idx += 1;
                }

                // Cursiva
                '*' if !in_code_block => {}

                // Código inline
                '`' if !in_code_block => {
                    at_line_start = false;
                }

                // Salto de línea
                '\n' => {
                    result.push(ch);
                    display_char_count += 1;
                    at_line_start = true;
                    indent_spaces = 0;
                }

                // Espacios
                ' ' if at_line_start && !in_code_block => {
                    result.push(ch);
                    display_char_count += 1;
                    indent_spaces += 1;
                }

                // Default
                _ => {
                    result.push(ch);
                    display_char_count += 1;
                    at_line_start = false;
                    indent_spaces = 0;
                }
            }
        }

        // If we reached end and stop_at_display_pos was requested
        if stop_at_display_pos.is_some() {
            return (result, original_idx);
        }

        (result, display_char_count)
    }

    /// Genera un ID de anchor para un heading al estilo markdown
    /// Convierte "Conexión al MCP Server" → "conexión-al-mcp-server"
    fn generate_heading_id(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c
                } else if c.is_whitespace() || c == '-' {
                    '-'
                } else {
                    // Eliminar caracteres especiales
                    '\0'
                }
            })
            .filter(|&c| c != '\0')
            .collect::<String>()
            // Eliminar guiones duplicados
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }

    /// Aplica estilos markdown al texto limpio (sin símbolos)
    fn apply_markdown_styles_to_clean_text(&self, clean_text: &str) {
        let start = self.text_buffer.start_iter();
        let end = self.text_buffer.end_iter();
        self.text_buffer.remove_all_tags(&start, &end);

        // Limpiar widgets de imágenes, videos y TODOs anteriores
        // IMPORTANTE: Primero remover los child anchors del buffer para limpiar WebViews
        for video_widget in self.video_widgets.borrow().iter() {
            if let Some(parent) = video_widget.parent() {
                // Si el padre es un ChildAnchor, necesitamos removerlo del TextView
                // pero GTK lo maneja automáticamente al eliminar el anchor del buffer
                video_widget.unparent();
            }
        }

        self.image_widgets.borrow_mut().clear();
        self.video_widgets.borrow_mut().clear();
        self.table_widgets.borrow_mut().clear();
        self.todo_widgets.borrow_mut().clear();
        self.reminder_widgets.borrow_mut().clear();

        // Limpiar y recolectar headings para anchor links
        self.heading_anchors.borrow_mut().clear();

        // Obtener texto original para detectar markdown
        let original_text = self.buffer.to_string();
        let original_lines: Vec<&str> = original_text.lines().collect();

        // Preparar líneas limpias para mapearlas a las originales
        let clean_lines: Vec<&str> = clean_text.lines().collect();
        self.link_spans.borrow_mut().clear();
        self.tag_spans.borrow_mut().clear();
        self.youtube_video_spans.borrow_mut().clear();
        let mut clean_idx = 0usize;
        let mut orig_idx = 0usize;
        let mut in_code_block = false;
        let mut current_iter = self.text_buffer.start_iter();

        while orig_idx < original_lines.len() {
            let original_line = original_lines[orig_idx];
            let trimmed = original_line.trim();

            // Las líneas que contienen ``` NO aparecen en el texto limpio,
            // pero sí afectan al estado del bloque de código.
            if trimmed.starts_with("```") {
                in_code_block = !in_code_block;
                orig_idx += 1;
                // NO incrementar clean_idx porque esta línea no existe en clean_text
                continue;
            }

            // Si ya no hay más líneas limpias, terminar
            if clean_idx >= clean_lines.len() {
                break;
            }

            let clean_line = clean_lines[clean_idx];
            let line_start = current_iter.clone();

            if !current_iter.ends_line() {
                current_iter.forward_to_line_end();
            }
            let line_end = current_iter.clone();

            if !current_iter.is_end() {
                current_iter.forward_line();
            }

            // Asignar tag de bloque según la línea original
            let tag_name = if in_code_block {
                Some("codeblock")
            } else if original_line.starts_with("### ") {
                // Recolectar heading nivel 3
                let heading_text = original_line[4..].trim();
                let heading_id = Self::generate_heading_id(heading_text);
                self.heading_anchors.borrow_mut().push(HeadingAnchor {
                    id: heading_id,
                    line_offset: line_start.offset(),
                    text: heading_text.to_string(),
                });
                Some("h3")
            } else if original_line.starts_with("## ") {
                // Recolectar heading nivel 2
                let heading_text = original_line[3..].trim();
                let heading_id = Self::generate_heading_id(heading_text);
                self.heading_anchors.borrow_mut().push(HeadingAnchor {
                    id: heading_id,
                    line_offset: line_start.offset(),
                    text: heading_text.to_string(),
                });
                Some("h2")
            } else if original_line.starts_with("# ") {
                // Recolectar heading nivel 1
                let heading_text = original_line[2..].trim();
                let heading_id = Self::generate_heading_id(heading_text);
                self.heading_anchors.borrow_mut().push(HeadingAnchor {
                    id: heading_id,
                    line_offset: line_start.offset(),
                    text: heading_text.to_string(),
                });
                Some("h1")
            } else if original_line.starts_with("> ") {
                Some("blockquote")
            } else if original_line.starts_with("- ") || original_line.starts_with("* ") {
                Some("list")
            } else if original_line
                .chars()
                .next()
                .map_or(false, |c| c.is_numeric())
                && original_line.contains(". ")
            {
                Some("list")
            } else {
                None
            };

            if let Some(tag) = tag_name {
                if let Some(text_tag) = self.text_buffer.tag_table().lookup(tag) {
                    self.text_buffer
                        .apply_tag(&text_tag, &line_start, &line_end);
                }
            }

            if !in_code_block {
                let base_offset = line_start.offset();
                self.apply_inline_styles(clean_line, original_line, &line_start, base_offset);
            }

            clean_idx += 1;
            orig_idx += 1;
        }

        // IMPORTANTE: Procesar imágenes, videos y TODOs DESPUÉS de aplicar todos los estilos
        // para evitar invalidar los iteradores
        self.process_all_images_in_buffer();

        // Procesar videos de YouTube usando marcadores [VIDEO:...] solo en modo NORMAL
        if *self.mode.borrow() == EditorMode::Normal {
            self.process_all_video_markers_in_buffer();
            self.process_all_tables_in_buffer();
        }

        self.process_all_todos_in_buffer();
        self.process_all_reminder_markers_in_buffer();
    }

    /// Procesa todos los marcadores de imagen [IMG:path] en el buffer completo
    fn process_all_images_in_buffer(&self) {
        // Obtener todo el texto del buffer
        let start = self.text_buffer.start_iter();
        let end = self.text_buffer.end_iter();
        let buffer_text = self.text_buffer.text(&start, &end, false).to_string();

        // Debug: ver si hay marcadores
        if buffer_text.contains("[IMG:") {
            println!("DEBUG: Buffer contiene marcadores de imagen");
        }

        // Buscar todos los marcadores y sus posiciones
        let mut images = Vec::new();
        let mut search_pos = 0;

        while let Some(img_start) = buffer_text[search_pos..].find("[IMG:") {
            let absolute_start = search_pos + img_start;

            // Buscar el cierre ]
            if let Some(img_end_relative) = buffer_text[absolute_start..].find(']') {
                let absolute_end = absolute_start + img_end_relative;

                // Extraer la ruta de la imagen
                let img_path = buffer_text[absolute_start + 5..absolute_end].to_string(); // +5 para saltar "[IMG:"

                println!(
                    "DEBUG: Encontrada imagen: {} en posición {}",
                    img_path, absolute_start
                );

                images.push((absolute_start, absolute_end + 1, img_path)); // +1 para incluir ]
                search_pos = absolute_end + 1;
            } else {
                break;
            }
        }

        // Procesar imágenes en orden inverso para no afectar las posiciones
        for (start_byte, end_byte, img_path) in images.into_iter().rev() {
            // Convertir byte offsets a char offsets para GtkTextIter
            let start_char = buffer_text[..start_byte].chars().count();
            let end_char = buffer_text[..end_byte].chars().count();

            // Verificar si se necesita un salto de línea después de la imagen
            // (si no hay uno ya en el texto original)
            let needs_newline = if end_byte < buffer_text.len() {
                !buffer_text[end_byte..].starts_with('\n')
            } else {
                false
            };

            // Crear iteradores usando offsets de caracteres desde el inicio del buffer
            let mut marker_start = self.text_buffer.start_iter();
            marker_start.set_offset(start_char as i32);

            let mut marker_end = self.text_buffer.start_iter();
            marker_end.set_offset(end_char as i32);
            let marker_text = self.text_buffer.text(&marker_start, &marker_end, false);

            // Eliminar el marcador del buffer
            self.text_buffer
                .delete(&mut marker_start.clone(), &mut marker_end.clone());

            // Recrear el iterador de inicio después del delete
            let mut anchor_pos = self.text_buffer.start_iter();
            anchor_pos.set_offset(start_char as i32);

            // Crear anchor en la posición donde estaba el marcador
            let anchor = self.text_buffer.create_child_anchor(&mut anchor_pos);

            // Crear un botón para la imagen (clickeable)
            let image_button = gtk::Button::new();
            image_button.set_can_focus(false);
            image_button.set_focusable(false);
            image_button.add_css_class("flat");
            image_button.set_has_frame(false);

            // Crear widget Picture para la imagen
            let picture = gtk::Picture::new();
            picture.set_can_shrink(true);
            picture.set_size_request(400, 300); // Tamaño máximo por defecto
            picture.set_can_focus(false);
            picture.set_focusable(false);

            // Resolver la ruta de la imagen
            let full_path = if img_path.starts_with("/") || img_path.starts_with("http") {
                img_path.clone()
            } else {
                // Ruta relativa a assets/
                let assets_dir = NotesConfig::assets_dir();
                format!("{}/{}", assets_dir.display(), img_path)
            };

            println!("DEBUG: Cargando imagen desde: {}", full_path);

            // Cargar la imagen
            if std::path::Path::new(&full_path).exists() {
                picture.set_filename(Some(&full_path));
                println!("DEBUG: Imagen cargada exitosamente");
            } else {
                println!("Advertencia: Imagen no encontrada: {}", full_path);
            }

            // Agregar la imagen al botón
            image_button.set_child(Some(&picture));

            // Conectar evento de click solo en modo Normal
            let full_path_clone = full_path.clone();
            let mode_ref = self.mode.clone();
            let main_window = self.main_window.clone();
            let i18n = self.i18n.clone();

            image_button.connect_clicked(move |_| {
                let current_mode = *mode_ref.borrow();
                if current_mode == EditorMode::Normal {
                    // Mostrar diálogo con imagen ampliada
                    show_image_viewer_dialog(&main_window, &full_path_clone, &i18n.borrow());
                }
            });

            // Anclar el botón al TextView
            self.text_view.add_child_at_anchor(&image_button, &anchor);

            if needs_newline {
                // Insertar salto de línea después de la imagen si no existe
                // Esto evita que el texto siguiente quede pegado a la imagen (inline)
                let mut after_anchor = self.text_buffer.start_iter();
                after_anchor.set_offset(start_char as i32 + 1);
                self.text_buffer.insert(&mut after_anchor, "\n");
            }

            // Guardar referencia al widget
            self.image_widgets.borrow_mut().push(picture);
        }
    }

    /// Procesa todos los marcadores de video [VIDEO:video_id] en el buffer completo
    fn process_all_video_markers_in_buffer(&self) {
        // Obtener todo el texto del buffer
        let start = self.text_buffer.start_iter();
        let end = self.text_buffer.end_iter();
        let buffer_text = self.text_buffer.text(&start, &end, false).to_string();

        // Debug: ver si hay marcadores
        if buffer_text.contains("[VIDEO:") {
            println!("DEBUG: Buffer contiene marcadores de video");
        }

        // Buscar todos los marcadores y sus posiciones
        let mut videos = Vec::new();
        let mut search_pos = 0;

        while let Some(video_start) = buffer_text[search_pos..].find("[VIDEO:") {
            let absolute_start = search_pos + video_start;

            // Buscar el cierre ]
            if let Some(video_end_relative) = buffer_text[absolute_start..].find(']') {
                let absolute_end = absolute_start + video_end_relative;

                // Extraer el video_id
                let video_id = buffer_text[absolute_start + 7..absolute_end].to_string(); // +7 para saltar "[VIDEO:"

                println!(
                    "DEBUG: Encontrado marcador de video: {} en posición {}",
                    video_id, absolute_start
                );

                // Convertir offsets de bytes a caracteres para GTK
                let start_char = buffer_text[..absolute_start].chars().count();
                let end_char = buffer_text[..absolute_end + 1].chars().count();

                videos.push((start_char, end_char, video_id));
                search_pos = absolute_end + 1;
            } else {
                break;
            }
        }

        // Procesar videos en orden inverso para no afectar las posiciones
        for (start, end, video_id) in videos.into_iter().rev() {
            // Eliminar el marcador del buffer
            let mut marker_start = self.text_buffer.start_iter();
            marker_start.set_offset(start as i32);
            let mut marker_end = self.text_buffer.start_iter();
            marker_end.set_offset(end as i32);
            self.text_buffer.delete(&mut marker_start, &mut marker_end);

            // Crear anchor en la posición donde estaba el marcador
            let mut anchor_pos = self.text_buffer.start_iter();
            anchor_pos.set_offset(start as i32);
            let anchor = self.text_buffer.create_child_anchor(&mut anchor_pos);

            // Crear contenedor para el video
            let video_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
            video_container.set_margin_top(8);
            video_container.set_margin_bottom(8);
            video_container.set_width_request(640);

            // Crear WebView
            use webkit6::WebView;
            use webkit6::prelude::WebViewExt;

            let webview = WebView::new();
            webview.set_size_request(640, 360);

            // Configurar settings del WebView
            if let Some(settings) = WebViewExt::settings(&webview) {
                settings.set_enable_javascript(true);
                settings.set_enable_media(true);
                settings.set_media_playback_requires_user_gesture(false);
                settings.set_enable_media_stream(true);
                settings.set_enable_webgl(true);
                settings.set_enable_webaudio(true);
                settings.set_enable_write_console_messages_to_stdout(true);
                settings.set_allow_universal_access_from_file_urls(true);
                settings.set_allow_file_access_from_file_urls(true);
                settings.set_javascript_can_access_clipboard(true);
                settings.set_enable_html5_database(true);
                settings.set_enable_html5_local_storage(true);
                settings.set_enable_encrypted_media(true);
                settings.set_enable_media_capabilities(true);
                settings.set_enable_back_forward_navigation_gestures(true);
                settings.set_enable_developer_extras(true);
                settings.set_user_agent(Some("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"));
                settings
                    .set_hardware_acceleration_policy(webkit6::HardwareAccelerationPolicy::Always);
            }

            // Registrar video en el servidor
            let local_url = self.youtube_server.register_video(video_id.clone());

            // Añadir WebView al contenedor
            video_container.append(&webview);

            // Cargar URL de forma asíncrona
            let webview_clone = webview.clone();
            let local_url_clone = local_url.clone();
            glib::idle_add_local_once(move || {
                webview_clone.load_uri(&local_url_clone);
            });

            // Anclar al TextView
            self.text_view
                .add_child_at_anchor(&video_container, &anchor);

            // Guardar referencia
            self.video_widgets.borrow_mut().push(video_container);
        }
    }

    /// Procesa todas las tablas Markdown en el buffer y las reemplaza con WebViews
    fn process_all_tables_in_buffer(&self) {
        // Obtener todo el texto del buffer
        let start = self.text_buffer.start_iter();
        let end = self.text_buffer.end_iter();
        let buffer_text = self.text_buffer.text(&start, &end, false).to_string();

        // Detectar bloques de tablas
        let lines: Vec<&str> = buffer_text.lines().collect();
        let mut table_blocks = Vec::new();
        let mut current_block_start: Option<usize> = None;
        let mut current_block_end: Option<usize> = None;

        // Rastreador de offset de caracteres
        let mut char_offset = 0;

        for line in lines.iter() {
            let trimmed = line.trim();
            // Criterio simple: empieza con |
            let is_table_row = trimmed.starts_with('|');

            let line_len = line.chars().count(); // Longitud en caracteres (no bytes)

            if is_table_row {
                if current_block_start.is_none() {
                    current_block_start = Some(char_offset);
                }
                // El final del bloque se actualiza con cada línea de tabla consecutiva
                current_block_end = Some(char_offset + line_len);
            } else if let (Some(start), Some(end)) = (current_block_start, current_block_end) {
                // Fin de un bloque detectado
                table_blocks.push((start, end));
                current_block_start = None;
                current_block_end = None;
            }

            // +1 por el salto de línea que lines() consume
            char_offset += line_len + 1;
        }

        // Capturar último bloque si termina en EOF
        if let (Some(start), Some(end)) = (current_block_start, current_block_end) {
            table_blocks.push((start, end));
        }

        // Procesar en orden inverso para mantener validez de offsets
        for (start_char, end_char) in table_blocks.into_iter().rev() {
            // Obtener texto del bloque para validación
            let mut iter_start = self.text_buffer.start_iter();
            iter_start.set_offset(start_char as i32);
            let mut iter_end = self.text_buffer.start_iter();
            iter_end.set_offset(end_char as i32);

            let table_md = self
                .text_buffer
                .text(&iter_start, &iter_end, false)
                .to_string();

            // Validación mínima: debe tener separador de cabecera |---|
            if !table_md.contains("|---") && !table_md.contains("| ---") {
                continue;
            }

            // Renderizar HTML usando pulldown-cmark
            let mut options = Options::empty();
            options.insert(Options::ENABLE_TABLES);
            let parser = Parser::new_ext(&table_md, options);
            let mut html_output = String::new();
            html::push_html(&mut html_output, parser);

            // Estilar la tabla para que coincida con el tema oscuro/AI chat
            // Usamos colores hardcoded oscuros por ahora, idealmente deberían venir del tema
            let styled_html = format!(
                r#"
                <html>
                <head>
                <style>
                    body {{ 
                        font-family: 'Segoe UI', sans-serif; 
                        padding: 0; 
                        margin: 0; 
                        background: transparent; 
                        color: #e0e0e0; 
                        font-size: 14px;
                    }}
                    table {{ 
                        border-collapse: collapse; 
                        width: 100%; 
                        border: 1px solid #454545; 
                        background-color: #1e1e1e;
                        border-radius: 4px;
                        overflow: hidden;
                    }}
                    th, td {{ 
                        text-align: left; 
                        padding: 10px 12px; 
                        border: 1px solid #454545; 
                    }}
                    th {{ 
                        background-color: #2d2d2d; 
                        font-weight: 600; 
                        color: #ffffff;
                    }}
                    tr:nth-child(even) {{ 
                        background-color: #252525; 
                    }}
                    tr:hover {{ 
                        background-color: #333333; 
                    }}
                </style>
                </head>
                <body>
                {}
                </body>
                </html>
                "#,
                html_output
            );

            // Eliminar texto original del buffer
            self.text_buffer.delete(&mut iter_start, &mut iter_end);

            // Insertar un salto de línea antes si es necesario (opcional)

            // Crear anchor en la posición
            let mut anchor_pos = self.text_buffer.start_iter();
            anchor_pos.set_offset(start_char as i32);
            let anchor = self.text_buffer.create_child_anchor(&mut anchor_pos);

            // Crear WebView
            use webkit6::WebView;
            use webkit6::prelude::WebViewExt;
            let webview = webkit6::WebView::new();
            webview.load_html(&styled_html, None);

            // Configurar fondo transparente para integrarse con el editor
            webview.set_background_color(&gtk::gdk::RGBA::new(0.0, 0.0, 0.0, 0.0));

            // Contenedor para el WebView
            let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
            // Altura estimada basada en número de líneas (aprox 30px por línea + header)
            let num_lines = table_md.lines().count();
            let estimated_height = (num_lines * 35) + 20;
            container.set_height_request(estimated_height as i32);

            // Obtener el ancho del TextView para hacer la tabla responsiva
            let text_view_width = self.text_view.allocated_width();
            let table_width = if text_view_width > 100 {
                text_view_width - 50
            } else {
                700
            };
            container.set_width_request(table_width);

            webview.set_vexpand(true);
            webview.set_hexpand(true);
            container.append(&webview);

            // Añadir al TextView
            self.text_view.add_child_at_anchor(&container, &anchor);

            // Insertar salto de línea después para separar del siguiente texto
            let mut after_anchor = self.text_buffer.start_iter();
            after_anchor.set_offset(start_char as i32 + 1);
            self.text_buffer.insert(&mut after_anchor, "\n");

            // Guardar referencia
            self.table_widgets.borrow_mut().push(container);
        }
    }
}

/// Procesa todos los enlaces de YouTube de forma asíncrona (función standalone)
fn process_youtube_videos_async_with_spans(
    text_buffer: &gtk::TextBuffer,
    video_spans: Vec<YouTubeVideoSpan>,
    text_view: &gtk::TextView,
    video_widgets: &Rc<RefCell<Vec<gtk::Box>>>,
    youtube_server: &Rc<crate::youtube_server::YouTubeEmbedServer>,
) {
    for video_span in video_spans.iter() {
        let start = video_span.start;
        let end = video_span.end;
        let video_id = &video_span.video_id;

        // Eliminar el texto del enlace [texto](url) del buffer
        let mut start_iter = text_buffer.start_iter();
        start_iter.set_offset(start);
        let mut end_iter = text_buffer.start_iter();
        end_iter.set_offset(end);
        text_buffer.delete(&mut start_iter, &mut end_iter);

        // Insertar salto de línea donde estaba el enlace
        let mut anchor_pos = text_buffer.start_iter();
        anchor_pos.set_offset(start);
        text_buffer.insert(&mut anchor_pos, "\n");

        // Actualizar posición después de la inserción
        anchor_pos.set_offset(start + 1);

        // Crear anchor en la posición donde estaba el marcador
        let anchor = text_buffer.create_child_anchor(&mut anchor_pos);

        // Crear contenedor para el video
        let video_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        video_container.set_margin_top(8);
        video_container.set_margin_bottom(8);
        video_container.set_width_request(640);

        // Crear WebView para embeber el video desde servidor local
        use webkit6::WebView;
        use webkit6::prelude::WebViewExt;

        let webview = WebView::new();
        webview.set_size_request(640, 360); // Tamaño 16:9

        // Configurar settings del WebView con User-Agent de navegador real y permisos máximos
        if let Some(settings) = WebViewExt::settings(&webview) {
            settings.set_enable_javascript(true);
            settings.set_enable_media(true);
            settings.set_media_playback_requires_user_gesture(false);
            settings.set_enable_media_stream(true);
            settings.set_enable_webgl(true);
            settings.set_enable_webaudio(true);
            settings.set_enable_write_console_messages_to_stdout(true);
            settings.set_allow_universal_access_from_file_urls(true);
            settings.set_allow_file_access_from_file_urls(true);
            settings.set_javascript_can_access_clipboard(true);
            settings.set_enable_html5_database(true);
            settings.set_enable_html5_local_storage(true);
            settings.set_enable_encrypted_media(true);
            settings.set_enable_media_capabilities(true);
            settings.set_enable_back_forward_navigation_gestures(true);
            settings.set_enable_developer_extras(true);
            // User-Agent de Chrome/Firefox actual para evitar restricciones
            settings.set_user_agent(Some("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"));
            settings.set_hardware_acceleration_policy(webkit6::HardwareAccelerationPolicy::Always);
        }

        // Registrar el video en el servidor HTTP local
        let local_url = youtube_server.register_video(video_id.clone());

        // Añadir el WebView al contenedor PRIMERO (sin cargar URL aún)
        video_container.append(&webview);

        // Cargar la URL de forma asíncrona usando glib::idle_add para no bloquear la UI
        let webview_clone = webview.clone();
        let local_url_clone = local_url.clone();
        glib::idle_add_local_once(move || {
            // Cargar la URL después de que la UI se haya renderizado
            webview_clone.load_uri(&local_url_clone);
        });

        text_view.add_child_at_anchor(&video_container, &anchor);

        // Insertar salto de línea después del video para separación
        let mut after_anchor = text_buffer.start_iter();
        after_anchor.set_offset(start + 1);
        text_buffer.insert(&mut after_anchor, "\n");

        // Guardar referencia al widget
        video_widgets.borrow_mut().push(video_container);
    }
}

impl MainApp {
    /// Procesa todos los enlaces de YouTube detectados y los embebe con WebKit
    /// (Versión simplificada que delega a la función async)
    fn process_youtube_videos_in_buffer(&self) {
        let video_spans = self.youtube_video_spans.borrow().clone();
        process_youtube_videos_async_with_spans(
            &self.text_buffer,
            video_spans,
            &self.text_view,
            &self.video_widgets,
            &self.youtube_server,
        );
    }

    /// Procesa todos los marcadores de TODO [TODO:unchecked] y [TODO:checked] en el buffer completo
    fn process_all_todos_in_buffer(&self) {
        // Obtener todo el texto del buffer
        let start = self.text_buffer.start_iter();
        let end = self.text_buffer.end_iter();
        let buffer_text = self.text_buffer.text(&start, &end, false).to_string();

        // Obtener el texto original del buffer interno para encontrar las posiciones de TODOs
        let original_text = self.buffer.to_string();

        // Encontrar todas las posiciones de TODOs en el texto ORIGINAL (no renderizado)
        let original_todo_positions = find_all_todos_in_text(&original_text);

        // Buscar todos los marcadores TODO en el buffer renderizado
        let mut todos = Vec::new();
        let buffer_chars: Vec<char> = buffer_text.chars().collect();
        let mut search_pos = 0;

        // Función auxiliar para convertir posición de byte a posición de carácter
        let byte_to_char_pos =
            |byte_pos: usize| -> usize { buffer_text[..byte_pos].chars().count() };

        // Buscar [TODO:unchecked]
        while let Some(todo_start) = buffer_text[search_pos..].find("[TODO:unchecked]") {
            let absolute_start_bytes = search_pos + todo_start;
            let absolute_start_chars = byte_to_char_pos(absolute_start_bytes);
            let marker_len = "[TODO:unchecked]".chars().count();
            todos.push((
                absolute_start_chars,
                absolute_start_chars + marker_len,
                false,
            ));
            search_pos = absolute_start_bytes + "[TODO:unchecked]".len();
        }

        // Buscar [TODO:checked]
        search_pos = 0;
        while let Some(todo_start) = buffer_text[search_pos..].find("[TODO:checked]") {
            let absolute_start_bytes = search_pos + todo_start;
            let absolute_start_chars = byte_to_char_pos(absolute_start_bytes);
            let marker_len = "[TODO:checked]".chars().count();
            todos.push((
                absolute_start_chars,
                absolute_start_chars + marker_len,
                true,
            ));
            search_pos = absolute_start_bytes + "[TODO:checked]".len();
        }

        // Ordenar por posición (de mayor a menor para procesarlos en orden inverso)
        todos.sort_by(|a, b| b.0.cmp(&a.0));

        // Asociar cada marcador con su posición original usando índice
        let mut todo_index = original_todo_positions.len();

        // Procesar TODOs en orden inverso para no afectar las posiciones
        for (start, end, is_checked) in todos {
            todo_index = todo_index.saturating_sub(1);
            // Crear iteradores usando offsets de caracteres desde el inicio del buffer
            let mut marker_start = self.text_buffer.start_iter();
            marker_start.set_offset(start as i32);

            let mut marker_end = self.text_buffer.start_iter();
            marker_end.set_offset(end as i32);

            // Eliminar el marcador del buffer
            self.text_buffer
                .delete(&mut marker_start.clone(), &mut marker_end.clone());

            // Recrear el iterador de inicio después del delete
            let mut anchor_pos = self.text_buffer.start_iter();
            anchor_pos.set_offset(start as i32);

            // Crear anchor en la posición donde estaba el marcador
            let anchor = self.text_buffer.create_child_anchor(&mut anchor_pos);

            // Crear CheckButton para el TODO
            let checkbox = gtk::CheckButton::new();
            checkbox.set_active(is_checked);
            checkbox.set_can_focus(false);
            checkbox.set_focusable(false);

            // Obtener la posición del TODO original usando el índice
            if let Some(&todo_pos) = original_todo_positions.get(todo_index) {
                // Crear variables para el closure
                let mode = self.mode.clone();
                let app_sender = self.app_sender.clone();

                checkbox.connect_toggled(move |cb| {
                    // Solo procesar en modo Normal
                    if *mode.borrow() != EditorMode::Normal {
                        return;
                    }

                    let is_now_checked = cb.is_active();

                    // Enviar mensaje para actualizar el buffer interno
                    if let Some(sender) = app_sender.borrow().as_ref() {
                        sender.input(AppMsg::ToggleTodo {
                            line_number: todo_pos,
                            new_state: is_now_checked,
                        });
                    }
                });
            }

            // Anclar el checkbox al TextView
            self.text_view.add_child_at_anchor(&checkbox, &anchor);

            // Guardar referencia al widget
            self.todo_widgets.borrow_mut().push(checkbox);
        }
    }

    /// Procesa todos los marcadores de REMINDER [REMINDER:params:text] en el buffer completo
    fn process_all_reminder_markers_in_buffer(&self) {
        let mut reminders = Vec::new();
        let mut iter = self.text_buffer.start_iter();

        // Usar forward_search para encontrar los marcadores respetando la estructura del buffer (anchors, etc.)
        while let Some((match_start, match_end)) =
            iter.forward_search("[REMINDER:", gtk::TextSearchFlags::empty(), None)
        {
            // Buscar el cierre ]
            let search_end = match_end.clone();
            if let Some((_, close_end)) =
                search_end.forward_search("]", gtk::TextSearchFlags::empty(), None)
            {
                // Extraer contenido
                // match_end apunta después de "[REMINDER:"
                // close_end apunta después de "]"
                // El contenido está entre match_end y (close_end - 1 char)

                let mut content_end = close_end.clone();
                content_end.backward_char(); // Retroceder sobre ']'

                let marker_content = self
                    .text_buffer
                    .text(&match_end, &content_end, false)
                    .to_string();

                if let Some(pipe_pos) = marker_content.find('|') {
                    let params = &marker_content[..pipe_pos];
                    let text = &marker_content[pipe_pos + 1..];

                    reminders.push((
                        match_start.offset(),
                        close_end.offset(),
                        params.to_string(),
                        text.to_string(),
                    ));
                }

                iter = close_end;
            } else {
                break;
            }
        }

        // Procesar recordatorios en orden inverso para no afectar las posiciones
        for (start, end, params, text) in reminders.into_iter().rev() {
            // Crear iteradores usando offsets de caracteres desde el inicio del buffer
            let mut marker_start = self.text_buffer.start_iter();
            marker_start.set_offset(start);

            let mut marker_end = self.text_buffer.start_iter();
            marker_end.set_offset(end);

            // Eliminar el marcador del buffer
            self.text_buffer
                .delete(&mut marker_start.clone(), &mut marker_end.clone());

            // Recrear el iterador de inicio después del delete
            let mut anchor_pos = self.text_buffer.start_iter();
            anchor_pos.set_offset(start);

            // Crear anchor en la posición donde estaba el marcador
            let anchor = self.text_buffer.create_child_anchor(&mut anchor_pos);

            // Insertar un espacio después del anchor para permitir escribir después del widget
            // El anchor ocupa 1 caracter, así que insertamos en start + 1
            let mut after_anchor = self.text_buffer.start_iter();
            after_anchor.set_offset(start + 1);
            self.text_buffer.insert(&mut after_anchor, " ");

            // Crear widget para el recordatorio (versión inline compacta)
            let reminder_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            reminder_box.add_css_class("reminder-inline-compact");
            reminder_box.set_valign(gtk::Align::Baseline);
            reminder_box.set_margin_top(0);
            reminder_box.set_margin_bottom(0);

            // Icono pequeño
            let icon = gtk::Image::from_icon_name("alarm-symbolic");
            icon.set_pixel_size(14);
            icon.add_css_class("dim-label");
            reminder_box.append(&icon);

            // Texto compacto: mostrar params (fecha/hora/prioridad) y texto si existe
            let display_text = if text.trim().is_empty() {
                // Solo params (fecha/hora/prioridad)
                format!("📅 {}", params.trim())
            } else if params.trim().is_empty() {
                // Solo texto
                text.trim().to_string()
            } else {
                // Ambos: "texto (fecha prioridad)"
                format!("{} (📅 {})", text.trim(), params.trim())
            };

            let label = gtk::Label::new(Some(&display_text));
            label.set_xalign(0.0);
            label.add_css_class("reminder-text-compact");
            reminder_box.append(&label);

            // Anclar el widget al TextView
            self.text_view.add_child_at_anchor(&reminder_box, &anchor);

            // Guardar referencia al widget
            self.reminder_widgets.borrow_mut().push(reminder_box);
        }
    }

    /// Aplica estilos inline dentro de una línea (negrita, cursiva, código, links, tags)
    fn apply_inline_styles(
        &self,
        clean_line: &str,
        original_line: &str,
        line_start: &gtk::TextIter,
        line_offset: i32,
    ) {
        // IMPORTANTE: Detectar tags inline mapeando de línea original a limpia
        self.detect_inline_tags_with_mapping(clean_line, original_line, line_offset);

        // Detectar menciones @ de notas para backlinks
        self.detect_note_mentions(clean_line, line_offset);

        let mut clean_pos = 0;
        let mut orig_pos = 0;
        let mut in_bold = false;
        let mut in_italic = false;
        let mut in_code = false;
        let mut in_link = false;
        let mut link_start_offset: Option<i32> = None;

        let orig_chars: Vec<char> = original_line.chars().collect();
        let clean_chars: Vec<char> = clean_line.chars().collect();

        while orig_pos < orig_chars.len() {
            let ch = orig_chars[orig_pos];

            // Detectar inicio/fin de negrita **
            if ch == '*' && orig_pos + 1 < orig_chars.len() && orig_chars[orig_pos + 1] == '*' {
                in_bold = !in_bold;
                orig_pos += 2;
                continue;
            }

            // Detectar inicio/fin de cursiva *
            if ch == '*' {
                in_italic = !in_italic;
                orig_pos += 1;
                continue;
            }

            // Detectar inicio/fin de código inline `
            if ch == '`' {
                in_code = !in_code;
                orig_pos += 1;
                continue;
            }

            // Detectar links [texto](url)
            if ch == '[' && !in_link {
                in_link = true;
                link_start_offset = Some(line_offset + clean_pos as i32);
                orig_pos += 1;
                continue;
            }

            if ch == ']' && in_link {
                orig_pos += 1;
                let mut url = String::new();
                if orig_pos < orig_chars.len() && orig_chars[orig_pos] == '(' {
                    orig_pos += 1;
                    while orig_pos < orig_chars.len() && orig_chars[orig_pos] != ')' {
                        url.push(orig_chars[orig_pos]);
                        orig_pos += 1;
                    }
                    if orig_pos < orig_chars.len() && orig_chars[orig_pos] == ')' {
                        orig_pos += 1;
                    }
                }
                if let Some(start) = link_start_offset.take() {
                    if !url.is_empty() {
                        let end_offset = line_offset + clean_pos as i32;

                        // Verificar si es un enlace de YouTube
                        if let Some(video_id) = Self::extract_youtube_video_id(&url) {
                            self.youtube_video_spans
                                .borrow_mut()
                                .push(YouTubeVideoSpan {
                                    start,
                                    end: end_offset,
                                    video_id,
                                    url: url.clone(),
                                });
                        }

                        // Siempre guardar como link normal también
                        self.link_spans.borrow_mut().push(LinkSpan {
                            start,
                            end: end_offset,
                            url,
                        });
                    }
                }
                in_link = false;
                continue;
            }

            // El carácter aparece en el texto limpio
            if clean_pos < clean_chars.len() && clean_chars[clean_pos] == ch {
                // Aplicar tags activos
                let mut start_iter = line_start.clone();
                start_iter.forward_chars(clean_pos as i32);
                let mut end_iter = start_iter.clone();
                end_iter.forward_chars(1);

                if in_bold {
                    if let Some(tag) = self.text_buffer.tag_table().lookup("bold") {
                        self.text_buffer.apply_tag(&tag, &start_iter, &end_iter);
                    }
                }

                if in_italic {
                    if let Some(tag) = self.text_buffer.tag_table().lookup("italic") {
                        self.text_buffer.apply_tag(&tag, &start_iter, &end_iter);
                    }
                }

                if in_code {
                    if let Some(tag) = self.text_buffer.tag_table().lookup("code") {
                        self.text_buffer.apply_tag(&tag, &start_iter, &end_iter);
                    }
                }

                if in_link {
                    if let Some(tag) = self.text_buffer.tag_table().lookup("link") {
                        self.text_buffer.apply_tag(&tag, &start_iter, &end_iter);
                    }
                }

                clean_pos += 1;
            }

            orig_pos += 1;
        }
    }

    /// Detecta tags inline (#tag) mapeando posiciones de línea original a línea limpia
    fn detect_inline_tags_with_mapping(
        &self,
        clean_line: &str,
        original_line: &str,
        line_offset: i32,
    ) {
        let orig_chars: Vec<char> = original_line.chars().collect();
        let clean_chars: Vec<char> = clean_line.chars().collect();

        let mut orig_pos = 0;
        let mut clean_pos = 0;

        while orig_pos < orig_chars.len() {
            // Buscar # que esté al inicio o después de espacio/puntuación
            if orig_chars[orig_pos] == '#' {
                let is_tag_start = orig_pos == 0 || {
                    let prev = orig_chars[orig_pos - 1];
                    prev.is_whitespace() || prev == '(' || prev == '[' || prev == ','
                };

                if is_tag_start {
                    let tag_start_orig = orig_pos;
                    let tag_start_clean = clean_pos; // Posición en texto limpio
                    orig_pos += 1;

                    // IMPORTANTE: Si después del # viene un espacio, es un heading, no un tag
                    if orig_pos < orig_chars.len() && orig_chars[orig_pos].is_whitespace() {
                        orig_pos += 1;
                        continue; // Saltar, no es un tag
                    }

                    // Extraer el nombre del tag (letras, números, guiones)
                    let mut tag_name = String::new();
                    let mut tag_chars_count = 0;
                    while orig_pos < orig_chars.len() {
                        let ch = orig_chars[orig_pos];
                        if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                            tag_name.push(ch);
                            tag_chars_count += 1;
                            orig_pos += 1;
                        } else {
                            break;
                        }
                    }

                    // Si encontramos un tag válido, guardarlo con posiciones del texto limpio
                    if !tag_name.is_empty() {
                        // En el texto limpio, el # se mantiene, así que las posiciones son:
                        // start: donde está el # en clean_line
                        // end: start + 1 (el #) + longitud del tag
                        let start_offset = line_offset + tag_start_clean as i32;
                        let end_offset = line_offset + tag_start_clean as i32 + 1 + tag_chars_count;

                        self.tag_spans.borrow_mut().push(TagSpan {
                            start: start_offset,
                            end: end_offset,
                            tag: tag_name,
                        });
                    }
                    continue;
                }
            }

            // Sincronizar posiciones: si el carácter actual aparece en clean_line
            if clean_pos < clean_chars.len() && orig_chars[orig_pos] == clean_chars[clean_pos] {
                clean_pos += 1;
            }
            orig_pos += 1;
        }

        // IMPORTANTE: Detectar también tags en formato YAML de lista (frontmatter con • o -)
        // Buscar líneas como:
        //   • tag
        //   - tag
        // Que son típicas del formato visual del frontmatter parseado
        let trimmed_line = clean_line.trim_start();

        // Detectar "• tag" o "- tag" (lista de tags en frontmatter)
        if trimmed_line.starts_with("• ") || trimmed_line.starts_with("- ") {
            let tag_text = if trimmed_line.starts_with("• ") {
                trimmed_line[3..].trim() // Después de "• " (bullet es 3 bytes UTF-8)
            } else {
                trimmed_line[2..].trim() // Después de "- "
            };

            // Verificar que sea solo una palabra (tag válido, sin espacios)
            if !tag_text.is_empty() && !tag_text.contains(char::is_whitespace) {
                // Calcular offset del tag dentro de la línea
                let bullet_pos = clean_line.find(trimmed_line).unwrap_or(0);
                let tag_start_in_line = if trimmed_line.starts_with("• ") {
                    bullet_pos + 3 // Después de "• " (3 bytes UTF-8)
                } else {
                    bullet_pos + 2 // Después de "- "
                };
                let tag_end_in_line = tag_start_in_line + tag_text.len();

                let start_offset = line_offset + tag_start_in_line as i32;
                let end_offset = line_offset + tag_end_in_line as i32;

                self.tag_spans.borrow_mut().push(TagSpan {
                    start: start_offset,
                    end: end_offset,
                    tag: tag_text.to_string(),
                });
            }
        }
    }

    /// Detecta tags inline (#tag) en el texto y los almacena
    fn detect_inline_tags(&self, line: &str, line_offset: i32) {
        let chars: Vec<char> = line.chars().collect();
        let mut pos = 0;

        while pos < chars.len() {
            // Buscar # que esté al inicio o después de espacio/puntuación
            if chars[pos] == '#' {
                let is_tag_start = pos == 0 || {
                    let prev = chars[pos - 1];
                    prev.is_whitespace() || prev == '(' || prev == '[' || prev == ','
                };

                if is_tag_start {
                    let tag_start = pos;
                    pos += 1;

                    // IMPORTANTE: Si después del # viene un espacio, es un heading, no un tag
                    if pos < chars.len() && chars[pos].is_whitespace() {
                        pos += 1;
                        continue; // Saltar, no es un tag
                    }

                    // Extraer el nombre del tag (letras, números, guiones)
                    let mut tag_name = String::new();
                    while pos < chars.len() {
                        let ch = chars[pos];
                        if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                            tag_name.push(ch);
                            pos += 1;
                        } else {
                            break;
                        }
                    }

                    // Si encontramos un tag válido, guardarlo
                    if !tag_name.is_empty() {
                        let start_offset = line_offset + tag_start as i32;
                        let end_offset = line_offset + pos as i32;

                        self.tag_spans.borrow_mut().push(TagSpan {
                            start: start_offset,
                            end: end_offset,
                            tag: tag_name,
                        });
                    }
                    continue;
                }
            }
            pos += 1;
        }

        // Detectar también tags en formato YAML de lista (frontmatter con • o -)
        // Buscar líneas como:
        //   • tag
        //   - tag
        // Que son típicas del formato visual del frontmatter parseado
        let trimmed_line = line.trim_start();

        // Detectar "• tag" o "- tag" (lista de tags en frontmatter)
        if trimmed_line.starts_with("• ") || trimmed_line.starts_with("- ") {
            let tag_text = if trimmed_line.starts_with("• ") {
                trimmed_line[3..].trim() // Después de "• " (bullet es 3 bytes UTF-8)
            } else {
                trimmed_line[2..].trim() // Después de "- "
            };

            // Verificar que sea solo una palabra (tag válido, sin espacios)
            if !tag_text.is_empty() && !tag_text.contains(char::is_whitespace) {
                // Calcular offset del tag dentro de la línea
                let bullet_pos = line.find(trimmed_line).unwrap_or(0);
                let tag_start_in_line = if trimmed_line.starts_with("• ") {
                    bullet_pos + 3 // Después de "• " (3 bytes UTF-8)
                } else {
                    bullet_pos + 2 // Después de "- "
                };
                let tag_end_in_line = tag_start_in_line + tag_text.len();

                let start_offset = line_offset + tag_start_in_line as i32;
                let end_offset = line_offset + tag_end_in_line as i32;

                self.tag_spans.borrow_mut().push(TagSpan {
                    start: start_offset,
                    end: end_offset,
                    tag: tag_text.to_string(),
                });
            }
        }
    }

    /// Detecta menciones @ de notas para backlinks
    fn detect_note_mentions(&self, line: &str, line_offset: i32) {
        let chars: Vec<char> = line.chars().collect();
        let mut pos = 0;

        while pos < chars.len() {
            // Buscar @ que esté al inicio o después de espacio/puntuación
            if chars[pos] == '@' {
                let is_mention_start = pos == 0 || {
                    let prev = chars[pos - 1];
                    prev.is_whitespace() || prev == '(' || prev == '[' || prev == ','
                };

                if is_mention_start {
                    let mention_start = pos;
                    pos += 1;

                    // Extraer el nombre de la nota (letras, números, guiones, espacios, barras)
                    let mut note_name = String::new();
                    while pos < chars.len() {
                        let ch = chars[pos];
                        // Permitir caracteres alfanuméricos, guiones, guiones bajos, espacios y barras (para carpetas)
                        if ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' || ch == '/'
                        {
                            note_name.push(ch);
                            pos += 1;
                        } else {
                            break;
                        }
                    }

                    // Limpiar espacios finales
                    let note_name = note_name.trim_end().to_string();

                    // Si encontramos una mención válida, guardarla
                    if !note_name.is_empty() {
                        let start_offset = line_offset + mention_start as i32;
                        let end_offset = line_offset
                            + mention_start as i32
                            + 1
                            + note_name.chars().count() as i32;

                        self.note_mention_spans.borrow_mut().push(NoteMentionSpan {
                            start: start_offset,
                            end: end_offset,
                            note_name,
                        });
                    }
                    continue;
                }
            }

            pos += 1;
        }
    }

    fn create_text_tags(&self) {
        let tag_table = self.text_buffer.tag_table();

        // Heading 1 - Más grande y en negrita (sin forzar colores)
        let h1_tag = gtk::TextTag::new(Some("h1"));
        h1_tag.set_weight(800);
        h1_tag.set_scale(1.8);
        tag_table.add(&h1_tag);

        // Heading 2
        let h2_tag = gtk::TextTag::new(Some("h2"));
        h2_tag.set_weight(700);
        h2_tag.set_scale(1.5);
        tag_table.add(&h2_tag);

        // Heading 3
        let h3_tag = gtk::TextTag::new(Some("h3"));
        h3_tag.set_weight(700);
        h3_tag.set_scale(1.25);
        tag_table.add(&h3_tag);

        // Bold
        let bold_tag = gtk::TextTag::new(Some("bold"));
        bold_tag.set_weight(700);
        tag_table.add(&bold_tag);

        // Italic
        let italic_tag = gtk::TextTag::new(Some("italic"));
        italic_tag.set_style(gtk::pango::Style::Italic);
        tag_table.add(&italic_tag);

        // Code inline - fondo del tema
        let code_tag = gtk::TextTag::new(Some("code"));
        code_tag.set_family(Some("monospace"));
        code_tag.set_size_points(10.0);
        tag_table.add(&code_tag);

        // Code block
        let codeblock_tag = gtk::TextTag::new(Some("codeblock"));
        codeblock_tag.set_family(Some("monospace"));
        codeblock_tag.set_left_margin(20);
        codeblock_tag.set_size_points(10.0);
        tag_table.add(&codeblock_tag);

        // Link - subrayado, color del tema
        let link_tag = gtk::TextTag::new(Some("link"));
        link_tag.set_underline(gtk::pango::Underline::Single);
        tag_table.add(&link_tag);

        // Lista - con margen
        let list_tag = gtk::TextTag::new(Some("list"));
        list_tag.set_left_margin(20);
        tag_table.add(&list_tag);

        // Blockquote - cursiva y margen
        let blockquote_tag = gtk::TextTag::new(Some("blockquote"));
        blockquote_tag.set_style(gtk::pango::Style::Italic);
        blockquote_tag.set_left_margin(20);
        tag_table.add(&blockquote_tag);

        // Aplicar colores del tema
        self.update_text_tag_colors();
    }

    fn update_text_tag_colors(&self) {
        let tag_table = self.text_buffer.tag_table();

        // Intentar obtener los colores del tema actual
        // Parseamos el CSS cargado para extraer las variables
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
        let theme_dir = format!("{}/.config/omarchy/current/theme", home_dir);

        // Leer walker.css que tiene las variables de color
        let walker_css_path = format!("{}/walker.css", theme_dir);
        let theme_colors = if let Ok(content) = std::fs::read_to_string(&walker_css_path) {
            Self::parse_theme_colors(&content)
        } else {
            // Valores por defecto si no se puede leer el tema
            ThemeColors::default()
        };

        // Actualizar el tag de código inline
        if let Some(code_tag) = tag_table.lookup("code") {
            code_tag.set_background_rgba(Some(&theme_colors.code_bg));
        }

        // Actualizar el tag de bloque de código
        if let Some(codeblock_tag) = tag_table.lookup("codeblock") {
            codeblock_tag.set_background_rgba(Some(&theme_colors.code_bg));
        }

        // Actualizar el tag de link
        if let Some(link_tag) = tag_table.lookup("link") {
            link_tag.set_foreground_rgba(Some(&theme_colors.link_color));
        }
    }

    fn parse_theme_colors(css_content: &str) -> ThemeColors {
        let mut colors = ThemeColors::default();

        // Buscar @define-color selected-text #RRGGBB;
        if let Some(selected_text) = Self::extract_color(css_content, "selected-text") {
            colors.link_color = selected_text;
        }

        // Buscar @define-color border #RRGGBB; para el fondo de código
        if let Some(border) = Self::extract_color(css_content, "border") {
            // Usar el color del borde con transparencia para el fondo de código
            colors.code_bg = gtk::gdk::RGBA::new(
                border.red(),
                border.green(),
                border.blue(),
                0.15, // Transparencia
            );
        }

        colors
    }

    fn extract_color(css_content: &str, var_name: &str) -> Option<gtk::gdk::RGBA> {
        // Buscar líneas como: @define-color selected-text #7EBAE4;
        let pattern = format!("@define-color {} ", var_name);

        for line in css_content.lines() {
            let line = line.trim();
            if line.starts_with(&pattern) {
                // Extraer el valor del color (después del nombre de la variable)
                if let Some(color_start) = line.find('#') {
                    let color_str = &line[color_start..];
                    // Tomar hasta el punto y coma
                    let color_hex = color_str.split(';').next().unwrap_or("").trim();

                    // Parsear color hex #RRGGBB
                    if color_hex.len() == 7 {
                        if let (Ok(r), Ok(g), Ok(b)) = (
                            u8::from_str_radix(&color_hex[1..3], 16),
                            u8::from_str_radix(&color_hex[3..5], 16),
                            u8::from_str_radix(&color_hex[5..7], 16),
                        ) {
                            return Some(gtk::gdk::RGBA::new(
                                r as f32 / 255.0,
                                g as f32 / 255.0,
                                b as f32 / 255.0,
                                1.0,
                            ));
                        }
                    }
                }
            }
        }
        None
    }

    fn apply_markdown_styles(&self) {
        // Primero remover todos los tags existentes
        let start = self.text_buffer.start_iter();
        let end = self.text_buffer.end_iter();
        self.text_buffer.remove_all_tags(&start, &end);

        let text = self.buffer.to_string();
        let parser = MarkdownParser::new(text.clone());
        let styles = parser.parse();

        for style in styles {
            // Convertir byte offset a char offset
            let char_start = text[..style.start.min(text.len())].chars().count();
            let char_end = text[..style.end.min(text.len())].chars().count();

            let mut start_iter = self.text_buffer.start_iter();
            start_iter.set_offset(char_start as i32);

            let mut end_iter = self.text_buffer.start_iter();
            end_iter.set_offset(char_end as i32);

            let tag_name = match &style.style_type {
                StyleType::Heading1 => "h1",
                StyleType::Heading2 => "h2",
                StyleType::Heading3 => "h3",
                StyleType::Bold => "bold",
                StyleType::Italic => "italic",
                StyleType::Code => "code",
                StyleType::CodeBlock => "codeblock",
                StyleType::Image { .. } => {
                    // Las imágenes se manejan con widgets anclados, no con tags de texto
                    continue;
                }
                _ => continue,
            };

            if let Some(tag) = self.text_buffer.tag_table().lookup(tag_name) {
                self.text_buffer.apply_tag(&tag, &start_iter, &end_iter);
            }
        }
    }

    fn update_status_bar(&self, _sender: &ComponentSender<Self>) {
        let i18n = self.i18n.borrow();
        let line_count = self.buffer.len_lines();
        let word_count = self.buffer.to_string().split_whitespace().count();
        let current_mode = *self.mode.borrow();

        // Actualizar etiqueta de modo
        let mode_text = match current_mode {
            EditorMode::Normal => "<b>NORMAL</b>",
            EditorMode::Insert => "<b>INSERT</b>",
            EditorMode::Command => "<b>COMMAND</b>",
            EditorMode::Visual => "<b>VISUAL</b>",
            EditorMode::ChatAI => "<b>CHAT AI</b>",
        };
        self.mode_label.set_markup(mode_text);

        // Actualizar estadísticas con indicador de cambios sin guardar
        let unsaved_indicator = if self.has_unsaved_changes { " •" } else { "" };
        self.stats_label.set_label(&format!(
            "{} {} | {} {}{}",
            line_count,
            i18n.t("lines"),
            word_count,
            i18n.t("words"),
            unsaved_indicator
        ));

        // Actualizar título de ventana con nombre de nota, carpeta e indicador de cambios
        let title = if let Some(note) = &self.current_note {
            let modified_marker = if self.has_unsaved_changes { "● " } else { "" };

            // Obtener la carpeta relativa si existe
            let folder_str = note
                .path()
                .strip_prefix(self.notes_dir.root())
                .ok()
                .and_then(|p| p.parent())
                .and_then(|p| p.to_str())
                .filter(|s| !s.is_empty());

            let display_name = if let Some(folder) = folder_str {
                // Si el nombre de la nota ya incluye la carpeta (ej: "Carpeta/Nota"),
                // extraer solo el nombre base para evitar duplicación
                if let Some(stripped) = note.name().strip_prefix(&format!("{}/", folder)) {
                    format!("{} / {}", folder, stripped)
                } else {
                    format!("{} / {}", folder, note.name())
                }
            } else {
                note.name().to_string()
            };

            format!("{}{}", modified_marker, display_name)
        } else {
            i18n.t("untitled")
        };
        self.window_title.set_text(&title);

        println!(
            "Modo: {:?} | {} {} | {} {}",
            current_mode,
            line_count,
            i18n.t("lines"),
            word_count,
            i18n.t("words")
        );

        // Actualizar tags se hace en RefreshTags para tener acceso al sender
    }

    fn refresh_tags_display_with_sender(&self, sender: &ComponentSender<Self>) {
        // Limpiar tags actuales
        while let Some(row) = self.tags_list_box.row_at_index(0) {
            self.tags_list_box.remove(&row);
        }

        // Obtener todos los tags (frontmatter + inline)
        if let Some(ref _note) = self.current_note {
            let content = self.buffer.to_string();
            let all_tags = extract_all_tags(&content);

            if all_tags.is_empty() {
                // Mostrar mensaje si no hay tags
                let empty_label = gtk::Label::new(Some("No hay tags"));
                empty_label.add_css_class("dim-label");
                empty_label.set_margin_all(8);
                self.tags_list_box.append(&empty_label);
            } else {
                // Crear row para cada tag
                for tag in &all_tags {
                    let row = self.create_tag_row(tag, sender);
                    self.tags_list_box.append(&row);
                }
            }
        }
    }

    fn create_tag_row(&self, tag: &str, sender: &ComponentSender<Self>) -> gtk::Box {
        let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row_box.set_margin_all(4);

        // Crear un botón en lugar de label para hacerlo clickeable
        let tag_button = gtk::Button::new();
        tag_button.set_label(&format!("#{}", tag));
        tag_button.set_halign(gtk::Align::Start);
        tag_button.set_hexpand(true);
        tag_button.add_css_class("flat");
        tag_button.set_tooltip_text(Some("Buscar notas con este tag"));

        // Conectar evento para buscar el tag
        let tag_for_search = tag.to_string();
        tag_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                // Abrir barra flotante y buscar el tag
                sender.input(AppMsg::SearchNotes(format!("#{}", tag_for_search)));
            }
        ));

        let remove_button = gtk::Button::new();
        remove_button.set_icon_name("user-trash-symbolic");
        remove_button.add_css_class("flat");
        remove_button.add_css_class("circular");
        remove_button.set_tooltip_text(Some("Eliminar tag"));

        // Conectar evento para eliminar tag
        let tag_clone = tag.to_string();
        remove_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::RemoveTag(tag_clone.clone()));
            }
        ));

        row_box.append(&tag_button);
        row_box.append(&remove_button);

        row_box
    }

    fn refresh_tags_display(&self) {
        // Versión sin sender - simplemente limpia
        while let Some(row) = self.tags_list_box.row_at_index(0) {
            self.tags_list_box.remove(&row);
        }
    }

    fn refresh_todos_summary(&self) {
        // Limpiar lista anterior
        while let Some(row) = self.todos_list_box.row_at_index(0) {
            self.todos_list_box.remove(&row);
        }

        // Obtener el texto del buffer
        let text = self.buffer.to_string();

        // Analizar TODOs agrupados por sección
        let todo_sections = self.analyze_todos_by_section(&text);

        if todo_sections.is_empty() {
            let i18n = self.i18n.borrow();
            let empty_label = gtk::Label::new(Some(&i18n.t("no_todos")));
            empty_label.add_css_class("dim-label");
            empty_label.set_margin_all(8);
            self.todos_list_box.append(&empty_label);
            return;
        }

        // Mostrar cada sección con su resumen
        let i18n = self.i18n.borrow();
        for section in todo_sections {
            let section_box = self.create_todo_section_row(&section, &i18n);
            self.todos_list_box.append(&section_box);
        }
    }

    fn create_todo_section_row(&self, section: &TodoSection, i18n: &I18n) -> gtk::Box {
        // Box principal vertical (igual margen que tags)
        let row_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        row_box.set_margin_all(4);

        // Título de la sección
        let title_label = gtk::Label::new(Some(&section.title));
        title_label.set_xalign(0.0);
        title_label.set_wrap(true);
        title_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        title_label.set_max_width_chars(30);
        title_label.set_markup(&format!(
            "<b>{}</b>",
            glib::markup_escape_text(&section.title)
        ));
        row_box.append(&title_label);

        // Calcular estadísticas de subtareas
        let main_tasks = section.todos.iter().filter(|t| t.indent_level == 0).count();
        let main_completed = section
            .todos
            .iter()
            .filter(|t| t.indent_level == 0 && t.completed)
            .count();
        let subtasks = section.todos.iter().filter(|t| t.indent_level > 0).count();
        let subtasks_completed = section
            .todos
            .iter()
            .filter(|t| t.indent_level > 0 && t.completed)
            .count();

        // Progreso y porcentaje en una sola línea
        let progress_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        progress_box.set_margin_top(2);

        let progress_text = if subtasks > 0 {
            format!(
                "{}/{} {} · {}/{} subtareas",
                section.completed,
                section.total,
                i18n.t("completed"),
                subtasks_completed,
                subtasks
            )
        } else {
            format!(
                "{}/{} {}",
                section.completed,
                section.total,
                i18n.t("completed")
            )
        };

        let progress_label = gtk::Label::new(Some(&progress_text));
        progress_label.set_xalign(0.0);
        progress_label.add_css_class("dim-label");
        progress_label.set_hexpand(true);
        progress_box.append(&progress_label);

        let percentage_label = gtk::Label::new(Some(&format!("{}%", section.percentage)));
        percentage_label.set_xalign(1.0);

        // Usar clases CSS estándar de GTK según el porcentaje
        if section.percentage == 100 {
            percentage_label.add_css_class("success");
        } else if section.percentage >= 70 {
            percentage_label.add_css_class("warning");
        }

        progress_box.append(&percentage_label);
        row_box.append(&progress_box);

        // Barra de progreso visual
        let progress_bar = gtk::ProgressBar::new();
        progress_bar.set_fraction(section.percentage as f64 / 100.0);
        progress_bar.set_margin_top(2);
        progress_bar.set_show_text(false);
        row_box.append(&progress_bar);

        // Separar tareas pendientes y completadas
        let pending_todos: Vec<&TodoItem> = section.todos.iter().filter(|t| !t.completed).collect();
        let completed_todos: Vec<&TodoItem> =
            section.todos.iter().filter(|t| t.completed).collect();

        // Lista de TODOs individuales con indentación y líneas de conexión
        let todos_container = gtk::Box::new(gtk::Orientation::Vertical, 2);
        todos_container.set_margin_top(4);

        println!(
            "DEBUG: Mostrando {} TODOs para sección '{}'",
            section.todos.len(),
            section.title
        );

        // Mostrar primero las tareas pendientes
        for (index, todo) in pending_todos.iter().enumerate() {
            // Box horizontal que contendrá la línea de conexión y el contenido del TODO
            let todo_wrapper = gtk::Box::new(gtk::Orientation::Horizontal, 0);

            // Si es una subtarea, agregar línea de conexión visual
            if todo.indent_level > 0 {
                // Crear box para las líneas de conexión
                let line_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                let base_indent = 4; // Margen base

                // Agregar espaciado para cada nivel de indentación
                for level in 0..todo.indent_level {
                    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                    spacer.set_width_request(12); // 12px por nivel
                    line_box.append(&spacer);
                }

                // Línea vertical y horizontal (carácter de árbol)
                let tree_char = gtk::Label::new(Some("└─"));
                tree_char.add_css_class("dim-label");
                tree_char.set_xalign(0.0);
                line_box.append(&tree_char);

                todo_wrapper.append(&line_box);
            } else {
                // Para tareas principales, solo agregar margen
                let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                spacer.set_width_request(4);
                todo_wrapper.append(&spacer);
            }

            // Contenido del TODO
            let todo_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            todo_row.set_hexpand(false); // No expandir

            // Icono de checkbox
            let icon_name = if todo.completed {
                "checkbox-checked-symbolic"
            } else {
                "checkbox-symbolic"
            };
            let checkbox_icon = gtk::Image::from_icon_name(icon_name);
            checkbox_icon.set_pixel_size(12);
            todo_row.append(&checkbox_icon);

            // Texto de la tarea (truncado si es muy largo)
            let text_label = gtk::Label::new(Some(&todo.text));
            text_label.set_xalign(0.0);
            text_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            text_label.set_max_width_chars(22); // Reducido más para compensar indentación
            text_label.set_wrap(false);
            text_label.set_width_request(180); // Ancho fijo reducido
            text_label.set_hexpand(false); // No expandir
            text_label.add_css_class("dim-label");

            // Si está completado, agregar estilo tachado
            if todo.completed {
                text_label.set_markup(&format!("<s>{}</s>", glib::markup_escape_text(&todo.text)));
            }

            todo_row.append(&text_label);
            todo_wrapper.append(&todo_row);

            todos_container.append(&todo_wrapper);
        }

        // Si hay tareas completadas, agregar sección colapsable
        if !completed_todos.is_empty() {
            // Crear revealer para las tareas completadas
            let completed_revealer = gtk::Revealer::new();
            completed_revealer.set_reveal_child(false); // Oculto por defecto
            completed_revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
            completed_revealer.set_transition_duration(150); // Reducido a 150ms para más suavidad

            // Container para las tareas completadas
            let completed_container = gtk::Box::new(gtk::Orientation::Vertical, 2);

            for todo in completed_todos.iter() {
                let todo_wrapper = gtk::Box::new(gtk::Orientation::Horizontal, 0);

                // Si es una subtarea, agregar línea de conexión visual
                if todo.indent_level > 0 {
                    let line_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);

                    for level in 0..todo.indent_level {
                        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                        spacer.set_width_request(12);
                        line_box.append(&spacer);
                    }

                    let tree_char = gtk::Label::new(Some("└─"));
                    tree_char.add_css_class("dim-label");
                    tree_char.set_xalign(0.0);
                    line_box.append(&tree_char);

                    todo_wrapper.append(&line_box);
                } else {
                    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                    spacer.set_width_request(4);
                    todo_wrapper.append(&spacer);
                }

                let todo_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                todo_row.set_hexpand(false); // No expandir

                let checkbox_icon = gtk::Image::from_icon_name("checkbox-checked-symbolic");
                checkbox_icon.set_pixel_size(12);
                todo_row.append(&checkbox_icon);

                let text_label = gtk::Label::new(Some(&todo.text));
                text_label.set_xalign(0.0);
                text_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                text_label.set_max_width_chars(22); // Reducido más para compensar indentación
                text_label.set_wrap(false);
                text_label.set_width_request(180); // Ancho fijo reducido
                text_label.set_hexpand(false); // No expandir
                text_label.add_css_class("dim-label");
                text_label.set_markup(&format!("<s>{}</s>", glib::markup_escape_text(&todo.text)));

                todo_row.append(&text_label);
                todo_wrapper.append(&todo_row);
                completed_container.append(&todo_wrapper);
            }

            completed_revealer.set_child(Some(&completed_container));

            // Botón para expandir/colapsar tareas completadas
            let toggle_button = gtk::Button::new();
            toggle_button.add_css_class("flat");
            toggle_button.set_margin_top(4);

            let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            button_box.set_halign(gtk::Align::Start);

            let icon = gtk::Image::from_icon_name("pan-end-symbolic");
            icon.set_pixel_size(12);
            button_box.append(&icon);

            let label = gtk::Label::new(Some(&format!("{} completadas", completed_todos.len())));
            label.add_css_class("dim-label");
            button_box.append(&label);

            toggle_button.set_child(Some(&button_box));

            // Conectar señal para expandir/colapsar
            let revealer_clone = completed_revealer.clone();
            let icon_clone = icon.clone();
            toggle_button.connect_clicked(move |_| {
                let is_revealed = revealer_clone.reveals_child();
                revealer_clone.set_reveal_child(!is_revealed);

                // Cambiar icono
                if is_revealed {
                    icon_clone.set_icon_name(Some("pan-end-symbolic"));
                } else {
                    icon_clone.set_icon_name(Some("pan-down-symbolic"));
                }
            });

            todos_container.append(&toggle_button);
            todos_container.append(&completed_revealer);
        }

        row_box.append(&todos_container);

        row_box
    }

    fn analyze_todos_by_section(&self, text: &str) -> Vec<TodoSection> {
        let lines: Vec<&str> = text.lines().collect();
        let mut sections = Vec::new();
        let i18n = self.i18n.borrow();
        let mut current_section = i18n.t("no_section");
        let mut current_todos: Vec<TodoItem> = Vec::new();

        for line in lines {
            // Detectar encabezados (h1, h2, h3)
            if line.starts_with("# ") {
                // Guardar sección anterior si tiene TODOs
                if !current_todos.is_empty() {
                    sections.push(self.create_todo_section(&current_section, &current_todos));
                    current_todos.clear();
                }
                current_section = line.trim_start_matches('#').trim().to_string();
            } else if line.starts_with("## ") {
                if !current_todos.is_empty() {
                    sections.push(self.create_todo_section(&current_section, &current_todos));
                    current_todos.clear();
                }
                current_section = line.trim_start_matches('#').trim().to_string();
            } else if line.starts_with("### ") {
                if !current_todos.is_empty() {
                    sections.push(self.create_todo_section(&current_section, &current_todos));
                    current_todos.clear();
                }
                current_section = line.trim_start_matches('#').trim().to_string();
            }

            // Detectar TODOs con indentación
            // Contar espacios al inicio para determinar nivel de indentación
            let leading_spaces = line.chars().take_while(|c| *c == ' ').count();
            let indent_level = leading_spaces / 2; // 2 espacios = 1 nivel de indentación

            let trimmed = line.trim_start();
            if trimmed.starts_with("- [ ]") {
                let text = trimmed[5..].trim().to_string();
                current_todos.push(TodoItem {
                    completed: false,
                    indent_level,
                    text,
                });
            } else if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
                let text = trimmed[5..].trim().to_string();
                current_todos.push(TodoItem {
                    completed: true,
                    indent_level,
                    text,
                });
            }
        }

        // Agregar última sección si tiene TODOs
        if !current_todos.is_empty() {
            sections.push(self.create_todo_section(&current_section, &current_todos));
        }

        sections
    }

    fn create_todo_section(&self, title: &str, todos: &[TodoItem]) -> TodoSection {
        let total = todos.len();
        let completed = todos.iter().filter(|todo| todo.completed).count();
        let percentage = if total > 0 {
            (completed * 100) / total
        } else {
            0
        };

        TodoSection {
            title: title.to_string(),
            todos: todos.to_vec(),
            total,
            completed,
            percentage,
        }
    }

    fn show_tag_suggestions(&self, prefix: &str, sender: &ComponentSender<Self>) {
        // Limpiar sugerencias anteriores
        while let Some(row) = self.tag_completion_list.row_at_index(0) {
            self.tag_completion_list.remove(&row);
        }

        // Buscar tags que coincidan
        if let Ok(all_tags) = self.notes_db.get_tags() {
            let matches: Vec<_> = all_tags
                .iter()
                .filter(|t| t.name.to_lowercase().starts_with(prefix))
                .take(5) // Limitar a 5 sugerencias
                .collect();

            if matches.is_empty() {
                self.tag_completion_popup.popdown();
                return;
            }

            // Añadir cada sugerencia
            for tag in matches {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                row.set_margin_all(8);

                let label = gtk::Label::new(Some(&format!("#{}", tag.name)));
                label.set_xalign(0.0);
                label.set_hexpand(true);

                let count_label = gtk::Label::new(Some(&format!("({})", tag.usage_count)));
                count_label.add_css_class("dim-label");

                row.append(&label);
                row.append(&count_label);

                // Crear ListBoxRow en lugar de Button
                let list_row = gtk::ListBoxRow::new();
                list_row.set_child(Some(&row));
                list_row.set_activatable(true);

                let tag_name = tag.name.clone();
                let gesture = gtk::GestureClick::new();
                gesture.connect_released(gtk::glib::clone!(
                    #[strong]
                    sender,
                    move |_, _, _, _| {
                        sender.input(AppMsg::CompleteTag(tag_name.clone()));
                    }
                ));
                list_row.add_controller(gesture);

                self.tag_completion_list.append(&list_row);
                list_row.show(); // Forzar visibilidad inmediata
            }

            // Posicionar el popover cerca del cursor
            let cursor_mark = self.text_buffer.get_insert();
            let cursor_iter = self.text_buffer.iter_at_mark(&cursor_mark);
            let cursor_rect = self.text_view.iter_location(&cursor_iter);

            // Convertir coordenadas del buffer a coordenadas de la ventana
            let (window_x, window_y) = self.text_view.buffer_to_window_coords(
                gtk::TextWindowType::Widget,
                cursor_rect.x(),
                cursor_rect.y() + cursor_rect.height(),
            );

            let rect = gtk::gdk::Rectangle::new(window_x, window_y, 1, 1);
            self.tag_completion_popup.set_pointing_to(Some(&rect));
            self.tag_completion_popup.popup();
        }
    }

    fn show_note_mention_suggestions(&self, prefix: &str, sender: &ComponentSender<Self>) {
        println!(
            "DEBUG: show_note_mention_suggestions llamado con prefix: '{}'",
            prefix
        );

        // Limpiar sugerencias anteriores
        while let Some(row) = self.note_mention_list.row_at_index(0) {
            self.note_mention_list.remove(&row);
        }

        // Obtener todas las notas y filtrar las que coincidan
        if let Ok(notes) = self.notes_dir.list_notes() {
            println!("DEBUG: Total de notas disponibles: {}", notes.len());

            let matches: Vec<_> = notes
                .iter()
                .filter(|note| {
                    let note_name = note.name().to_lowercase();
                    // Buscar coincidencias en el nombre de la nota (sin la carpeta)
                    let base_name = if let Some(idx) = note_name.rfind('/') {
                        &note_name[idx + 1..]
                    } else {
                        &note_name
                    };
                    base_name.contains(prefix)
                })
                .take(8) // Limitar a 8 sugerencias
                .collect();

            println!("DEBUG: Notas que coinciden: {}", matches.len());

            if matches.is_empty() {
                println!("DEBUG: No hay coincidencias, cerrando popup");
                self.note_mention_popup.popdown();
                return;
            }

            // Añadir cada sugerencia
            for note in matches {
                let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
                row.set_margin_all(8);

                // Nombre de la nota (sin extensión .md)
                let note_name = note.name();
                let note_name_no_ext = note_name.trim_end_matches(".md");

                // Extraer solo el nombre base (sin carpeta) para mostrar
                let display_name = if let Some(idx) = note_name_no_ext.rfind('/') {
                    &note_name_no_ext[idx + 1..]
                } else {
                    note_name_no_ext
                };

                let label = gtk::Label::new(Some(display_name));
                label.set_xalign(0.0);
                label.set_hexpand(true);
                label.set_visible(true);
                label.add_css_class("heading");

                // Mostrar la carpeta si existe
                if let Some(folder_idx) = note_name.rfind('/') {
                    let folder = &note_name[..folder_idx];
                    let folder_label = gtk::Label::new(Some(&format!("📁 {}", folder)));
                    folder_label.set_xalign(0.0);
                    folder_label.set_visible(true);
                    folder_label.add_css_class("dim-label");
                    folder_label.add_css_class("caption");
                    row.append(&folder_label);
                }

                row.prepend(&label);
                row.set_visible(true);

                // Crear un ListBoxRow en lugar de usar Button directamente
                let list_row = gtk::ListBoxRow::new();
                list_row.set_child(Some(&row));
                list_row.set_activatable(true);
                list_row.set_visible(true);

                // Guardar el nombre completo (con carpeta) para la mención
                let note_name_for_mention = note_name_no_ext.to_string();

                // Usar gesture click en el row
                let gesture = gtk::GestureClick::new();
                gesture.connect_released(gtk::glib::clone!(
                    #[strong]
                    sender,
                    move |_, _, _, _| {
                        println!(
                            "DEBUG: Click en row, enviando CompleteMention({})",
                            note_name_for_mention
                        );
                        sender.input(AppMsg::CompleteMention(note_name_for_mention.clone()));
                    }
                ));
                list_row.add_controller(gesture);

                println!("DEBUG: Agregando sugerencia de nota: {}", display_name);
                self.note_mention_list.append(&list_row);
                list_row.show(); // Forzar visibilidad inmediata
            }

            // Posicionar el popover cerca del cursor
            let cursor_mark = self.text_buffer.get_insert();
            let cursor_iter = self.text_buffer.iter_at_mark(&cursor_mark);
            let cursor_rect = self.text_view.iter_location(&cursor_iter);

            // Convertir coordenadas del buffer a coordenadas de la ventana
            let (window_x, window_y) = self.text_view.buffer_to_window_coords(
                gtk::TextWindowType::Widget,
                cursor_rect.x(),
                cursor_rect.y() + cursor_rect.height(),
            );

            let rect = gtk::gdk::Rectangle::new(window_x, window_y, 1, 1);
            self.note_mention_popup.set_pointing_to(Some(&rect));
            println!(
                "DEBUG: Mostrando popup en posición ({}, {})",
                window_x, window_y
            );
            self.note_mention_popup.popup();
        } else {
            println!("DEBUG: Error al listar notas");
        }
    }

    fn show_chat_note_suggestions(&self, prefix: &str, sender: &ComponentSender<Self>) {
        // Limpiar sugerencias anteriores
        while let Some(child) = self.chat_note_suggestions_list.first_child() {
            self.chat_note_suggestions_list.remove(&child);
        }

        // Obtener todas las notas y filtrar
        if let Ok(notes) = self.notes_dir.list_notes() {
            let prefix_lower = prefix.to_lowercase();

            let mut matching_notes: Vec<_> = notes
                .into_iter()
                .filter_map(|note| {
                    let name = note.name();
                    let name_lower = name.to_lowercase();
                    let name_no_ext = name.trim_end_matches(".md");
                    let name_no_ext_lower = name_no_ext.to_lowercase();

                    // Si el prefijo está vacío, mostrar todas las notas
                    if prefix.is_empty() {
                        return Some((note, 1000));
                    }

                    // Calcular score de relevancia
                    let score = if name_no_ext_lower == prefix_lower {
                        1000 // Coincidencia exacta
                    } else if name_no_ext_lower.starts_with(&prefix_lower) {
                        500 // Empieza con el prefijo
                    } else if name_no_ext_lower.contains(&prefix_lower) {
                        let pos = name_no_ext_lower.find(&prefix_lower).unwrap();
                        250 - pos // Contiene el prefijo (mejor si está al inicio)
                    } else if Self::fuzzy_match(&name_lower, &prefix_lower) {
                        100 // Fuzzy match
                    } else {
                        return None;
                    };

                    Some((note, score))
                })
                .collect();

            // Ordenar por score (mayor a menor)
            matching_notes.sort_by(|a, b| b.1.cmp(&a.1));

            // Tomar solo los primeros 10
            let top_matches: Vec<_> = matching_notes
                .into_iter()
                .take(10)
                .map(|(note, _)| note)
                .collect();

            if top_matches.is_empty() {
                self.chat_note_suggestions_popover.popdown();
                return;
            }

            // Agregar cada sugerencia
            for (index, note) in top_matches.iter().enumerate() {
                let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
                row.set_margin_all(8);

                let note_name = note.name();
                let note_name_no_ext = note_name.trim_end_matches(".md");

                // Extraer solo el nombre base (sin carpeta) para mostrar
                let display_name = if let Some(idx) = note_name_no_ext.rfind('/') {
                    &note_name_no_ext[idx + 1..]
                } else {
                    note_name_no_ext
                };

                let label = gtk::Label::new(Some(display_name));
                label.set_xalign(0.0);
                label.set_hexpand(true);
                label.set_visible(true);
                label.add_css_class("heading");

                // Mostrar la carpeta si existe
                if let Some(folder_idx) = note_name.rfind('/') {
                    let folder = &note_name[..folder_idx];
                    let folder_label = gtk::Label::new(Some(&format!("📁 {}", folder)));
                    folder_label.set_xalign(0.0);
                    folder_label.set_visible(true);
                    folder_label.add_css_class("dim-label");
                    folder_label.add_css_class("caption");
                    row.append(&folder_label);
                }

                row.prepend(&label);
                row.set_visible(true);

                // Crear ListBoxRow directamente con el Box como hijo (igual que en backlinks)
                let list_row = gtk::ListBoxRow::new();
                list_row.set_child(Some(&row));
                list_row.set_activatable(true);
                list_row.set_visible(true);
                list_row.set_can_focus(false);
                list_row.set_focusable(false);

                let note_name_for_completion = note_name_no_ext.to_string();

                // Guardar el nombre de la nota en el row para recuperarlo al pulsar Tab
                unsafe {
                    list_row.set_data("note_name", note_name_for_completion.clone());
                }

                // Usar gesture click como en backlinks
                let gesture = gtk::GestureClick::new();
                gesture.connect_released(gtk::glib::clone!(
                    #[strong]
                    sender,
                    move |_, _, _, _| {
                        sender.input(AppMsg::CompleteChatNote(note_name_for_completion.clone()));
                    }
                ));
                list_row.add_controller(gesture);

                self.chat_note_suggestions_list.append(&list_row);
                list_row.show(); // Forzar visibilidad inmediata
            }

            // Mostrar popover SIN robar el foco
            self.chat_note_suggestions_popover.popup();
        }
    }

    fn refresh_style_manager(&self) {
        // Ya no necesitamos StyleManager de Adwaita
        // El tema GTK del sistema se aplica automáticamente

        // Recrear tags de texto para asegurarnos de que están actualizados
        let tag_table = self.text_buffer.tag_table();
        for tag_name in &[
            "h1",
            "h2",
            "h3",
            "bold",
            "italic",
            "code",
            "codeblock",
            "link",
            "list",
            "blockquote",
        ] {
            if let Some(tag) = tag_table.lookup(tag_name) {
                tag_table.remove(&tag);
            }
        }

        self.create_text_tags();

        // Re-aplicar estilos markdown si está habilitado
        if self.markdown_enabled {
            self.sync_to_view();
        }
    }

    fn apply_8bit_font(&self) {
        if self.bit8_mode {
            // Modo 8BIT activado - aplicar fuente retro a toda la app
            let css = r#"
                /* Fuentes 8-bit para toda la aplicación */
                window, textview, textview text, label, button, headerbar {
                    font-family: "VT323", "Press Start 2P", "Px437 IBM VGA8", "Perfect DOS VGA 437", "unifont", monospace;
                }

                /* TextView con fuente 8-bit - tamaño ajustado para VT323 */
                textview, textview text {
                    font-size: 13px;
                    line-height: 1.5;
                    letter-spacing: 0px;
                    background-color: inherit;
                    color: inherit;
                }

                /* Labels del footer más grandes y legibles */
                .status-bar label {
                    font-size: 1.15em;
                    letter-spacing: 0.5px;
                }

                /* Botones y header */
                headerbar, button {
                    font-size: 1.0em;
                }

                /* Togglebutton 8BIT específico */
                .status-bar togglebutton {
                    font-size: 1.15em;
                    letter-spacing: 0.5px;
                }
            "#;

            let css_provider = gtk::CssProvider::new();
            css_provider.load_from_data(css);

            gtk::style_context_add_provider_for_display(
                &gtk::gdk::Display::default().unwrap(),
                &css_provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );

            println!("Modo 8BIT activado - Fuentes retro aplicadas");
        } else {
            // Modo normal - restaurar fuentes por defecto
            let css = r#"
                /* Restaurar fuentes normales - hereda colores del tema */
                window, label, button, headerbar {
                    font-family: inherit;
                }

                textview, textview text {
                    font-family: monospace;
                    font-size: 11pt;
                    line-height: 1.5;
                    letter-spacing: 0px;
                    background-color: inherit;
                    color: inherit;
                }

                .status-bar label {
                    font-size: 0.8em;
                    letter-spacing: 0px;
                }

                headerbar, button {
                    font-family: inherit;
                    font-size: inherit;
                }

                .status-bar togglebutton {
                    font-size: inherit;
                    letter-spacing: 0px;
                }
            "#;

            let css_provider = gtk::CssProvider::new();
            css_provider.load_from_data(css);

            gtk::style_context_add_provider_for_display(
                &gtk::gdk::Display::default().unwrap(),
                &css_provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );

            println!("Modo normal restaurado");
        }
    }

    fn animate_sidebar(&self, target_position: i32) {
        let split_view = self.split_view.clone();
        let current_position = split_view.position();
        let distance = (target_position - current_position).abs();
        let steps = 15;
        let step_size = distance / steps;
        let direction = if target_position > current_position {
            1
        } else {
            -1
        };

        let mut step_count = 0;
        gtk::glib::source::timeout_add_local(std::time::Duration::from_millis(10), move || {
            step_count += 1;
            let current = split_view.position();
            let next_position = if step_count >= steps {
                target_position
            } else {
                current + (step_size * direction)
            };

            split_view.set_position(next_position);

            if step_count >= steps {
                gtk::glib::ControlFlow::Break
            } else {
                gtk::glib::ControlFlow::Continue
            }
        });
    }

    /// Borra el texto seleccionado
    fn delete_selection(&mut self) {
        if let Some((start, end)) = self.text_buffer.selection_bounds() {
            let start_offset = start.offset() as usize;
            let end_offset = end.offset() as usize;

            // Borrar el rango del buffer interno
            self.buffer.delete(start_offset..end_offset);

            // Mover el cursor al inicio de la selección
            self.cursor_position = start_offset;

            self.has_unsaved_changes = true;
        }
    }

    /// Guarda la nota actual en su archivo .md
    fn save_current_note(&mut self, generate_embeddings: bool) {
        if let Some(note) = &self.current_note {
            // Obtener contenido anterior y nuevo
            let old_content = note.read().unwrap_or_default();
            let new_content = self.buffer.to_string();

            // Optimización: Si el contenido no ha cambiado, no hacer nada
            // Esto evita escrituras innecesarias en disco y regeneración de embeddings
            if old_content == new_content {
                // println!("Nota sin cambios. Omitiendo guardado.");
                self.has_unsaved_changes = false;
                return;
            }

            // Crear backup antes de guardar cambios
            if let Err(e) = note.backup(&self.notes_dir) {
                eprintln!("Error creando backup de historial: {}", e);
                // Continuamos con el guardado aunque falle el backup
            }

            if let Err(e) = note.write(&new_content) {
                eprintln!("Error guardando nota: {}", e);
            } else {
                println!("Nota guardada: {}", note.name());
                self.has_unsaved_changes = false;

                // Limpiar imágenes no referenciadas
                self.cleanup_unused_images(&old_content, &new_content);

                // Extraer nombre sin carpeta para búsqueda en BD
                // note.name() puede ser "Docs VS/NOTA" o "NOTA"
                let note_name_only = note.name().split('/').last().unwrap_or(note.name());

                // Actualizar índice en base de datos
                if let Err(e) = self.notes_db.update_note(note_name_only, &new_content) {
                    eprintln!("Error actualizando índice: {}", e);
                } else {
                    println!("Índice actualizado");

                    // Indexar embeddings si está habilitado y solicitado
                    if generate_embeddings && self.notes_config.borrow().get_embeddings_enabled() {
                        self.index_note_embeddings_async(note.path(), &new_content);
                    }

                    // Actualizar tags
                    if let Ok(Some(note_meta)) = self.notes_db.get_note(note_name_only) {
                        // Obtener tags actuales del contenido (frontmatter + inline #tags)
                        let new_tags = extract_all_tags(&new_content);

                        // Obtener tags existentes en DB
                        if let Ok(existing_tags) = self.notes_db.get_note_tags(note_meta.id) {
                            let existing_tag_names: Vec<String> =
                                existing_tags.iter().map(|t| t.name.clone()).collect();

                            // Remover tags que ya no están
                            for old_tag in &existing_tag_names {
                                if !new_tags.contains(old_tag) {
                                    let _ = self.notes_db.remove_tag(note_meta.id, old_tag);
                                }
                            }

                            // Añadir tags nuevos
                            for new_tag in &new_tags {
                                if !existing_tag_names.contains(new_tag) {
                                    let _ = self.notes_db.add_tag(note_meta.id, new_tag);
                                }
                            }
                        }

                        println!("Tags actualizados: {:?}", new_tags);
                    }
                }
            }
        } else {
            // Si no hay nota actual, crear una nueva con timestamp
            let timestamp = chrono::Utc::now().format("%Y-%m-%d_%H-%M-%S");
            let name = format!("nota_{}", timestamp);
            if let Err(e) = self.create_new_note(&name) {
                eprintln!("Error creando nota automática: {}", e);
            }
        }
    }

    /// Extrae todas las rutas de imágenes del contenido markdown
    fn extract_image_paths(content: &str) -> Vec<String> {
        let mut paths = Vec::new();
        let mut chars = content.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '!' && chars.peek() == Some(&'[') {
                chars.next(); // Consumir [

                // Saltar el alt text
                while let Some(&next_ch) = chars.peek() {
                    chars.next();
                    if next_ch == ']' {
                        break;
                    }
                }

                // Verificar si hay (url)
                if chars.peek() == Some(&'(') {
                    chars.next(); // Consumir (

                    let mut path = String::new();
                    while let Some(&next_ch) = chars.peek() {
                        chars.next();
                        if next_ch == ')' {
                            break;
                        }
                        path.push(next_ch);
                    }

                    if !path.is_empty() {
                        paths.push(path);
                    }
                }
            }
        }

        paths
    }

    /// Limpia las imágenes que ya no están referenciadas en el contenido
    fn cleanup_unused_images(&self, old_content: &str, new_content: &str) {
        let old_images = Self::extract_image_paths(old_content);
        let new_images = Self::extract_image_paths(new_content);

        let assets_dir = NotesConfig::assets_dir();

        // Para cada imagen que estaba en el contenido antiguo
        for old_image in old_images {
            // Si ya no está en el nuevo contenido
            if !new_images.contains(&old_image) {
                // Determinar la ruta completa del archivo
                let file_path = if old_image.starts_with("/") {
                    std::path::PathBuf::from(&old_image)
                } else {
                    assets_dir.join(&old_image)
                };

                // Verificar si el archivo existe en assets y eliminarlo
                if file_path.exists() && file_path.starts_with(&assets_dir) {
                    match std::fs::remove_file(&file_path) {
                        Ok(_) => println!("Imagen eliminada de assets: {}", file_path.display()),
                        Err(e) => {
                            eprintln!("Error eliminando imagen {}: {}", file_path.display(), e)
                        }
                    }
                }
            }
        }
    }

    /// Carga una nota desde archivo
    fn load_note(&mut self, name: &str) -> anyhow::Result<()> {
        let note = self
            .notes_dir
            .find_note(name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada: {}", name))?;

        let content = note.read()?;
        self.buffer = NoteBuffer::from_text(&content);
        self.cursor_position = 0;
        self.current_note = Some(note);

        // Guardar como última nota abierta
        self.notes_config
            .borrow_mut()
            .set_last_opened_note(Some(name.to_string()));
        if let Err(e) = self.notes_config.borrow().save(NotesConfig::default_path()) {
            eprintln!("Error guardando última nota abierta: {}", e);
        }

        println!("Nota cargada: {}", name);
        Ok(())
    }

    /// Crea una nueva nota
    fn create_new_note(&mut self, name: &str) -> anyhow::Result<()> {
        // Detectar si solo quiere crear una carpeta (termina en /)
        if name.ends_with('/') {
            let folder_name = name.trim_matches('/'); // Quitar / del inicio Y del final
            if folder_name.is_empty() {
                anyhow::bail!("El nombre de la carpeta no puede estar vacío");
            }

            // Crear la carpeta directamente
            let folder_path = self.notes_dir.root().join(folder_name);
            std::fs::create_dir_all(&folder_path)?;

            println!("✅ Carpeta creada: {}", folder_name);

            // Expandir la carpeta automáticamente
            self.expanded_folders.insert(folder_name.to_string());

            // No crear nota, solo retornar
            return Ok(());
        }

        // Separar carpeta y nombre
        let (folder, base_name) = if name.contains('/') {
            let parts: Vec<&str> = name.rsplitn(2, '/').collect();
            (Some(parts[1]), parts[0])
        } else {
            (None, name)
        };

        // Generar nombre único si ya existe
        let unique_name = self.generate_unique_note_name(folder, base_name);
        let final_name = if let Some(f) = folder {
            format!("{}/{}", f, unique_name)
        } else {
            unique_name.clone()
        };

        // Contenido inicial vacío para nueva nota
        let initial_content = format!("# {}\n\n", unique_name);

        let note = if let Some(folder_path) = folder {
            // Crear en carpeta
            self.notes_dir
                .create_note_in_folder(folder_path, &unique_name, &initial_content)?
        } else {
            // Crear en la raíz
            self.notes_dir.create_note(&unique_name, &initial_content)?
        };

        // Indexar en base de datos
        let folder_for_db = self.notes_dir.relative_folder(note.path());
        if let Err(e) = self.notes_db.index_note(
            note.name(),
            note.path().to_str().unwrap_or(""),
            &initial_content,
            folder_for_db.as_deref(),
        ) {
            eprintln!("Error indexando nueva nota: {}", e);
        } else {
            println!("Nueva nota indexada: {}", final_name);
        }

        // Cargar la nueva nota en el buffer
        self.buffer = NoteBuffer::from_text(&initial_content);
        self.cursor_position = initial_content.len();
        self.current_note = Some(note.clone());
        self.has_unsaved_changes = false;

        if unique_name != base_name {
            println!(
                "Nueva nota creada: {} (renombrada desde '{}')",
                final_name, name
            );
        } else {
            println!("Nueva nota creada: {}", final_name);
        }
        Ok(())
    }

    /// Genera un nombre único para una nota verificando si ya existe
    /// y añadiendo (1), (2), etc. si es necesario
    fn generate_unique_note_name(&self, folder: Option<&str>, base_name: &str) -> String {
        let notes_root = self.notes_dir.root();
        let target_dir = if let Some(f) = folder {
            notes_root.join(f)
        } else {
            notes_root.to_path_buf()
        };

        // Verificar si el nombre base ya existe
        let base_path = target_dir.join(format!("{}.md", base_name));
        if !base_path.exists() {
            return base_name.to_string();
        }

        // Si existe, buscar el primer número disponible
        for i in 1..1000 {
            let new_name = format!("{} ({})", base_name, i);
            let new_path = target_dir.join(format!("{}.md", new_name));
            if !new_path.exists() {
                return new_name;
            }
        }

        // Si llegamos aquí (muy improbable), usar timestamp
        format!("{} ({})", base_name, chrono::Local::now().timestamp())
    }

    /// Configura drag and drop para una fila específica del sidebar
    fn setup_drag_and_drop_for_row(&self, row: &gtk::ListBoxRow, sender: &ComponentSender<Self>) {
        use gtk::gdk;
        use gtk::prelude::*;

        // Obtener información de la fila
        let is_folder = unsafe {
            row.data::<bool>("is_folder")
                .map(|data| *data.as_ref())
                .unwrap_or(false)
        };

        let item_name = if is_folder {
            unsafe {
                row.data::<String>("folder_name")
                    .map(|d| d.as_ref().clone())
            }
        } else {
            // Obtener el nombre de la nota desde el label
            row.child()
                .and_then(|child| child.downcast::<gtk::Box>().ok())
                .and_then(|box_widget| box_widget.first_child())
                .and_then(|icon| icon.next_sibling())
                .and_then(|label_widget| label_widget.downcast::<gtk::Label>().ok())
                .map(|label| label.text().to_string())
        };

        if item_name.is_none() {
            return;
        }

        let item_name = item_name.unwrap();

        // Para notas, obtener la carpeta actual y su padre desde la base de datos
        let (target_folder, target_parent_folder) = if !is_folder {
            // Buscar la carpeta de esta nota en la base de datos
            let folder = self
                .notes_db
                .get_note(&item_name)
                .ok()
                .flatten()
                .and_then(|note_meta| note_meta.folder);

            // Calcular la carpeta padre (para drag & drop de carpetas sobre notas)
            let parent_folder = folder.as_ref().and_then(|f| {
                // Si la carpeta es "A/B/C", el padre es "A/B"
                // Si es "A", el padre es None (raíz)
                f.rsplit_once('/').map(|(parent, _)| parent.to_string())
            });

            (folder, parent_folder)
        } else {
            (None, None)
        };

        // Configurar DragSource
        let drag_source = gtk::DragSource::new();
        drag_source.set_actions(gdk::DragAction::MOVE);

        let drag_item_name = item_name.clone();
        let drag_is_folder = is_folder;
        drag_source.connect_prepare(move |_source, _x, _y| {
            let data_str = if drag_is_folder {
                format!("folder:{}", drag_item_name)
            } else {
                format!("note:{}", drag_item_name)
            };

            Some(gdk::ContentProvider::for_value(&data_str.to_value()))
        });

        row.add_controller(drag_source);

        // Configurar DropTarget
        let drop_target = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::MOVE);

        let sender_clone = sender.clone();
        let target_item_name = item_name.clone();
        let target_is_folder = is_folder;
        let target_folder_path = target_folder.clone();
        let target_parent_folder_path = target_parent_folder.clone();

        drop_target.connect_drop(move |_target, value, _x, _y| {
            if let Ok(data_str) = value.get::<String>() {
                // Parsear el dato arrastrado
                if let Some((drag_type, drag_name)) = data_str.split_once(':') {
                    match (drag_type, target_is_folder) {
                        ("note", true) => {
                            // Arrastrar nota sobre carpeta -> mover nota a carpeta
                            sender_clone.input(AppMsg::MoveNoteToFolder {
                                note_name: drag_name.to_string(),
                                folder_name: Some(target_item_name.clone()),
                            });
                            return true;
                        }
                        ("note", false) => {
                            // Arrastrar nota sobre nota -> reordenar (y mover a la misma carpeta si es necesario)
                            sender_clone.input(AppMsg::ReorderNotes {
                                source_name: drag_name.to_string(),
                                target_name: target_item_name.clone(),
                            });
                            return true;
                        }
                        ("folder", true) => {
                            // Arrastrar carpeta sobre carpeta -> mover carpeta dentro de esta carpeta
                            sender_clone.input(AppMsg::MoveFolder {
                                folder_name: drag_name.to_string(),
                                target_folder: Some(target_item_name.clone()),
                            });
                            return true;
                        }
                        ("folder", false) => {
                            // Arrastrar carpeta sobre nota -> mover carpeta al mismo nivel que la nota
                            // (al padre de la carpeta de la nota)
                            println!("🔄 Drag folder '{}' over note '{}' (note's folder: {:?}, parent: {:?})",
                                drag_name, target_item_name, target_folder_path, target_parent_folder_path);
                            sender_clone.input(AppMsg::MoveFolder {
                                folder_name: drag_name.to_string(),
                                target_folder: target_parent_folder_path.clone(),
                            });
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            false
        });

        row.add_controller(drop_target);
    }

    /// Rellena la lista de notas en el sidebar
    fn populate_notes_list(&self, sender: &ComponentSender<Self>) {
        use std::collections::HashMap;

        // Activar flag para evitar que el hover cargue notas durante la repoblación
        *self.is_populating_list.borrow_mut() = true;

        // Guardar la nota actual para re-seleccionarla después
        let current_note_name = self.current_note.as_ref().map(|n| n.name().to_string());

        // NO deseleccionar aquí para evitar scroll no deseado
        // El código al final re-seleccionará la nota actual

        // Limpiar lista actual (solo ListBoxRows, no el popover)
        let mut child = self.notes_list.first_child();
        while let Some(widget) = child {
            let next = widget.next_sibling();
            if widget.type_().name() == "GtkListBoxRow" {
                self.notes_list.remove(&widget);
            }
            child = next;
        }

        // Obtener todas las notas desde la base de datos (ya ordenadas por order_index)
        if let Ok(notes_metadata) = self.notes_db.list_notes(None) {
            // Filtrar solo las notas que realmente existen en el filesystem
            let existing_notes: Vec<_> = notes_metadata
                .into_iter()
                .filter(|note_meta| {
                    // Verificar que el archivo existe
                    std::path::Path::new(&note_meta.path).exists()
                })
                .collect();

            // Organizar por carpetas manteniendo el orden de order_index
            let mut by_folder: HashMap<String, Vec<String>> = HashMap::new();

            // Pre-cargar iconos personalizados con colores para carpetas y notas
            let folder_icons = self
                .notes_db
                .get_all_folder_icons_with_colors()
                .unwrap_or_default();
            let note_icons = self
                .notes_db
                .get_all_note_icons_with_colors()
                .unwrap_or_default();

            for note_meta in existing_notes {
                let folder = note_meta.folder.as_deref().unwrap_or("/").to_string();
                by_folder
                    .entry(folder)
                    .or_insert_with(Vec::new)
                    .push(note_meta.name);
            }

            // Escanear notas en la papelera (que no están en la BD)
            let trash_path = self.notes_dir.trash_path();
            if trash_path.exists() {
                if let Ok(entries) = std::fs::read_dir(&trash_path) {
                    for entry in entries.flatten() {
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.is_file() {
                                let path = entry.path();
                                if path.extension().and_then(|s| s.to_str()) == Some("md") {
                                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                                        // Añadir a la carpeta .trash
                                        by_folder
                                            .entry(".trash".to_string())
                                            .or_insert_with(Vec::new)
                                            .push(name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Escanear el filesystem RECURSIVAMENTE para incluir carpetas vacías
            fn scan_folders_recursive(
                path: &std::path::Path,
                root: &std::path::Path,
                folders_set: &mut HashMap<String, Vec<String>>,
            ) {
                if let Ok(entries) = std::fs::read_dir(path) {
                    for entry in entries.flatten() {
                        if let Ok(metadata) = entry.metadata() {
                            if metadata.is_dir() {
                                // Obtener el path relativo desde el root
                                if let Ok(relative) = entry.path().strip_prefix(root) {
                                    if let Some(folder_name) = relative.to_str() {
                                        // Ignorar carpetas ocultas (excepto .trash)
                                        // Verificar si ALGUNA parte del path empieza por .
                                        if folder_name
                                            .split('/')
                                            .any(|part| part.starts_with('.') && part != ".trash")
                                        {
                                            continue;
                                        }

                                        // Asegurar que la carpeta esté en el HashMap
                                        folders_set
                                            .entry(folder_name.to_string())
                                            .or_insert_with(Vec::new);

                                        // Escanear recursivamente dentro de esta carpeta
                                        scan_folders_recursive(&entry.path(), root, folders_set);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let notes_root = self.notes_dir.root().to_path_buf();
            scan_folders_recursive(&notes_root, &notes_root, &mut by_folder);

            // Ordenar solo las carpetas, no las notas (las notas ya vienen ordenadas por order_index)
            let mut folders: Vec<_> = by_folder.keys().cloned().collect();
            folders.sort_by(|a, b| {
                // .trash siempre al final
                if a == ".trash" {
                    std::cmp::Ordering::Greater
                } else if b == ".trash" {
                    std::cmp::Ordering::Less
                } else {
                    a.cmp(b)
                }
            });

            // Ordenar notas dentro de .trash por timestamp descendente (más reciente arriba)
            if let Some(trash_notes) = by_folder.get_mut(".trash") {
                trash_notes.sort_by(|a, b| {
                    // Extraer timestamp del final
                    let get_ts = |name: &str| -> u64 {
                        name.rsplit('_')
                            .next()
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(0)
                    };
                    let ts_a = get_ts(a);
                    let ts_b = get_ts(b);
                    // Orden descendente (b cmp a)
                    ts_b.cmp(&ts_a)
                });
            }

            for folder in folders {
                // Ocultar carpeta .history y otras ocultas de la interfaz
                if folder != ".trash" && folder.split('/').any(|p| p.starts_with('.')) {
                    continue;
                }

                if let Some(notes_in_folder) = by_folder.get(&folder) {
                    // Si no es la raíz, mostrar carpeta como encabezado expandible
                    if folder != "/" {
                        // Verificar que la carpeta existe en el filesystem
                        let folder_path = self.notes_dir.root().join(&folder);
                        if !folder_path.exists() || !folder_path.is_dir() {
                            continue;
                        }

                        // Verificar si alguna carpeta padre está contraída
                        // Si test está contraída, test/test2 no debe mostrarse
                        let mut parent_collapsed = false;
                        let parts: Vec<&str> = folder.split('/').collect();
                        for i in 1..parts.len() {
                            let parent_path = parts[..i].join("/");
                            if !self.expanded_folders.contains(&parent_path) {
                                parent_collapsed = true;
                                break;
                            }
                        }

                        // Si alguna carpeta padre está contraída, saltar esta carpeta completa
                        if parent_collapsed {
                            continue;
                        }

                        let is_expanded = self.expanded_folders.contains(&folder);
                        let arrow_icon = if is_expanded {
                            "pan-down-symbolic"
                        } else {
                            "pan-end-symbolic"
                        };

                        let folder_row = gtk::Box::builder()
                            .orientation(gtk::Orientation::Horizontal)
                            .spacing(6)
                            .margin_start(8)
                            .margin_end(12)
                            .margin_top(6)
                            .margin_bottom(4)
                            .build();

                        let arrow = gtk::Image::builder()
                            .icon_name(arrow_icon)
                            .pixel_size(12)
                            .build();

                        // Verificar si hay un icono personalizado para esta carpeta
                        let custom_folder_icon = folder_icons.get(&folder);

                        // Crear el widget de icono (icono del sistema o emoji)
                        let folder_icon_widget: gtk::Widget = if let Some((icon, color)) =
                            custom_folder_icon
                        {
                            // Verificar si es un icono del sistema (termina en -symbolic)
                            if icon.ends_with("-symbolic") {
                                let image =
                                    gtk::Image::builder().icon_name(icon).pixel_size(16).build();
                                image.add_css_class("folder-custom-icon");

                                // Aplicar color si está definido
                                if let Some(hex_color) = color {
                                    let css_provider = gtk::CssProvider::new();
                                    let css = format!(
                                        "image {{ color: {}; -gtk-icon-style: symbolic; }}",
                                        hex_color
                                    );
                                    css_provider.load_from_data(&css);
                                    image.style_context().add_provider(
                                        &css_provider,
                                        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                                    );
                                }

                                image.upcast()
                            } else {
                                // Es un emoji/caracter Unicode
                                let label = gtk::Label::builder().label(icon).build();
                                label.add_css_class("folder-emoji-icon");

                                // Aplicar color si está definido
                                if let Some(hex_color) = color {
                                    let css_provider = gtk::CssProvider::new();
                                    let css = format!("label {{ color: {}; }}", hex_color);
                                    css_provider.load_from_data(&css);
                                    label.style_context().add_provider(
                                        &css_provider,
                                        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                                    );
                                }

                                label.upcast()
                            }
                        } else {
                            // Usar icono del sistema por defecto
                            let icon_name = if folder == ".trash" {
                                "user-trash-symbolic"
                            } else {
                                "folder-symbolic"
                            };
                            gtk::Image::builder()
                                .icon_name(icon_name)
                                .pixel_size(16)
                                .build()
                                .upcast()
                        };

                        // Obtener solo el nombre de la carpeta (última parte del path)
                        let folder_display_name = if folder == ".trash" {
                            "Papelera".to_string()
                        } else {
                            folder.split('/').last().unwrap_or(&folder).to_string()
                        };

                        // Calcular nivel de indentación (número de '/' en el path)
                        let depth = folder.matches('/').count();
                        let indent = 8 + (depth * 16);

                        folder_row.set_margin_start(indent as i32);

                        folder_row.append(&arrow);
                        folder_row.append(&folder_icon_widget);

                        // Verificar si esta carpeta está siendo renombrada
                        let is_renaming_folder = self
                            .renaming_item
                            .borrow()
                            .as_ref()
                            .map(|(name, is_folder)| *is_folder && name == &folder)
                            .unwrap_or(false);

                        if is_renaming_folder {
                            // Mostrar Entry editable para carpeta
                            let entry = gtk::Entry::builder()
                                .text(&folder_display_name)
                                .hexpand(true)
                                .build();

                            let old_folder_name = folder.clone();
                            let folder_display_clone = folder_display_name.clone();
                            let renaming_clone = self.renaming_item.clone();
                            let notes_dir = self.notes_dir.clone();
                            let notes_db_clone = self.notes_db.clone_connection();
                            let sender_clone = sender.clone();

                            entry.connect_activate(move |entry| {
                                let new_name = entry.text().to_string().trim().to_string();
                                if !new_name.is_empty() && new_name != folder_display_clone {
                                    let old_path = notes_dir.root().join(&old_folder_name);
                                    let new_path = notes_dir.root().join(&new_name);

                                    if let Err(e) = std::fs::rename(&old_path, &new_path) {
                                        eprintln!("Error al renombrar carpeta: {}", e);
                                    } else {
                                        // Actualizar todas las notas de la carpeta en la BD (incluyendo embeddings)
                                        if let Err(e) = notes_db_clone.update_notes_folder(
                                            &old_folder_name,
                                            &new_name,
                                            notes_dir.root().to_str().unwrap_or(""),
                                        ) {
                                            eprintln!(
                                                "Error actualizando BD al renombrar carpeta: {}",
                                                e
                                            );
                                        }
                                    }
                                }

                                *renaming_clone.borrow_mut() = None;
                                sender_clone.input(AppMsg::RefreshSidebar);
                            });

                            // Al perder foco, cancelar renombrado
                            let focus_controller = gtk::EventControllerFocus::new();
                            let renaming_clone2 = self.renaming_item.clone();
                            let sender_clone2 = sender.clone();
                            focus_controller.connect_leave(move |_| {
                                *renaming_clone2.borrow_mut() = None;
                                sender_clone2.input(AppMsg::RefreshSidebar);
                            });
                            entry.add_controller(focus_controller);

                            folder_row.append(&entry);

                            // Dar foco al entry
                            gtk::glib::source::timeout_add_local(
                                std::time::Duration::from_millis(50),
                                gtk::glib::clone!(
                                    #[strong]
                                    entry,
                                    move || {
                                        entry.grab_focus();
                                        entry.select_region(0, -1);
                                        gtk::glib::ControlFlow::Break
                                    }
                                ),
                            );
                        } else {
                            // Mostrar Label normal para carpeta
                            let folder_label = gtk::Label::builder()
                                .label(&folder_display_name)
                                .xalign(0.0)
                                .hexpand(true)
                                .ellipsize(gtk::pango::EllipsizeMode::End)
                                .max_width_chars(30)
                                .build();

                            folder_label.add_css_class("heading");
                            folder_row.append(&folder_label);
                        }

                        // Agregar como row seleccionable y activatable
                        let list_row = gtk::ListBoxRow::builder()
                            .selectable(true)
                            .activatable(true)
                            .build();
                        list_row.set_child(Some(&folder_row));

                        // Guardar el nombre de la carpeta en el row
                        unsafe {
                            list_row.set_data("folder_name", folder.clone());
                            list_row.set_data("is_folder", true);
                        }

                        self.notes_list.append(&list_row);

                        // Configurar drag-and-drop para la carpeta
                        self.setup_drag_and_drop_for_row(&list_row, sender);

                        // Si no está expandida, no mostrar las notas
                        if !is_expanded {
                            continue;
                        }
                    }

                    // Pre-calcular colisiones de nombres para la papelera
                    let mut trash_name_counts = std::collections::HashMap::new();
                    if folder == ".trash" {
                        for note_name in notes_in_folder {
                            if let Some(idx) = note_name.rfind('_') {
                                let (name_part, _) = note_name.split_at(idx);
                                *trash_name_counts.entry(name_part.to_string()).or_insert(0) += 1;
                            }
                        }
                    }

                    // Mostrar notas de esta carpeta (solo si está expandida)
                    // Las notas ya vienen ordenadas por order_index desde la base de datos
                    for note_name in notes_in_folder {
                        // Calcular indentación según profundidad de la carpeta
                        let depth = if folder == "/" {
                            0
                        } else {
                            folder.matches('/').count()
                        };
                        let note_indent = if folder == "/" {
                            12
                        } else {
                            8 + ((depth + 1) * 16)
                        };

                        let row = gtk::Box::builder()
                            .orientation(gtk::Orientation::Horizontal)
                            .spacing(8)
                            .margin_start(note_indent as i32)
                            .margin_end(12)
                            .margin_top(3)
                            .margin_bottom(3)
                            .build();

                        // Verificar si hay un icono personalizado para esta nota
                        let custom_note_icon = note_icons.get(note_name);

                        // Crear el widget de icono (icono del sistema o emoji)
                        let note_icon_widget: gtk::Widget = if let Some((icon, color)) =
                            custom_note_icon
                        {
                            // Verificar si es un icono del sistema (termina en -symbolic)
                            if icon.ends_with("-symbolic") {
                                let image =
                                    gtk::Image::builder().icon_name(icon).pixel_size(14).build();
                                image.add_css_class("note-custom-icon");

                                // Aplicar color si está definido
                                if let Some(hex_color) = color {
                                    let css_provider = gtk::CssProvider::new();
                                    let css = format!(
                                        "image {{ color: {}; -gtk-icon-style: symbolic; }}",
                                        hex_color
                                    );
                                    css_provider.load_from_data(&css);
                                    image.style_context().add_provider(
                                        &css_provider,
                                        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                                    );
                                }

                                image.upcast()
                            } else {
                                // Es un emoji/caracter Unicode
                                let label = gtk::Label::builder().label(icon).build();
                                label.add_css_class("note-emoji-icon");

                                // Aplicar color si está definido
                                if let Some(hex_color) = color {
                                    let css_provider = gtk::CssProvider::new();
                                    let css = format!("label {{ color: {}; }}", hex_color);
                                    css_provider.load_from_data(&css);
                                    label.style_context().add_provider(
                                        &css_provider,
                                        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                                    );
                                }

                                label.upcast()
                            }
                        } else {
                            // Usar icono del sistema por defecto
                            gtk::Image::builder()
                                .icon_name("text-x-generic-symbolic")
                                .pixel_size(14)
                                .build()
                                .upcast()
                        };

                        row.append(&note_icon_widget);

                        // Clonar note_name para uso posterior
                        let note_name_str = note_name.as_str();
                        let note_name_owned = note_name.to_string();

                        // Verificar si esta nota está siendo renombrada
                        let is_renaming = self
                            .renaming_item
                            .borrow()
                            .as_ref()
                            .map(|(name, is_folder)| !is_folder && name.as_str() == note_name_str)
                            .unwrap_or(false);

                        if is_renaming {
                            // Mostrar Entry editable
                            let entry = gtk::Entry::builder()
                                .text(&note_name_owned)
                                .hexpand(true)
                                .build();

                            // Al presionar Enter, renombrar
                            let old_name = note_name_owned.clone();
                            let renaming_clone = self.renaming_item.clone();
                            let notes_dir = self.notes_dir.clone();
                            let sender_clone = sender.clone();

                            let notes_db_clone = self.notes_db.clone_connection();
                            entry.connect_activate(move |entry| {
                                let new_name = entry.text().to_string().trim().to_string();
                                if !new_name.is_empty() && new_name != old_name {
                                    // Renombrar archivo
                                    if let Ok(Some(note)) = notes_dir.find_note(&old_name) {
                                        let old_path = note.path();

                                        // Construir nuevo path (misma carpeta)
                                        let new_path = if let Some(parent) = old_path.parent() {
                                            parent.join(format!("{}.md", new_name))
                                        } else {
                                            notes_dir
                                                .root()
                                                .join("notes")
                                                .join(format!("{}.md", new_name))
                                        };

                                        if let Err(e) = std::fs::rename(&old_path, &new_path) {
                                            eprintln!("Error al renombrar: {}", e);
                                        } else {
                                            // Actualizar en la base de datos (incluyendo embeddings)
                                            let folder = notes_dir.relative_folder(&new_path);

                                            if let Err(e) = notes_db_clone.rename_note(
                                                &old_name,
                                                &new_name,
                                                new_path.to_str().unwrap_or(""),
                                                folder.as_deref(),
                                            ) {
                                                eprintln!("⚠️ Error actualizando BD después de renombrar: {}", e);
                                            }
                                        }
                                    }
                                }

                                // Desactivar modo renombrado
                                *renaming_clone.borrow_mut() = None;

                                // Refrescar sidebar
                                sender_clone.input(AppMsg::RefreshSidebar);
                            });

                            // Al perder foco, cancelar renombrado
                            let focus_controller = gtk::EventControllerFocus::new();
                            let renaming_clone2 = self.renaming_item.clone();
                            let sender_clone2 = sender.clone();
                            focus_controller.connect_leave(move |_| {
                                *renaming_clone2.borrow_mut() = None;
                                sender_clone2.input(AppMsg::RefreshSidebar);
                            });
                            entry.add_controller(focus_controller);

                            row.append(&entry);

                            // Dar foco al entry
                            gtk::glib::source::timeout_add_local(
                                std::time::Duration::from_millis(50),
                                gtk::glib::clone!(
                                    #[strong]
                                    entry,
                                    move || {
                                        entry.grab_focus();
                                        entry.select_region(0, -1);
                                        gtk::glib::ControlFlow::Break
                                    }
                                ),
                            );
                        } else {
                            // Mostrar Label normal
                            // Si estamos en la papelera, limpiar el nombre y mostrar tooltip
                            let (display_name, tooltip_text) = if folder == ".trash" {
                                if let Some(idx) = note_name_owned.rfind('_') {
                                    let (name_part, ts_part) = note_name_owned.split_at(idx);
                                    // ts_part incluye el '_', así que lo saltamos
                                    let ts_str = &ts_part[1..];

                                    // Verificar si hay colisión visual
                                    let count = trash_name_counts.get(name_part).unwrap_or(&0);
                                    let show_date = *count > 1;

                                    // Verificar si es un timestamp (solo dígitos y longitud razonable)
                                    if ts_str.chars().all(char::is_numeric) && ts_str.len() > 8 {
                                        let (tooltip, date_suffix) =
                                            if let Ok(ts) = ts_str.parse::<i64>() {
                                                if let Some(datetime) =
                                                    chrono::DateTime::from_timestamp(ts, 0)
                                                {
                                                    let local: chrono::DateTime<chrono::Local> =
                                                        chrono::DateTime::from(datetime);
                                                    let tooltip = format!(
                                                        "Borrado el: {}",
                                                        local.format("%Y-%m-%d %H:%M:%S")
                                                    );
                                                    let suffix = if show_date {
                                                        format!(" ({})", local.format("%H:%M"))
                                                    } else {
                                                        String::new()
                                                    };
                                                    (Some(tooltip), suffix)
                                                } else {
                                                    (None, String::new())
                                                }
                                            } else {
                                                (None, String::new())
                                            };
                                        (format!("{}{}", name_part, date_suffix), tooltip)
                                    } else {
                                        (note_name_owned.clone(), Some(note_name_owned.clone()))
                                    }
                                } else {
                                    (note_name_owned.clone(), Some(note_name_owned.clone()))
                                }
                            } else {
                                (note_name_owned.clone(), Some(note_name_owned.clone()))
                            };

                            let label = gtk::Label::builder()
                                .label(&display_name)
                                .xalign(0.0)
                                .hexpand(true)
                                .ellipsize(gtk::pango::EllipsizeMode::End)
                                .max_width_chars(40)
                                .tooltip_text(&tooltip_text.unwrap_or_default())
                                .build();

                            // Si es papelera, usar un color más tenue
                            if folder == ".trash" {
                                label.add_css_class("dim-label");
                            }

                            row.append(&label);
                        }

                        // Envolver en ListBoxRow para drag-and-drop
                        let list_row = gtk::ListBoxRow::builder()
                            .selectable(true)
                            .activatable(true)
                            .child(&row)
                            .build();

                        // Guardar el nombre de la nota en el row
                        unsafe {
                            list_row.set_data("note_name", note_name_owned.clone());
                            list_row.set_data("is_folder", false);
                        }

                        self.notes_list.append(&list_row);

                        // Configurar drag-and-drop para la nota
                        self.setup_drag_and_drop_for_row(&list_row, sender);
                    }
                }
            }
        }

        // Re-seleccionar la nota actual si existía
        if let Some(note_name) = current_note_name {
            // Buscar la fila con esta nota
            let mut current_row = self.notes_list.first_child();
            while let Some(row) = current_row {
                if let Ok(list_row) = row.clone().downcast::<gtk::ListBoxRow>() {
                    if list_row.is_selectable() {
                        if let Some(child) = list_row.child() {
                            if let Ok(box_widget) = child.downcast::<gtk::Box>() {
                                if let Some(label_widget) =
                                    box_widget.first_child().and_then(|w| w.next_sibling())
                                {
                                    if let Ok(label) = label_widget.downcast::<gtk::Label>() {
                                        if label.text() == note_name {
                                            self.notes_list.select_row(Some(&list_row));

                                            // Hacer scroll hasta la nota seleccionada
                                            let scrolled_window = self
                                                .notes_list
                                                .parent()
                                                .and_then(|p| p.parent())
                                                .and_then(|p| {
                                                    p.downcast::<gtk::ScrolledWindow>().ok()
                                                });

                                            if let Some(sw) = scrolled_window {
                                                let vadj = sw.vadjustment();
                                                // Obtener posición de la fila
                                                let allocation = list_row.allocation();
                                                let row_y = allocation.y() as f64;
                                                let page_size = vadj.page_size();

                                                // Centrar la fila en la vista
                                                let target_value =
                                                    (row_y - page_size / 2.0).max(0.0);
                                                vadj.set_value(target_value);
                                            }

                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                current_row = row.next_sibling();
            }
        }

        // Desactivar flag después de repoblar la lista
        *self.is_populating_list.borrow_mut() = false;
    }

    /// Realiza búsqueda FTS5 y muestra resultados en el sidebar
    fn perform_search(&self, query: &str, sender: &ComponentSender<Self>) {
        // Activar flag para evitar que el hover cargue notas durante la repoblación
        *self.is_populating_list.borrow_mut() = true;

        // Guardar la nota actual para re-seleccionarla después
        let current_note_name = self.current_note.as_ref().map(|n| n.name().to_string());

        // Deseleccionar cualquier fila actual
        self.notes_list.select_row(gtk::ListBoxRow::NONE);

        // Limpiar lista actual
        let mut child = self.notes_list.first_child();
        while let Some(widget) = child {
            let next = widget.next_sibling();
            if widget.type_().name() == "GtkListBoxRow" {
                self.notes_list.remove(&widget);
            }
            child = next;
        }

        // Verificar si los embeddings están habilitados
        let embeddings_enabled = self.notes_config.borrow().get_embeddings_enabled();
        let has_api_key = self
            .notes_config
            .borrow()
            .get_embeddings_api_key()
            .is_some();

        // Realizar búsqueda semántica SOLO si el usuario la activó con el toggle
        let semantic_results = if self.semantic_search_enabled
            && embeddings_enabled
            && has_api_key
            && query.len() >= 3
        {
            self.perform_semantic_search(query)
        } else {
            Vec::new()
        };

        let has_semantic_results = !semantic_results.is_empty();

        // Realizar búsqueda tradicional en la base de datos (siempre, o solo si no hay semántica)
        let traditional_results = if self.semantic_search_enabled && has_semantic_results {
            // Si búsqueda semántica está activa y tiene resultados, no hacer FTS
            Vec::new()
        } else {
            // Búsqueda tradicional FTS
            match self.notes_db.search_notes(query) {
                Ok(results) => results,
                Err(e) => {
                    eprintln!("Error al buscar notas: {}", e);
                    Vec::new()
                }
            }
        };

        // Combinar resultados, priorizando semánticos
        let combined_results =
            self.merge_search_results(semantic_results, traditional_results, query);

        self.floating_search_rows.borrow_mut().clear();

        if combined_results.is_empty() {
            // Mostrar mensaje de sin resultados
            let no_results = gtk::Label::builder()
                .label(&format!("No se encontraron resultados para '{}'", query))
                .xalign(0.5)
                .margin_top(16)
                .margin_bottom(16)
                .margin_start(12)
                .margin_end(12)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .justify(gtk::Justification::Center)
                .css_classes(vec!["dim-label"])
                .build();

            let row = gtk::ListBoxRow::builder()
                .selectable(false)
                .activatable(false)
                .child(&no_results)
                .build();

            self.notes_list.append(&row);
        } else {
            // Mostrar encabezado de resultados si hay búsqueda semántica
            if embeddings_enabled && has_api_key && has_semantic_results {
                let header_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(8)
                    .margin_start(8)
                    .margin_end(12)
                    .margin_top(8)
                    .margin_bottom(4)
                    .hexpand(true)
                    .halign(gtk::Align::Fill)
                    .build();

                let icon_label = gtk::Label::builder().label("🧠").build();

                let header_label = gtk::Label::builder()
                    .label("Resultados por similitud semántica")
                    .xalign(0.0)
                    .hexpand(true)
                    .build();
                header_label.add_css_class("caption");
                header_label.add_css_class("dim-label");

                header_box.append(&icon_label);
                header_box.append(&header_label);

                let header_wrapper = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .hexpand(true)
                    .halign(gtk::Align::Fill)
                    .margin_start(12)
                    .margin_end(12)
                    .build();
                header_wrapper.append(&header_box);

                let header_row = gtk::ListBoxRow::builder()
                    .selectable(false)
                    .activatable(false)
                    .hexpand(true)
                    .halign(gtk::Align::Fill)
                    .child(&header_wrapper)
                    .build();

                self.notes_list.append(&header_row);
            }

            // Mostrar resultados (imitando estilo de filas normales)
            for result in combined_results {
                // Contenedor principal con mismos márgenes que notas normales (raíz = 12)
                let outer = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .spacing(2)
                    .margin_start(12)
                    .margin_end(12)
                    .margin_top(3)
                    .margin_bottom(3)
                    .build();

                // Línea de nombre + similitud
                let name_row = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(6)
                    .build();

                let name_label = gtk::Label::builder()
                    .label(&result.note_name)
                    .xalign(0.0)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .max_width_chars(24) // ligeramente más ancho, luego se recorta por el listbox
                    .wrap(false)
                    .build();
                name_label.add_css_class("body");
                name_row.append(&name_label);

                if let Some(similarity) = result.similarity {
                    let similarity_badge = gtk::Label::builder()
                        .label(&format!("{:.0}%", similarity * 100.0))
                        .xalign(1.0)
                        .build();
                    similarity_badge.add_css_class("caption");
                    similarity_badge.add_css_class("accent");
                    name_row.append(&similarity_badge);
                }

                outer.append(&name_row);

                // Snippet debajo (2 líneas máx)
                let snippet_label = gtk::Label::builder()
                    .label(&result.snippet)
                    .xalign(0.0)
                    .wrap(true)
                    .wrap_mode(gtk::pango::WrapMode::WordChar)
                    .lines(2)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .max_width_chars(28)
                    .build();
                snippet_label.add_css_class("dim-label");
                snippet_label.add_css_class("caption");
                outer.append(&snippet_label);

                let list_row = gtk::ListBoxRow::builder()
                    .selectable(true)
                    .activatable(true)
                    .child(&outer)
                    .build();

                println!(
                    "[DEBUG perform_search] Row creado - note_name: '{}', snippet len: {}",
                    result.note_name,
                    result.snippet.len()
                );
                unsafe {
                    list_row.set_data("note_name", result.note_name.clone());
                    list_row.set_data("snippet", result.snippet.clone());
                }

                self.notes_list.append(&list_row);

                if let Some(ref current_name) = current_note_name {
                    if &result.note_name == current_name {
                        self.notes_list.select_row(Some(&list_row));
                    }
                }
            }
        }

        *self.is_populating_list.borrow_mut() = false;
    }

    /// Realiza búsqueda semántica usando embeddings
    fn perform_semantic_search(&self, query: &str) -> Vec<SearchResult> {
        {
            // Usar sistema NoteMemory de RIG si está disponible
            if let Some(memory) = self.note_memory.borrow().as_ref() {
                let memory_clone = memory.clone();
                let query_str = query.to_string();

                let rt = match tokio::runtime::Runtime::new() {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Error al crear runtime: {}", e);
                        return Vec::new();
                    }
                };

                let results = rt.block_on(async { memory_clone.search(&query_str, 10).await });

                match results {
                    Ok(rig_results) => {
                        return rig_results
                            .into_iter()
                            .map(|(score, note_name, _metadata, _content)| {
                                // Buscar info de la nota en la base de datos
                                let note_info = self.notes_db.get_note(&note_name).ok().flatten();

                                SearchResult {
                                    note_id: note_info.as_ref().map(|n| n.id).unwrap_or(0),
                                    note_name: note_name.clone(),
                                    note_path: note_info
                                        .as_ref()
                                        .map(|n| n.path.clone())
                                        .unwrap_or_else(|| format!("{}.md", note_name)),
                                    snippet: format!("Relevancia: {:.2}", score),
                                    relevance: score,
                                    matched_tags: vec![],
                                    similarity: Some(score),
                                }
                            })
                            .collect();
                    }
                    Err(e) => {
                        eprintln!("Error en búsqueda semántica con NoteMemory: {}", e);
                        return Vec::new();
                    }
                }
            }
        }

        // Si llegamos aquí, no hay NoteMemory disponible
        Vec::new()
    }

    /// Realiza búsqueda y muestra resultados en la barra flotante
    fn perform_floating_search(&self, query: &str, sender: &ComponentSender<Self>) {
        // Ocultar respuesta del agente de búsqueda anterior
        self.semantic_search_answer_box.set_visible(false);
        self.semantic_search_answer_row.set_visible(false);
        *self.semantic_search_answer_visible.borrow_mut() = false;

        // Limpiar lista actual (pero mantener el semantic_search_answer_row)
        let answer_row_ptr = self.semantic_search_answer_row.as_ptr();
        let mut child = self.floating_search_results_list.first_child();
        while let Some(widget) = child {
            let next = widget.next_sibling();
            // No eliminar el semantic_search_answer_row
            if widget.as_ptr() != answer_row_ptr as *mut _ {
                self.floating_search_results_list.remove(&widget);
            }
            child = next;
        }

        // Si estamos en modo "buscar en nota actual", filtrar solo esa nota
        let current_note_filter = if *self.floating_search_in_current_note.borrow() {
            self.current_note.as_ref().map(|n| {
                let name = n.name();
                println!("🔍 Filtrando por nota actual: '{}'", name);
                name
            })
        } else {
            None
        };

        // Verificar configuración de embeddings
        let embeddings_enabled = self.notes_config.borrow().get_embeddings_enabled();
        let has_api_key = self
            .notes_config
            .borrow()
            .get_embeddings_api_key()
            .is_some();

        // Realizar búsqueda semántica si está habilitada
        let semantic_results = if self.semantic_search_enabled
            && embeddings_enabled
            && has_api_key
            && query.len() >= 3
        {
            let all_results = self.perform_semantic_search(query);
            // Filtrar por nota actual si es necesario
            if let Some(ref note_name) = current_note_filter {
                all_results
                    .into_iter()
                    .filter(|r| {
                        let matches = &r.note_name == note_name
                            || r.note_name.ends_with(&format!("/{}", note_name))
                            || note_name.ends_with(&format!("/{}", r.note_name));
                        println!(
                            "  Comparando '{}' con '{}': {}",
                            r.note_name, note_name, matches
                        );
                        matches
                    })
                    .collect()
            } else {
                all_results
            }
        } else {
            Vec::new()
        };

        let has_semantic_results = !semantic_results.is_empty();

        // Si hay resultados semánticos, invocar al agente de IA para generar una respuesta
        if has_semantic_results && !*self.floating_search_in_current_note.borrow() {
            // Clonar datos necesarios para el mensaje async
            let query_clone = query.to_string();
            let results_clone = semantic_results.clone();
            sender.input(AppMsg::PerformSemanticSearchWithAI {
                query: query_clone,
                results: results_clone,
            });

            // No mostrar resultados de lista cuando se usa IA
            // Solo mostrar un mensaje de carga
            let loading_label = gtk::Label::builder()
                .label("🔄 El asistente de IA está analizando los resultados...")
                .margin_top(24)
                .margin_bottom(24)
                .margin_start(24)
                .margin_end(24)
                .justify(gtk::Justification::Center)
                .css_classes(vec!["dim-label"])
                .build();

            let row = gtk::ListBoxRow::builder()
                .selectable(false)
                .activatable(false)
                .child(&loading_label)
                .build();

            self.floating_search_results_list.append(&row);

            // Salir temprano, no mostrar resultados normales
            return;
        }

        // Realizar búsqueda tradicional si no hay semántica
        let traditional_results = if self.semantic_search_enabled && has_semantic_results {
            Vec::new()
        } else {
            match self.notes_db.search_notes(query) {
                Ok(results) => {
                    println!(
                        "📋 Búsqueda tradicional devolvió {} resultados",
                        results.len()
                    );
                    // Filtrar por nota actual si es necesario
                    if let Some(ref note_name) = current_note_filter {
                        results
                            .into_iter()
                            .filter(|r| {
                                let matches = &r.note_name == note_name
                                    || r.note_name.ends_with(&format!("/{}", note_name))
                                    || note_name.ends_with(&format!("/{}", r.note_name));
                                println!(
                                    "  Comparando '{}' con '{}': {}",
                                    r.note_name, note_name, matches
                                );
                                matches
                            })
                            .collect()
                    } else {
                        results
                    }
                }
                Err(e) => {
                    eprintln!("Error al buscar notas: {}", e);
                    Vec::new()
                }
            }
        };

        // Combinar resultados
        let combined_results =
            self.merge_search_results(semantic_results, traditional_results, query);

        if combined_results.is_empty() {
            // Mostrar mensaje de sin resultados
            let message = if *self.floating_search_in_current_note.borrow() {
                format!("No se encontraron resultados para '{}' en esta nota", query)
            } else {
                format!("No se encontraron resultados para '{}'", query)
            };

            let no_results = gtk::Label::builder()
                .label(&message)
                .margin_top(24)
                .margin_bottom(24)
                .margin_start(24)
                .margin_end(24)
                .justify(gtk::Justification::Center)
                .css_classes(vec!["dim-label"])
                .build();

            let row = gtk::ListBoxRow::builder()
                .selectable(false)
                .activatable(false)
                .child(&no_results)
                .build();

            self.floating_search_results_list.append(&row);
        } else {
            // Mostrar resultados
            for result in combined_results {
                let result_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .spacing(4)
                    .margin_top(12)
                    .margin_bottom(12)
                    .margin_start(12)
                    .margin_end(12)
                    .build();

                // Línea superior: nombre + badge de similitud (si existe)
                let title_row = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(8)
                    .build();

                let name_label = gtk::Label::builder()
                    .label(&result.note_name)
                    .xalign(0.0)
                    .hexpand(true)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build();
                name_label.add_css_class("heading");
                title_row.append(&name_label);

                if let Some(similarity) = result.similarity {
                    let similarity_badge = gtk::Label::builder()
                        .label(&format!("🧠 {:.0}%", similarity * 100.0))
                        .build();
                    similarity_badge.add_css_class("caption");
                    similarity_badge.add_css_class("accent");
                    title_row.append(&similarity_badge);
                }

                result_box.append(&title_row);

                // Snippet
                let snippet_label = gtk::Label::builder()
                    .label(&result.snippet)
                    .xalign(0.0)
                    .wrap(true)
                    .wrap_mode(gtk::pango::WrapMode::WordChar)
                    .lines(3)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build();
                snippet_label.add_css_class("dim-label");
                result_box.append(&snippet_label);

                let list_row = gtk::ListBoxRow::builder()
                    .selectable(true)
                    .activatable(true)
                    .child(&result_box)
                    .build();

                unsafe {
                    list_row.set_data("note_name", result.note_name.clone());
                }

                self.floating_search_rows
                    .borrow_mut()
                    .push(list_row.clone());
                self.floating_search_results_list.append(&list_row);
            }

            // Seleccionar automáticamente el primer resultado
            if let Some(first_row) = self.floating_search_rows.borrow().first().cloned() {
                self.floating_search_results_list
                    .select_row(Some(&first_row));
            }
        }
    }

    /// Combina resultados de búsqueda tradicional y semántica
    fn merge_search_results(
        &self,
        semantic_results: Vec<SearchResult>,
        traditional_results: Vec<SearchResult>,
        query: &str,
    ) -> Vec<SearchResult> {
        use std::collections::HashMap;

        let mut combined: HashMap<String, SearchResult> = HashMap::new();

        // Agregar resultados semánticos primero (prioridad)
        for result in semantic_results {
            combined.insert(result.note_path.clone(), result);
        }

        // Agregar resultados tradicionales que no estén ya presentes
        for result in traditional_results {
            if !combined.contains_key(&result.note_path) {
                combined.insert(result.note_path.clone(), result);
            }
        }

        // Convertir a vector y ordenar por relevancia/similitud
        let mut results: Vec<SearchResult> = combined.into_values().collect();

        results.sort_by(|a, b| {
            // Priorizar resultados con similarity (semánticos)
            match (a.similarity, b.similarity) {
                (Some(sa), Some(sb)) => sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => b
                    .relevance
                    .partial_cmp(&a.relevance)
                    .unwrap_or(std::cmp::Ordering::Equal),
            }
        });

        results
    }

    /// Indexa embeddings de una nota de forma asíncrona (no bloquea la UI)
    fn index_note_embeddings_async(&self, note_path: &std::path::Path, content: &str) {
        // Verificar que NoteMemory está inicializado
        let memory = match self.note_memory.borrow().as_ref() {
            Some(mem) => mem.clone(),
            None => {
                eprintln!("⚠️ NoteMemory no inicializado, no se puede indexar");
                return;
            }
        };

        let note_path_buf = note_path.to_path_buf();
        let content_string = content.to_string();
        let embedding_config = self.notes_config.borrow().get_embedding_config().clone();

        // Ejecutar en segundo plano para no bloquear la UI
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("⚠️ Error creando runtime para embeddings: {}", e);
                    return;
                }
            };

            rt.block_on(async {
                // Parse frontmatter to get metadata
                use crate::core::frontmatter::Frontmatter;
                let (frontmatter, _) = Frontmatter::parse_or_empty(&content_string);
                let metadata = serde_json::json!({
                    "tags": frontmatter.tags,
                    "path": note_path_buf.to_string_lossy()
                });

                // Chunking
                let chunk_config = crate::core::ChunkConfig {
                    max_tokens: embedding_config.max_chunk_tokens,
                    overlap_tokens: embedding_config.overlap_tokens,
                    ..Default::default()
                };
                let chunker = crate::core::TextChunker::with_config(chunk_config);
                let chunks = chunker.chunk_text(&content_string).unwrap_or_default();

                let mut success_count = 0;
                for (i, chunk) in chunks.iter().enumerate() {
                    let chunk_id = format!("{}#{}", note_path_buf.to_string_lossy(), i);
                    if let Err(e) = memory
                        .index_note(&chunk_id, &chunk.text, metadata.clone())
                        .await
                    {
                        eprintln!("⚠️ Error indexando chunk {} con RIG: {}", i, e);
                    } else {
                        success_count += 1;
                    }
                }

                if success_count > 0 {
                    println!(
                        "🧠 Nota indexada con RIG: {} ({} chunks)",
                        note_path_buf.display(),
                        success_count
                    );
                }
            });
        });
    }

    /// Muestra un diálogo modal centrado para crear una nueva nota
    fn show_create_note_dialog(&self, sender: &ComponentSender<Self>) {
        let i18n = self.i18n.borrow();

        // Crear ventana de diálogo centrada y compacta
        let dialog = gtk::Window::builder()
            .transient_for(&self.main_window)
            .modal(true)
            .default_width(360)
            .default_height(180)
            .resizable(false)
            .build();

        // Contenedor principal con márgenes
        let main_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .build();

        // Header con título
        let header = gtk::HeaderBar::builder()
            .title_widget(
                &gtk::Label::builder()
                    .label(&i18n.t("create_note_title"))
                    .build(),
            )
            .build();

        // Contenido
        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .margin_start(24)
            .margin_end(24)
            .margin_top(24)
            .margin_bottom(24)
            .vexpand(true)
            .valign(gtk::Align::Center)
            .build();

        let entry = gtk::Entry::builder()
            .placeholder_text(&i18n.t("note_name_hint"))
            .build();

        // Crear popover de autocompletado
        let completion_popover = gtk::Popover::builder()
            .autohide(false)
            .has_arrow(false)
            .build();
        completion_popover.set_parent(&entry);
        completion_popover.add_css_class("mention-completion");
        completion_popover.set_position(gtk::PositionType::Bottom);
        completion_popover.set_can_focus(false);
        completion_popover.set_focusable(false);

        let completion_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(vec!["navigation-sidebar"])
            .build();
        completion_list.show();

        let scrolled = gtk::ScrolledWindow::builder()
            .child(&completion_list)
            .min_content_width(300)
            .max_content_height(200)
            .build();
        scrolled.show();

        completion_popover.set_child(Some(&scrolled));

        // Obtener carpetas existentes escaneando el directorio RECURSIVAMENTE
        let mut folders: Vec<String> = Vec::new();

        fn scan_all_folders(
            path: &std::path::Path,
            root: &std::path::Path,
            folders_list: &mut Vec<String>,
        ) {
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_dir() {
                            // Ignorar carpetas ocultas (que empiezan con '.')
                            if let Some(name) = entry.file_name().to_str() {
                                if name.starts_with('.') {
                                    continue;
                                }
                            }
                            if let Ok(relative) = entry.path().strip_prefix(root) {
                                if let Some(folder_name) = relative.to_str() {
                                    if !folders_list.contains(&folder_name.to_string()) {
                                        folders_list.push(folder_name.to_string());
                                    }
                                    // Escanear recursivamente
                                    scan_all_folders(&entry.path(), root, folders_list);
                                }
                            }
                        }
                    }
                }
            }
        }

        let notes_root = self.notes_dir.root();
        scan_all_folders(notes_root, notes_root, &mut folders);
        folders.sort();
        println!("DEBUG: Found {} folders for autocomplete", folders.len());

        let hint_label = gtk::Label::builder()
            .label(&format!("<small>{}</small>", i18n.t("create_folder_hint")))
            .use_markup(true)
            .xalign(0.0)
            .build();
        hint_label.add_css_class("dim-label");

        // Botones
        let button_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .halign(gtk::Align::End)
            .margin_top(8)
            .build();

        let cancel_button = gtk::Button::builder().label(&i18n.t("cancel")).build();

        let create_button = gtk::Button::builder().label(&i18n.t("create")).build();
        create_button.add_css_class("suggested-action");

        button_box.append(&cancel_button);
        button_box.append(&create_button);

        content_box.append(&entry);
        content_box.append(&hint_label);
        content_box.append(&button_box);

        main_box.append(&header);
        main_box.append(&content_box);

        dialog.set_child(Some(&main_box));

        // Conectar botones
        let dialog_clone = dialog.clone();
        cancel_button.connect_clicked(move |_| {
            dialog_clone.close();
        });

        let dialog_clone2 = dialog.clone();
        create_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            entry,
            move |_| {
                let text = entry.text();
                let name = text.trim();

                if !name.is_empty() {
                    sender.input(AppMsg::CreateNewNote(name.to_string()));
                    dialog_clone2.close();
                }
            }
        ));

        // Autocompletado mientras escribe
        let folders_clone = Rc::new(folders);
        entry.connect_changed(gtk::glib::clone!(
            #[strong]
            completion_list,
            #[strong]
            completion_popover,
            #[strong]
            folders_clone,
            #[strong]
            entry,
            move |e| {
                let text = e.text().to_string();

                // Buscar el último componente después de '/'
                let parts: Vec<&str> = text.split('/').collect();
                let current_part = parts.last().unwrap_or(&"");

                // Si hay texto antes del último '/', es el prefijo de carpeta
                let folder_prefix = if parts.len() > 1 {
                    parts[..parts.len() - 1].join("/")
                } else {
                    String::new()
                };

                // Filtrar carpetas que coincidan (case-insensitive)
                let mut matches: Vec<String> = Vec::new();
                let current_part_lower = current_part.to_lowercase();

                // Si estamos escribiendo después de '/', mostrar carpetas que coincidan
                if !current_part.is_empty() || text.ends_with('/') {
                    for folder in folders_clone.iter() {
                        let folder_lower = folder.to_lowercase();

                        // Si ya hay un prefijo, solo mostrar subcarpetas
                        if !folder_prefix.is_empty() {
                            if folder.starts_with(&folder_prefix) && folder != &folder_prefix {
                                matches.push(folder.clone());
                            }
                        } else if folder_lower.contains(&current_part_lower) {
                            matches.push(folder.clone());
                        }
                    }
                }
                println!("DEBUG: Text '{}', Matches: {}", text, matches.len());

                // Actualizar lista de sugerencias
                while let Some(child) = completion_list.first_child() {
                    completion_list.remove(&child);
                }

                if !matches.is_empty() {
                    for folder in matches.iter().take(8) {
                        let label = gtk::Label::builder()
                            .label(folder)
                            .xalign(0.0)
                            .margin_start(8)
                            .margin_end(8)
                            .margin_top(4)
                            .margin_bottom(4)
                            .build();
                        label.show();

                        let row = gtk::ListBoxRow::builder().child(&label).build();
                        row.show();

                        // Guardar el folder en el row
                        unsafe {
                            row.set_data("folder", folder.clone());
                        }

                        completion_list.append(&row);
                    }
                    completion_popover.popup();
                } else {
                    completion_popover.popdown();
                }
            }
        ));

        // Seleccionar carpeta con Enter cuando hay sugerencias
        let completion_list_clone = completion_list.clone();
        let completion_popover_clone = completion_popover.clone();
        completion_list.connect_row_activated(gtk::glib::clone!(
            #[strong]
            entry,
            #[strong]
            completion_popover_clone,
            move |_, row| {
                if let Some(folder) =
                    unsafe { row.data::<String>("folder").map(|d| d.as_ref().clone()) }
                {
                    // Reemplazar el texto con la carpeta + '/'
                    entry.set_text(&format!("{}/", folder));
                    entry.set_position(-1); // Cursor al final
                    completion_popover_clone.popdown();
                }
            }
        ));

        // Navegación con flechas en el entry
        let entry_for_keys = entry.clone();
        let entry_key_controller = gtk::EventControllerKey::new();
        entry_key_controller.connect_key_pressed(gtk::glib::clone!(
            #[strong]
            completion_list_clone,
            #[strong]
            completion_popover_clone,
            #[strong]
            entry_for_keys,
            move |_, keyval, _, _| {
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

                if completion_popover_clone.is_visible() {
                    match key_name.as_str() {
                        "Down" => {
                            let selected = completion_list_clone.selected_row();
                            let next_index = if let Some(row) = selected {
                                row.index() + 1
                            } else {
                                0
                            };

                            if let Some(next_row) = completion_list_clone.row_at_index(next_index) {
                                completion_list_clone.select_row(Some(&next_row));
                            }
                            return gtk::glib::Propagation::Stop;
                        }
                        "Up" => {
                            let selected = completion_list_clone.selected_row();
                            let prev_index = if let Some(row) = selected {
                                if row.index() > 0 { row.index() - 1 } else { 0 }
                            } else {
                                0
                            };

                            if let Some(prev_row) = completion_list_clone.row_at_index(prev_index) {
                                completion_list_clone.select_row(Some(&prev_row));
                            }
                            return gtk::glib::Propagation::Stop;
                        }
                        "Tab" | "Return" => {
                            // Tab o Enter: completar con la fila seleccionada o la primera
                            let row_to_activate = completion_list_clone
                                .selected_row()
                                .or_else(|| completion_list_clone.row_at_index(0));

                            if let Some(row) = row_to_activate {
                                if let Some(folder) = unsafe {
                                    row.data::<String>("folder").map(|d| d.as_ref().clone())
                                } {
                                    entry_for_keys.set_text(&format!("{}/", folder));
                                    entry_for_keys.set_position(-1);
                                    completion_popover_clone.popdown();
                                    return gtk::glib::Propagation::Stop;
                                }
                            }

                            completion_popover_clone.popdown();
                            return gtk::glib::Propagation::Stop;
                        }
                        "Escape" => {
                            completion_popover_clone.popdown();
                            return gtk::glib::Propagation::Stop;
                        }
                        _ => {}
                    }
                }

                gtk::glib::Propagation::Proceed
            }
        ));
        entry.add_controller(entry_key_controller);

        // Enter también crea la nota
        let dialog_clone3 = dialog.clone();
        entry.connect_activate(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            completion_popover,
            move |entry| {
                // Si el popover está visible, no crear la nota
                if completion_popover.is_visible() {
                    completion_popover.popdown();
                    return;
                }

                let text = entry.text();
                let name = text.trim();

                if !name.is_empty() {
                    sender.input(AppMsg::CreateNewNote(name.to_string()));
                    dialog_clone3.close();
                }
            }
        ));

        // ESC cierra el diálogo
        let esc_controller = gtk::EventControllerKey::new();
        let dialog_clone4 = dialog.clone();
        esc_controller.connect_key_pressed(move |_, keyval, _, _| {
            let key_name = keyval.name().map(|s| s.to_string());
            if key_name.as_deref() == Some("Escape") {
                dialog_clone4.close();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        dialog.add_controller(esc_controller);

        // Mostrar el diálogo
        dialog.present();

        // Dar foco al entry
        gtk::glib::source::timeout_add_local(std::time::Duration::from_millis(100), move || {
            entry.grab_focus();
            gtk::glib::ControlFlow::Break
        });
    }

    fn show_insert_image_dialog(&self, sender: &ComponentSender<Self>) {
        use gtk::{FileChooserAction, FileChooserDialog, ResponseType};

        // Crear diálogo de selección de archivo
        let dialog = FileChooserDialog::new(
            Some("Seleccionar imagen"),
            Some(&self.main_window),
            FileChooserAction::Open,
            &[
                ("Cancelar", ResponseType::Cancel),
                ("Abrir", ResponseType::Accept),
            ],
        );

        // Crear filtro para imágenes
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Imágenes"));
        filter.add_mime_type("image/*");
        filter.add_pattern("*.png");
        filter.add_pattern("*.jpg");
        filter.add_pattern("*.jpeg");
        filter.add_pattern("*.gif");
        filter.add_pattern("*.webp");
        filter.add_pattern("*.svg");
        dialog.add_filter(&filter);

        let sender_clone = sender.clone();
        dialog.connect_response(move |dialog, response| {
            if response == ResponseType::Accept {
                if let Some(file) = dialog.file() {
                    if let Some(path) = file.path() {
                        if let Some(path_str) = path.to_str() {
                            sender_clone.input(AppMsg::InsertImageFromPath(path_str.to_string()));
                        }
                    }
                }
            }
            dialog.close();
        });

        dialog.show();
    }

    fn save_texture_and_insert(
        texture: &gtk::gdk::Texture,
        sender: &ComponentSender<Self>,
    ) -> anyhow::Result<()> {
        use chrono::Local;

        // Asegurarse de que el directorio de assets existe
        let assets_dir = NotesConfig::ensure_assets_dir()?;

        // Generar nombre único basado en timestamp
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("clipboard_{}.png", timestamp);
        let dest_path = assets_dir.join(&filename);

        // Guardar la textura como archivo PNG
        texture.save_to_png(
            dest_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Path inválido"))?,
        )?;

        // Enviar mensaje para insertar
        sender.input(AppMsg::InsertImageFromPath(
            dest_path.to_string_lossy().to_string(),
        ));

        Ok(())
    }

    fn insert_image_from_path(&mut self, source_path: &str, sender: &ComponentSender<Self>) {
        use std::fs;
        use std::path::Path;

        // Asegurarse de que el directorio de assets existe
        let assets_dir = match NotesConfig::ensure_assets_dir() {
            Ok(dir) => dir,
            Err(e) => {
                eprintln!("Error creando directorio de assets: {}", e);
                return;
            }
        };

        let source = Path::new(source_path);

        // Si la imagen ya está en el directorio de assets, no copiarla
        let dest_path = if source.starts_with(&assets_dir) {
            source.to_path_buf()
        } else {
            // Obtener el nombre del archivo
            let filename = match source.file_name() {
                Some(name) => name.to_string_lossy().to_string(),
                None => {
                    eprintln!("No se pudo obtener el nombre del archivo");
                    return;
                }
            };

            // Generar nombre único si es necesario
            let mut dest_filename = filename.clone();
            let mut counter = 1;
            let mut path = assets_dir.join(&dest_filename);

            while path.exists() {
                let stem = Path::new(&filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("image");
                let ext = Path::new(&filename)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("png");
                dest_filename = format!("{}_{}.{}", stem, counter, ext);
                path = assets_dir.join(&dest_filename);
                counter += 1;
            }

            // Copiar la imagen al directorio de assets
            if let Err(e) = fs::copy(source_path, &path) {
                eprintln!("Error copiando imagen: {}", e);
                return;
            }

            path
        };

        // Insertar sintaxis markdown para la imagen
        let markdown_syntax = format!(
            "![{}]({})",
            dest_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("imagen"),
            dest_path.to_string_lossy()
        );

        // Insertar en la posición del cursor
        if *self.mode.borrow() != EditorMode::Insert {
            // Cambiar a modo Insert primero
            *self.mode.borrow_mut() = EditorMode::Insert;
        }

        self.buffer.insert(self.cursor_position, &markdown_syntax);
        self.cursor_position += markdown_syntax.chars().count();
        self.has_unsaved_changes = true;

        // Sincronizar vista
        self.sync_to_view();
        self.update_status_bar(sender);

        println!("Imagen insertada: {}", markdown_syntax);
    }

    /// Detecta si una URL apunta a una imagen basándose en la extensión
    fn is_image_url(url: &str) -> bool {
        let url_lower = url.to_lowercase();
        url_lower.ends_with(".png")
            || url_lower.ends_with(".jpg")
            || url_lower.ends_with(".jpeg")
            || url_lower.ends_with(".gif")
            || url_lower.ends_with(".webp")
            || url_lower.ends_with(".svg")
            || url_lower.ends_with(".bmp")
            || url_lower.ends_with(".ico")
    }

    /// Detecta si una URL es de YouTube y extrae el video ID
    /// Soporta formatos:
    /// - https://youtube.com/watch?v=VIDEO_ID
    /// - https://www.youtube.com/watch?v=VIDEO_ID
    /// - https://youtu.be/VIDEO_ID
    /// - https://youtube.com/shorts/VIDEO_ID
    fn extract_youtube_video_id(url: &str) -> Option<String> {
        use regex::Regex;

        let patterns = [
            // youtube.com/watch?v=VIDEO_ID
            r"(?:youtube\.com|www\.youtube\.com)/watch\?v=([a-zA-Z0-9_-]{11})",
            // youtu.be/VIDEO_ID
            r"youtu\.be/([a-zA-Z0-9_-]{11})",
            // youtube.com/shorts/VIDEO_ID
            r"(?:youtube\.com|www\.youtube\.com)/shorts/([a-zA-Z0-9_-]{11})",
        ];

        for pattern in &patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(captures) = re.captures(url) {
                    if let Some(video_id) = captures.get(1) {
                        return Some(video_id.as_str().to_string());
                    }
                }
            }
        }

        None
    }

    /// Obtiene la transcripción de un video de YouTube de forma asíncrona
    /// TODO: Implementar con una librería compatible o API alternativa
    async fn fetch_youtube_transcript(_video_id: &str) -> anyhow::Result<String> {
        // Por ahora, devolvemos un mensaje indicando que la función está pendiente
        Err(anyhow::anyhow!(
            "Transcripción de YouTube no disponible actualmente. Esta función se implementará en una futura actualización."
        ))
    }

    /// Muestra un diálogo preguntando si transcribir el video de YouTube
    fn show_transcribe_dialog(
        &self,
        url: String,
        video_id: String,
        sender: &ComponentSender<Self>,
    ) {
        let i18n = self.i18n.borrow();

        let dialog = gtk::Window::builder()
            .transient_for(&self.main_window)
            .modal(true)
            .title(&i18n.t("transcribe_youtube"))
            .default_width(450)
            .default_height(180)
            .build();

        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_start(24)
            .margin_end(24)
            .margin_top(20)
            .margin_bottom(20)
            .spacing(16)
            .build();

        // Icono y mensaje
        let header_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let icon = gtk::Image::from_icon_name("video-x-generic-symbolic");
        icon.set_pixel_size(48);
        header_box.append(&icon);

        let text_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();

        let title = gtk::Label::builder()
            .label(&i18n.t("youtube_detected"))
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();
        title.add_css_class("heading");
        text_box.append(&title);

        let video_id_label = gtk::Label::builder()
            .label(&format!("Video ID: {}", video_id))
            .halign(gtk::Align::Start)
            .build();
        video_id_label.add_css_class("dim-label");
        text_box.append(&video_id_label);

        header_box.append(&text_box);
        content_box.append(&header_box);

        // Botones
        let buttons_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .halign(gtk::Align::End)
            .margin_top(8)
            .build();

        let cancel_button = gtk::Button::builder().label(&i18n.t("cancel")).build();

        let only_link_button = gtk::Button::builder().label(&i18n.t("only_link")).build();

        let transcribe_button = gtk::Button::builder()
            .label(&i18n.t("transcribe_and_insert"))
            .build();
        transcribe_button.add_css_class("suggested-action");

        // Conectar botones
        let dialog_clone = dialog.clone();
        cancel_button.connect_clicked(move |_| {
            dialog_clone.close();
        });

        let dialog_clone = dialog.clone();
        let sender_clone = sender.clone();
        let video_id_clone = video_id.clone();
        only_link_button.connect_clicked(move |_| {
            sender_clone.input(AppMsg::InsertYouTubeLink(video_id_clone.clone()));
            dialog_clone.close();
        });

        let dialog_clone = dialog.clone();
        let sender_clone = sender.clone();
        let video_id_clone = video_id.clone();
        transcribe_button.connect_clicked(move |_| {
            sender_clone.input(AppMsg::InsertYouTubeWithTranscript {
                video_id: video_id_clone.clone(),
            });
            dialog_clone.close();
        });

        buttons_box.append(&cancel_button);
        buttons_box.append(&only_link_button);
        buttons_box.append(&transcribe_button);

        content_box.append(&buttons_box);

        dialog.set_child(Some(&content_box));
        dialog.present();
    }

    /// Inserta un enlace de YouTube sin transcripción
    fn insert_youtube_link(&mut self, video_id: &str, sender: &ComponentSender<Self>) {
        let youtube_url = format!("https://www.youtube.com/watch?v={}", video_id);
        let markdown_syntax = format!("[🎥 Ver video en YouTube]({})", youtube_url);

        self.buffer.insert(self.cursor_position, &markdown_syntax);
        self.cursor_position += markdown_syntax.chars().count();
        self.has_unsaved_changes = true;

        // Sincronizar vista
        self.sync_to_view();
        self.update_status_bar(sender);

        println!("Enlace de YouTube insertado: {}", video_id);
    }

    /// Inserta un enlace de YouTube con transcripción
    fn insert_youtube_with_transcript(&mut self, video_id: &str, sender: &ComponentSender<Self>) {
        let youtube_url = format!("https://www.youtube.com/watch?v={}", video_id);
        let i18n = self.i18n.borrow();

        // Obtener traducciones
        let transcript_title = i18n.t("transcript_section");
        let loading_text = i18n.t("downloading_transcript");
        drop(i18n); // Liberar el borrow antes de modificar el buffer

        // Mostrar mensaje de carga inmediatamente
        let loading_message = format!(
            "[🎥 Ver video en YouTube]({})\n\n## {}\n\n_{}_\n\n",
            youtube_url, transcript_title, loading_text
        );

        self.buffer.insert(self.cursor_position, &loading_message);
        self.cursor_position += loading_message.chars().count();
        self.has_unsaved_changes = true;
        self.sync_to_view();
        self.update_status_bar(sender);

        // Obtener la transcripción en un hilo separado
        let video_id_clone = video_id.to_string();
        let sender_clone = sender.clone();

        std::thread::spawn(move || {
            println!("Obteniendo transcripción para video: {}", video_id_clone);

            match crate::youtube_transcript::get_transcript(&video_id_clone) {
                Ok(transcript) => {
                    println!(
                        "Transcripción obtenida exitosamente ({} caracteres)",
                        transcript.len()
                    );

                    // Enviar mensaje para actualizar el contenido
                    let video_id_for_update = video_id_clone.clone();
                    gtk::glib::MainContext::default().invoke(move || {
                        sender_clone.input(AppMsg::UpdateTranscript {
                            video_id: video_id_for_update,
                            transcript,
                        });
                    });
                }
                Err(e) => {
                    eprintln!("Error obteniendo transcripción: {}", e);

                    let video_id_for_error = video_id_clone.clone();
                    let error_msg = format!("Error: {}", e);
                    gtk::glib::MainContext::default().invoke(move || {
                        sender_clone.input(AppMsg::UpdateTranscript {
                            video_id: video_id_for_error,
                            transcript: error_msg,
                        });
                    });
                }
            }
        });

        println!("Solicitando transcripción para video: {}", video_id);
    }

    /// Actualiza el contenido del buffer con la transcripción obtenida
    fn update_transcript(
        &mut self,
        video_id: &str,
        transcript: &str,
        sender: &ComponentSender<Self>,
    ) {
        let content = self.buffer.to_string();
        let youtube_url = format!("https://www.youtube.com/watch?v={}", video_id);
        let i18n = self.i18n.borrow();

        // Obtener traducciones
        let transcript_title = i18n.t("transcript_section");
        let loading_text = i18n.t("downloading_transcript");
        drop(i18n); // Liberar el borrow

        // Buscar y reemplazar el mensaje de carga con la transcripción real
        let loading_pattern = format!(
            "[🎥 Ver video en YouTube]({})\n\n## {}\n\n_{}_\n\n",
            youtube_url, transcript_title, loading_text
        );

        let replacement = if transcript.starts_with("Error:") {
            // Es un mensaje de error
            format!(
                "[🎥 Ver video en YouTube]({})\n\n## {}\n\n_{}_\n\n",
                youtube_url, transcript_title, transcript
            )
        } else {
            // Es la transcripción exitosa
            format!(
                "[🎥 Ver video en YouTube]({})\n\n## {}\n\n{}\n",
                youtube_url, transcript_title, transcript
            )
        };

        if let Some(pos) = content.find(&loading_pattern) {
            // Reemplazar el mensaje de carga con la transcripción
            let new_content = content.replace(&loading_pattern, &replacement);
            self.buffer = NoteBuffer::from_text(&new_content);
            self.has_unsaved_changes = true;

            // Sincronizar vista
            self.sync_to_view();
            self.update_status_bar(sender);

            println!("Transcripción actualizada en el buffer");
        } else {
            println!("No se encontró el patrón de carga para reemplazar");
        }
    }

    /// Descarga una imagen desde una URL y la guarda en assets
    fn download_image_from_url(url: &str) -> anyhow::Result<std::path::PathBuf> {
        use chrono::Local;
        use std::io::Write;

        // Asegurarse de que el directorio de assets existe
        let assets_dir = NotesConfig::ensure_assets_dir()?;

        // Descargar la imagen
        let response = reqwest::blocking::get(url)?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Error descargando imagen: {}",
                response.status()
            ));
        }

        let bytes = response.bytes()?;

        // Obtener extensión de la URL o usar .png por defecto
        let extension = url
            .rsplit('.')
            .next()
            .and_then(|ext| {
                // Eliminar query params si existen
                let clean_ext = ext.split('?').next().unwrap_or(ext);
                if clean_ext.len() <= 5 && clean_ext.chars().all(|c| c.is_alphanumeric()) {
                    Some(clean_ext)
                } else {
                    None
                }
            })
            .unwrap_or("png");

        // Generar nombre único basado en timestamp
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("web_image_{}.{}", timestamp, extension);
        let dest_path = assets_dir.join(&filename);

        // Guardar la imagen
        let mut file = std::fs::File::create(&dest_path)?;
        file.write_all(&bytes)?;

        Ok(dest_path)
    }

    /// Procesa texto pegado: si es una URL de imagen, la descarga
    fn process_pasted_text(&mut self, text: &str, sender: &ComponentSender<Self>) {
        let trimmed = text.trim();

        // Verificar primero si es una URL de YouTube
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            if let Some(video_id) = Self::extract_youtube_video_id(trimmed) {
                println!(
                    "Detectada URL de YouTube: {} (video_id: {})",
                    trimmed, video_id
                );

                // Preguntar si desea transcribir
                sender.input(AppMsg::AskTranscribeYouTube {
                    url: trimmed.to_string(),
                    video_id,
                });
                return;
            }
        }

        // Verificar si es una URL de imagen
        if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
            && Self::is_image_url(trimmed)
        {
            println!("Detectada URL de imagen: {}", trimmed);

            // Descargar la imagen en un hilo separado
            let url = trimmed.to_string();
            let sender_clone = sender.clone();

            std::thread::spawn(move || match Self::download_image_from_url(&url) {
                Ok(path) => {
                    println!("Imagen descargada: {:?}", path);
                    sender_clone.input(AppMsg::InsertImageFromPath(
                        path.to_string_lossy().to_string(),
                    ));
                }
                Err(e) => {
                    eprintln!("Error descargando imagen: {}", e);
                }
            });
        } else if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
            && !trimmed.contains(' ')
            && trimmed.len() > 10
        {
            // Es una URL normal (no es YouTube ni imagen), convertir a markdown
            println!("Detectada URL normal: {}", trimmed);

            // Intentar extraer un texto descriptivo del dominio
            let display_text = if let Some(domain) = trimmed.split('/').nth(2) {
                domain.to_string()
            } else {
                trimmed.to_string()
            };

            let markdown_link = format!("[{}]({})", display_text, trimmed);
            self.buffer.insert(self.cursor_position, &markdown_link);
            self.cursor_position += markdown_link.chars().count();
            self.has_unsaved_changes = true;
            self.sync_to_view();
            self.update_status_bar(sender);
        } else {
            // Si no es una URL, insertar como texto normal
            self.buffer.insert(self.cursor_position, text);
            self.cursor_position += text.chars().count();
            self.has_unsaved_changes = true;
            self.sync_to_view();
            self.update_status_bar(sender);
        }
    }

    fn show_preferences_dialog(&self, sender: &ComponentSender<Self>) {
        let i18n = self.i18n.borrow();

        let dialog = gtk::Window::builder()
            .transient_for(&self.main_window)
            .modal(true)
            .title(&i18n.t("preferences"))
            .default_width(600)
            .default_height(700)
            .build();

        // ScrolledWindow para que el contenido sea scrollable
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scrolled.set_vexpand(true);

        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_start(20)
            .margin_end(20)
            .margin_top(20)
            .margin_bottom(20)
            .spacing(16)
            .build();

        // Título
        let title = gtk::Label::builder()
            .label(&i18n.t("preferences"))
            .halign(gtk::Align::Start)
            .build();
        title.add_css_class("title-2");
        content_box.append(&title);

        // Sección de Idioma
        let language_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();

        let language_label = gtk::Label::builder()
            .label(&i18n.t("language"))
            .halign(gtk::Align::Start)
            .build();
        language_label.add_css_class("heading");
        language_box.append(&language_label);

        let language_description = gtk::Label::builder()
            .label(&i18n.t("language_description"))
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();
        language_description.add_css_class("dim-label");
        language_box.append(&language_description);

        // Dropdown de idioma
        let language_dropdown = gtk::DropDown::from_strings(&["Español", "English"]);
        let current_lang = i18n.current_language();
        language_dropdown.set_selected(match current_lang {
            Language::Spanish => 0,
            Language::English => 1,
        });

        language_dropdown.connect_selected_notify(gtk::glib::clone!(
            #[strong]
            sender,
            move |dropdown| {
                let selected = dropdown.selected();
                let new_language = match selected {
                    0 => Language::Spanish,
                    1 => Language::English,
                    _ => Language::Spanish,
                };
                sender.input(AppMsg::ChangeLanguage(new_language));
            }
        ));

        language_box.append(&language_dropdown);
        content_box.append(&language_box);

        content_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        // Sección de Directorio de trabajo
        let workspace_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();

        let workspace_label = gtk::Label::builder()
            .label(&i18n.t("workspace"))
            .halign(gtk::Align::Start)
            .build();
        workspace_label.add_css_class("heading");
        workspace_box.append(&workspace_label);

        let workspace_description = gtk::Label::builder()
            .label(&i18n.t("workspace_description"))
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();
        workspace_description.add_css_class("dim-label");
        workspace_box.append(&workspace_description);

        // Mostrar ubicación actual
        let current_location = self.notes_dir.root().to_string_lossy().to_string();
        let location_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let location_label = gtk::Label::builder()
            .label(&format!(
                "{}: {}",
                i18n.t("workspace_location"),
                current_location
            ))
            .halign(gtk::Align::Start)
            .hexpand(true)
            .wrap(true)
            .build();
        location_label.add_css_class("dim-label");

        let change_button = gtk::Button::builder()
            .label(&i18n.t("change_workspace"))
            .build();

        let notes_dir_root = self.notes_dir.root().to_path_buf();
        let select_folder_text = i18n.t("select_workspace_folder");
        let cancel_text = i18n.t("cancel");
        let select_text = i18n.t("select");

        change_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            dialog,
            #[strong]
            select_folder_text,
            #[strong]
            cancel_text,
            #[strong]
            select_text,
            move |_| {
                // Crear diálogo para seleccionar carpeta
                let folder_dialog = gtk::FileChooserDialog::new(
                    Some(&select_folder_text),
                    Some(&dialog),
                    gtk::FileChooserAction::SelectFolder,
                    &[
                        (&cancel_text, gtk::ResponseType::Cancel),
                        (&select_text, gtk::ResponseType::Accept),
                    ],
                );

                // Establecer la carpeta actual como punto de inicio
                let _ = folder_dialog
                    .set_current_folder(Some(&gtk::gio::File::for_path(&notes_dir_root)));

                folder_dialog.connect_response(gtk::glib::clone!(
                    #[strong]
                    sender,
                    move |dialog, response| {
                        if response == gtk::ResponseType::Accept {
                            if let Some(folder) = dialog.file() {
                                if let Some(path) = folder.path() {
                                    // TODO: Implementar cambio de workspace
                                    // Por ahora solo mostramos un mensaje
                                    println!("Nueva carpeta seleccionada: {:?}", path);
                                    // La implementación completa requeriría:
                                    // 1. Guardar la nueva ruta en NotesConfig
                                    // 2. Reiniciar la aplicación o recargar NotesDirectory
                                }
                            }
                        }
                        dialog.close();
                    }
                ));

                folder_dialog.show();
            }
        ));

        location_box.append(&location_label);
        location_box.append(&change_button);
        workspace_box.append(&location_box);

        content_box.append(&workspace_box);

        content_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        // Sección de Inicio en segundo plano
        let background_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();

        let background_label = gtk::Label::builder()
            .label(&i18n.t("start_in_background"))
            .halign(gtk::Align::Start)
            .build();
        background_label.add_css_class("heading");
        background_box.append(&background_label);

        let background_switch_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let background_desc = gtk::Label::builder()
            .label(&i18n.t("start_in_background_desc"))
            .halign(gtk::Align::Start)
            .hexpand(true)
            .wrap(true)
            .build();
        background_desc.add_css_class("dim-label");

        let background_switch = gtk::Switch::builder()
            .active(self.notes_config.borrow().get_start_in_background())
            .valign(gtk::Align::Center)
            .build();

        background_switch.connect_state_set(gtk::glib::clone!(
            #[strong]
            sender,
            move |_, state| {
                sender.input(AppMsg::SetStartInBackground(state));
                gtk::glib::Propagation::Proceed
            }
        ));

        background_switch_box.append(&background_desc);
        background_switch_box.append(&background_switch);
        background_box.append(&background_switch_box);

        content_box.append(&background_box);

        content_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        // Sección de Tema
        let theme_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();

        let theme_label = gtk::Label::builder()
            .label(&i18n.t("theme"))
            .halign(gtk::Align::Start)
            .build();
        theme_label.add_css_class("heading");
        theme_box.append(&theme_label);

        let theme_description = gtk::Label::builder()
            .label(&i18n.t("theme_sync"))
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();
        theme_description.add_css_class("dim-label");
        theme_box.append(&theme_description);

        content_box.append(&theme_box);

        content_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        // Sección de Markdown
        let markdown_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();

        let markdown_label = gtk::Label::builder()
            .label(&i18n.t("markdown_rendering"))
            .halign(gtk::Align::Start)
            .build();
        markdown_label.add_css_class("heading");
        markdown_box.append(&markdown_label);

        let markdown_switch_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let markdown_desc = gtk::Label::builder()
            .label(&i18n.t("markdown_enabled"))
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();
        markdown_desc.add_css_class("dim-label");

        markdown_switch_box.append(&markdown_desc);
        markdown_box.append(&markdown_switch_box);

        content_box.append(&markdown_box);

        content_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        // Sección de Salida de Audio
        let audio_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();

        let audio_label = gtk::Label::builder()
            .label(&i18n.t("audio_output"))
            .halign(gtk::Align::Start)
            .build();
        audio_label.add_css_class("heading");
        audio_box.append(&audio_label);

        let audio_description = gtk::Label::builder()
            .label(&i18n.t("audio_output_description"))
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();
        audio_description.add_css_class("dim-label");
        audio_box.append(&audio_description);

        // Dropdown de salidas de audio
        let sinks = self.get_available_audio_sinks();

        if !sinks.is_empty() {
            let sink_names: Vec<String> = sinks.iter().map(|(_, desc)| desc.clone()).collect();
            let sink_ids: Vec<String> = sinks.iter().map(|(id, _)| id.clone()).collect();

            let audio_dropdown = gtk::DropDown::from_strings(
                &sink_names.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            );

            // Seleccionar la salida actual
            let config_borrow = self.notes_config.borrow();
            let current_sink = config_borrow.get_audio_output_sink().map(|s| s.to_string());
            drop(config_borrow); // Liberar el borrow antes de usarlo

            if let Some(current) = current_sink {
                if let Some(pos) = sink_ids.iter().position(|id| id == &current) {
                    audio_dropdown.set_selected(pos as u32);
                }
            }

            let sender_clone = sender.clone();
            audio_dropdown.connect_selected_notify(move |dropdown| {
                let selected = dropdown.selected() as usize;
                if selected < sink_ids.len() {
                    let sink_id = &sink_ids[selected];

                    // Aplicar el cambio usando pactl
                    let success = MainApp::set_default_audio_sink(sink_id);

                    if success {
                        // Cargar configuración actual, modificarla y guardarla
                        if let Ok(mut config) = NotesConfig::load(NotesConfig::default_path()) {
                            config.set_audio_output_sink(Some(sink_id.clone()));

                            if let Err(e) = config.save(NotesConfig::default_path()) {
                                eprintln!("Error guardando configuración de audio: {}", e);
                            } else {
                                println!("Configuración de audio guardada: {}", sink_id);
                                // Recargar la configuración en memoria
                                sender_clone.input(AppMsg::ReloadConfig);
                            }
                        } else {
                            eprintln!("Error cargando configuración para actualizar audio");
                        }
                    } else {
                        eprintln!("Error al cambiar la salida de audio");
                    }
                }
            });

            audio_box.append(&audio_dropdown);
        } else {
            let no_sinks_label = gtk::Label::builder()
                .label("No se encontraron salidas de audio disponibles")
                .halign(gtk::Align::Start)
                .build();
            no_sinks_label.add_css_class("dim-label");
            audio_box.append(&no_sinks_label);
        }

        content_box.append(&audio_box);

        content_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        // Sección de AI Assistant
        let ai_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();

        let ai_label = gtk::Label::builder()
            .label("AI Assistant")
            .halign(gtk::Align::Start)
            .build();
        ai_label.add_css_class("heading");
        ai_box.append(&ai_label);

        let ai_description = gtk::Label::builder()
            .label("Configura la API key y modelo para el asistente de IA")
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();
        ai_description.add_css_class("dim-label");
        ai_box.append(&ai_description);

        // API Key
        let api_key_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let api_key_label = gtk::Label::builder()
            .label("API Key:")
            .halign(gtk::Align::Start)
            .width_chars(12)
            .build();

        let api_key_entry = gtk::Entry::builder()
            .hexpand(true)
            .placeholder_text("sk-...")
            .visibility(false)
            .build();

        // Cargar API key actual
        if let Some(key) = &self.notes_config.borrow().get_ai_config().api_key {
            api_key_entry.set_text(key);
        }

        let sender_clone = sender.clone();
        api_key_entry.connect_changed(move |entry| {
            let api_key = entry.text().to_string();
            if let Ok(mut config) = NotesConfig::load(NotesConfig::default_path()) {
                config.set_ai_api_key(if api_key.is_empty() {
                    None
                } else {
                    Some(api_key)
                });
                let _ = config.save(NotesConfig::default_path());
                sender_clone.input(AppMsg::ReloadConfig);
            }
        });

        api_key_box.append(&api_key_label);
        api_key_box.append(&api_key_entry);
        ai_box.append(&api_key_box);

        // Provider dropdown
        let provider_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let provider_label = gtk::Label::builder()
            .label("Proveedor:")
            .halign(gtk::Align::Start)
            .width_chars(12)
            .build();

        let provider_dropdown =
            gtk::DropDown::from_strings(&["OpenRouter", "OpenAI", "Anthropic", "Ollama"]);
        let current_provider = self.notes_config.borrow().get_ai_config().provider.clone();
        provider_dropdown.set_selected(match current_provider.as_str() {
            "openai" => 1,
            "anthropic" => 2,
            "ollama" => 3,
            _ => 0, // openrouter por defecto
        });

        let sender_clone = sender.clone();
        provider_dropdown.connect_selected_notify(move |dropdown| {
            let provider = match dropdown.selected() {
                1 => "openai",
                2 => "anthropic",
                3 => "ollama",
                _ => "openrouter",
            };
            if let Ok(mut config) = NotesConfig::load(NotesConfig::default_path()) {
                config.set_ai_provider(provider.to_string());
                let _ = config.save(NotesConfig::default_path());
                sender_clone.input(AppMsg::ReloadConfig);
            }
        });

        provider_box.append(&provider_label);
        provider_box.append(&provider_dropdown);
        ai_box.append(&provider_box);

        // Model dropdown (cargando dinámicamente)
        let model_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();

        let model_header_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let model_label = gtk::Label::builder()
            .label(&i18n.t("model_label"))
            .halign(gtk::Align::Start)
            .width_chars(12)
            .build();

        let refresh_models_btn = gtk::Button::builder()
            .label("🔄")
            .tooltip_text(&i18n.t("refresh_models_tooltip"))
            .build();
        refresh_models_btn.add_css_class("flat");
        refresh_models_btn.add_css_class("circular");

        model_header_box.append(&model_label);
        model_header_box.append(&refresh_models_btn);
        model_box.append(&model_header_box);

        // Buscador de modelos
        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text(&i18n.t("search_model_placeholder"))
            .build();
        model_box.append(&search_entry);

        // Lista de modelos (usamos ListBox con scroll para mejor control)
        let models_scroll = gtk::ScrolledWindow::builder()
            .height_request(300)
            .vexpand(false)
            .hexpand(true)
            .build();
        models_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

        let models_listbox = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        models_listbox.add_css_class("boxed-list");

        models_scroll.set_child(Some(&models_listbox));

        // Modelos iniciales básicos
        let initial_models = vec![
            ("google/gemini-flash-1.5", "Gratis • 1M ctx 🖼️"),
            ("google/gemini-pro-1.5", "$1.25/1M • 2M ctx 🖼️"),
            ("anthropic/claude-3.5-sonnet", "$3.00/1M • 200K ctx 🖼️"),
            ("openai/gpt-4o", "$2.50/1M • 128K ctx 🖼️"),
            ("openai/gpt-4o-mini", "$0.15/1M • 128K ctx"),
            ("meta-llama/llama-3.1-70b-instruct", "$0.59/1M • 131K ctx"),
            ("qwen/qwen-2.5-72b-instruct", "Gratis • 32K ctx"),
            ("mistralai/mistral-small", "$0.20/1M • 32K ctx"),
            ("google/gemma-2-9b-it", "Gratis • 8K ctx"),
            ("meta-llama/llama-3.2-3b-instruct", "Gratis • 128K ctx"),
        ];

        let current_model = self.notes_config.borrow().get_ai_config().model.clone();
        let mut selected_row_index = 0;

        // Almacenar referencias a modelos
        let all_models = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));

        // Poblar lista inicial
        for (i, (model_id, info)) in initial_models.iter().enumerate() {
            let row = gtk::ListBoxRow::new();
            let box_row = gtk::Box::new(gtk::Orientation::Vertical, 4);
            box_row.set_margin_all(8);

            let id_label = gtk::Label::new(Some(model_id));
            id_label.set_xalign(0.0);
            id_label.add_css_class("heading");

            let info_label = gtk::Label::new(Some(info));
            info_label.set_xalign(0.0);
            info_label.add_css_class("caption");
            info_label.add_css_class("dim-label");

            box_row.append(&id_label);
            box_row.append(&info_label);
            row.set_child(Some(&box_row));

            models_listbox.append(&row);

            if *model_id == current_model {
                selected_row_index = i;
            }

            all_models.borrow_mut().push(model_id.to_string());
        }

        // Seleccionar modelo actual
        if let Some(row) = models_listbox.row_at_index(selected_row_index as i32) {
            models_listbox.select_row(Some(&row));
        }

        // Conectar selección de modelo - extraer ID directamente del label
        let sender_clone = sender.clone();
        models_listbox.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                // Verificar que no sea un separador
                if !row.is_selectable() {
                    return;
                }

                // Extraer el ID del modelo del label
                if let Some(child) = row.child() {
                    if let Ok(box_row) = child.downcast::<gtk::Box>() {
                        if let Some(label_widget) = box_row.first_child() {
                            if let Ok(id_label) = label_widget.downcast::<gtk::Label>() {
                                let model_id = id_label.text().to_string();

                                // Guardar configuración
                                if let Ok(mut config) =
                                    NotesConfig::load(NotesConfig::default_path())
                                {
                                    println!("💾 Guardando modelo seleccionado: {}", model_id);
                                    config.set_ai_model(model_id.clone());
                                    let _ = config.save(NotesConfig::default_path());
                                    sender_clone.input(AppMsg::ReloadConfig);
                                }
                            }
                        }
                    }
                }
            }
        });

        // Implementar búsqueda
        let listbox_clone = models_listbox.clone();
        let all_models_search = all_models.clone();
        let full_models = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let full_models_clone = full_models.clone();
        let i18n_search = self.i18n.clone();

        search_entry.connect_search_changed(move |entry| {
            let query = entry.text().to_string();
            let full = full_models_clone.borrow();

            // Si no hay modelos completos cargados, buscar en los iniciales
            if full.is_empty() {
                // Filtrar filas existentes
                let mut index = 0;
                while let Some(row) = listbox_clone.row_at_index(index) {
                    if let Some(child) = row.child() {
                        if let Ok(box_row) = child.downcast::<gtk::Box>() {
                            if let Some(label) = box_row.first_child() {
                                if let Ok(id_label) = label.downcast::<gtk::Label>() {
                                    let model_id = id_label.text().to_string();
                                    let visible = query.is_empty()
                                        || model_id.to_lowercase().contains(&query.to_lowercase());
                                    row.set_visible(visible);
                                }
                            }
                        }
                    }
                    index += 1;
                }
            } else {
                // Buscar en modelos completos
                let filtered = crate::ai_chat::search_models(&full, &query);

                // Limpiar lista
                while let Some(row) = listbox_clone.row_at_index(0) {
                    listbox_clone.remove(&row);
                }

                // Separar en gratuitos y de pago
                let free_filtered: Vec<_> = filtered
                    .iter()
                    .filter(|m| m.pricing.prompt == "0" || m.pricing.prompt.starts_with("0.00"))
                    .collect();
                let paid_filtered: Vec<_> = filtered
                    .iter()
                    .filter(|m| m.pricing.prompt != "0" && !m.pricing.prompt.starts_with("0.00"))
                    .collect();

                // Mostrar gratuitos primero
                if !free_filtered.is_empty() {
                    let separator = gtk::ListBoxRow::new();
                    separator.set_selectable(false);
                    separator.set_activatable(false);
                    let sep_label =
                        gtk::Label::new(Some(&i18n_search.borrow().t("ai_free_models")));
                    sep_label.add_css_class("heading");
                    sep_label.set_margin_all(8);
                    separator.set_child(Some(&sep_label));
                    listbox_clone.append(&separator);

                    for model in free_filtered.iter() {
                        let row = gtk::ListBoxRow::new();
                        let box_row = gtk::Box::new(gtk::Orientation::Vertical, 4);
                        box_row.set_margin_all(8);

                        let id_label = gtk::Label::new(Some(&model.id));
                        id_label.set_xalign(0.0);
                        id_label.add_css_class("heading");

                        let info = crate::ai_chat::format_model_display(model);
                        let info_label = gtk::Label::new(Some(&info));
                        info_label.set_xalign(0.0);
                        info_label.add_css_class("caption");
                        info_label.add_css_class("dim-label");

                        box_row.append(&id_label);
                        box_row.append(&info_label);

                        let tooltip = crate::ai_chat::format_model_tooltip(model);
                        row.set_tooltip_text(Some(&tooltip));

                        row.set_child(Some(&box_row));
                        listbox_clone.append(&row);
                    }
                }

                // Mostrar de pago después (sin límite)
                if !paid_filtered.is_empty() {
                    let separator = gtk::ListBoxRow::new();
                    separator.set_selectable(false);
                    separator.set_activatable(false);
                    let sep_label =
                        gtk::Label::new(Some(&i18n_search.borrow().t("ai_paid_models")));
                    sep_label.add_css_class("heading");
                    sep_label.set_margin_all(8);
                    separator.set_child(Some(&sep_label));
                    listbox_clone.append(&separator);

                    for model in paid_filtered.iter() {
                        let row = gtk::ListBoxRow::new();
                        let box_row = gtk::Box::new(gtk::Orientation::Vertical, 4);
                        box_row.set_margin_all(8);

                        let id_label = gtk::Label::new(Some(&model.id));
                        id_label.set_xalign(0.0);
                        id_label.add_css_class("heading");

                        let info = crate::ai_chat::format_model_display(model);
                        let info_label = gtk::Label::new(Some(&info));
                        info_label.set_xalign(0.0);
                        info_label.add_css_class("caption");
                        info_label.add_css_class("dim-label");

                        box_row.append(&id_label);
                        box_row.append(&info_label);

                        let tooltip = crate::ai_chat::format_model_tooltip(model);
                        row.set_tooltip_text(Some(&tooltip));

                        row.set_child(Some(&box_row));
                        listbox_clone.append(&row);
                    }
                }

                // Mensaje si no hay resultados
                if free_filtered.is_empty() && paid_filtered.is_empty() {
                    let row = gtk::ListBoxRow::new();
                    row.set_selectable(false);
                    row.set_activatable(false);
                    let label = gtk::Label::new(Some("No se encontraron modelos"));
                    label.add_css_class("dim-label");
                    label.set_margin_all(16);
                    row.set_child(Some(&label));
                    listbox_clone.append(&row);
                }
            }
        });

        // Botón para cargar modelos desde OpenRouter
        let listbox_refresh = models_listbox.clone();
        let sender_clone2 = sender.clone();
        let all_models_refresh = all_models.clone();
        let full_models_refresh = full_models.clone();
        let search_clone = search_entry.clone();
        let i18n_refresh = self.i18n.clone();

        refresh_models_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            btn.set_label("⏳");

            let listbox = listbox_refresh.clone();
            let btn_clone = btn.clone();
            let sender = sender_clone2.clone();
            let all_models_ref = all_models_refresh.clone();
            let full_models_ref = full_models_refresh.clone();
            let search_ref = search_clone.clone();
            let i18n = i18n_refresh.clone();

            gtk::glib::spawn_future_local(async move {
                match crate::ai_chat::fetch_openrouter_models().await {
                    Ok(mut models) => {
                        // Ordenar por ID
                        models.sort_by(|a, b| a.id.cmp(&b.id));

                        // Guardar modelos completos
                        *full_models_ref.borrow_mut() = models.clone();

                        // Limpiar lista actual
                        while let Some(row) = listbox.row_at_index(0) {
                            listbox.remove(&row);
                        }

                        // Separar modelos gratuitos y de pago
                        let free_models: Vec<_> = models
                            .iter()
                            .filter(|m| {
                                m.pricing.prompt == "0" || m.pricing.prompt.starts_with("0.00")
                            })
                            .collect();
                        let paid_models: Vec<_> = models
                            .iter()
                            .filter(|m| {
                                m.pricing.prompt != "0" && !m.pricing.prompt.starts_with("0.00")
                            })
                            .collect();

                        let mut model_ids = Vec::new();

                        // Agregar sección de modelos gratuitos
                        if !free_models.is_empty() {
                            let separator = gtk::ListBoxRow::new();
                            separator.set_selectable(false);
                            separator.set_activatable(false);
                            let sep_label =
                                gtk::Label::new(Some(&i18n.borrow().t("ai_free_models")));
                            sep_label.add_css_class("heading");
                            sep_label.set_margin_all(8);
                            separator.set_child(Some(&sep_label));
                            listbox.append(&separator);

                            for model in free_models.iter() {
                                let row = gtk::ListBoxRow::new();
                                let box_row = gtk::Box::new(gtk::Orientation::Vertical, 4);
                                box_row.set_margin_all(8);

                                let id_label = gtk::Label::new(Some(&model.id));
                                id_label.set_xalign(0.0);
                                id_label.add_css_class("heading");

                                let info = crate::ai_chat::format_model_display(model);
                                let info_label = gtk::Label::new(Some(&info));
                                info_label.set_xalign(0.0);
                                info_label.add_css_class("caption");
                                info_label.add_css_class("dim-label");

                                box_row.append(&id_label);
                                box_row.append(&info_label);

                                // Tooltip con info completa
                                let tooltip = crate::ai_chat::format_model_tooltip(model);
                                row.set_tooltip_text(Some(&tooltip));

                                row.set_child(Some(&box_row));
                                listbox.append(&row);
                                model_ids.push(model.id.clone());
                            }
                        }

                        // Agregar sección de modelos de pago (sin límite)
                        if !paid_models.is_empty() {
                            let separator = gtk::ListBoxRow::new();
                            separator.set_selectable(false);
                            separator.set_activatable(false);
                            let sep_label =
                                gtk::Label::new(Some(&i18n.borrow().t("ai_paid_models")));
                            sep_label.add_css_class("heading");
                            sep_label.set_margin_all(8);
                            separator.set_child(Some(&sep_label));
                            listbox.append(&separator);

                            for model in paid_models.iter() {
                                let row = gtk::ListBoxRow::new();
                                let box_row = gtk::Box::new(gtk::Orientation::Vertical, 4);
                                box_row.set_margin_all(8);

                                let id_label = gtk::Label::new(Some(&model.id));
                                id_label.set_xalign(0.0);
                                id_label.add_css_class("heading");

                                let info = crate::ai_chat::format_model_display(model);
                                let info_label = gtk::Label::new(Some(&info));
                                info_label.set_xalign(0.0);
                                info_label.add_css_class("caption");
                                info_label.add_css_class("dim-label");

                                box_row.append(&id_label);
                                box_row.append(&info_label);

                                // Tooltip con info completa
                                let tooltip = crate::ai_chat::format_model_tooltip(model);
                                row.set_tooltip_text(Some(&tooltip));

                                row.set_child(Some(&box_row));
                                listbox.append(&row);
                                model_ids.push(model.id.clone());
                            }
                        }

                        *all_models_ref.borrow_mut() = model_ids;

                        // Limpiar búsqueda
                        search_ref.set_text("");

                        println!("✅ Cargados {} modelos desde OpenRouter", models.len());
                    }
                    Err(e) => {
                        eprintln!("❌ Error cargando modelos: {}", e);
                    }
                }

                btn_clone.set_sensitive(true);
                btn_clone.set_label("🔄");
            });
        });

        // Cargar modelos automáticamente al abrir el diálogo
        refresh_models_btn.emit_clicked();

        model_box.append(&models_scroll);
        ai_box.append(&model_box);

        // Temperature slider
        let temp_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let temp_label = gtk::Label::builder()
            .label(&i18n.t("temperature_label"))
            .halign(gtk::Align::Start)
            .width_chars(12)
            .build();

        let temp_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 2.0, 0.1);
        temp_scale.set_hexpand(true);
        temp_scale.set_value(self.notes_config.borrow().get_ai_config().temperature as f64);
        temp_scale.set_draw_value(true);
        temp_scale.set_value_pos(gtk::PositionType::Right);

        let sender_clone = sender.clone();
        temp_scale.connect_value_changed(move |scale| {
            let temp = scale.value() as f32;
            if let Ok(mut config) = NotesConfig::load(NotesConfig::default_path()) {
                config.set_ai_temperature(temp);
                let _ = config.save(NotesConfig::default_path());
                sender_clone.input(AppMsg::ReloadConfig);
            }
        });

        temp_box.append(&temp_label);
        temp_box.append(&temp_scale);
        ai_box.append(&temp_box);

        // Max tokens slider with unlimited option
        let tokens_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();

        let tokens_header = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let tokens_label = gtk::Label::builder()
            .label(&i18n.t("max_tokens_label"))
            .halign(gtk::Align::Start)
            .width_chars(12)
            .build();

        let unlimited_check = gtk::CheckButton::builder()
            .label(&i18n.t("unlimited"))
            .build();

        tokens_header.append(&tokens_label);
        tokens_header.append(&unlimited_check);

        let tokens_slider_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let tokens_scale =
            gtk::Scale::with_range(gtk::Orientation::Horizontal, 100.0, 128000.0, 100.0);
        tokens_scale.set_hexpand(true);

        let current_tokens = self.notes_config.borrow().get_ai_config().max_tokens;
        let is_unlimited = current_tokens >= 1_000_000;

        if is_unlimited {
            unlimited_check.set_active(true);
            tokens_scale.set_sensitive(false);
            tokens_scale.set_value(128000.0);
        } else {
            tokens_scale.set_value(current_tokens as f64);
        }

        tokens_scale.set_draw_value(true);
        tokens_scale.set_value_pos(gtk::PositionType::Right);

        // Conectar checkbox unlimited
        let sender_clone = sender.clone();
        let scale_clone = tokens_scale.clone();
        unlimited_check.connect_toggled(move |check| {
            let is_unlimited = check.is_active();
            scale_clone.set_sensitive(!is_unlimited);

            if let Ok(mut config) = NotesConfig::load(NotesConfig::default_path()) {
                if is_unlimited {
                    config.set_ai_max_tokens(1_000_000); // Valor muy alto para "unlimited"
                } else {
                    // Al desmarcar, establecer un valor razonable por defecto
                    let tokens = if scale_clone.value() > 128000.0 {
                        4000 // Valor por defecto seguro
                    } else {
                        scale_clone.value() as u32
                    };
                    scale_clone.set_value(tokens as f64);
                    config.set_ai_max_tokens(tokens);
                }
                let _ = config.save(NotesConfig::default_path());
                sender_clone.input(AppMsg::ReloadConfig);
            }
        });

        // Conectar slider
        let sender_clone = sender.clone();
        let unlimited_clone = unlimited_check.clone();
        tokens_scale.connect_value_changed(move |scale| {
            if !unlimited_clone.is_active() {
                let tokens = scale.value() as u32;
                if let Ok(mut config) = NotesConfig::load(NotesConfig::default_path()) {
                    config.set_ai_max_tokens(tokens);
                    let _ = config.save(NotesConfig::default_path());
                    sender_clone.input(AppMsg::ReloadConfig);
                }
            }
        });

        tokens_slider_box.append(&tokens_scale);
        tokens_box.append(&tokens_header);
        tokens_box.append(&tokens_slider_box);
        ai_box.append(&tokens_box);

        // Save history toggle
        let history_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let history_label = gtk::Label::builder()
            .label(&i18n.t("save_history_label"))
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();

        let history_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        history_switch.set_active(self.notes_config.borrow().get_ai_config().save_history);

        let sender_clone = sender.clone();
        history_switch.connect_state_set(move |_, state| {
            if let Ok(mut config) = NotesConfig::load(NotesConfig::default_path()) {
                config.set_ai_save_history(state);
                let _ = config.save(NotesConfig::default_path());
                sender_clone.input(AppMsg::ReloadConfig);
            }
            gtk::glib::Propagation::Proceed
        });

        history_box.append(&history_label);
        history_box.append(&history_switch);
        ai_box.append(&history_box);

        content_box.append(&ai_box);

        content_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        // Sección de Búsqueda Semántica (Embeddings)
        let embeddings_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();

        let embeddings_label = gtk::Label::builder()
            .label(&i18n.t("semantic_search_title"))
            .halign(gtk::Align::Start)
            .build();
        embeddings_label.add_css_class("heading");
        embeddings_box.append(&embeddings_label);

        let embeddings_description = gtk::Label::builder()
            .label(&i18n.t("semantic_search_description"))
            .halign(gtk::Align::Start)
            .wrap(true)
            .build();
        embeddings_description.add_css_class("dim-label");
        embeddings_box.append(&embeddings_description);

        // Toggle de habilitación
        let enable_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let enable_label = gtk::Label::builder()
            .label(&i18n.t("enable_embeddings"))
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();

        let enable_switch = gtk::Switch::builder()
            .active(self.notes_config.borrow().get_embeddings_enabled())
            .valign(gtk::Align::Center)
            .build();

        let sender_clone = sender.clone();
        enable_switch.connect_state_set(move |_, state| {
            if let Ok(mut config) = NotesConfig::load(NotesConfig::default_path()) {
                config.set_embeddings_enabled(state);
                let _ = config.save(NotesConfig::default_path());
                sender_clone.input(AppMsg::ReloadConfig);
            }
            gtk::glib::Propagation::Proceed
        });

        enable_box.append(&enable_label);
        enable_box.append(&enable_switch);
        embeddings_box.append(&enable_box);

        // API Key para Embeddings
        let emb_key_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let emb_key_label = gtk::Label::builder()
            .label("API Key:")
            .halign(gtk::Align::Start)
            .width_chars(12)
            .build();

        let emb_key_entry = gtk::Entry::builder()
            .hexpand(true)
            .placeholder_text("sk-or-v1-...")
            .visibility(false)
            .build();

        // Cargar API key actual de embeddings
        if let Some(key) = self.notes_config.borrow().get_embeddings_api_key() {
            emb_key_entry.set_text(&key);
        }

        let sender_clone = sender.clone();
        emb_key_entry.connect_changed(move |entry| {
            let api_key = entry.text().to_string();
            if let Ok(mut config) = NotesConfig::load(NotesConfig::default_path()) {
                config.set_embeddings_api_key(if api_key.is_empty() {
                    None
                } else {
                    Some(api_key)
                });
                let _ = config.save(NotesConfig::default_path());
                sender_clone.input(AppMsg::ReloadConfig);
            }
        });

        emb_key_box.append(&emb_key_label);
        emb_key_box.append(&emb_key_entry);
        embeddings_box.append(&emb_key_box);

        // Modelo de embeddings
        let emb_model_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let emb_model_label = gtk::Label::builder()
            .label(&i18n.t("model_label"))
            .halign(gtk::Align::Start)
            .width_chars(12)
            .build();

        let emb_model_dropdown = gtk::DropDown::from_strings(&[
            "qwen/qwen3-embedding-8b (4096 dim)",
            "text-embedding-3-small (1536 dim)",
            "text-embedding-3-large (3072 dim)",
        ]);

        let current_emb_model = self.notes_config.borrow().get_embeddings_model();
        emb_model_dropdown.set_selected(match current_emb_model.as_str() {
            "text-embedding-3-small" => 1,
            "text-embedding-3-large" => 2,
            _ => 0, // qwen por defecto
        });

        let sender_clone = sender.clone();
        emb_model_dropdown.connect_selected_notify(move |dropdown| {
            let model = match dropdown.selected() {
                1 => "text-embedding-3-small",
                2 => "text-embedding-3-large",
                _ => "qwen/qwen3-embedding-8b",
            };
            if let Ok(mut config) = NotesConfig::load(NotesConfig::default_path()) {
                config.set_embeddings_model(model.to_string());
                let _ = config.save(NotesConfig::default_path());
                sender_clone.input(AppMsg::ReloadConfig);
            }
        });

        emb_model_box.append(&emb_model_label);
        emb_model_box.append(&emb_model_dropdown);
        embeddings_box.append(&emb_model_box);

        // Botón para indexar notas
        let index_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_top(8)
            .build();

        let index_button = gtk::Button::builder()
            .label(&i18n.t("index_all_notes"))
            .hexpand(true)
            .build();

        let index_status = gtk::Label::builder()
            .label("")
            .halign(gtk::Align::Start)
            .build();
        index_status.add_css_class("caption");
        index_status.add_css_class("dim-label");

        let sender_clone = sender.clone();
        let status_clone = index_status.clone();
        let button_clone = index_button.clone();
        let notes_config_data = self.notes_config.borrow().clone(); // Clonar el contenido, no el Rc
        let t_indexing = i18n.t("indexing");

        index_button.connect_clicked(move |_| {
            // Deshabilitar botón durante indexación
            button_clone.set_sensitive(false);
            status_clone.set_label(&t_indexing);

            // Ejecutar indexación en segundo plano
            let sender_task = sender_clone.clone();
            let config_data = notes_config_data.clone();

            std::thread::spawn(move || {
                use crate::core::{NotesDatabase, NotesDirectory};
                use crate::i18n::I18n;
                use std::cell::RefCell;
                use std::rc::Rc;

                // Construir objetos necesarios para MCPToolExecutor
                let notes_path =
                    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
                        .join(".local/share/notnative/notes");
                let db_path = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
                    .join(".local/share/notnative/notes.db");

                let notes_dir = match NotesDirectory::new(&notes_path) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("❌ Error abriendo directorio: {}", e);
                        return;
                    }
                };

                let notes_db = match NotesDatabase::new(&db_path) {
                    Ok(db) => Rc::new(RefCell::new(db)),
                    Err(e) => {
                        eprintln!("❌ Error abriendo base de datos: {}", e);
                        return;
                    }
                };

                let i18n = Rc::new(RefCell::new(I18n::new(crate::i18n::Language::Spanish)));

                // Usar la configuración actual de la app (con la API key recién configurada)
                let notes_config = Rc::new(RefCell::new(config_data));

                let mcp_executor =
                    crate::mcp::MCPToolExecutor::new(notes_dir, notes_db, notes_config, i18n);

                match mcp_executor.execute(crate::mcp::MCPToolCall::ReindexAllNotes {}) {
                    Ok(result) => {
                        if result.success {
                            let msg = result
                                .data
                                .and_then(|d| d.as_str().map(String::from))
                                .unwrap_or_else(|| "Indexación completada".to_string());
                            println!("✅ {}", msg);
                        } else {
                            let error = result
                                .error
                                .unwrap_or_else(|| "Error desconocido".to_string());
                            eprintln!("❌ Error: {}", error);
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Error: {}", e);
                    }
                }
            });
        });

        index_box.append(&index_button);
        embeddings_box.append(&index_box);
        embeddings_box.append(&index_status);

        // Info box con costos
        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .build();
        info_box.add_css_class("card");
        info_box.set_margin_all(8);

        let info_icon_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let info_icon = gtk::Label::builder().label("💡").build();

        let info_text = gtk::Label::builder()
            .label(&i18n.t("estimated_cost"))
            .halign(gtk::Align::Start)
            .hexpand(true)
            .wrap(true)
            .build();
        info_text.add_css_class("caption");

        info_icon_box.append(&info_icon);
        info_icon_box.append(&info_text);
        info_box.append(&info_icon_box);

        let link_text = i18n.t("get_api_key_openrouter");
        let link_label = gtk::Label::builder()
            .label(&format!(
                "<a href='https://openrouter.ai'>{}</a>",
                link_text
            ))
            .use_markup(true)
            .halign(gtk::Align::Start)
            .build();
        link_label.add_css_class("caption");
        info_box.append(&link_label);

        embeddings_box.append(&info_box);

        content_box.append(&embeddings_box);

        // Botón cerrar
        let button_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .halign(gtk::Align::End)
            .spacing(8)
            .margin_top(20)
            .build();

        let close_button = gtk::Button::builder().label(&i18n.t("close")).build();
        close_button.add_css_class("suggested-action");

        let dialog_clone = dialog.clone();
        close_button.connect_clicked(move |_| {
            dialog_clone.close();
        });

        button_box.append(&close_button);
        content_box.append(&button_box);

        scrolled.set_child(Some(&content_box));
        dialog.set_child(Some(&scrolled));

        // Permitir cerrar con Escape
        let esc_controller = gtk::EventControllerKey::new();
        let dialog_clone2 = dialog.clone();
        esc_controller.connect_key_pressed(move |_, keyval, _, _| {
            let key_name = keyval.name().map(|s| s.to_string());
            if key_name.as_deref() == Some("Escape") {
                dialog_clone2.close();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        dialog.add_controller(esc_controller);

        dialog.present();
    }

    fn show_keyboard_shortcuts(&self) {
        let i18n = self.i18n.borrow();

        let dialog = gtk::Window::builder()
            .transient_for(&self.main_window)
            .modal(true)
            .title(&i18n.t("keyboard_shortcuts"))
            .default_width(650)
            .default_height(600)
            .build();

        let scrolled = gtk::ScrolledWindow::builder().vexpand(true).build();

        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_start(20)
            .margin_end(20)
            .margin_top(20)
            .margin_bottom(20)
            .spacing(16)
            .build();

        // Título
        let title = gtk::Label::builder()
            .label(&i18n.t("keyboard_shortcuts"))
            .halign(gtk::Align::Start)
            .build();
        title.add_css_class("title-2");
        content_box.append(&title);

        // Lista de atajos actualizada basada en KEYBINDINGS.md
        let shortcuts: Vec<(String, Vec<(&str, String)>)> = vec![
            (
                i18n.t("shortcuts_global"),
                vec![
                    ("Ctrl+F", i18n.t("shortcut_global_search")),
                    ("Alt+F", i18n.t("shortcut_note_search")),
                    ("Ctrl+Shift+A", i18n.t("shortcut_enter_ai_chat")),
                    ("Ctrl+S", i18n.t("shortcut_save")),
                ],
            ),
            (
                i18n.t("shortcuts_quick_notes"),
                vec![
                    ("Esc", i18n.t("shortcut_back_or_close")),
                    ("Ctrl+S", i18n.t("shortcut_save")),
                ],
            ),
            (
                i18n.t("shortcuts_normal_navigation"),
                vec![
                    ("h / ←", i18n.t("shortcut_left")),
                    ("j / ↓", i18n.t("shortcut_down")),
                    ("k / ↑", i18n.t("shortcut_up")),
                    ("l / →", i18n.t("shortcut_right")),
                    ("0", i18n.t("shortcut_line_start")),
                    ("$", i18n.t("shortcut_line_end")),
                    ("gg", i18n.t("shortcut_doc_start")),
                    ("G", i18n.t("shortcut_doc_end")),
                ],
            ),
            (
                i18n.t("shortcuts_normal_editing"),
                vec![
                    ("i", i18n.t("shortcut_insert_mode")),
                    ("a", i18n.t("shortcut_ai_chat_mode")),
                    ("v", i18n.t("shortcut_visual_mode")),
                    (":", i18n.t("shortcut_command_mode")),
                    ("n", i18n.t("shortcut_new_note")),
                    ("x", i18n.t("shortcut_delete_char_under")),
                    ("dd", i18n.t("shortcut_delete_line_complete")),
                    ("u", i18n.t("shortcut_undo")),
                    ("t", i18n.t("shortcut_toggle_sidebar")),
                ],
            ),
            (
                i18n.t("shortcuts_insert_mode"),
                vec![
                    ("Esc", i18n.t("shortcut_normal_mode")),
                    ("Ctrl+S", i18n.t("shortcut_save")),
                    ("Ctrl+T", i18n.t("shortcut_insert_table")),
                    ("Ctrl+Shift+I", i18n.t("shortcut_insert_image")),
                    ("Tab", i18n.t("shortcut_tab_autocomplete")),
                    ("Ctrl+Z", i18n.t("shortcut_undo")),
                    ("Ctrl+R", i18n.t("shortcut_redo")),
                ],
            ),
            (
                i18n.t("shortcuts_ai_chat"),
                vec![
                    ("Esc", i18n.t("shortcut_exit_chat")),
                    ("i", i18n.t("shortcut_exit_chat_insert")),
                    ("Enter", i18n.t("shortcut_send_message")),
                    ("Shift+Enter", i18n.t("shortcut_new_line")),
                    ("↑/↓", i18n.t("shortcut_navigate_suggestions")),
                    ("Tab", i18n.t("shortcut_accept_suggestion")),
                ],
            ),
            (
                i18n.t("shortcuts_sidebar"),
                vec![
                    ("j / ↓", i18n.t("shortcut_next_note")),
                    ("k / ↑", i18n.t("shortcut_prev_note")),
                    ("Enter", i18n.t("shortcut_open_note")),
                    ("Esc", i18n.t("shortcut_focus_editor")),
                ],
            ),
            (
                i18n.t("shortcuts_floating_search"),
                vec![
                    ("Esc", i18n.t("shortcut_close_search")),
                    ("Ctrl", i18n.t("shortcut_toggle_semantic")),
                    ("↑/↓", i18n.t("shortcut_navigate_results")),
                    ("Enter", i18n.t("shortcut_open_selected")),
                ],
            ),
        ];

        for (section, items) in shortcuts {
            let section_label = gtk::Label::builder()
                .label(section.as_str())
                .halign(gtk::Align::Start)
                .margin_top(12)
                .build();
            section_label.add_css_class("heading");
            content_box.append(&section_label);

            let list_box = gtk::ListBox::builder()
                .selection_mode(gtk::SelectionMode::None)
                .build();
            list_box.add_css_class("boxed-list");

            for (shortcut, description) in items {
                let row_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(12)
                    .margin_start(12)
                    .margin_end(12)
                    .margin_top(12)
                    .margin_bottom(12)
                    .build();

                let shortcut_label = gtk::Label::builder()
                    .label(shortcut)
                    .halign(gtk::Align::Start)
                    .width_chars(16)
                    .build();
                shortcut_label.add_css_class("monospace");

                let desc_label = gtk::Label::builder()
                    .label(description.as_str())
                    .halign(gtk::Align::Start)
                    .hexpand(true)
                    .wrap(true)
                    .build();
                desc_label.add_css_class("dim-label");

                row_box.append(&shortcut_label);
                row_box.append(&desc_label);

                list_box.append(&row_box);
            }

            content_box.append(&list_box);
        }

        scrolled.set_child(Some(&content_box));
        dialog.set_child(Some(&scrolled));

        // Agregar botón cerrar
        let header_bar = gtk::HeaderBar::new();
        dialog.set_titlebar(Some(&header_bar));

        // Permitir cerrar con Escape
        let esc_controller = gtk::EventControllerKey::new();
        let dialog_clone = dialog.clone();
        esc_controller.connect_key_pressed(move |_, keyval, _, _| {
            let key_name = keyval.name().map(|s| s.to_string());
            if key_name.as_deref() == Some("Escape") {
                dialog_clone.close();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        dialog.add_controller(esc_controller);

        dialog.present();
    }
}

/// Encuentra todas las posiciones de TODOs en el texto original
/// Devuelve un vector con las posiciones de inicio de cada `- [ ]` o `- [x]`
/// Ahora también detecta TODOs con indentación (espacios al inicio)
fn find_all_todos_in_text(text: &str) -> Vec<usize> {
    let chars: Vec<char> = text.chars().collect();
    let mut positions = Vec::new();

    let mut pos = 0;
    while pos + 4 < chars.len() {
        // Saltar espacios al inicio (indentación)
        let start_pos = pos;
        while pos < chars.len() && chars[pos] == ' ' {
            pos += 1;
        }

        // Verificar si hay suficiente espacio para el patrón TODO
        if pos + 4 >= chars.len() {
            break;
        }

        // Buscar el patrón - [ ] o - [x] después de la indentación
        if chars[pos] == '-'
            && chars[pos + 1] == ' '
            && chars[pos + 2] == '['
            && (chars[pos + 3] == ' ' || chars[pos + 3] == 'x' || chars[pos + 3] == 'X')
            && chars[pos + 4] == ']'
        {
            positions.push(pos); // Guardar la posición del '-', no del inicio de la línea
            pos += 5; // Saltar el TODO completo
        } else if pos > start_pos {
            // Si saltamos espacios pero no encontramos TODO, retroceder
            pos = start_pos + 1;
        } else {
            pos += 1;
        }
    }

    positions
}

/// Muestra un diálogo con la imagen ampliada y opción para abrir su ubicación
fn show_image_viewer_dialog(parent_window: &gtk::ApplicationWindow, image_path: &str, i18n: &I18n) {
    let dialog = gtk::Window::builder()
        .transient_for(parent_window)
        .modal(true)
        .title(&i18n.t("image_viewer"))
        .default_width(800)
        .default_height(600)
        .build();

    let main_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();

    // Área de imagen con scroll
    let scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .build();

    let picture = gtk::Picture::new();
    picture.set_can_shrink(true);
    picture.set_keep_aspect_ratio(true);

    if std::path::Path::new(image_path).exists() {
        picture.set_filename(Some(image_path));
    }

    scrolled.set_child(Some(&picture));
    main_box.append(&scrolled);

    // Barra inferior con botón
    let bottom_bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(12)
        .margin_end(12)
        .margin_top(8)
        .margin_bottom(12)
        .halign(gtk::Align::End)
        .build();

    let open_location_button = gtk::Button::builder()
        .label(&i18n.t("open_file_location"))
        .build();

    let image_path_clone = image_path.to_string();
    open_location_button.connect_clicked(move |_| {
        // Abrir el directorio que contiene la imagen
        if let Some(parent_dir) = std::path::Path::new(&image_path_clone).parent() {
            let path_str = parent_dir.to_string_lossy().to_string();
            std::thread::spawn(move || {
                if let Err(e) = open::that(&path_str) {
                    eprintln!("Error abriendo ubicación de imagen: {}", e);
                }
            });
        }
    });

    bottom_bar.append(&open_location_button);
    main_box.append(&bottom_bar);

    dialog.set_child(Some(&main_box));

    // Agregar header bar
    let header_bar = gtk::HeaderBar::new();
    dialog.set_titlebar(Some(&header_bar));

    // Permitir cerrar con Escape
    let esc_controller = gtk::EventControllerKey::new();
    let dialog_clone = dialog.clone();
    esc_controller.connect_key_pressed(move |_, keyval, _, _| {
        let key_name = keyval.name().map(|s| s.to_string());
        if key_name.as_deref() == Some("Escape") {
            dialog_clone.close();
            return gtk::glib::Propagation::Stop;
        }
        gtk::glib::Propagation::Proceed
    });
    dialog.add_controller(esc_controller);

    dialog.present();
}

impl MainApp {
    /// Muestra una notificación toast temporal en la parte inferior de la pantalla
    fn show_notification(&self, message: &str) {
        self.notification_label.set_label(message);
        self.notification_revealer.set_reveal_child(true);

        // Auto-ocultar después de 3 segundos
        let revealer = self.notification_revealer.clone();
        gtk::glib::timeout_add_seconds_local_once(3, move || {
            revealer.set_reveal_child(false);
        });
    }

    /// Convierte [[Nombre de Nota]] en enlaces clickeables con markup de Pango
    fn convert_note_links_to_markup(&self, text: &str) -> String {
        use regex::Regex;

        // Escapar HTML/XML primero para evitar problemas con < > & etc
        let escaped = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");

        // Patrón para detectar [[Nombre]]
        let re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();

        // Reemplazar [[Nombre]] por enlaces clickeables
        re.replace_all(&escaped, |caps: &regex::Captures| {
            let note_name = &caps[1];
            // Usar el esquema 'note://' para identificar que es un enlace a nota
            format!(
                "<a href=\"note://{}\">{}</a>",
                note_name.replace('"', "&quot;"),
                note_name
            )
        })
        .to_string()
    }

    fn show_about_dialog(&self) {
        let i18n = self.i18n.borrow();

        let dialog = gtk::AboutDialog::builder()
            .transient_for(&self.main_window)
            .modal(true)
            .program_name("NotNative")
            .version("0.1.13")
            .comments(&i18n.t("app_description"))
            .website("https://github.com/k4ditano/notnative-app")
            .website_label(&i18n.t("website"))
            .license_type(gtk::License::MitX11)
            .authors(vec!["k4ditano".to_string()])
            .build();

        dialog.present();
    }

    fn show_mcp_server_info_dialog(&self) {
        let i18n = self.i18n.borrow();

        // Crear ventana de diálogo
        let dialog = gtk::Window::builder()
            .transient_for(&self.main_window)
            .modal(true)
            .title(&i18n.t("mcp_server_title"))
            .default_width(500)
            .default_height(300)
            .build();

        // Contenedor principal
        let main_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .margin_top(20)
            .margin_bottom(20)
            .margin_start(20)
            .margin_end(20)
            .build();

        // Header con icono y título
        let header_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let icon_label = gtk::Label::builder().label("🚀").build();
        icon_label.add_css_class("title-1");

        let title_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .hexpand(true)
            .build();

        let title_label = gtk::Label::builder()
            .label(&format!("<b>{}</b>", i18n.t("mcp_server_active")))
            .use_markup(true)
            .xalign(0.0)
            .build();
        title_label.add_css_class("title-2");

        let subtitle_label = gtk::Label::builder()
            .label(&i18n.t("mcp_server_subtitle"))
            .xalign(0.0)
            .build();
        subtitle_label.add_css_class("dim-label");

        title_box.append(&title_label);
        title_box.append(&subtitle_label);

        header_box.append(&icon_label);
        header_box.append(&title_box);

        // Información del servidor
        let info_frame = gtk::Frame::builder().build();

        let info_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();

        // Estado
        let status_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let status_label_key = gtk::Label::builder()
            .label(&format!("<b>{}:</b>", i18n.t("mcp_status")))
            .use_markup(true)
            .xalign(0.0)
            .width_chars(15)
            .build();

        let status_indicator = gtk::Label::builder()
            .label(&i18n.t("status_active"))
            .xalign(0.0)
            .build();

        status_row.append(&status_label_key);
        status_row.append(&status_indicator);

        // URL
        let url_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        let url_label_key = gtk::Label::builder()
            .label("<b>URL:</b>")
            .use_markup(true)
            .xalign(0.0)
            .width_chars(15)
            .build();

        let url_value = gtk::Label::builder()
            .label("http://localhost:8788")
            .xalign(0.0)
            .selectable(true)
            .build();

        url_row.append(&url_label_key);
        url_row.append(&url_value);

        // Endpoints
        let endpoints_label = gtk::Label::builder()
            .label(&format!("<b>{}:</b>", i18n.t("mcp_endpoints_available")))
            .use_markup(true)
            .xalign(0.0)
            .margin_top(8)
            .build();

        let endpoints_list_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .margin_start(16)
            .build();

        let endpoint1 = gtk::Label::builder()
            .label("• GET  /health")
            .xalign(0.0)
            .build();
        endpoint1.add_css_class("monospace");

        let endpoint2 = gtk::Label::builder()
            .label("• POST /mcp/list_tools")
            .xalign(0.0)
            .build();
        endpoint2.add_css_class("monospace");

        let endpoint3 = gtk::Label::builder()
            .label("• POST /mcp/call_tool")
            .xalign(0.0)
            .build();
        endpoint3.add_css_class("monospace");

        endpoints_list_box.append(&endpoint1);
        endpoints_list_box.append(&endpoint2);
        endpoints_list_box.append(&endpoint3);

        info_box.append(&status_row);
        info_box.append(&url_row);
        info_box.append(&endpoints_label);
        info_box.append(&endpoints_list_box);

        info_frame.set_child(Some(&info_box));

        // Botones de acción
        let buttons_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .halign(gtk::Align::Center)
            .build();

        let copy_url_btn = gtk::Button::builder().label(&i18n.t("copy_url")).build();

        let docs_btn = gtk::Button::builder().label(&i18n.t("view_docs")).build();

        let close_btn = gtk::Button::builder().label(&i18n.t("close")).build();

        buttons_box.append(&copy_url_btn);
        buttons_box.append(&docs_btn);
        buttons_box.append(&close_btn);

        // Conectar eventos
        let copied_text = i18n.t("copied");
        copy_url_btn.connect_clicked(move |btn| {
            if let Some(display) = gtk::gdk::Display::default() {
                let clipboard = display.clipboard();
                clipboard.set_text("http://localhost:8788");

                let original_label = btn.label().unwrap_or_default();
                btn.set_label(&copied_text);

                let btn_clone = btn.clone();
                gtk::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(1500),
                    move || {
                        btn_clone.set_label(&original_label);
                    },
                );
            }
        });

        let notes_dir_for_docs = self.notes_dir.clone();
        let dialog_for_docs = dialog.clone();
        docs_btn.connect_clicked(move |_| {
            dialog_for_docs.close();

            // Intentar varias rutas posibles para la documentación
            let possible_paths = vec![
                std::path::PathBuf::from(
                    "/home/abel/Programacion/notnative/notnative-app/docs/MCP_INTEGRATION.md",
                ),
                std::path::PathBuf::from("docs/MCP_INTEGRATION.md"),
                notes_dir_for_docs
                    .root()
                    .parent()
                    .map(|p| p.join("notnative-app/docs/MCP_INTEGRATION.md"))
                    .unwrap_or_else(|| std::path::PathBuf::from("docs/MCP_INTEGRATION.md")),
            ];

            // Buscar el primer path que exista
            let docs_path = possible_paths
                .into_iter()
                .find(|p| p.exists())
                .unwrap_or_else(|| std::path::PathBuf::from("docs/MCP_INTEGRATION.md"));

            std::thread::spawn(move || {
                if let Err(e) = open::that(&docs_path) {
                    eprintln!("Error abriendo documentación MCP: {}", e);
                }
            });
        });

        let dialog_for_close = dialog.clone();
        close_btn.connect_clicked(move |_| {
            dialog_for_close.close();
        });

        // Ensamblar todo
        main_box.append(&header_box);
        main_box.append(&info_frame);
        main_box.append(&buttons_box);

        dialog.set_child(Some(&main_box));
        dialog.present();
    }

    fn update_ui_language(&self, sender: &ComponentSender<Self>) {
        let i18n = self.i18n.borrow();

        // Actualizar tooltips
        self.sidebar_toggle_button
            .set_tooltip_text(Some(&i18n.t("show_hide_notes")));
        self.search_toggle_button
            .set_tooltip_text(Some(&i18n.t("search_notes")));
        self.new_note_button
            .set_tooltip_text(Some(&i18n.t("new_note")));
        self.settings_button
            .set_tooltip_text(Some(&i18n.t("settings")));
        self.tags_menu_button
            .set_tooltip_text(Some(&i18n.t("tags_note")));
        self.todos_menu_button
            .set_tooltip_text(Some(&i18n.t("todos_note")));
        self.music_player_button
            .set_tooltip_text(Some(&i18n.t("music_player")));
        self.reminders_button
            .set_tooltip_text(Some(&i18n.t("reminder_tooltip")));

        // Actualizar labels del sidebar
        self.sidebar_notes_label.set_label(&i18n.t("notes"));

        // Actualizar placeholder del floating search entry
        self.floating_search_entry
            .set_placeholder_text(Some(&i18n.t("search_placeholder")));

        // Actualizar título de ventana si no hay nota cargada
        if self.current_note.is_none() {
            self.window_title.set_text(&i18n.t("app_title"));
        }

        // Actualizar barra de estado (el modo y las estadísticas usan el idioma actual)
        let line_count = self.buffer.len_lines();
        let word_count = self.buffer.to_string().split_whitespace().count();
        let unsaved_indicator = if self.has_unsaved_changes { " •" } else { "" };

        self.stats_label.set_label(&format!(
            "{} {} | {} {}{}",
            line_count,
            i18n.t("lines"),
            word_count,
            i18n.t("words"),
            unsaved_indicator
        ));

        // Recrear el popover del settings button con textos actualizados
        self.recreate_settings_popover(sender);

        // Actualizar menú contextual
        self.update_context_menu_labels();

        // Actualizar display de tags
        self.refresh_tags_display_after_language_change();

        // Actualizar display de TODOs
        self.refresh_todos_summary();

        println!("UI actualizada al idioma: {:?}", i18n.current_language());
    }

    fn create_settings_popover(&self, sender: &ComponentSender<Self>) {
        let i18n = self.i18n.borrow();

        // Crear el box que contendrá los botones
        let menu_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .build();

        // Botón de Preferencias
        let preferences_button = gtk::Button::builder()
            .label(&i18n.t("preferences"))
            .halign(gtk::Align::Fill)
            .build();
        preferences_button.add_css_class("flat");
        preferences_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ShowPreferences);
            }
        ));

        // Botón de Atajos de teclado
        let shortcuts_button = gtk::Button::builder()
            .label(&i18n.t("keyboard_shortcuts"))
            .halign(gtk::Align::Fill)
            .build();
        shortcuts_button.add_css_class("flat");
        shortcuts_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ShowKeyboardShortcuts);
            }
        ));

        // Botón de Acerca de
        let about_button = gtk::Button::builder()
            .label(&i18n.t("about"))
            .halign(gtk::Align::Fill)
            .build();
        about_button.add_css_class("flat");
        about_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ShowAboutDialog);
            }
        ));

        // Botón para abrir carpeta de trabajo
        let workspace_button = gtk::Button::builder()
            .label(&i18n.t("open_workspace_folder"))
            .halign(gtk::Align::Fill)
            .build();
        workspace_button.add_css_class("flat");

        let notes_dir_path = self.notes_dir.root().to_path_buf();
        let settings_btn = self.settings_button.clone();
        workspace_button.connect_clicked(move |_| {
            // Cerrar el popover primero
            if let Some(popover) = settings_btn.popover() {
                popover.popdown();
            }

            // Abrir la carpeta en un hilo separado para no bloquear la UI
            let path = notes_dir_path.clone();
            std::thread::spawn(move || {
                if let Err(e) = open::that(&path) {
                    eprintln!("Error abriendo carpeta de trabajo: {}", e);
                }
            });
        });

        // Agregar botones al box
        menu_box.append(&preferences_button);
        menu_box.append(&workspace_button);
        menu_box.append(&shortcuts_button);

        // Botón de MCP Server Info
        let mcp_server_button = gtk::Button::builder()
            .label("MCP Server")
            .halign(gtk::Align::Fill)
            .build();
        mcp_server_button.add_css_class("flat");
        mcp_server_button.connect_clicked(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ShowMCPServerInfo);
            }
        ));
        menu_box.append(&mcp_server_button);

        menu_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        menu_box.append(&about_button);

        // Crear el popover
        let popover = gtk::Popover::builder()
            .child(&menu_box)
            .autohide(true)
            .build();
        popover.add_css_class("menu");

        // Asignar el popover al MenuButton
        self.settings_button.set_popover(Some(&popover));
    }

    fn recreate_settings_popover(&self, sender: &ComponentSender<Self>) {
        // Recrear el popover con los textos actualizados
        self.create_settings_popover(sender);
    }

    /// Muestra un popover para seleccionar iconos/emojis personalizados
    fn show_icon_picker_popover(
        &self,
        name: &str,
        is_folder: bool,
        sender: &ComponentSender<Self>,
    ) {
        println!(
            "🎨 Mostrando icon picker para: {} (is_folder: {})",
            name, is_folder
        );

        // Cerrar el context menu primero
        self.context_menu.popdown();

        // Iconos del sistema GTK (simbólicos) - uniformes y escalables
        // Estos iconos vienen con Adwaita/GTK y son monocromáticos
        let system_icons: &[(&str, &str)] = &[
            // Documentos y notas
            ("text-x-generic-symbolic", "Documento"),
            ("document-new-symbolic", "Nuevo"),
            ("document-edit-symbolic", "Editar"),
            ("document-save-symbolic", "Guardado"),
            ("document-properties-symbolic", "Propiedades"),
            ("x-office-document-symbolic", "Ofimática"),
            ("x-office-spreadsheet-symbolic", "Hoja cálculo"),
            ("x-office-presentation-symbolic", "Presentación"),
            // Carpetas y organización
            ("folder-symbolic", "Carpeta"),
            ("folder-documents-symbolic", "Documentos"),
            ("folder-download-symbolic", "Descargas"),
            ("folder-music-symbolic", "Música"),
            ("folder-pictures-symbolic", "Imágenes"),
            ("folder-videos-symbolic", "Vídeos"),
            ("folder-templates-symbolic", "Plantillas"),
            ("folder-saved-search-symbolic", "Búsqueda"),
            // Favoritos y marcadores
            ("starred-symbolic", "Favorito"),
            ("non-starred-symbolic", "No favorito"),
            ("bookmark-new-symbolic", "Marcador"),
            ("user-bookmarks-symbolic", "Marcadores"),
            ("emblem-favorite-symbolic", "Corazón"),
            ("emblem-important-symbolic", "Importante"),
            // Estado y alertas
            ("emblem-ok-symbolic", "OK"),
            ("emblem-default-symbolic", "Defecto"),
            ("dialog-warning-symbolic", "Advertencia"),
            ("dialog-error-symbolic", "Error"),
            ("dialog-information-symbolic", "Info"),
            ("dialog-question-symbolic", "Pregunta"),
            // Comunicación
            ("mail-unread-symbolic", "Correo"),
            ("mail-mark-important-symbolic", "Importante"),
            ("chat-message-new-symbolic", "Chat"),
            ("call-start-symbolic", "Llamada"),
            // Multimedia
            ("audio-x-generic-symbolic", "Audio"),
            ("video-x-generic-symbolic", "Vídeo"),
            ("image-x-generic-symbolic", "Imagen"),
            ("camera-photo-symbolic", "Foto"),
            ("media-playback-start-symbolic", "Play"),
            // Herramientas y configuración
            ("preferences-system-symbolic", "Sistema"),
            ("applications-system-symbolic", "Apps"),
            ("emblem-system-symbolic", "Engranaje"),
            ("utilities-terminal-symbolic", "Terminal"),
            ("accessories-text-editor-symbolic", "Editor"),
            // Tiempo y calendario
            ("alarm-symbolic", "Alarma"),
            ("appointment-new-symbolic", "Cita"),
            ("x-office-calendar-symbolic", "Calendario"),
            // Web y red
            ("network-server-symbolic", "Servidor"),
            ("web-browser-symbolic", "Web"),
            ("folder-remote-symbolic", "Remoto"),
            // Seguridad
            ("channel-secure-symbolic", "Seguro"),
            ("changes-prevent-symbolic", "Bloqueado"),
            ("system-lock-screen-symbolic", "Candado"),
            // Navegación
            ("go-home-symbolic", "Inicio"),
            ("view-pin-symbolic", "Pin"),
            ("find-location-symbolic", "Ubicación"),
            ("mark-location-symbolic", "Marca"),
            // Acciones
            ("list-add-symbolic", "Añadir"),
            ("edit-find-symbolic", "Buscar"),
            ("view-list-symbolic", "Lista"),
            ("view-grid-symbolic", "Cuadrícula"),
            ("star-new-symbolic", "Nueva estrella"),
            ("object-select-symbolic", "Seleccionar"),
            // Misc
            ("help-about-symbolic", "Acerca de"),
            ("avatar-default-symbolic", "Usuario"),
            ("weather-clear-symbolic", "Sol"),
            ("weather-overcast-symbolic", "Nublado"),
        ];

        // Colores disponibles (hex)
        let colors = [
            ("#ff6b6b", "Rojo"),
            ("#ff9f43", "Naranja"),
            ("#feca57", "Amarillo"),
            ("#48dbfb", "Cian"),
            ("#1dd1a1", "Verde"),
            ("#5f27cd", "Púrpura"),
            ("#ff6b9d", "Rosa"),
            ("#00d2d3", "Turquesa"),
            ("#54a0ff", "Azul"),
            ("#c8d6e5", "Gris"),
        ];

        // Crear ventana de diálogo
        let dialog = gtk::Window::builder()
            .title(if is_folder {
                "Icono de carpeta"
            } else {
                "Icono de nota"
            })
            .modal(true)
            .transient_for(&self.main_window)
            .default_width(380)
            .default_height(520)
            .resizable(false)
            .build();
        dialog.add_css_class("icon-picker-dialog");

        // Contenedor principal con scroll
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .build();

        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();

        // Header con nombre del item
        let header_label = gtk::Label::builder()
            .label(name)
            .halign(gtk::Align::Center)
            .build();
        header_label.add_css_class("title-4");
        content_box.append(&header_label);

        // Selector de color
        let color_label = gtk::Label::builder()
            .label("Color del icono")
            .halign(gtk::Align::Start)
            .build();
        color_label.add_css_class("heading");
        content_box.append(&color_label);

        // Estado compartido para el color seleccionado
        let selected_color: std::rc::Rc<std::cell::RefCell<Option<String>>> =
            std::rc::Rc::new(std::cell::RefCell::new(None));

        let color_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .halign(gtk::Align::Center)
            .build();

        // Botón para "sin color" (usa el color por defecto del tema)
        let no_color_btn = gtk::Button::builder()
            .label("○")
            .tooltip_text("Sin color (tema)")
            .width_request(32)
            .height_request(32)
            .build();
        no_color_btn.add_css_class("flat");
        no_color_btn.add_css_class("circular");
        no_color_btn.add_css_class("color-btn-selected");

        let selected_color_clone = selected_color.clone();
        let color_box_weak = color_box.downgrade();
        no_color_btn.connect_clicked(move |btn| {
            *selected_color_clone.borrow_mut() = None;
            // Actualizar apariencia de todos los botones
            if let Some(cbox) = color_box_weak.upgrade() {
                let mut child = cbox.first_child();
                while let Some(widget) = child {
                    widget.remove_css_class("color-btn-selected");
                    child = widget.next_sibling();
                }
            }
            btn.add_css_class("color-btn-selected");
        });
        color_box.append(&no_color_btn);

        for (hex, tooltip) in colors {
            let color_btn = gtk::Button::new();
            color_btn.set_tooltip_text(Some(tooltip));
            color_btn.set_width_request(32);
            color_btn.set_height_request(32);
            color_btn.add_css_class("flat");
            color_btn.add_css_class("circular");
            color_btn.add_css_class("color-picker-btn");

            // Aplicar color de fondo usando CSS inline
            let css_provider = gtk::CssProvider::new();
            let css = format!(
                "button {{ background-color: {}; min-width: 24px; min-height: 24px; }}
                 button:hover {{ opacity: 0.8; }}",
                hex
            );
            css_provider.load_from_data(&css);
            color_btn
                .style_context()
                .add_provider(&css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

            let hex_owned = hex.to_string();
            let selected_color_clone = selected_color.clone();
            let color_box_weak = color_box.downgrade();

            color_btn.connect_clicked(move |btn| {
                *selected_color_clone.borrow_mut() = Some(hex_owned.clone());
                // Actualizar apariencia de todos los botones
                if let Some(cbox) = color_box_weak.upgrade() {
                    let mut child = cbox.first_child();
                    while let Some(widget) = child {
                        widget.remove_css_class("color-btn-selected");
                        child = widget.next_sibling();
                    }
                }
                btn.add_css_class("color-btn-selected");
            });

            color_box.append(&color_btn);
        }
        content_box.append(&color_box);

        content_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        // Label para iconos
        let icons_label = gtk::Label::builder()
            .label("Selecciona un icono")
            .halign(gtk::Align::Start)
            .build();
        icons_label.add_css_class("heading");
        content_box.append(&icons_label);

        // Grid de iconos (8 columnas)
        let grid = gtk::Grid::builder()
            .column_spacing(4)
            .row_spacing(4)
            .halign(gtk::Align::Center)
            .build();

        let name_owned = name.to_string();
        let mut col = 0;
        let mut row = 0;

        for (icon_name, tooltip) in system_icons {
            // Crear botón con imagen del sistema
            let image = gtk::Image::builder()
                .icon_name(*icon_name)
                .pixel_size(20)
                .build();

            let button = gtk::Button::builder()
                .child(&image)
                .tooltip_text(*tooltip)
                .width_request(36)
                .height_request(36)
                .build();
            button.add_css_class("flat");
            button.add_css_class("icon-picker-btn");

            let icon_str = (*icon_name).to_string();
            let name_clone = name_owned.clone();
            let is_folder_clone = is_folder;
            let sender_clone = sender.clone();
            let dialog_weak = dialog.downgrade();
            let selected_color_clone = selected_color.clone();

            button.connect_clicked(move |_btn| {
                let color = selected_color_clone.borrow().clone();

                // Cerrar el diálogo
                if let Some(dlg) = dialog_weak.upgrade() {
                    dlg.close();
                }

                if is_folder_clone {
                    sender_clone.input(AppMsg::SetFolderIcon {
                        folder_path: name_clone.clone(),
                        icon: Some(icon_str.clone()),
                        color,
                    });
                } else {
                    sender_clone.input(AppMsg::SetNoteIcon {
                        note_name: name_clone.clone(),
                        icon: Some(icon_str.clone()),
                        color,
                    });
                }
            });

            grid.attach(&button, col, row, 1, 1);
            col += 1;
            if col >= 8 {
                col = 0;
                row += 1;
            }
        }

        content_box.append(&grid);

        scrolled.set_child(Some(&content_box));

        // Contenedor exterior con botones fijos abajo
        let outer_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .build();

        outer_box.append(&scrolled);

        // Separador
        outer_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        // Botones de acción (fijos abajo)
        let button_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .halign(gtk::Align::End)
            .margin_top(12)
            .margin_bottom(12)
            .margin_end(16)
            .build();

        // Botón para quitar icono personalizado
        let remove_button = gtk::Button::builder().label("Por defecto").build();
        remove_button.add_css_class("flat");

        let name_for_remove = name_owned.clone();
        let sender_for_remove = sender.clone();
        let dialog_weak_remove = dialog.downgrade();
        remove_button.connect_clicked(move |_| {
            if let Some(dlg) = dialog_weak_remove.upgrade() {
                dlg.close();
            }

            if is_folder {
                sender_for_remove.input(AppMsg::SetFolderIcon {
                    folder_path: name_for_remove.clone(),
                    icon: None,
                    color: None,
                });
            } else {
                sender_for_remove.input(AppMsg::SetNoteIcon {
                    note_name: name_for_remove.clone(),
                    icon: None,
                    color: None,
                });
            }
        });
        button_box.append(&remove_button);

        // Botón cancelar
        let cancel_button = gtk::Button::builder().label("Cancelar").build();
        let dialog_weak_cancel = dialog.downgrade();
        cancel_button.connect_clicked(move |_| {
            if let Some(dlg) = dialog_weak_cancel.upgrade() {
                dlg.close();
            }
        });
        button_box.append(&cancel_button);

        outer_box.append(&button_box);

        dialog.set_child(Some(&outer_box));
        dialog.present();
    }

    fn update_context_menu_labels(&self) {
        // El menú contextual se recrea cada vez que se muestra en ShowContextMenu
        // con las traducciones actuales, no necesitamos hacer nada aquí
    }

    fn refresh_tags_display_after_language_change(&self) {
        let i18n = self.i18n.borrow();

        // Limpiar la lista de tags
        while let Some(child) = self.tags_list_box.first_child() {
            self.tags_list_box.remove(&child);
        }

        // Si hay una nota cargada, volver a extraer y mostrar sus tags
        if let Some(ref note) = self.current_note {
            if let Ok(content) = note.read() {
                let tags = extract_all_tags(&content);

                if tags.is_empty() {
                    let no_tags_label = gtk::Label::builder()
                        .label(&i18n.t("no_tags"))
                        .halign(gtk::Align::Start)
                        .build();
                    no_tags_label.add_css_class("dim-label");

                    let row = gtk::ListBoxRow::new();
                    row.set_child(Some(&no_tags_label));
                    row.set_selectable(false);
                    row.set_activatable(false);
                    self.tags_list_box.append(&row);
                } else {
                    for tag in tags {
                        let tag_box = gtk::Box::builder()
                            .orientation(gtk::Orientation::Horizontal)
                            .spacing(8)
                            .build();

                        let tag_label = gtk::Label::builder()
                            .label(&format!("#{}", tag))
                            .halign(gtk::Align::Start)
                            .hexpand(true)
                            .build();

                        let remove_button = gtk::Button::builder()
                            .icon_name("user-trash-symbolic")
                            .tooltip_text(&i18n.t("remove_tag"))
                            .valign(gtk::Align::Center)
                            .build();
                        remove_button.add_css_class("flat");
                        remove_button.add_css_class("circular");

                        tag_box.append(&tag_label);
                        tag_box.append(&remove_button);

                        let row = gtk::ListBoxRow::new();
                        row.set_child(Some(&tag_box));
                        row.set_selectable(false);
                        row.set_activatable(false);

                        self.tags_list_box.append(&row);
                    }
                }
            }
        }
    }

    fn apply_initial_translations(&self) {
        let i18n = self.i18n.borrow();

        // Actualizar todos los tooltips con el idioma inicial
        self.sidebar_toggle_button
            .set_tooltip_text(Some(&i18n.t("show_hide_notes")));
        self.search_toggle_button
            .set_tooltip_text(Some(&i18n.t("search_notes")));
        self.new_note_button
            .set_tooltip_text(Some(&i18n.t("new_note")));
        self.settings_button
            .set_tooltip_text(Some(&i18n.t("settings")));
        self.tags_menu_button
            .set_tooltip_text(Some(&i18n.t("tags_note")));

        // Actualizar labels
        self.sidebar_notes_label.set_label(&i18n.t("notes"));

        // Actualizar placeholders
        self.floating_search_entry
            .set_placeholder_text(Some(&i18n.t("search_placeholder")));
    }

    /// Mover una nota a una carpeta específica
    fn move_note_to_folder(
        &mut self,
        note_name: &str,
        folder_name: Option<&str>,
        sender: &ComponentSender<Self>,
    ) {
        println!("Moving note '{}' to folder {:?}", note_name, folder_name);

        // Encontrar la nota en el directorio
        if let Ok(Some(note)) = self.notes_dir.find_note(note_name) {
            // Obtener la ruta actual de la nota
            let current_path = note.path();

            // Calcular la nueva ruta
            let new_path = if let Some(folder) = folder_name {
                // Mover a una carpeta específica
                self.notes_dir
                    .root()
                    .join(folder)
                    .join(format!("{}.md", note_name))
            } else {
                // Mover a la raíz
                self.notes_dir.root().join(format!("{}.md", note_name))
            };

            // Solo mover si la ruta cambió
            if current_path != new_path {
                // Crear el directorio padre si no existe
                if let Some(parent) = new_path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        eprintln!("Error creando directorio: {}", e);
                        return;
                    }
                }

                // Mover el archivo
                if let Err(e) = std::fs::rename(&current_path, &new_path) {
                    eprintln!("Error moviendo nota: {}", e);
                    return;
                }

                // Actualizar la base de datos
                match self.notes_db.get_note(note.name()) {
                    Ok(Some(metadata)) => {
                        // La nota ya existe en la BD, solo actualizar su ubicación
                        if let Err(e) = self.notes_db.move_note_to_folder(
                            metadata.id,
                            folder_name,
                            &new_path.to_string_lossy(),
                        ) {
                            eprintln!("Error actualizando base de datos: {}", e);
                        }
                    }
                    Ok(None) | Err(_) => {
                        // La nota no existe en la BD, indexarla
                        if let Ok(content) = std::fs::read_to_string(&new_path) {
                            if let Err(e) = self.notes_db.index_note(
                                note.name(),
                                &new_path.to_string_lossy(),
                                &content,
                                folder_name,
                            ) {
                                eprintln!("Error indexando nota en BD: {}", e);
                            }
                        }
                    }
                }

                // Refrescar el sidebar
                sender.input(AppMsg::RefreshSidebar);
            }
        } else {
            eprintln!("Nota '{}' no encontrada", note_name);
        }
    }

    /// Reordenar notas dentro de la misma carpeta (cambiar el orden alfabético)
    fn reorder_notes(
        &mut self,
        source_name: &str,
        target_name: &str,
        sender: &ComponentSender<Self>,
    ) {
        println!(
            "Reordering notes: '{}' relative to '{}'",
            source_name, target_name
        );

        // Obtener metadata de source y target
        let source_meta = match self.notes_db.get_note(source_name) {
            Ok(Some(meta)) => meta,
            _ => {
                eprintln!("No se encontró la nota source: {}", source_name);
                return;
            }
        };

        let target_meta = match self.notes_db.get_note(target_name) {
            Ok(Some(meta)) => meta,
            _ => {
                eprintln!("No se encontró la nota target: {}", target_name);
                return;
            }
        };

        // Si no están en la misma carpeta, mover primero
        if source_meta.folder != target_meta.folder {
            println!("Moving note to target folder first");
            self.move_note_to_folder(source_name, target_meta.folder.as_deref(), sender);

            // Recargar metadata después de mover
            let source_meta = match self.notes_db.get_note(source_name) {
                Ok(Some(meta)) => meta,
                _ => {
                    eprintln!("No se pudo recargar metadata de source después de mover");
                    return;
                }
            };

            // Continuar con el reordenamiento
            self.reorder_notes_in_same_folder(source_meta, &target_meta, sender);
        } else {
            // Ya están en la misma carpeta, solo reordenar
            self.reorder_notes_in_same_folder(source_meta, &target_meta, sender);
        }
    }

    /// Reordena notas que ya están en la misma carpeta
    fn reorder_notes_in_same_folder(
        &mut self,
        source_meta: crate::core::database::NoteMetadata,
        target_meta: &crate::core::database::NoteMetadata,
        sender: &ComponentSender<Self>,
    ) {
        let source_name = &source_meta.name;
        let target_name = &target_meta.name;

        // Obtener todas las notas de esta carpeta
        let folder = source_meta.folder.as_deref();
        let mut notes = match self.notes_db.list_notes(folder) {
            Ok(notes) => notes,
            Err(e) => {
                eprintln!("Error obteniendo notas de la carpeta: {}", e);
                return;
            }
        };

        // Encontrar índices de source y target
        let source_idx = notes.iter().position(|n| &n.name == source_name);
        let target_idx = notes.iter().position(|n| &n.name == target_name);

        if let (Some(src_idx), Some(tgt_idx)) = (source_idx, target_idx) {
            // Si la source y target son la misma, no hacer nada
            if src_idx == tgt_idx {
                return;
            }

            println!(
                "Reordenando: source_idx={}, target_idx={}",
                src_idx, tgt_idx
            );

            // Remover source de su posición actual
            let source = notes.remove(src_idx);

            // Calcular la posición de inserción
            // Queremos tomar la posición del target, empujándolo
            let insert_pos = if src_idx < tgt_idx {
                // Source estaba ANTES de target (arrastrando hacia abajo)
                // Al remover source, target se desplaza -1: ahora está en tgt_idx - 1
                // Queremos tomar la posición ORIGINAL del target (antes de que se moviera)
                // Para eso insertamos en tgt_idx (que es donde estaba target originalmente)
                // Esto empuja al target hacia arriba (a la posición tgt_idx)
                tgt_idx
            } else {
                // Source estaba DESPUÉS de target (arrastrando hacia arriba)
                // Target no se movió al remover source (sigue en tgt_idx)
                // Insertamos en tgt_idx para tomar su posición y empujarlo hacia abajo
                tgt_idx
            };

            println!("Insertando en posición: {}", insert_pos);

            // Insertar en la posición calculada
            notes.insert(insert_pos, source);

            // Actualizar order_index de todas las notas de esta carpeta
            for (idx, note) in notes.iter().enumerate() {
                if let Err(e) = self.notes_db.update_note_order(note.id, idx as i32) {
                    eprintln!("Error actualizando order_index para {}: {}", note.name, e);
                }
            }

            println!("Notas reordenadas exitosamente");
        }

        sender.input(AppMsg::RefreshSidebar);
    }

    /// Mover una carpeta a otra carpeta
    fn move_folder(
        &mut self,
        folder_name: &str,
        target_folder: Option<&str>,
        sender: &ComponentSender<Self>,
    ) {
        println!("Moving folder '{}' to {:?}", folder_name, target_folder);

        // Construir la ruta de la carpeta fuente
        let source_path = self.notes_dir.root().join(folder_name);

        // Verificar que la carpeta fuente existe
        if !source_path.exists() || !source_path.is_dir() {
            eprintln!(
                "Carpeta fuente '{}' no existe en {:?}",
                folder_name, source_path
            );
            return;
        }

        // Obtener solo el nombre base de la carpeta (última parte del path)
        let folder_base_name = folder_name.split('/').last().unwrap_or(folder_name);

        // Calcular la nueva ruta
        let new_path = if let Some(target) = target_folder {
            if target.is_empty() || target == "/" {
                // Mover a la raíz
                self.notes_dir.root().join(folder_base_name)
            } else {
                // Mover a una carpeta específica
                self.notes_dir.root().join(target).join(folder_base_name)
            }
        } else {
            // Mover a la raíz
            self.notes_dir.root().join(folder_base_name)
        };

        println!("Source path: {:?}, New path: {:?}", source_path, new_path);

        // Solo mover si la ruta cambió
        if source_path != new_path {
            // Verificar si el destino ya existe
            if new_path.exists() {
                eprintln!("⚠️ El destino ya existe: {:?}", new_path);
                return;
            }

            // Crear el directorio padre si no existe
            if let Some(parent) = new_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("Error creando directorio padre: {}", e);
                    return;
                }
            }

            // Mover la carpeta completa
            println!(
                "📦 Intentando mover carpeta de {:?} a {:?}",
                source_path, new_path
            );
            if let Err(e) = std::fs::rename(&source_path, &new_path) {
                eprintln!("❌ Error moviendo carpeta: {}", e);
                return;
            }
            println!("✅ Carpeta movida exitosamente");

            // Actualizar todas las notas en la base de datos que estaban en esta carpeta
            if let Ok(notes) = self.notes_db.list_notes(None) {
                for note in notes {
                    // Verificar si la nota está en la carpeta que se está moviendo
                    if let Some(ref note_folder) = note.folder {
                        // La nota está en la carpeta si note_folder == folder_name o empieza con folder_name/
                        if note_folder == folder_name
                            || note_folder.starts_with(&format!("{}/", folder_name))
                        {
                            // Calcular la nueva carpeta para esta nota
                            let new_folder = if let Some(target) = target_folder {
                                if target.is_empty() || target == "/" {
                                    // Moviendo a raíz
                                    if note_folder == folder_name {
                                        folder_base_name.to_string()
                                    } else {
                                        // Subcarpeta dentro de la carpeta movida
                                        note_folder.replace(folder_name, folder_base_name)
                                    }
                                } else {
                                    // Moviendo a otra carpeta
                                    if note_folder == folder_name {
                                        format!("{}/{}", target, folder_base_name)
                                    } else {
                                        // Subcarpeta dentro de la carpeta movida
                                        note_folder.replace(
                                            folder_name,
                                            &format!("{}/{}", target, folder_base_name),
                                        )
                                    }
                                }
                            } else {
                                // Moviendo a raíz
                                if note_folder == folder_name {
                                    folder_base_name.to_string()
                                } else {
                                    note_folder.replace(folder_name, folder_base_name)
                                }
                            };

                            // Calcular la nueva ruta del archivo
                            let new_note_path = self
                                .notes_dir
                                .root()
                                .join(&new_folder)
                                .join(format!("{}.md", note.name));

                            println!(
                                "Updating note {} from folder '{}' to '{}'",
                                note.name, note_folder, new_folder
                            );

                            // Actualizar en la base de datos
                            if let Err(e) = self.notes_db.move_note_to_folder(
                                note.id,
                                Some(&new_folder),
                                new_note_path.to_str().unwrap_or(""),
                            ) {
                                eprintln!("Error actualizando nota {}: {}", note.name, e);
                            }
                        }
                    }
                }
            }

            // Refrescar el sidebar
            sender.input(AppMsg::RefreshSidebar);
        }
    }

    /// Obtiene la lista de salidas de audio disponibles usando pactl
    fn get_available_audio_sinks(&self) -> Vec<(String, String)> {
        let output = std::process::Command::new("pactl")
            .arg("list")
            .arg("sinks")
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                self.parse_pactl_sinks_output(&stdout)
            }
            _ => {
                eprintln!("Error ejecutando pactl list sinks");
                Vec::new()
            }
        }
    }

    /// Parsea la salida de 'pactl list sinks' para extraer nombres y descripciones
    fn parse_pactl_sinks_output(&self, output: &str) -> Vec<(String, String)> {
        let mut sinks = Vec::new();
        let mut current_sink = None;
        let mut current_description = None;

        for line in output.lines() {
            let line = line.trim();

            if line.starts_with("Name: ") {
                // Guardar el sink anterior si existe
                if let (Some(name), Some(desc)) = (current_sink.take(), current_description.take())
                {
                    sinks.push((name, desc));
                }

                // Extraer el nombre del nuevo sink
                current_sink = Some(line[6..].to_string());
            } else if line.starts_with("Description: ") {
                // Extraer la descripción
                current_description = Some(line[13..].to_string());
            }
        }

        // Guardar el último sink
        if let (Some(name), Some(desc)) = (current_sink, current_description) {
            sinks.push((name, desc));
        }

        sinks
    }

    /// Establece la salida de audio por defecto usando pactl
    fn set_default_audio_sink(sink_name: &str) -> bool {
        let output = std::process::Command::new("pactl")
            .arg("set-default-sink")
            .arg(sink_name)
            .output();

        match output {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    // ==================== CHAT AI HELPERS ====================

    /// Convierte Markdown simple a Pango markup
    fn markdown_to_pango(text: &str) -> String {
        let mut result = String::new();
        let mut in_code_block = false;
        let mut code_lang = String::new();

        // Pre-compile regexes
        let link_re = regex::Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").ok();
        let wiki_re = regex::Regex::new(r"\[\[([^\]]+)\]\]").ok();

        for line in text.lines() {
            // Code blocks
            if line.trim_start().starts_with("```") {
                if in_code_block {
                    result.push_str("</span>\n");
                    in_code_block = false;
                    code_lang.clear();
                } else {
                    code_lang = line.trim_start().trim_start_matches("```").to_string();
                    result.push_str(
                        "<span font_family='monospace' background='#2d2d2d' foreground='#d4d4d4'>",
                    );
                    in_code_block = true;
                }
                continue;
            }

            if in_code_block {
                // Escapar contenido de código
                let escaped = glib::markup_escape_text(line);
                result.push_str(&escaped);
                result.push('\n');
                continue;
            }

            let mut processed = line.to_string();

            // Headers
            if processed.starts_with("### ") {
                processed = format!(
                    "<span size='large' weight='bold'>{}</span>",
                    glib::markup_escape_text(&processed[4..])
                );
            } else if processed.starts_with("## ") {
                processed = format!(
                    "<span size='x-large' weight='bold'>{}</span>",
                    glib::markup_escape_text(&processed[3..])
                );
            } else if processed.starts_with("# ") {
                processed = format!(
                    "<span size='xx-large' weight='bold'>{}</span>",
                    glib::markup_escape_text(&processed[2..])
                );
            } else {
                // Escapar primero para seguridad
                processed = glib::markup_escape_text(&processed).to_string();

                // Inline code primero (para evitar procesar ** dentro de código): `code`
                processed = processed.replace("`", "⟨CODE⟩");
                let parts: Vec<&str> = processed.split("⟨CODE⟩").collect();
                let mut result_parts = Vec::new();
                for (i, part) in parts.iter().enumerate() {
                    if i % 2 == 1 {
                        // Es código inline
                        result_parts.push(format!("<tt>{}</tt>", part));
                    } else {
                        // Es texto normal, procesar markdown
                        let mut text = part.to_string();

                        // Bold: **text** o __text__
                        while let Some(start) = text.find("**") {
                            if let Some(end) = text[start + 2..].find("**") {
                                let before = &text[..start];
                                let content = &text[start + 2..start + 2 + end];
                                let after = &text[start + 4 + end..];
                                text = format!("{}<b>{}</b>{}", before, content, after);
                            } else {
                                break;
                            }
                        }

                        while let Some(start) = text.find("__") {
                            if let Some(end) = text[start + 2..].find("__") {
                                let before = &text[..start];
                                let content = &text[start + 2..start + 2 + end];
                                let after = &text[start + 4 + end..];
                                text = format!("{}<b>{}</b>{}", before, content, after);
                            } else {
                                break;
                            }
                        }

                        // Italic: *text* (simple, evitar ** ya procesados)
                        let mut chars: Vec<char> = text.chars().collect();
                        let mut i = 0;
                        while i < chars.len() {
                            if chars[i] == '*'
                                && (i == 0 || chars[i - 1] != '*')
                                && (i + 1 < chars.len() && chars[i + 1] != '*')
                            {
                                // Buscar cierre
                                let mut j = i + 1;
                                while j < chars.len() {
                                    if chars[j] == '*'
                                        && (j + 1 >= chars.len() || chars[j + 1] != '*')
                                    {
                                        // Encontrado cierre
                                        let before: String = chars[..i].iter().collect();
                                        let content: String = chars[i + 1..j].iter().collect();
                                        let after: String = chars[j + 1..].iter().collect();
                                        text = format!("{}<i>{}</i>{}", before, content, after);
                                        chars = text.chars().collect();
                                        i = before.len() + 7 + content.len(); // Skip <i></i>
                                        break;
                                    }
                                    j += 1;
                                }
                            }
                            i += 1;
                        }

                        result_parts.push(text);
                    }
                }
                processed = result_parts.join("");

                // Links: [Text](Link)
                if let Some(re) = &link_re {
                    processed = re
                        .replace_all(&processed, "<a href=\"$2\">$1</a>")
                        .to_string();
                }

                // WikiLinks: [[Note]]
                if let Some(re) = &wiki_re {
                    processed = re
                        .replace_all(&processed, "<a href=\"$1\">$1</a>")
                        .to_string();
                }

                // Lists
                if processed.trim_start().starts_with("- ")
                    || processed.trim_start().starts_with("* ")
                {
                    let indent = processed.chars().take_while(|c| c.is_whitespace()).count();
                    let content = processed
                        .trim_start()
                        .trim_start_matches(['-', '*'])
                        .trim_start();
                    processed = format!("{}• {}", " ".repeat(indent), content);
                }
            }

            result.push_str(&processed);
            result.push('\n');
        }

        result
    }

    /// Agrega un mensaje al historial de chat en la UI
    fn append_chat_message(
        &self,
        role: crate::ai_chat::MessageRole,
        content: &str,
        sender: Option<ComponentSender<Self>>,
    ) {
        let timestamp = Local::now().format("%H:%M").to_string();

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        row.set_hexpand(true);
        row.add_css_class("chat-row");

        let avatar = gtk::Label::new(None);
        avatar.add_css_class("chat-avatar");
        avatar.set_valign(gtk::Align::Start); // Alinear arriba

        let bubble = gtk::Box::new(gtk::Orientation::Vertical, 6);
        bubble.add_css_class("chat-bubble");
        bubble.set_spacing(6);
        bubble.set_hexpand(true); // NUEVO: Permitir expansión horizontal

        let meta_label = gtk::Label::new(None);
        meta_label.add_css_class("chat-meta");
        meta_label.set_wrap(false);

        bubble.append(&meta_label);

        match role {
            crate::ai_chat::MessageRole::User => {
                row.add_css_class("chat-row-user");
                row.set_halign(gtk::Align::End);

                avatar.set_text("🙂");
                avatar.add_css_class("chat-avatar-user");

                bubble.add_css_class("chat-bubble-user");
                meta_label.set_text(&format!("Tú · {}", timestamp));
                meta_label.add_css_class("chat-meta-user");
                meta_label.set_xalign(1.0);

                let message_label = gtk::Label::new(Some(content));
                message_label.set_wrap(true);
                message_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                message_label.set_selectable(true);
                message_label.set_xalign(1.0);
                message_label.add_css_class("chat-message");
                message_label.add_css_class("chat-message-user");
                message_label.set_use_markup(false);

                bubble.append(&message_label);

                row.append(&bubble);
                row.append(&avatar);
            }
            crate::ai_chat::MessageRole::Assistant => {
                row.add_css_class("chat-row-assistant");
                row.set_halign(gtk::Align::Start);

                avatar.set_text("🤖");
                avatar.add_css_class("chat-avatar-assistant");

                bubble.add_css_class("chat-bubble-assistant");
                meta_label.set_text(&format!("NotNative AI · {}", timestamp));
                meta_label.add_css_class("chat-meta-assistant");
                meta_label.set_xalign(0.0);

                // Usar renderizado avanzado para soportar tablas
                self.render_chat_content(content, &bubble, sender.clone());

                // Botones de acción (Copiar, Crear Nota)
                let actions_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                actions_box.set_halign(gtk::Align::End);
                actions_box.set_margin_top(4);

                // Botón Copiar
                let copy_btn = gtk::Button::builder()
                    .icon_name("edit-copy-symbolic")
                    .css_classes(vec!["flat", "circular", "chat-action-btn"])
                    .tooltip_text("Copiar respuesta")
                    .build();

                let content_clone = content.to_string();
                let sender_clone = sender.clone();
                copy_btn.connect_clicked(move |_| {
                    if let Some(s) = &sender_clone {
                        s.input(AppMsg::CopyText(content_clone.clone()));
                    }
                });
                actions_box.append(&copy_btn);

                // Botón Crear Nota
                let note_btn = gtk::Button::builder()
                    .icon_name("document-new-symbolic")
                    .css_classes(vec!["flat", "circular", "chat-action-btn"])
                    .tooltip_text("Crear nota con esta respuesta")
                    .build();

                let content_clone2 = content.to_string();
                let sender_clone2 = sender.clone();
                note_btn.connect_clicked(move |_| {
                    if let Some(s) = &sender_clone2 {
                        s.input(AppMsg::CreateNoteFromContent(content_clone2.clone()));
                    }
                });
                actions_box.append(&note_btn);

                bubble.append(&actions_box);

                row.append(&avatar);
                row.append(&bubble);
            }
            crate::ai_chat::MessageRole::System => {
                row.add_css_class("chat-row-system");
                row.set_halign(gtk::Align::Center);

                bubble.add_css_class("chat-bubble-system");
                meta_label.set_text(&format!("Sistema · {}", timestamp));
                meta_label.add_css_class("chat-meta-system");
                meta_label.set_xalign(0.5);

                let message_label = gtk::Label::new(Some(content));
                message_label.set_wrap(true);
                message_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                message_label.set_selectable(true);
                message_label.set_xalign(0.5);
                message_label.add_css_class("chat-message");
                message_label.add_css_class("chat-message-system");

                bubble.append(&message_label);

                row.append(&bubble);
            }
        }

        self.chat_history_list.append(&row);
        self.schedule_chat_scroll();
    }

    fn schedule_chat_scroll(&self) {
        let adjustment_immediate = self.chat_history_scroll.vadjustment();
        gtk::glib::idle_add_local_once(move || {
            Self::scroll_adjustment_to_bottom(&adjustment_immediate);
        });

        let adjustment_delayed = self.chat_history_scroll.vadjustment();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(240), move || {
            Self::scroll_adjustment_to_bottom(&adjustment_delayed);
        });
    }

    fn scroll_adjustment_to_bottom(adjustment: &gtk::Adjustment) {
        let lower = adjustment.lower();
        let upper = adjustment.upper();
        let page = adjustment.page_size();
        let target = if page > 0.0 && upper > page {
            upper - page
        } else {
            upper
        };
        adjustment.set_value(target.max(lower));
    }

    /// Asegura que existe un contenedor para mostrar el pensamiento del agente (thinking steps)
    /// Si no existe, lo crea como un Expander colapsable
    fn ensure_thinking_container(&self) {
        // Si ya existe, no hacer nada
        if self.chat_thinking_container.borrow().is_some() {
            return;
        }

        // Crear un contenedor principal (fila del chat)
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        row.set_hexpand(true);
        row.set_halign(gtk::Align::Start);
        row.add_css_class("chat-row");
        row.add_css_class("chat-row-assistant");

        let avatar = gtk::Label::new(Some("🤖"));
        avatar.add_css_class("chat-avatar");
        avatar.add_css_class("chat-avatar-assistant");
        avatar.set_valign(gtk::Align::Start);
        row.append(&avatar);

        // Crear un expander para mostrar/ocultar el pensamiento
        let expander = gtk::Expander::new(Some("🧠 Proceso de pensamiento del agente"));
        expander.set_expanded(false); // Colapsado por defecto - el usuario puede expandirlo
        expander.add_css_class("agent-thinking-expander");

        // Contenedor interno para los steps
        let steps_container = gtk::Box::new(gtk::Orientation::Vertical, 4);
        steps_container.set_margin_all(8);
        steps_container.add_css_class("agent-thinking-steps");

        expander.set_child(Some(&steps_container));
        row.append(&expander);

        // Agregar a la lista de historial
        self.chat_history_list.append(&row);

        // Guardar referencia al contenedor interno
        *self.chat_thinking_container.borrow_mut() = Some(steps_container);

        self.schedule_chat_scroll();
    }

    /// Muestra un indicador de que la IA está "escribiendo"
    fn append_chat_typing_indicator(&self, status_text: &str) {
        // Primero eliminar cualquier indicador previo
        self.remove_chat_typing_indicator();

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        row.set_hexpand(true);
        row.set_halign(gtk::Align::Start);
        row.add_css_class("chat-row");
        row.add_css_class("chat-row-assistant");
        row.add_css_class("typing-indicator");

        let avatar = gtk::Label::new(Some("🤖"));
        avatar.add_css_class("chat-avatar");
        avatar.add_css_class("chat-avatar-assistant");
        avatar.set_valign(gtk::Align::Start); // Alinear arriba
        row.append(&avatar);

        let bubble = gtk::Box::new(gtk::Orientation::Vertical, 4);
        bubble.add_css_class("chat-bubble");
        bubble.add_css_class("chat-bubble-assistant");
        bubble.add_css_class("chat-bubble-typing");

        let label = gtk::Label::new(Some(status_text));
        label.add_css_class("chat-typing-indicator");
        label.set_xalign(0.0);
        bubble.append(&label);

        row.append(&bubble);
        self.chat_history_list.append(&row);
        self.schedule_chat_scroll();
    }

    /// Elimina el indicador de "escribiendo"
    fn remove_chat_typing_indicator(&self) {
        let mut child = self.chat_history_list.first_child();
        while let Some(widget) = child {
            let next = widget.next_sibling();
            let mut is_indicator = widget.has_css_class("typing-indicator");

            if !is_indicator {
                if let Some(inner) = widget.first_child() {
                    if inner.has_css_class("typing-indicator") {
                        is_indicator = true;
                    }
                }
            }

            if is_indicator {
                self.chat_history_list.remove(&widget);
                println!("🗑️ Eliminado indicador de escribiendo");
                break;
            }
            child = next;
        }
    }

    /// Convierte un tipo de herramienta MCP en un mensaje de estado amigable
    fn tool_to_status_message(tool: &crate::mcp::tools::MCPToolCall) -> String {
        use crate::mcp::tools::MCPToolCall;

        match tool {
            // Gestión de notas
            MCPToolCall::ReadNote { .. } => "Leyendo nota...".to_string(),
            MCPToolCall::UpdateNote { .. } => "Actualizando nota...".to_string(),
            MCPToolCall::CreateNote { .. } => "Creando nota...".to_string(),
            MCPToolCall::DeleteNote { .. } => "Eliminando nota...".to_string(),
            MCPToolCall::RenameNote { .. } => "Renombrando nota...".to_string(),
            MCPToolCall::DuplicateNote { .. } => "Duplicando nota...".to_string(),
            MCPToolCall::AppendToNote { .. } => "Añadiendo contenido...".to_string(),
            MCPToolCall::ListNotes { .. } => "Listando notas...".to_string(),

            // Búsqueda
            MCPToolCall::SearchNotes { .. } => "Buscando notas...".to_string(),
            MCPToolCall::FuzzySearch { .. } => "Buscando...".to_string(),
            MCPToolCall::SearchByTag { .. } => "Buscando por etiqueta...".to_string(),
            MCPToolCall::GetNotesWithTag { .. } => "Buscando por etiqueta...".to_string(),
            MCPToolCall::SearchByDateRange { .. } => "Buscando por fechas...".to_string(),

            // Organización
            MCPToolCall::CreateFolder { .. } => "Creando carpeta...".to_string(),
            MCPToolCall::DeleteFolder { .. } => "Eliminando carpeta...".to_string(),
            MCPToolCall::RenameFolder { .. } => "Renombrando carpeta...".to_string(),
            MCPToolCall::MoveFolder { .. } => "Moviendo carpeta...".to_string(),
            MCPToolCall::MoveNote { .. } => "Moviendo nota...".to_string(),
            MCPToolCall::AddTag { .. } => "Añadiendo etiqueta...".to_string(),
            MCPToolCall::RemoveTag { .. } => "Eliminando etiqueta...".to_string(),
            MCPToolCall::CreateTag { .. } => "Creando etiqueta...".to_string(),
            MCPToolCall::AddMultipleTags { .. } => "Añadiendo etiquetas...".to_string(),
            MCPToolCall::AnalyzeAndTagNote { .. } => {
                "Analizando nota para sugerir tags...".to_string()
            }
            MCPToolCall::ArchiveNote { .. } => "Archivando nota...".to_string(),

            // Análisis
            MCPToolCall::GetNoteStats { .. } => "Analizando nota...".to_string(),
            MCPToolCall::AnalyzeNoteStructure { .. } => "Analizando estructura...".to_string(),
            MCPToolCall::GetWordCount { .. } => "Contando palabras...".to_string(),
            MCPToolCall::FindBrokenLinks { .. } => "Buscando enlaces rotos...".to_string(),
            MCPToolCall::SuggestRelatedNotes { .. } => "Buscando notas relacionadas...".to_string(),
            MCPToolCall::GetRecentNotes { .. } => "Obteniendo notas recientes...".to_string(),
            MCPToolCall::GetAllTags { .. } => "Obteniendo etiquetas...".to_string(),
            MCPToolCall::ListFolders { .. } => "Listando carpetas...".to_string(),
            MCPToolCall::GetNoteGraph { .. } => "Generando grafo de notas...".to_string(),

            // Transformaciones
            MCPToolCall::GenerateTableOfContents { .. } => "Generando índice...".to_string(),
            MCPToolCall::ExtractCodeBlocks { .. } => "Extrayendo código...".to_string(),
            MCPToolCall::FormatNote { .. } => "Formateando nota...".to_string(),
            MCPToolCall::MergeNotes { .. } => "Combinando notas...".to_string(),
            MCPToolCall::SplitNote { .. } => "Dividiendo nota...".to_string(),

            // Control de UI (DESHABILITADO)
            // MCPToolCall::OpenNote { .. } => "Abriendo nota...".to_string(),
            // MCPToolCall::ShowNotification { .. } => "Mostrando notificación...".to_string(),
            // MCPToolCall::HighlightNote { .. } => "Resaltando nota...".to_string(),
            // MCPToolCall::ToggleSidebar => "Alternando barra lateral...".to_string(),
            // MCPToolCall::SwitchMode { .. } => "Cambiando modo...".to_string(),
            // MCPToolCall::RefreshSidebar => "Actualizando barra lateral...".to_string(),
            // MCPToolCall::FocusSearch => "Enfocando búsqueda...".to_string(),

            // Exportación
            MCPToolCall::ExportNote { .. } => "Exportando nota...".to_string(),
            MCPToolCall::ExportMultipleNotes { .. } => "Exportando notas...".to_string(),
            MCPToolCall::BackupNotes { .. } => "Creando respaldo...".to_string(),
            MCPToolCall::ImportFromUrl { .. } => "Importando desde URL...".to_string(),

            // Multimedia
            MCPToolCall::InsertImage { .. } => "Insertando imagen...".to_string(),
            MCPToolCall::InsertYouTubeVideo { .. } => "Insertando video...".to_string(),

            // Default
            _ => "Procesando...".to_string(),
        }
    }

    /// Detecta si un mensaje es un resultado de búsqueda MCP
    fn is_search_result(&self, content: &str) -> bool {
        // Patrón: "✓ N nota(s) encontrada(s):\n  1. ..."
        content.contains("✓")
            && content.contains("nota(s) encontrada(s):")
            && content
                .lines()
                .any(|line| line.trim().starts_with("1.") || line.trim().starts_with("2."))
    }

    /// Renderiza resultados de búsqueda como widget clickable
    fn append_search_results_widget(&self, content: &str, sender: &ComponentSender<Self>) {
        // Parsear el contenido para extraer las notas
        let mut notes = Vec::new();
        let mut count = 0;

        for line in content.lines() {
            // Detectar la línea del header para obtener el count
            if line.contains("nota(s) encontrada(s):") {
                if let Some(num_str) = line.split_whitespace().next() {
                    if let Some(stripped) = num_str.strip_prefix("✓") {
                        count = stripped.trim().parse().unwrap_or(0);
                    }
                }
            }
            // Parsear líneas numeradas: "  1. NotaNombre"
            else if let Some(rest) = line.trim().strip_prefix(|c: char| c.is_numeric()) {
                if let Some(note_name) = rest.strip_prefix('.').map(|s| s.trim()) {
                    notes.push(note_name.to_string());
                }
            }
        }

        if notes.is_empty() {
            // Fallback: mostrar como texto normal
            self.append_chat_message(
                crate::ai_chat::MessageRole::Assistant,
                content,
                Some(sender.clone()),
            );
            return;
        }

        // Crear widget clickable
        let results_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        results_row.set_margin_top(6);
        results_row.set_margin_bottom(6);
        results_row.set_hexpand(true);
        results_row.add_css_class("chat-row");
        results_row.add_css_class("chat-row-assistant");
        results_row.set_halign(gtk::Align::Start);

        let avatar = gtk::Label::new(Some("🔍"));
        avatar.add_css_class("chat-avatar");
        avatar.add_css_class("chat-avatar-assistant");
        avatar.set_valign(gtk::Align::Start);

        let bubble = gtk::Box::new(gtk::Orientation::Vertical, 4);
        bubble.add_css_class("chat-bubble");
        bubble.add_css_class("chat-bubble-assistant");

        let header = gtk::Label::new(Some(&format!(
            "✓ {} nota(s) encontrada(s):",
            if count > 0 { count } else { notes.len() }
        )));
        header.set_xalign(0.0);
        header.add_css_class("heading");
        bubble.append(&header);

        // Crear botones clickables
        for (i, note_name) in notes.iter().enumerate() {
            let note_btn = gtk::Button::builder()
                .label(&format!("  {}. {}", i + 1, note_name))
                .halign(gtk::Align::Start)
                .build();
            note_btn.add_css_class("flat");
            note_btn.set_margin_top(2);

            let sender_clone = sender.clone();
            let note_name_owned = note_name.clone();
            note_btn.connect_clicked(move |_| {
                sender_clone.input(AppMsg::LoadNote {
                    name: note_name_owned.clone(),
                    highlight_text: None,
                });
                sender_clone.input(AppMsg::ExitChatMode);
            });

            bubble.append(&note_btn);
        }

        results_row.append(&avatar);
        results_row.append(&bubble);
        self.chat_history_list.append(&results_row);
        self.schedule_chat_scroll();
    }

    /// Actualiza la lista de notas en el contexto del chat
    fn refresh_context_list(&self) {
        // Limpiar lista actual
        while let Some(child) = self.chat_context_list.first_child() {
            self.chat_context_list.remove(&child);
        }

        // Agregar notas del contexto
        if let Some(session) = self.chat_session.borrow().as_ref() {
            if session.attached_notes.is_empty() {
                let empty_label = gtk::Label::new(Some("Sin notas en contexto"));
                empty_label.add_css_class("dim-label");
                empty_label.add_css_class("chat-context-empty");
                empty_label.set_margin_all(12);
                self.chat_context_list.append(&empty_label);
            } else {
                for note in &session.attached_notes {
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                    row.set_margin_all(0);
                    row.set_hexpand(true);
                    row.set_halign(gtk::Align::Fill);
                    row.add_css_class("chat-context-entry");

                    let icon = gtk::Label::new(Some("📄"));
                    icon.add_css_class("chat-context-icon");
                    row.append(&icon);

                    // Mostrar solo el nombre del archivo, no la ruta completa
                    let display_name = std::path::Path::new(note.name())
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or(note.name());

                    let label = gtk::Label::new(Some(display_name));
                    label.set_xalign(0.0);
                    label.set_hexpand(true);
                    label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                    label.add_css_class("chat-context-label");
                    row.append(&label);

                    // Botón para remover
                    let remove_btn = gtk::Button::new();
                    remove_btn.set_icon_name("list-remove-symbolic");
                    remove_btn.set_tooltip_text(Some("Remover del contexto"));
                    remove_btn.add_css_class("flat");
                    remove_btn.add_css_class("circular");
                    remove_btn.add_css_class("chat-context-remove");

                    let note_name = note.name().to_string();
                    let sender = self.app_sender.borrow().clone();
                    remove_btn.connect_clicked(move |_| {
                        if let Some(s) = &sender {
                            s.input(AppMsg::DetachNoteFromContext(note_name.clone()));
                        }
                    });
                    row.append(&remove_btn);

                    self.chat_context_list.append(&row);
                }
            }
        }
    }

    /// Formatea el texto de una acción para mostrar de forma más legible
    fn format_action_text(action: &str) -> String {
        // Intentar extraer el nombre de la herramienta del Debug format
        // Ej: "CreateNote { name: \"...\", ... }" -> "create_note"
        if let Some(tool_name_end) = action.find('{').or_else(|| action.find('(')) {
            let tool_name = action[..tool_name_end].trim();
            // Convertir de PascalCase a snake_case
            let snake_case = tool_name
                .chars()
                .enumerate()
                .flat_map(|(i, c)| {
                    if c.is_uppercase() && i > 0 {
                        vec!['_', c.to_lowercase().next().unwrap()]
                    } else {
                        vec![c.to_lowercase().next().unwrap()]
                    }
                })
                .collect::<String>();

            format!("🔧 {}", snake_case)
        } else {
            action.to_string()
        }
    }

    /// Formatea el texto de una observación para mostrar de forma más legible
    fn format_observation_text(observation: &str) -> String {
        // Intentar parsear como JSON y extraer el mensaje principal
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(observation) {
            if let Some(obj) = json.as_object() {
                // Buscar campo "message" o "data.message"
                if let Some(msg) = obj.get("message").and_then(|v| v.as_str()) {
                    return format!("✓ {}", msg);
                }

                if let Some(data) = obj.get("data").and_then(|v| v.as_object()) {
                    if let Some(msg) = data.get("message").and_then(|v| v.as_str()) {
                        return format!("✓ {}", msg);
                    }
                }

                // Si tiene campo "success"
                if let Some(success) = obj.get("success").and_then(|v| v.as_bool()) {
                    if success {
                        return "✓ Operación exitosa".to_string();
                    } else {
                        if let Some(error) = obj.get("error").and_then(|v| v.as_str()) {
                            return format!("✗ Error: {}", error);
                        }
                        return "✗ Operación fallida".to_string();
                    }
                }
            }
        }

        // Si no se puede parsear, devolver tal cual (truncado si es muy largo)
        observation.to_string()
    }

    // ==================== FUNCIONES DE RECORDATORIOS ====================

    /// Crea una fila de recordatorio para la lista
    fn create_reminder_row(
        &self,
        reminder: &crate::reminders::Reminder,
        sender: relm4::ComponentSender<MainApp>,
    ) -> gtk::Box {
        use crate::reminders::{Priority, ReminderStatus};

        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_margin_all(8);
        row.add_css_class("reminder-row");

        // Icono de prioridad
        let priority_icon = match reminder.priority {
            Priority::Urgent => "🔴",
            Priority::High => "🟠",
            Priority::Medium => "🟡",
            Priority::Low => "🟢",
        };
        let icon_label = gtk::Label::new(Some(priority_icon));
        row.append(&icon_label);

        // Contenido (texto + fecha)
        let content_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        content_box.set_hexpand(true);

        let text_label = gtk::Label::new(Some(&reminder.title));
        text_label.set_xalign(0.0);
        text_label.set_wrap(true);
        text_label.add_css_class("reminder-text");

        let i18n = self.i18n.borrow();
        let is_spanish = i18n.current_language() == crate::i18n::Language::Spanish;
        let date_label = gtk::Label::new(Some(&reminder.format_due_date(is_spanish)));
        date_label.set_xalign(0.0);
        date_label.add_css_class("reminder-date");
        date_label.add_css_class("dim-label");

        content_box.append(&text_label);
        content_box.append(&date_label);
        row.append(&content_box);

        // Botones de acción
        if reminder.status != ReminderStatus::Completed {
            // Botón completar
            let complete_btn = gtk::Button::new();
            complete_btn.set_icon_name("emblem-ok-symbolic");
            complete_btn.set_tooltip_text(Some(&i18n.t("reminder_complete")));
            complete_btn.add_css_class("flat");
            complete_btn.add_css_class("circular");

            let id = reminder.id;
            let sender_clone = sender.clone();
            complete_btn.connect_clicked(move |_| {
                sender_clone.input(AppMsg::CompleteReminder(id));
            });
            row.append(&complete_btn);

            // Botón posponer
            let snooze_btn = gtk::Button::new();
            snooze_btn.set_icon_name("alarm-symbolic");
            snooze_btn.set_tooltip_text(Some(&i18n.t("reminder_snooze")));
            snooze_btn.add_css_class("flat");
            snooze_btn.add_css_class("circular");

            let sender_clone = sender.clone();
            snooze_btn.connect_clicked(move |_| {
                sender_clone.input(AppMsg::SnoozeReminder { id, minutes: 15 });
            });
            row.append(&snooze_btn);
        }

        // Botón eliminar
        let delete_btn = gtk::Button::new();
        delete_btn.set_icon_name("user-trash-symbolic");
        delete_btn.set_tooltip_text(Some(&i18n.t("reminder_delete")));
        delete_btn.add_css_class("flat");
        delete_btn.add_css_class("circular");
        delete_btn.add_css_class("destructive-action");

        let id = reminder.id;
        delete_btn.connect_clicked(move |_| {
            sender.input(AppMsg::DeleteReminder(id));
        });
        row.append(&delete_btn);

        row
    }

    /// Actualiza el badge de recordatorios pendientes
    fn update_reminder_badge(&self, count: usize) {
        if count > 0 {
            self.reminders_pending_badge.set_text(&count.to_string());
            self.reminders_pending_badge.set_visible(true);
        } else {
            self.reminders_pending_badge.set_visible(false);
        }
    }
}
