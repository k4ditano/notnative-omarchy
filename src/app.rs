use gtk::glib;
use relm4::gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent, component, gtk};
use std::cell::RefCell;
use std::rc::Rc;

use crate::core::{
    CommandParser, EditorAction, EditorMode, KeyModifiers, MarkdownParser, NoteBuffer, NoteFile,
    NotesConfig, NotesDatabase, NotesDirectory, StyleType, extract_all_tags,
};
use crate::i18n::{I18n, Language};

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
struct TagSpan {
    start: i32,
    end: i32,
    tag: String,
}

#[derive(Debug, Clone)]
struct YouTubeVideoSpan {
    start: i32,
    end: i32,
    video_id: String,
    url: String,
}

#[derive(Debug, Clone)]
struct TodoSection {
    title: String,
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
    notes_config: NotesConfig,
    current_note: Option<NoteFile>,
    has_unsaved_changes: bool,
    markdown_enabled: bool,
    bit8_mode: bool,
    text_view: gtk::TextView,
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
    tag_spans: Rc<RefCell<Vec<TagSpan>>>,
    youtube_video_spans: Rc<RefCell<Vec<YouTubeVideoSpan>>>,
    tags_menu_button: gtk::MenuButton,
    tags_list_box: gtk::ListBox,
    todos_menu_button: gtk::MenuButton,
    todos_list_box: gtk::ListBox,
    tag_completion_popup: gtk::Popover,
    tag_completion_list: gtk::ListBox,
    current_tag_prefix: Rc<RefCell<Option<String>>>, // Tag que se está escribiendo actualmente
    just_completed_tag: Rc<RefCell<bool>>, // Bandera para evitar reabrir el popover después de completar
    search_bar: gtk::Box,
    search_entry: gtk::SearchEntry,
    search_toggle_button: gtk::ToggleButton,
    search_active: bool,
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
    // Sender para comunicación asíncrona desde closures
    app_sender: Rc<RefCell<Option<ComponentSender<Self>>>>,
    // Servidor HTTP local para embeds de YouTube
    youtube_server: Rc<crate::youtube_server::YouTubeEmbedServer>,
}

#[derive(Debug)]
pub enum AppMsg {
    ToggleTheme,
    #[allow(dead_code)]
    SetTheme(ThemePreference),
    RefreshTheme, // Nuevo: actualizar cuando el tema del sistema cambia
    Toggle8BitMode,
    ToggleSidebar,
    OpenSidebarAndFocus,
    ShowCreateNoteDialog,
    ToggleFolder(String),
    ShowContextMenu(f64, f64, String, bool), // x, y, nombre, es_carpeta
    DeleteItem(String, bool),                // nombre, es_carpeta
    RenameItem(String, bool),                // nombre, es_carpeta
    RefreshSidebar,
    KeyPress {
        key: String,
        modifiers: KeyModifiers,
    },
    ProcessAction(EditorAction),
    SaveCurrentNote,
    AutoSave,
    LoadNote(String),
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
    CheckTagCompletion,  // Verificar si hay que mostrar autocompletado
    CompleteTag(String), // Completar tag seleccionado
    ToggleSearch(bool),  // Toggle search bar
    SearchNotes(String), // Buscar notas
    ShowPreferences,
    ShowKeyboardShortcuts,
    ShowAboutDialog,
    ChangeLanguage(Language),
    ReloadConfig,                // Recargar configuración desde disco
    InsertImage,                 // Abrir diálogo para seleccionar imagen
    InsertImageFromPath(String), // Insertar imagen desde una ruta
    ProcessPastedText(String),   // Procesar texto pegado (puede ser URL de imagen o YouTube)
    ToggleTodo {
        line_number: usize,
        new_state: bool,
    }, // Marcar/desmarcar TODO
    AskTranscribeYouTube {
        url: String,
        video_id: String,
    }, // Preguntar si transcribir video
    InsertYouTubeLink(String),   // Insertar solo el enlace del video
    InsertYouTubeWithTranscript {
        video_id: String,
    }, // Insertar video con transcripción
    UpdateTranscript {
        video_id: String,
        transcript: String,
    }, // Actualizar con transcripción obtenida
    MoveNoteToFolder {
        note_name: String,
        folder_name: Option<String>,
    }, // Mover nota a carpeta
    ReorderNotes {
        source_name: String,
        target_name: String,
    }, // Reordenar notas
    MoveFolder {
        folder_name: String,
        target_folder: Option<String>,
    }, // Mover carpeta
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

                            append = search_toggle_button = &gtk::ToggleButton {
                                set_icon_name: "system-search-symbolic",
                                set_tooltip_text: Some("Buscar (Ctrl+F)"),
                                add_css_class: "flat",
                                add_css_class: "circular",
                                connect_toggled[sender] => move |btn| {
                                    sender.input(AppMsg::ToggleSearch(btn.is_active()));
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

                        append = search_bar = &gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 0,
                            set_margin_start: 8,
                            set_margin_end: 8,
                            set_margin_top: 0,
                            set_margin_bottom: 8,
                            set_visible: false,

                            append = search_entry = &gtk::SearchEntry {
                                set_placeholder_text: Some("Buscar notas..."),
                                set_hexpand: true,
                                set_width_request: 50,
                            },
                        },

                        append = &gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_hexpand: true,
                            set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                            #[wrap(Some)]
                            set_child = notes_list = &gtk::ListBox {
                                add_css_class: "navigation-sidebar",
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

                        append = &gtk::ScrolledWindow {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_policy: (gtk::PolicyType::Automatic, gtk::PolicyType::Automatic),

                        #[wrap(Some)]
                        set_child = text_view = &gtk::TextView::builder()
                            .monospace(true)
                            .wrap_mode(gtk::WrapMode::WordChar)
                            .editable(true)
                            .cursor_visible(true)
                            .accepts_tab(false)
                            .left_margin(16)
                            .right_margin(16)
                            .top_margin(12)
                            .bottom_margin(12)
                            .build(),
                    },

                        append = status_bar = &gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,
                            set_margin_all: 6,
                            add_css_class: "status-bar",

                            append = mode_label = &gtk::Label {
                                set_markup: "<b>NORMAL</b>",
                                set_xalign: 0.0,
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

                                    #[wrap(Some)]
                                    set_child = &gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_spacing: 8,
                                        set_margin_all: 12,
                                        set_width_request: 280,

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

        let text_buffer = widgets.text_view.buffer();
        let mode = Rc::new(RefCell::new(EditorMode::Normal));

        // Inicializar directorio de notas (por defecto ~/.local/share/notnative/notes)
        let notes_dir = NotesDirectory::default();

        // Inicializar base de datos
        let db_path = notes_dir.db_path();
        let notes_db = NotesDatabase::new(&db_path).expect("No se pudo crear la base de datos");

        // Cargar configuración
        let config_path = NotesConfig::default_path();
        let notes_config = NotesConfig::load(&config_path).unwrap_or_else(|_| {
            println!("No se pudo cargar configuración, creando una nueva");
            NotesConfig::new()
        });

        // Determinar idioma: usar configuración guardada o detectar del sistema
        let language = if let Some(lang_code) = notes_config.get_language() {
            Language::from_code(lang_code)
        } else {
            Language::from_env()
        };

        let i18n = Rc::new(RefCell::new(I18n::new(language)));
        println!("Idioma detectado: {:?}", language);

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

        // Intentar cargar la nota "bienvenida" o crearla si no existe
        let (initial_buffer, current_note) = match notes_dir.find_note("bienvenida") {
            Ok(Some(note)) => match note.read() {
                Ok(content) => {
                    println!("Nota 'bienvenida' cargada");
                    (NoteBuffer::from_text(&content), Some(note))
                }
                Err(_) => (NoteBuffer::new(), None),
            },
            _ => {
                // Crear nota de bienvenida
                let welcome_content = "# Bienvenido a NotNative

Esta es tu primera nota. NotNative guarda cada nota como un archivo .md independiente.

## Comandos básicos

- `i` → Modo INSERT (editar)
- `Esc` → Modo NORMAL
- `h/j/k/l` → Navegar (izquierda/abajo/arriba/derecha)
- `x` → Eliminar carácter
- `u` → Deshacer
- `Ctrl+S` → Guardar

Las notas se guardan automáticamente en: ~/.local/share/notnative/notes/
";
                match notes_dir.create_note("bienvenida", welcome_content) {
                    Ok(note) => {
                        println!("Nota de bienvenida creada");
                        (NoteBuffer::from_text(welcome_content), Some(note))
                    }
                    Err(_) => (NoteBuffer::new(), None),
                }
            }
        };

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
        completion_popover.set_parent(&widgets.text_view);
        completion_popover.add_css_class("tag-completion");
        completion_popover.set_autohide(false);
        completion_popover.set_child(Some(&scrolled));

        let model = MainApp {
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
            notes_config,
            current_note,
            has_unsaved_changes: false,
            markdown_enabled: true, // Ahora con parser robusto usando offsets de pulldown-cmark
            bit8_mode: false,
            text_view: widgets.text_view.clone(),
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
            tag_spans: Rc::new(RefCell::new(Vec::new())),
            youtube_video_spans: Rc::new(RefCell::new(Vec::new())),
            tags_menu_button: widgets.tags_menu_button.clone(),
            tags_list_box: widgets.tags_list_box.clone(),
            todos_menu_button: widgets.todos_menu_button.clone(),
            todos_list_box: widgets.todos_list_box.clone(),
            tag_completion_popup: completion_popover.clone(),
            tag_completion_list: completion_list_box.clone(),
            current_tag_prefix: Rc::new(RefCell::new(None)),
            just_completed_tag: Rc::new(RefCell::new(false)),
            search_bar: widgets.search_bar.clone(),
            search_entry: widgets.search_entry.clone(),
            search_toggle_button: widgets.search_toggle_button.clone(),
            search_active: false,
            i18n,
            sidebar_toggle_button: widgets.sidebar_toggle_button.clone(),
            sidebar_notes_label: widgets.sidebar_notes_label.clone(),
            new_note_button: widgets.new_note_button.clone(),
            settings_button: widgets.settings_button.clone(),
            image_widgets: Rc::new(RefCell::new(Vec::new())),
            todo_widgets: Rc::new(RefCell::new(Vec::new())),
            video_widgets: Rc::new(RefCell::new(Vec::new())),
            app_sender: Rc::new(RefCell::new(None)),
            youtube_server: {
                let server = Rc::new(crate::youtube_server::YouTubeEmbedServer::new(8787));
                // Iniciar el servidor en un thread separado
                if let Err(e) = server.start() {
                    eprintln!("Error iniciando servidor YouTube: {}", e);
                }
                server
            },
        };

        // Guardar el sender en el modelo
        *model.app_sender.borrow_mut() = Some(sender.clone());

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

        let action_group = gtk::gio::SimpleActionGroup::new();
        action_group.add_action(&rename_action);
        action_group.add_action(&delete_action);
        context_menu.insert_action_group("item", Some(&action_group));

        // Crear tags de estilo para markdown
        model.create_text_tags();

        // Crear popover del settings button con textos traducidos
        model.create_settings_popover(&sender);

        // Aplicar traducciones iniciales a todos los widgets
        model.apply_initial_translations();

        // Sincronizar contenido inicial con la vista
        model.sync_to_view();
        model.update_status_bar(&sender);

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

        // Conectar señal de cierre para guardar antes de cerrar
        widgets.main_window.connect_close_request(gtk::glib::clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::SaveCurrentNote);
                gtk::glib::Propagation::Proceed
            }
        ));

        // Conectar shortcut Ctrl+F para activar búsqueda
        let search_toggle_ref = model.search_toggle_button.clone();
        let split_view_ref = model.split_view.clone();
        let search_entry_ref = model.search_entry.clone();
        let window_key_controller = gtk::EventControllerKey::new();
        window_key_controller.connect_key_pressed(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            search_toggle_ref,
            #[strong]
            split_view_ref,
            #[strong]
            search_entry_ref,
            move |_controller, keyval, _keycode, modifiers| {
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

                // Ctrl+F para activar búsqueda
                if key_name == "f" && modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                    // Abrir sidebar si está cerrado (posición < 200px indica cerrado)
                    let current_position = split_view_ref.position();
                    if current_position < 200 {
                        sender.input(AppMsg::ToggleSidebar);

                        // Esperar a que termine la animación del sidebar antes de activar búsqueda
                        let search_toggle_clone = search_toggle_ref.clone();
                        let search_entry_clone = search_entry_ref.clone();
                        gtk::glib::timeout_add_local_once(
                            std::time::Duration::from_millis(280),
                            move || {
                                search_toggle_clone.set_active(true);
                                search_entry_clone.grab_focus();
                            },
                        );
                    } else {
                        // Sidebar ya está abierto, activar búsqueda inmediatamente
                        search_toggle_ref.set_active(true);

                        // Usar timeout para asegurar que el focus se aplica
                        let search_entry_clone = search_entry_ref.clone();
                        gtk::glib::timeout_add_local_once(
                            std::time::Duration::from_millis(50),
                            move || {
                                search_entry_clone.grab_focus();
                            },
                        );
                    }

                    return gtk::glib::Propagation::Stop;
                }

                gtk::glib::Propagation::Proceed
            }
        ));
        widgets.main_window.add_controller(window_key_controller);

        // Conectar eventos de teclado al TextView
        let search_toggle_textview = model.search_toggle_button.clone();
        let split_view_textview = model.split_view.clone();
        let search_entry_textview = model.search_entry.clone();
        let key_controller = gtk::EventControllerKey::new();
        key_controller.connect_key_pressed(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            mode,
            #[strong]
            search_toggle_textview,
            #[strong]
            split_view_textview,
            #[strong]
            search_entry_textview,
            move |_controller, keyval, _keycode, modifiers| {
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

                // PRIORIDAD MÁXIMA: Ctrl+F siempre funciona, sin importar el modo
                if key_name == "f" && modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                    // Abrir sidebar si está cerrado
                    let current_position = split_view_textview.position();
                    if current_position < 200 {
                        sender.input(AppMsg::ToggleSidebar);

                        let search_toggle_clone = search_toggle_textview.clone();
                        let search_entry_clone = search_entry_textview.clone();
                        gtk::glib::timeout_add_local_once(
                            std::time::Duration::from_millis(280),
                            move || {
                                search_toggle_clone.set_active(true);
                                search_entry_clone.grab_focus();
                            },
                        );
                    } else {
                        search_toggle_textview.set_active(true);

                        let search_entry_clone = search_entry_textview.clone();
                        gtk::glib::timeout_add_local_once(
                            std::time::Duration::from_millis(50),
                            move || {
                                search_entry_clone.grab_focus();
                            },
                        );
                    }

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
        widgets.text_view.add_controller(key_controller);

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
        let click_text_view = widgets.text_view.clone();
        // Conectar eventos de clic para actualizar posición del cursor o abrir enlaces/tags
        let click_controller = gtk::GestureClick::new();
        let tag_spans_for_click = model.tag_spans.clone();
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
                                // Buscar notas con este tag
                                sender.input(AppMsg::OpenSidebarAndFocus);
                                sender.input(AppMsg::ToggleSearch(true));
                                sender.input(AppMsg::SearchNotes(format!("#{}", tag_span.tag)));
                                return;
                            }

                            // Verificar si es un link
                            if let Some(link) = link_spans
                                .borrow()
                                .iter()
                                .find(|span| offset >= span.start && offset < span.end)
                            {
                                gesture.set_state(gtk::EventSequenceState::Claimed);
                                if let Err(err) = gtk::gio::AppInfo::launch_default_for_uri(
                                    &link.url,
                                    None::<&gtk::gio::AppLaunchContext>,
                                ) {
                                    eprintln!("Error al abrir enlace {}: {}", link.url, err);
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
        widgets.text_view.add_controller(click_controller);

        // Agregar controlador de movimiento del mouse para cambiar cursor sobre links y tags
        let motion_controller = gtk::EventControllerMotion::new();
        let motion_text_view = widgets.text_view.clone();
        let tag_spans_for_motion = model.tag_spans.clone();
        motion_controller.connect_motion(gtk::glib::clone!(
            #[strong(rename_to = text_view)]
            motion_text_view,
            #[strong]
            mode,
            #[strong]
            link_spans,
            #[strong]
            tag_spans_for_motion,
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

                        let is_over_link = link_spans
                            .borrow()
                            .iter()
                            .any(|span| offset >= span.start && offset < span.end);

                        if is_over_link || is_over_tag {
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
        widgets.text_view.add_controller(motion_controller);

        // Configurar DropTarget para detectar cuando se arrastra contenido
        let drop_target = gtk::DropTarget::new(gtk::glib::Type::STRING, gtk::gdk::DragAction::COPY);
        drop_target.connect_drop(gtk::glib::clone!(
            #[strong]
            sender,
            move |_target, value, _x, _y| {
                if let Ok(text) = value.get::<String>() {
                    // Procesar el texto arrastrado (puede ser URL de imagen)
                    sender.input(AppMsg::ProcessPastedText(text));
                    true
                } else {
                    false
                }
            }
        ));
        widgets.text_view.add_controller(drop_target);

        // Conectar eventos del search_entry con debounce
        let search_generation: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));
        widgets
            .search_entry
            .connect_search_changed(gtk::glib::clone!(
                #[strong]
                sender,
                #[strong]
                search_generation,
                move |entry| {
                    let query = entry.text().to_string();

                    // Incrementar generación para invalidar búsquedas anteriores
                    *search_generation.borrow_mut() += 1;
                    let current_gen = *search_generation.borrow();

                    let sender_clone = sender.clone();
                    let search_gen_clone = search_generation.clone();

                    // Crear timeout de 300ms
                    gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(300),
                        move || {
                            // Solo ejecutar si la generación no cambió (no hubo nuevas teclas)
                            if *search_gen_clone.borrow() == current_gen {
                                sender_clone.input(AppMsg::SearchNotes(query));
                            }
                        },
                    );
                }
            ));

        // Conectar tecla Escape y flechas para navegación
        let search_toggle_ref2 = model.search_toggle_button.clone();
        let notes_list_for_nav = model.notes_list.clone();
        let text_view_for_focus = model.text_view.clone();
        let search_key_controller = gtk::EventControllerKey::new();
        search_key_controller.connect_key_pressed(gtk::glib::clone!(
            #[strong]
            sender,
            #[strong]
            search_toggle_ref2,
            #[strong]
            notes_list_for_nav,
            #[strong]
            text_view_for_focus,
            move |_controller, keyval, _keycode, _modifiers| {
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

                match key_name.as_str() {
                    "Escape" => {
                        search_toggle_ref2.set_active(false);
                        // Poner foco en el text_view con un pequeño delay
                        let text_view_clone = text_view_for_focus.clone();
                        gtk::glib::timeout_add_local_once(
                            std::time::Duration::from_millis(10),
                            move || {
                                text_view_clone.grab_focus();
                            },
                        );
                        return gtk::glib::Propagation::Stop;
                    }
                    "Down" | "Up" => {
                        // Obtener la fila seleccionada actual
                        let current_row = notes_list_for_nav.selected_row();

                        if key_name == "Down" {
                            // Navegar hacia abajo
                            if let Some(row) = current_row {
                                if let Some(next) = row.next_sibling() {
                                    if let Ok(next_row) = next.downcast::<gtk::ListBoxRow>() {
                                        if next_row.is_selectable() {
                                            notes_list_for_nav.select_row(Some(&next_row));
                                            return gtk::glib::Propagation::Stop;
                                        }
                                    }
                                }
                            } else {
                                // Si no hay selección, seleccionar la primera fila
                                if let Some(first_child) = notes_list_for_nav.first_child() {
                                    if let Ok(first_row) = first_child.downcast::<gtk::ListBoxRow>()
                                    {
                                        if first_row.is_selectable() {
                                            notes_list_for_nav.select_row(Some(&first_row));
                                            return gtk::glib::Propagation::Stop;
                                        }
                                    }
                                }
                            }
                        } else {
                            // Navegar hacia arriba
                            if let Some(row) = current_row {
                                if let Some(prev) = row.prev_sibling() {
                                    if let Ok(prev_row) = prev.downcast::<gtk::ListBoxRow>() {
                                        if prev_row.is_selectable() {
                                            notes_list_for_nav.select_row(Some(&prev_row));
                                            return gtk::glib::Propagation::Stop;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "Return" => {
                        // Activar la fila seleccionada con Enter
                        if let Some(row) = notes_list_for_nav.selected_row() {
                            // Revisar si es una carpeta y alternar expansión
                            let is_folder = unsafe {
                                row.data::<bool>("is_folder")
                                    .map(|data| *data.as_ref())
                                    .unwrap_or(false)
                            };

                            if is_folder {
                                if let Some(folder_name) = unsafe {
                                    row.data::<String>("folder_name")
                                        .map(|data| data.as_ref().clone())
                                } {
                                    sender.input(AppMsg::ToggleFolder(folder_name));
                                }
                                return gtk::glib::Propagation::Stop;
                            }

                            // Obtener el nombre de la nota y cargarla
                            let note_name = unsafe {
                                row.data::<String>("note_name")
                                    .map(|data| data.as_ref().clone())
                            };

                            if let Some(name) = note_name {
                                sender.input(AppMsg::LoadNote(name));
                            } else {
                                // Si no está en set_data, intentar obtenerlo del label
                                if let Some(child) = row.child() {
                                    if let Ok(box_widget) = child.downcast::<gtk::Box>() {
                                        if let Some(label_widget) =
                                            box_widget.first_child().and_then(|w| w.next_sibling())
                                        {
                                            if let Ok(label) = label_widget.downcast::<gtk::Label>()
                                            {
                                                let note_name = label.text().to_string();
                                                sender.input(AppMsg::LoadNote(note_name));
                                            }
                                        }
                                    }
                                }
                            }
                            return gtk::glib::Propagation::Stop;
                        }
                    }
                    _ => {}
                }

                gtk::glib::Propagation::Proceed
            }
        ));
        widgets.search_entry.add_controller(search_key_controller);

        // Poblar la lista de notas
        model.populate_notes_list(&sender);
        *model.is_populating_list.borrow_mut() = false;

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
                        sender.input(AppMsg::LoadNote(name));
                        return;
                    }

                    // Si no está en set_data, obtener desde el label (lista normal)
                    if let Some(child) = row.child() {
                        if let Ok(box_widget) = child.downcast::<gtk::Box>() {
                            // El label es el segundo hijo (después del icono)
                            if let Some(label_widget) = box_widget.first_child().and_then(|w| w.next_sibling()) {
                                if let Ok(label) = label_widget.downcast::<gtk::Label>() {
                                    let note_name = label.text().to_string();
                                    sender.input(AppMsg::LoadNote(note_name));
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

                if let Some(name) = note_name {
                    sender.input(AppMsg::LoadNote(name));
                    return;
                }

                // Si no está en set_data, intentar obtenerlo del label (lista normal)
                if let Some(child) = row.child() {
                    if let Ok(box_widget) = child.downcast::<gtk::Box>() {
                        // El label es el segundo hijo (después del icono)
                        if let Some(label_widget) =
                            box_widget.first_child().and_then(|w| w.next_sibling())
                        {
                            if let Ok(label) = label_widget.downcast::<gtk::Label>() {
                                let note_name = label.text().to_string();
                                sender.input(AppMsg::LoadNote(note_name));
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

                            if let Some(name) = note_name {
                                sender.input(AppMsg::LoadNote(name));
                                return;
                            }

                            // Si no está en set_data, obtener desde el label (lista normal)
                            if let Some(child) = row.child() {
                                if let Ok(box_widget) = child.downcast::<gtk::Box>() {
                                    // El label es el segundo hijo (después del icono)
                                    if let Some(label_widget) =
                                        box_widget.first_child().and_then(|w| w.next_sibling())
                                    {
                                        if let Ok(label) = label_widget.downcast::<gtk::Label>() {
                                            let note_name = label.text().to_string();
                                            sender.input(AppMsg::LoadNote(note_name));
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

        // Agregar manejador de Escape para el notes_list
        let text_view_for_list_escape = model.text_view.clone();
        let search_toggle_for_list = model.search_toggle_button.clone();
        let notes_list_for_keys = model.notes_list.clone();
        let list_key_controller = gtk::EventControllerKey::new();
        list_key_controller.connect_key_pressed(gtk::glib::clone!(
            #[strong]
            text_view_for_list_escape,
            #[strong]
            search_toggle_for_list,
            #[strong]
            notes_list_for_keys,
            move |_controller, keyval, _keycode, _modifiers| {
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

                match key_name.as_str() {
                    "Escape" => {
                        // Si el buscador está activo, cerrarlo
                        if search_toggle_for_list.is_active() {
                            search_toggle_for_list.set_active(false);
                        }
                        // Poner foco en el text_view con un pequeño delay
                        let text_view_clone = text_view_for_list_escape.clone();
                        gtk::glib::timeout_add_local_once(
                            std::time::Duration::from_millis(10),
                            move || {
                                text_view_clone.grab_focus();
                            },
                        );
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
                        if let Some(child) = row.child() {
                            if let Ok(box_widget) = child.downcast::<gtk::Box>() {
                                // Buscar el label (nota o carpeta)
                                let mut current_child = box_widget.first_child();

                                while let Some(widget) = current_child {
                                    let next = widget.next_sibling();

                                    if let Ok(label) = widget.clone().downcast::<gtk::Label>() {
                                        let item_name = label.text().to_string();
                                        let is_folder = label.has_css_class("heading");
                                        sender.input(AppMsg::ShowContextMenu(
                                            x, y, item_name, is_folder,
                                        ));
                                        break;
                                    }
                                    current_child = next;
                                }
                            }
                        }
                    }
                }))
                .map_err(|e| eprintln!("Panic capturado en right_click: {:?}", e));
            }
        ));
        widgets.notes_list.add_controller(right_click);

        // Agregar hover para cargar notas al pasar el ratón
        let motion_controller = gtk::EventControllerMotion::new();
        let is_populating_clone = model.is_populating_list.clone();
        motion_controller.connect_motion(gtk::glib::clone!(
            #[strong(rename_to = notes_list)]
            widgets.notes_list,
            #[strong]
            sender,
            move |_controller, _x, y| {
                // No cargar notas si se está repoblando la lista
                if *is_populating_clone.borrow() {
                    return;
                }

                // Obtener la fila bajo el cursor
                if let Some(row) = notes_list.row_at_y(y as i32) {
                    if row.is_selectable() {
                        // Seleccionar la fila visualmente
                        notes_list.select_row(Some(&row));

                        // Cargar la nota
                        if let Some(child) = row.child() {
                            if let Ok(box_widget) = child.downcast::<gtk::Box>() {
                                if let Some(label_widget) =
                                    box_widget.first_child().and_then(|w| w.next_sibling())
                                {
                                    if let Ok(label) = label_widget.downcast::<gtk::Label>() {
                                        let note_name = label.text().to_string();
                                        sender.input(AppMsg::LoadNote(note_name));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        ));
        widgets.notes_list.add_controller(motion_controller);

        // Agregar control de teclado al ListBox para navegación con j/k
        let notes_key_controller = gtk::EventControllerKey::new();
        notes_key_controller.connect_key_pressed(gtk::glib::clone!(
            #[strong(rename_to = notes_list)]
            widgets.notes_list,
            #[strong]
            sender,
            move |_controller, keyval, _keycode, _modifiers| {
                let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

                match key_name.as_str() {
                    "j" | "Down" => {
                        // Mover a la siguiente nota
                        if let Some(selected_row) = notes_list.selected_row() {
                            let index = selected_row.index();
                            if let Some(next_row) = notes_list.row_at_index(index + 1) {
                                notes_list.select_row(Some(&next_row));
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
        widgets.text_view.grab_focus();

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
                self.sidebar_visible = !self.sidebar_visible;
                let target_position = if self.sidebar_visible { 250 } else { 0 };
                self.animate_sidebar(target_position);

                // Si estamos cerrando el sidebar, devolver foco al editor
                if !self.sidebar_visible {
                    let text_view = self.text_view.clone();
                    gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(160),
                        move || {
                            text_view.grab_focus();
                        },
                    );
                }
            }
            AppMsg::OpenSidebarAndFocus => {
                // Abrir sidebar si está cerrado
                if !self.sidebar_visible {
                    self.sidebar_visible = true;
                    self.animate_sidebar(250);
                }

                // Dar foco al ListBox después de un pequeño delay para que termine la animación
                let notes_list = self.notes_list.clone();
                gtk::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(160),
                    move || {
                        notes_list.grab_focus();
                        // Seleccionar el primer elemento si no hay nada seleccionado
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
                            sender.input(AppMsg::CompleteTag(first_match.name.clone()));
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

                let action = match current_mode {
                    EditorMode::Normal => self.command_parser.parse_normal_mode(&key, modifiers),
                    EditorMode::Insert => self.command_parser.parse_insert_mode(&key, modifiers),
                    EditorMode::Command => {
                        // En modo comando, acumular input hasta Enter
                        // Por ahora, simplificamos
                        EditorAction::None
                    }
                    EditorMode::Visual => EditorAction::None,
                };

                if action != EditorAction::None {
                    sender.input(AppMsg::ProcessAction(action));
                }
            }
            AppMsg::ProcessAction(action) => {
                self.execute_action(action, &sender);
            }
            AppMsg::SaveCurrentNote => {
                self.save_current_note();
            }
            AppMsg::AutoSave => {
                // Solo guardar si hay cambios sin guardar
                if self.has_unsaved_changes {
                    self.save_current_note();
                    println!("Autoguardado ejecutado");
                }
            }
            AppMsg::LoadNote(name) => {
                if let Err(e) = self.load_note(&name) {
                    eprintln!("Error cargando nota '{}': {}", name, e);
                } else {
                    // Sincronizar vista y actualizar UI
                    self.sync_to_view();
                    self.update_status_bar(&sender);
                    self.refresh_tags_display_with_sender(&sender);
                    self.refresh_todos_summary();
                    self.window_title.set_label(&name);
                    self.has_unsaved_changes = false;
                }
            }
            AppMsg::CreateNewNote(name) => {
                if let Err(e) = self.create_new_note(&name) {
                    eprintln!("Error creando nota '{}': {}", name, e);
                } else {
                    // Sincronizar vista y actualizar UI
                    self.sync_to_view();
                    self.update_status_bar(&sender);
                    self.refresh_tags_display_with_sender(&sender);
                    self.refresh_todos_summary();
                    self.window_title.set_label(&name);

                    // Refrescar lista de notas en el sidebar
                    self.populate_notes_list(&sender);
                    *self.is_populating_list.borrow_mut() = false;

                    // Cambiar a modo Insert para empezar a escribir
                    *self.mode.borrow_mut() = EditorMode::Insert;
                }
            }
            AppMsg::UpdateCursorPosition(pos) => {
                // Actualizar la posición del cursor cuando el usuario hace clic
                self.cursor_position = pos;
            }
            AppMsg::ShowCreateNoteDialog => {
                self.show_create_note_dialog(&sender);
            }

            AppMsg::ToggleFolder(folder_name) => {
                // Activar flag durante la repoblación
                *self.is_populating_list.borrow_mut() = true;

                // Toggle el estado de la carpeta
                let was_expanded = self.expanded_folders.contains(&folder_name);
                if was_expanded {
                    self.expanded_folders.remove(&folder_name);
                } else {
                    self.expanded_folders.insert(folder_name.clone());
                }

                // Refrescar la lista para mostrar/ocultar las notas
                self.populate_notes_list(&sender);

                // Re-seleccionar la carpeta después de refrescar con un delay mayor
                let notes_list = self.notes_list.clone();
                let folder_name_clone = folder_name.clone();
                let is_populating_clone = self.is_populating_list.clone();
                gtk::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(50),
                    move || {
                        // Primero deseleccionar todo
                        notes_list.select_row(gtk::ListBoxRow::NONE);

                        // Buscar la carpeta en la lista y seleccionarla
                        let mut child = notes_list.first_child();
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
                                            notes_list.select_row(Some(&row));
                                            break;
                                        }
                                    }
                                }
                            }
                            child = widget.next_sibling();
                        }

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
                        // Eliminar carpeta y todo su contenido
                        if let Err(e) = std::fs::remove_dir_all(&folder_path) {
                            eprintln!("Error al eliminar carpeta: {}", e);
                        } else {
                            println!("Carpeta eliminada: {}", item_name);

                            // Eliminar todas las notas de la carpeta del índice
                            if let Ok(notes) = self.notes_dir.list_notes() {
                                for note in notes {
                                    if let Some(relative_path) =
                                        note.path().strip_prefix(self.notes_dir.root()).ok()
                                    {
                                        if relative_path.starts_with(&item_name) {
                                            let _ = self.notes_db.delete_note(note.name());
                                        }
                                    }
                                }
                            }

                            // Si la nota actual estaba en esta carpeta, limpiar el editor
                            if let Some(current) = &self.current_note {
                                if current.name().starts_with(&format!("{}/", item_name)) {
                                    self.current_note = None;
                                    self.buffer = NoteBuffer::new();
                                    self.sync_to_view();
                                    self.window_title.set_label("NotNative");
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
                        if let Err(e) = std::fs::remove_file(note.path()) {
                            eprintln!("Error al eliminar nota: {}", e);
                        } else {
                            // Eliminar de la base de datos
                            if let Err(e) = self.notes_db.delete_note(&item_name) {
                                eprintln!("Error al eliminar nota del índice: {}", e);
                            } else {
                                println!("Nota eliminada del índice");
                            }

                            // Si era la nota actual, limpiar el editor
                            if let Some(current) = &self.current_note {
                                if current.name() == item_name {
                                    self.current_note = None;
                                    self.buffer = NoteBuffer::new();
                                    self.sync_to_view();
                                    self.window_title.set_label("NotNative");
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

            AppMsg::RefreshSidebar => {
                self.populate_notes_list(&sender);
                *self.is_populating_list.borrow_mut() = false;
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

            AppMsg::ToggleSearch(active) => {
                self.search_active = active;
                self.search_bar.set_visible(active);
                self.search_toggle_button.set_active(active);

                if active {
                    self.search_entry.grab_focus();
                } else {
                    // Limpiar búsqueda y volver a mostrar todas las notas
                    self.search_entry.set_text("");
                    self.populate_notes_list(&sender);
                }
            }

            AppMsg::SearchNotes(query) => {
                if query.trim().is_empty() {
                    // Si el query está vacío, mostrar todas las notas
                    self.populate_notes_list(&sender);
                } else {
                    // Asegurarse de que el search bar esté visible y actualizar el texto
                    if !self.search_active {
                        self.search_active = true;
                        self.search_bar.set_visible(true);
                        self.search_toggle_button.set_active(true);
                    }

                    // Actualizar el texto del search entry
                    self.search_entry.set_text(&query);

                    // Realizar búsqueda
                    self.perform_search(&query, &sender);
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

            AppMsg::ChangeLanguage(new_language) => {
                // Actualizar idioma en I18n
                self.i18n.borrow_mut().set_language(new_language);

                // Guardar preferencia en configuración
                self.notes_config
                    .set_language(Some(new_language.code().to_string()));
                if let Err(e) = self.notes_config.save(NotesConfig::default_path()) {
                    eprintln!("Error guardando configuración de idioma: {}", e);
                }

                println!("Idioma cambiado a: {:?}", new_language);

                // Actualizar todos los textos de la UI
                self.update_ui_language(&sender);
            }

            AppMsg::ReloadConfig => {
                // Recargar configuración desde disco
                if let Ok(config) = NotesConfig::load(NotesConfig::default_path()) {
                    self.notes_config = config;
                    println!("Configuración recargada desde disco");
                } else {
                    eprintln!("Error recargando configuración");
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
                self.save_current_note();

                // Actualizar resumen de TODOs
                self.refresh_todos_summary();

                // Actualizar barra de estado
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
        }
    }
}

impl MainApp {
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

        let mut combined_css = String::new();
        let mut theme_loaded = false;

        for css_file in &css_files {
            if let Ok(content) = std::fs::read_to_string(css_file) {
                combined_css.push_str(&content);
                combined_css.push('\n');
                theme_loaded = true;
            }
        }

        // Cargar el CSS de la aplicación
        // Prioridad: 1) Sistema instalado, 2) Desarrollo local
        let app_css = std::fs::read_to_string("/usr/share/notnative/assets/style.css")
            .ok()
            .or_else(|| {
                // Rutas de desarrollo
                if let Ok(exe_path) = std::env::current_exe() {
                    exe_path
                        .parent()
                        .and_then(|p| p.parent())
                        .and_then(|p| p.parent())
                        .map(|p| p.join("assets/style.css"))
                        .and_then(|path| std::fs::read_to_string(&path).ok())
                } else {
                    None
                }
            })
            .or_else(|| std::fs::read_to_string("assets/style.css").ok())
            .or_else(|| std::fs::read_to_string("./notnative-app/assets/style.css").ok());

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
                *self.mode.borrow_mut() = new_mode;
                println!("Cambiado a modo: {:?}", new_mode);
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
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            EditorAction::MoveCursorRight => {
                if self.cursor_position < self.buffer.len_chars() {
                    self.cursor_position += 1;
                }
            }
            EditorAction::MoveCursorUp => {
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
            EditorAction::MoveCursorDown => {
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

    fn sync_to_view(&self) {
        // Activar flag para evitar que connect_changed nos sincronice de vuelta
        *self.is_syncing_to_gtk.borrow_mut() = true;
        println!("sync_to_view activado. Flag is_syncing_to_gtk = true");
        let sync_flag = self.is_syncing_to_gtk.clone();
        gtk::glib::idle_add_local_once(move || {
            println!("sync_to_view completado. Reiniciando flag is_syncing_to_gtk");
            *sync_flag.borrow_mut() = false;
        });

        let buffer_text = self.buffer.to_string();
        let current_mode = *self.mode.borrow();

        // En modo Normal, mostrar texto limpio (sin símbolos markdown)
        // En modo Insert, mostrar texto crudo con todos los símbolos
        let display_text = if current_mode == EditorMode::Normal && self.markdown_enabled {
            self.render_clean_markdown(&buffer_text)
        } else {
            buffer_text.clone()
        };

        // Solo actualizar si el texto es diferente
        let current_text = self
            .text_buffer
            .text(
                &self.text_buffer.start_iter(),
                &self.text_buffer.end_iter(),
                false,
            )
            .to_string();

        if current_text != display_text {
            // Calcular posición del cursor en el texto mostrado
            let cursor_offset = if current_mode == EditorMode::Normal && self.markdown_enabled {
                // Mapear posición del buffer original al texto limpio
                self.map_buffer_pos_to_display(&buffer_text, self.cursor_position)
            } else {
                self.cursor_position.min(self.buffer.len_chars())
            };

            // Bloquear señales GTK durante la actualización
            self.text_buffer.begin_user_action();

            // Reemplazar todo el contenido
            let start_iter = self.text_buffer.start_iter();
            let end_iter = self.text_buffer.end_iter();
            self.text_buffer
                .delete(&mut start_iter.clone(), &mut end_iter.clone());
            self.text_buffer
                .insert(&mut self.text_buffer.start_iter(), &display_text);

            // Restaurar cursor ANTES de terminar la acción de usuario
            let mut iter = self.text_buffer.start_iter();
            iter.set_offset(cursor_offset as i32);
            self.text_buffer.place_cursor(&iter);

            self.text_buffer.end_user_action();

            // Aplicar estilos markdown DESPUÉS de terminar la acción de usuario
            // Solo en modo Normal (en Insert mode no aplicamos estilos)
            if current_mode == EditorMode::Normal && self.markdown_enabled {
                self.apply_markdown_styles_to_clean_text(&display_text);
            }
        } else {
            // Aunque el texto no cambió, actualizar la posición del cursor
            let cursor_offset = if current_mode == EditorMode::Normal && self.markdown_enabled {
                // Mapear posición del buffer original al texto limpio
                self.map_buffer_pos_to_display(&buffer_text, self.cursor_position)
            } else {
                self.cursor_position.min(self.buffer.len_chars())
            };
            let mut iter = self.text_buffer.start_iter();
            iter.set_offset(cursor_offset as i32);
            self.text_buffer.place_cursor(&iter);
        }
    }

    /// Renderiza el texto markdown sin los símbolos de formato
    fn render_clean_markdown(&self, text: &str) -> String {
        println!(
            "DEBUG render_clean_markdown: Entrada: {:?}",
            text.lines().take(3).collect::<Vec<_>>()
        );
        let mut result = String::new();
        let mut chars = text.chars().peekable();
        let mut in_code_block = false;
        let mut at_line_start = true; // Flag para saber si estamos al inicio de una línea

        while let Some(ch) = chars.next() {
            match ch {
                // Code blocks: ```
                '`' if chars.peek() == Some(&'`') => {
                    let mut backtick_count = 1;
                    while chars.peek() == Some(&'`') {
                        chars.next();
                        backtick_count += 1;
                    }

                    if backtick_count >= 3 {
                        // Toggle code block (``` o más)
                        in_code_block = !in_code_block;

                        // Consumir toda la línea incluyendo el \n
                        // Esta línea NO debe aparecer en el texto limpio
                        while let Some(&next_ch) = chars.peek() {
                            chars.next();
                            if next_ch == '\n' {
                                at_line_start = true; // Después del \n estamos al inicio de línea
                                break; // Consumir el \n y salir
                            }
                        }

                        continue;
                    } else if backtick_count == 1 {
                        // Código inline - no agregar el backtick
                        at_line_start = false;
                        continue;
                    }
                }

                // Encabezados: remover # (solo si no estamos en code block)
                '#' if !in_code_block && at_line_start => {
                    // Contar cuántos # hay
                    let mut hash_count = 1;
                    while chars.peek() == Some(&'#') {
                        chars.next();
                        hash_count += 1;
                    }
                    // Saltar espacio después de #
                    if chars.peek() == Some(&' ') {
                        chars.next();
                    }
                    at_line_start = false; // Ya no estamos al inicio de línea
                }

                // Listas y TODOs: detectar - [ ] o - [x] para TODOs, o - para bullets normales
                '-' if !in_code_block && at_line_start => {
                    // Colectar los próximos caracteres para analizar el patrón
                    let mut lookahead = Vec::new();
                    let mut temp_chars = chars.clone();

                    // Leer hasta 6 caracteres adelante (suficiente para "- [ ] ")
                    for _ in 0..6 {
                        if let Some(c) = temp_chars.next() {
                            lookahead.push(c);
                        } else {
                            break;
                        }
                    }

                    println!(
                        "DEBUG: Detectado '-' al inicio de línea. Posición en result: {}. at_line_start: {}",
                        result.len(),
                        at_line_start
                    );

                    // Verificar si es un TODO
                    if lookahead.len() >= 5
                        && lookahead[0] == ' '
                        && lookahead[1] == '['
                        && lookahead[2] == ' '
                        && lookahead[3] == ']'
                        && lookahead[4] == ' '
                    {
                        // Es un TODO sin marcar: "- [ ] "
                        println!(
                            "DEBUG: TODO sin marcar detectado, lookahead: {:?}",
                            lookahead
                        );
                        for _ in 0..5 {
                            chars.next();
                        }
                        result.push_str("[TODO:unchecked] ");
                        at_line_start = false; // Ya no estamos al inicio de línea
                    } else if lookahead.len() >= 5
                        && lookahead[0] == ' '
                        && lookahead[1] == '['
                        && (lookahead[2] == 'x' || lookahead[2] == 'X')
                        && lookahead[3] == ']'
                        && lookahead[4] == ' '
                    {
                        // Es un TODO marcado: "- [x] " o "- [X] "
                        println!("DEBUG: TODO marcado detectado, lookahead: {:?}", lookahead);
                        for _ in 0..5 {
                            chars.next();
                        }
                        result.push_str("[TODO:checked] ");
                        at_line_start = false; // Ya no estamos al inicio de línea
                    } else if lookahead.len() >= 1 && lookahead[0] == ' ' {
                        // Lista normal con bullet: "- "
                        chars.next(); // Consumir el espacio
                        result.push('•');
                        result.push(' ');
                        at_line_start = false; // Ya no estamos al inicio de línea
                    } else {
                        // No es ni lista ni TODO, es solo un guión
                        println!(
                            "DEBUG: No se detectó TODO ni lista. Lookahead: {:?}",
                            lookahead
                        );
                        result.push(ch);
                        at_line_start = false; // Ya no estamos al inicio de línea
                    }
                }

                // Blockquotes: remover >
                '>' if !in_code_block && at_line_start => {
                    if chars.peek() == Some(&' ') {
                        chars.next(); // Saltar el espacio
                    }
                    at_line_start = false; // Ya no estamos al inicio de línea
                }

                // Links e Imágenes: [texto](url) o ![alt](url)
                '!' if !in_code_block && chars.peek() == Some(&'[') => {
                    // Es una imagen ![alt](url)
                    chars.next(); // Consumir [

                    // Extraer alt text (lo ignoramos)
                    while let Some(&next_ch) = chars.peek() {
                        chars.next();
                        if next_ch == ']' {
                            break;
                        }
                    }

                    // Verificar si hay (url)
                    if chars.peek() == Some(&'(') {
                        chars.next(); // Consumir (

                        // Extraer la URL de la imagen
                        let mut img_src = String::new();
                        while let Some(&next_ch) = chars.peek() {
                            chars.next();
                            if next_ch == ')' {
                                break;
                            }
                            img_src.push(next_ch);
                        }

                        // Insertar marcador especial con la ruta
                        let marker = format!("[IMG:{}]", img_src);
                        println!(
                            "DEBUG render_clean_markdown: Insertando marcador: {}",
                            marker
                        );
                        result.push_str(&marker);
                    } else {
                        // No era una imagen válida
                        result.push_str("![");
                    }
                }

                // Links: [texto](url) -> mostrar solo texto (o marcador de video si es YouTube)
                '[' if !in_code_block => {
                    let mut link_text = String::new();
                    let mut found_close = false;

                    // Extraer texto del link
                    while let Some(&next_ch) = chars.peek() {
                        chars.next();
                        if next_ch == ']' {
                            found_close = true;
                            break;
                        }
                        link_text.push(next_ch);
                    }

                    // Si encontramos ](, extraer y analizar la URL
                    if found_close && chars.peek() == Some(&'(') {
                        chars.next(); // Consumir (
                        let mut url = String::new();
                        while let Some(&next_ch) = chars.peek() {
                            chars.next();
                            if next_ch == ')' {
                                break;
                            }
                            url.push(next_ch);
                        }

                        // Verificar si es un enlace de YouTube
                        if let Some(video_id) = Self::extract_youtube_video_id(&url) {
                            // Insertar marcador especial para videos de YouTube
                            let marker = format!("[VIDEO:{}]", video_id);
                            result.push_str(&marker);
                        } else {
                            // Link normal, mostrar solo el texto
                            result.push_str(&link_text);
                        }
                    } else {
                        // No era un link válido, restaurar [
                        result.push('[');
                        result.push_str(&link_text);
                        if found_close {
                            result.push(']');
                        }
                    }
                }

                // Negrita: remover **
                '*' if !in_code_block && chars.peek() == Some(&'*') => {
                    chars.next(); // Consumir el segundo *
                }

                // Cursiva: remover * (solo si no es parte de **)
                '*' if !in_code_block => {
                    // Omitir el *
                }

                // Código inline: remover `
                '`' if !in_code_block => {
                    // Omitir el `
                    at_line_start = false;
                }

                // Salto de línea: resetear flag de inicio de línea
                '\n' => {
                    result.push(ch);
                    at_line_start = true; // Ahora estamos al inicio de la siguiente línea
                }

                // Todo lo demás: mantener
                _ => {
                    result.push(ch);
                    at_line_start = false; // Ya no estamos al inicio de línea
                }
            }
        }

        println!(
            "DEBUG render_clean_markdown: Salida: {:?}",
            result.lines().take(3).collect::<Vec<_>>()
        );
        result
    }

    /// Mapea una posición del buffer original al texto limpio (sin símbolos markdown)
    fn map_buffer_pos_to_display(&self, original_text: &str, buffer_pos: usize) -> usize {
        let mut display_pos = 0;
        let mut original_pos = 0;
        let mut chars = original_text.chars().peekable();

        while original_pos < buffer_pos && chars.peek().is_some() {
            let ch = chars.next().unwrap();
            original_pos += 1;

            match ch {
                // Encabezados: saltar #
                '#' if display_pos == 0 || original_text[..original_pos - 1].ends_with('\n') => {
                    // Contar cuántos # hay
                    while chars.peek() == Some(&'#') && original_pos < buffer_pos {
                        chars.next();
                        original_pos += 1;
                    }
                    // Saltar espacio después de #
                    if chars.peek() == Some(&' ') && original_pos < buffer_pos {
                        chars.next();
                        original_pos += 1;
                    }
                }
                // Negrita: saltar **
                '*' if chars.peek() == Some(&'*') => {
                    if original_pos < buffer_pos {
                        chars.next();
                        original_pos += 1;
                    }
                }
                // Cursiva o código: saltar * o `
                '*' | '`' => {
                    // No incrementar display_pos
                }
                // Todo lo demás: mantener
                _ => {
                    display_pos += 1;
                }
            }
        }

        display_pos.min(self.render_clean_markdown(original_text).chars().count())
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
        self.todo_widgets.borrow_mut().clear();

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
                Some("h3")
            } else if original_line.starts_with("## ") {
                Some("h2")
            } else if original_line.starts_with("# ") {
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
        }

        self.process_all_todos_in_buffer();
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
        for (start, end, img_path) in images.into_iter().rev() {
            // Crear iteradores usando offsets de caracteres desde el inicio del buffer
            let mut marker_start = self.text_buffer.start_iter();
            marker_start.set_offset(start as i32);

            let mut marker_end = self.text_buffer.start_iter();
            marker_end.set_offset(end as i32);
            let marker_text = self.text_buffer.text(&marker_start, &marker_end, false);
            println!("DEBUG todos: marker_text='{}'", marker_text);

            // Eliminar el marcador del buffer
            self.text_buffer
                .delete(&mut marker_start.clone(), &mut marker_end.clone());

            // Recrear el iterador de inicio después del delete
            let mut anchor_pos = self.text_buffer.start_iter();
            anchor_pos.set_offset(start as i32);

            // Insertar salto de línea antes de la imagen para separación
            self.text_buffer.insert(&mut anchor_pos, "\n");

            // Actualizar posición después de la inserción
            anchor_pos.set_offset(start as i32 + 1);

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

            // Insertar salto de línea después de la imagen para separación
            let mut after_anchor = self.text_buffer.start_iter();
            after_anchor.set_offset(start as i32 + 1);
            self.text_buffer.insert(&mut after_anchor, "\n");

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

                videos.push((absolute_start, absolute_end + 1, video_id)); // +1 para incluir ]
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

            // Insertar salto de línea
            let mut anchor_pos = self.text_buffer.start_iter();
            anchor_pos.set_offset(start as i32);
            self.text_buffer.insert(&mut anchor_pos, "\n");

            // Actualizar posición y crear anchor
            anchor_pos.set_offset(start as i32 + 1);
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

            // Insertar salto de línea después
            let mut after_anchor = self.text_buffer.start_iter();
            after_anchor.set_offset(start as i32 + 1);
            self.text_buffer.insert(&mut after_anchor, "\n");

            // Guardar referencia
            self.video_widgets.borrow_mut().push(video_container);
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
        println!(
            "DEBUG todos: buffer_text first lines: {:?}",
            buffer_text.lines().take(5).collect::<Vec<_>>()
        );

        // Obtener el texto original del buffer interno para encontrar las posiciones de TODOs
        let original_text = self.buffer.to_string();
        println!(
            "DEBUG todos: original_text first lines: {:?}",
            original_text.lines().take(5).collect::<Vec<_>>()
        );

        // Encontrar todas las posiciones de TODOs en el texto ORIGINAL (no renderizado)
        let original_todo_positions = find_all_todos_in_text(&original_text);
        println!(
            "DEBUG todos: {} posiciones originales",
            original_todo_positions.len()
        );

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
        println!("DEBUG todos: {} marcadores en display", todos.len());

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
            println!(
                "DEBUG todos: eliminado marcador en {}..{} checked={} (todo_index={})",
                start, end, is_checked, todo_index
            );

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

        let start = self.text_buffer.start_iter();
        let end = self.text_buffer.end_iter();
        let remaining = self.text_buffer.text(&start, &end, false);
        println!(
            "DEBUG todos: texto tras procesar -> {:?}",
            remaining.lines().take(5).collect::<Vec<_>>()
        );
    }

    /// Aplica estilos inline dentro de una línea (negrita, cursiva, código, links, tags)
    fn apply_inline_styles(
        &self,
        clean_line: &str,
        original_line: &str,
        line_start: &gtk::TextIter,
        line_offset: i32,
    ) {
        // Primero detectar tags inline en el texto limpio
        self.detect_inline_tags(clean_line, line_offset);

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
            let folder_info = note
                .path()
                .strip_prefix(self.notes_dir.root())
                .ok()
                .and_then(|p| p.parent())
                .and_then(|p| p.to_str())
                .filter(|s| !s.is_empty())
                .map(|folder| format!("{} / ", folder))
                .unwrap_or_default();

            format!("{}{}{}", modified_marker, folder_info, note.name())
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
                // Abrir sidebar y activar búsqueda
                sender.input(AppMsg::OpenSidebarAndFocus);
                sender.input(AppMsg::ToggleSearch(true));
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

        // Progreso y porcentaje en una sola línea
        let progress_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        progress_box.set_margin_top(2);

        let progress_label = gtk::Label::new(Some(&format!(
            "{}/{} {}",
            section.completed,
            section.total,
            i18n.t("completed")
        )));
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

        row_box
    }

    fn analyze_todos_by_section(&self, text: &str) -> Vec<TodoSection> {
        let lines: Vec<&str> = text.lines().collect();
        let mut sections = Vec::new();
        let i18n = self.i18n.borrow();
        let mut current_section = i18n.t("no_section");
        let mut current_todos: Vec<bool> = Vec::new(); // true = completado, false = pendiente

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

            // Detectar TODOs
            let trimmed = line.trim_start();
            if trimmed.starts_with("- [ ]") {
                current_todos.push(false);
            } else if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
                current_todos.push(true);
            }
        }

        // Agregar última sección si tiene TODOs
        if !current_todos.is_empty() {
            sections.push(self.create_todo_section(&current_section, &current_todos));
        }

        sections
    }

    fn create_todo_section(&self, title: &str, todos: &[bool]) -> TodoSection {
        let total = todos.len();
        let completed = todos.iter().filter(|&&done| done).count();
        let percentage = if total > 0 {
            (completed * 100) / total
        } else {
            0
        };

        TodoSection {
            title: title.to_string(),
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

                // Hacer clickeable
                let button = gtk::Button::new();
                button.set_child(Some(&row));
                button.add_css_class("flat");

                let tag_name = tag.name.clone();
                button.connect_clicked(gtk::glib::clone!(
                    #[strong]
                    sender,
                    move |_| {
                        sender.input(AppMsg::CompleteTag(tag_name.clone()));
                    }
                ));

                self.tag_completion_list.append(&button);
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
    fn save_current_note(&mut self) {
        if let Some(note) = &self.current_note {
            // Obtener contenido anterior y nuevo
            let old_content = note.read().unwrap_or_default();
            let new_content = self.buffer.to_string();

            if let Err(e) = note.write(&new_content) {
                eprintln!("Error guardando nota: {}", e);
            } else {
                println!("Nota guardada: {}", note.name());
                self.has_unsaved_changes = false;

                // Limpiar imágenes no referenciadas
                self.cleanup_unused_images(&old_content, &new_content);

                // Actualizar índice en base de datos
                if let Err(e) = self.notes_db.update_note(note.name(), &new_content) {
                    eprintln!("Error actualizando índice: {}", e);
                } else {
                    println!("Índice actualizado");

                    // Actualizar tags
                    if let Ok(Some(note_meta)) = self.notes_db.get_note(note.name()) {
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

        println!("Nota cargada: {}", name);
        Ok(())
    }

    /// Crea una nueva nota
    fn create_new_note(&mut self, name: &str) -> anyhow::Result<()> {
        // Contenido inicial vacío para nueva nota
        let initial_content = format!("# {}\n\n", name.split('/').last().unwrap_or(name));

        let note = if name.contains('/') {
            // Si contiene '/', separar carpeta y nombre
            let parts: Vec<&str> = name.rsplitn(2, '/').collect();
            let note_name = parts[0];
            let folder = parts[1];
            self.notes_dir
                .create_note_in_folder(folder, note_name, &initial_content)?
        } else {
            // Crear en la raíz
            self.notes_dir.create_note(name, &initial_content)?
        };

        // Indexar en base de datos
        let folder = self.notes_dir.relative_folder(note.path());
        if let Err(e) = self.notes_db.index_note(
            note.name(),
            note.path().to_str().unwrap_or(""),
            &initial_content,
            folder.as_deref(),
        ) {
            eprintln!("Error indexando nueva nota: {}", e);
        } else {
            println!("Nueva nota indexada: {}", name);
        }

        // Cargar la nueva nota en el buffer
        self.buffer = NoteBuffer::from_text(&initial_content);
        self.cursor_position = initial_content.len();
        self.current_note = Some(note.clone());
        self.has_unsaved_changes = false;

        println!("Nueva nota creada: {}", name);
        Ok(())
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

        // Para notas, obtener también la carpeta actual desde la base de datos
        let target_folder = if !is_folder {
            // Buscar la carpeta de esta nota en la base de datos
            self.notes_db
                .get_note(&item_name)
                .ok()
                .flatten()
                .and_then(|note_meta| note_meta.folder)
        } else {
            None
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
                            // Arrastrar carpeta sobre carpeta -> mover carpeta
                            sender_clone.input(AppMsg::MoveFolder {
                                folder_name: drag_name.to_string(),
                                target_folder: Some(target_item_name.clone()),
                            });
                            return true;
                        }
                        ("folder", false) => {
                            // Arrastrar carpeta sobre nota -> mover carpeta a la misma carpeta que la nota
                            sender_clone.input(AppMsg::MoveFolder {
                                folder_name: drag_name.to_string(),
                                target_folder: target_folder_path.clone(),
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

        // Deseleccionar cualquier fila actual
        self.notes_list.select_row(gtk::ListBoxRow::NONE);

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

            for note_meta in existing_notes {
                let folder = note_meta.folder.as_deref().unwrap_or("/").to_string();
                by_folder
                    .entry(folder)
                    .or_insert_with(Vec::new)
                    .push(note_meta.name);
            }

            // Ordenar solo las carpetas, no las notas (las notas ya vienen ordenadas por order_index)
            let mut folders: Vec<_> = by_folder.keys().cloned().collect();
            folders.sort();

            for folder in folders {
                if let Some(notes_in_folder) = by_folder.get(&folder) {
                    // Saltar carpetas vacías
                    if notes_in_folder.is_empty() {
                        continue;
                    }

                    // Si no es la raíz, mostrar carpeta como encabezado expandible
                    if folder != "/" {
                        // Verificar que la carpeta existe en el filesystem
                        let folder_path = self.notes_dir.root().join(&folder);
                        if !folder_path.exists() || !folder_path.is_dir() {
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

                        let folder_icon = gtk::Image::builder()
                            .icon_name("folder-symbolic")
                            .pixel_size(16)
                            .build();

                        // Obtener solo el nombre de la carpeta (última parte del path)
                        let folder_display_name = folder.split('/').last().unwrap_or(&folder);

                        // Calcular nivel de indentación (número de '/' en el path)
                        let depth = folder.matches('/').count();
                        let indent = 8 + (depth * 16);

                        folder_row.set_margin_start(indent as i32);

                        let folder_label = gtk::Label::builder()
                            .label(folder_display_name)
                            .xalign(0.0)
                            .hexpand(true)
                            .ellipsize(gtk::pango::EllipsizeMode::End)
                            .max_width_chars(30)
                            .build();

                        folder_label.add_css_class("heading");

                        folder_row.append(&arrow);
                        folder_row.append(&folder_icon);
                        folder_row.append(&folder_label);

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

                        let icon = gtk::Image::builder()
                            .icon_name("text-x-generic-symbolic")
                            .pixel_size(14)
                            .build();

                        row.append(&icon);

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

                                        if let Err(e) = std::fs::rename(old_path, &new_path) {
                                            eprintln!("Error al renombrar: {}", e);
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
                            let label = gtk::Label::builder()
                                .label(&note_name_owned)
                                .xalign(0.0)
                                .hexpand(true)
                                .ellipsize(gtk::pango::EllipsizeMode::End)
                                .max_width_chars(40)
                                .build();

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

        // NO desactivar flag aquí - se hace con timeout en ToggleFolder
        // o manualmente en otros contextos
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

        // Realizar búsqueda en la base de datos
        match self.notes_db.search_notes(query) {
            Ok(results) => {
                if results.is_empty() {
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
                    // Mostrar resultados
                    for result in results {
                        let result_box = gtk::Box::builder()
                            .orientation(gtk::Orientation::Vertical)
                            .spacing(4)
                            .margin_start(12)
                            .margin_end(12)
                            .margin_top(8)
                            .margin_bottom(8)
                            .build();

                        // Nombre de la nota
                        let name_label = gtk::Label::builder()
                            .label(&result.note_name)
                            .xalign(0.0)
                            .css_classes(vec!["heading"])
                            .build();

                        // Snippet con contexto
                        let snippet_label = gtk::Label::builder()
                            .label(&result.snippet)
                            .xalign(0.0)
                            .wrap(true)
                            .wrap_mode(gtk::pango::WrapMode::Word)
                            .max_width_chars(40)
                            .css_classes(vec!["dim-label"])
                            .build();

                        result_box.append(&name_label);
                        result_box.append(&snippet_label);

                        let row = gtk::ListBoxRow::builder()
                            .selectable(true)
                            .activatable(true)
                            .child(&result_box)
                            .build();

                        // Guardar el nombre de la nota en el row para poder cargarlo
                        unsafe {
                            row.set_data("note_name", result.note_name.clone());
                        }

                        self.notes_list.append(&row);

                        // Re-seleccionar la nota actual si está en los resultados
                        if let Some(ref current_name) = current_note_name {
                            if &result.note_name == current_name {
                                self.notes_list.select_row(Some(&row));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error al buscar notas: {}", e);
                // Mostrar mensaje de error
                let error_label = gtk::Label::builder()
                    .label(&format!("Error al buscar: {}", e))
                    .xalign(0.5)
                    .margin_top(24)
                    .margin_bottom(24)
                    .margin_start(24)
                    .margin_end(24)
                    .css_classes(vec!["error"])
                    .build();

                let row = gtk::ListBoxRow::builder()
                    .selectable(false)
                    .activatable(false)
                    .child(&error_label)
                    .build();

                self.notes_list.append(&row);
            }
        }

        *self.is_populating_list.borrow_mut() = false;
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

        let completion_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(vec!["navigation-sidebar"])
            .build();

        let scrolled = gtk::ScrolledWindow::builder()
            .child(&completion_list)
            .min_content_width(300)
            .max_content_height(200)
            .build();

        completion_popover.set_child(Some(&scrolled));

        // Obtener carpetas existentes escaneando el directorio
        let mut folders: Vec<String> = Vec::new();
        if let Ok(notes) = self.notes_dir.list_notes() {
            for note in notes {
                let note_path = note.path();
                if let Some(folder) = self.notes_dir.relative_folder(note_path) {
                    if !folders.contains(&folder) {
                        folders.push(folder);
                    }
                }
            }
        }
        folders.sort();

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

                        let row = gtk::ListBoxRow::builder().child(&label).build();

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
                            // Seleccionar primera fila si no hay ninguna seleccionada
                            if completion_list_clone.selected_row().is_none() {
                                if let Some(first) = completion_list_clone.first_child() {
                                    if let Ok(row) = first.downcast::<gtk::ListBoxRow>() {
                                        completion_list_clone.select_row(Some(&row));
                                    }
                                }
                            }
                            return gtk::glib::Propagation::Stop;
                        }
                        "Tab" => {
                            // Tab autocompleta con la primera sugerencia
                            let row = if let Some(selected) = completion_list_clone.selected_row() {
                                Some(selected)
                            } else {
                                // Si no hay nada seleccionado, usar la primera fila
                                completion_list_clone
                                    .first_child()
                                    .and_then(|child| child.downcast::<gtk::ListBoxRow>().ok())
                            };

                            if let Some(row) = row {
                                if let Some(folder) = unsafe {
                                    row.data::<String>("folder").map(|d| d.as_ref().clone())
                                } {
                                    entry_for_keys.set_text(&format!("{}/", folder));
                                    entry_for_keys.set_position(-1);
                                    completion_popover_clone.popdown();
                                    return gtk::glib::Propagation::Stop;
                                }
                            }
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
        } else {
            // Si no es una URL de YouTube ni imagen, insertar como texto normal
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
            .default_width(500)
            .default_height(450)
            .build();

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
            let current_sink = self.notes_config.get_audio_output_sink();
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

        dialog.set_child(Some(&content_box));

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
            .default_width(600)
            .default_height(500)
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

        // Lista de atajos (solo los que están implementados)
        let shortcuts = vec![
            (
                "General",
                vec![
                    ("Ctrl+S", "Guardar nota actual"),
                    ("Ctrl+F", "Abrir búsqueda de notas"),
                    ("Ctrl+B / t", "Abrir/cerrar sidebar (en modo Normal)"),
                    ("n", "Nueva nota (en modo Normal)"),
                ],
            ),
            (
                "Modos de edición (desde Normal)",
                vec![
                    ("i", "Entrar en modo Insert"),
                    (":", "Entrar en modo Command"),
                    ("v", "Entrar en modo Visual"),
                    ("Escape", "Volver a modo Normal (desde Insert)"),
                ],
            ),
            (
                "Navegación (modo Normal)",
                vec![
                    ("h / ←", "Izquierda"),
                    ("j / ↓", "Abajo"),
                    ("k / ↑", "Arriba"),
                    ("l / →", "Derecha"),
                    ("0", "Inicio de línea"),
                    ("$", "Fin de línea"),
                    ("gg", "Inicio del documento"),
                    ("G", "Fin del documento"),
                ],
            ),
            (
                "Navegación (modo Insert)",
                vec![("←/→/↑/↓", "Mover cursor")],
            ),
            (
                "Edición (modo Normal)",
                vec![
                    ("x", "Eliminar carácter bajo el cursor"),
                    ("dd", "Eliminar línea completa"),
                    ("u", "Deshacer"),
                    ("Ctrl+Z", "Deshacer"),
                    ("Ctrl+R", "Rehacer"),
                    ("Ctrl+C", "Copiar texto seleccionado"),
                    ("Ctrl+X", "Cortar texto seleccionado"),
                    ("Ctrl+V", "Pegar desde portapapeles"),
                ],
            ),
            (
                "Edición (modo Insert)",
                vec![
                    ("Backspace", "Eliminar carácter anterior"),
                    ("Delete", "Eliminar carácter siguiente"),
                    ("Enter", "Nueva línea"),
                    ("Tab", "Insertar tabulación"),
                    ("Ctrl+C", "Copiar texto seleccionado"),
                    ("Ctrl+X", "Cortar texto seleccionado"),
                    ("Ctrl+V", "Pegar desde portapapeles"),
                    ("Ctrl+Z", "Deshacer"),
                    ("Ctrl+R", "Rehacer"),
                ],
            ),
            (
                "Búsqueda y Sidebar",
                vec![
                    ("Ctrl+F", "Activar búsqueda"),
                    ("Escape", "Cerrar búsqueda / Volver al editor"),
                    ("↑/↓", "Navegar resultados (con foco en sidebar)"),
                    ("Enter", "Abrir nota / Expandir carpeta"),
                ],
            ),
        ];

        for (section, items) in shortcuts {
            let section_label = gtk::Label::builder()
                .label(section)
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
                    .build();
                shortcut_label.add_css_class("monospace");

                let desc_label = gtk::Label::builder()
                    .label(description)
                    .halign(gtk::Align::Start)
                    .hexpand(true)
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
fn find_all_todos_in_text(text: &str) -> Vec<usize> {
    let chars: Vec<char> = text.chars().collect();
    let mut positions = Vec::new();

    let mut pos = 0;
    while pos + 4 < chars.len() {
        // Buscar el patrón - [ ] o - [x]
        if chars[pos] == '-'
            && chars[pos + 1] == ' '
            && chars[pos + 2] == '['
            && (chars[pos + 3] == ' ' || chars[pos + 3] == 'x' || chars[pos + 3] == 'X')
            && chars[pos + 4] == ']'
        {
            positions.push(pos);
            pos += 5; // Saltar el TODO completo
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
    fn show_about_dialog(&self) {
        let i18n = self.i18n.borrow();

        let dialog = gtk::AboutDialog::builder()
            .transient_for(&self.main_window)
            .modal(true)
            .program_name("NotNative")
            .version("0.1.0")
            .comments(&i18n.t("app_description"))
            .website("https://github.com/k4ditano/notnative-app")
            .website_label(&i18n.t("website"))
            .license_type(gtk::License::MitX11)
            .authors(vec!["k4ditano".to_string()])
            .build();

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

        // Actualizar labels del sidebar
        self.sidebar_notes_label.set_label(&i18n.t("notes"));

        // Actualizar placeholder del search entry
        self.search_entry
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
        menu_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        menu_box.append(&about_button);

        // Crear el popover
        let popover = gtk::Popover::builder().child(&menu_box).build();
        popover.add_css_class("menu");

        // Asignar el popover al MenuButton
        self.settings_button.set_popover(Some(&popover));
    }

    fn recreate_settings_popover(&self, sender: &ComponentSender<Self>) {
        // Recrear el popover con los textos actualizados
        self.create_settings_popover(sender);
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
        self.search_entry
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

                // Actualizar la base de datos si es necesario
                // Actualizar la base de datos
                if let Ok(Some(metadata)) = self.notes_db.get_note(note.name()) {
                    if let Err(e) = self.notes_db.move_note_to_folder(
                        metadata.id,
                        folder_name.as_deref(),
                        &new_path.to_string_lossy(),
                    ) {
                        eprintln!("Error actualizando base de datos: {}", e);
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
            // Crear el directorio padre si no existe
            if let Some(parent) = new_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("Error creando directorio padre: {}", e);
                    return;
                }
            }

            // Mover la carpeta completa
            if let Err(e) = std::fs::rename(&source_path, &new_path) {
                eprintln!("Error moviendo carpeta: {}", e);
                return;
            }

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
}
