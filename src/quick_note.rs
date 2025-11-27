// Quick Note - Ventana flotante para notas rápidas
//
// Este módulo implementa una ventana secundaria flotante que puede mostrarse
// en cualquier momento con un keybinding global, incluso sobre juegos/apps fullscreen.
//
// Características:
// - Always-on-top (pin en Hyprland)
// - Crear nuevas quick notes
// - Abrir quick notes existentes
// - Auto-guardado
// - Diseño minimalista

use gtk::prelude::*;
use relm4::{RelmWidgetExt, gtk};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::core::NotesDirectory;
use crate::i18n::I18n;

/// Nombre de la carpeta especial para quick notes
const QUICK_NOTES_FOLDER: &str = "quick-notes";

/// Estructura que representa una Quick Note
#[derive(Debug, Clone)]
pub struct QuickNote {
    pub name: String,
    pub path: PathBuf,
    pub preview: String,
    pub modified: String,
}

/// Manager de Quick Notes - maneja la creación, listado y persistencia
pub struct QuickNoteManager {
    notes_dir: NotesDirectory,
    quick_notes_path: PathBuf,
}

impl QuickNoteManager {
    pub fn new(notes_dir: NotesDirectory) -> Self {
        let quick_notes_path = notes_dir.root().join(QUICK_NOTES_FOLDER);

        // Crear carpeta de quick notes si no existe
        if !quick_notes_path.exists() {
            if let Err(e) = std::fs::create_dir_all(&quick_notes_path) {
                eprintln!("⚠️ Error creando carpeta de quick notes: {}", e);
            } else {
                println!("📁 Carpeta de quick notes creada: {:?}", quick_notes_path);
            }
        }

        Self {
            notes_dir,
            quick_notes_path,
        }
    }

    /// Lista todas las quick notes existentes
    pub fn list_quick_notes(&self) -> Vec<QuickNote> {
        let mut notes = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&self.quick_notes_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    let preview = content
                        .lines()
                        .take(2)
                        .collect::<Vec<_>>()
                        .join(" ")
                        .chars()
                        .take(80)
                        .collect::<String>();

                    let modified = if let Ok(metadata) = path.metadata() {
                        if let Ok(time) = metadata.modified() {
                            let datetime: chrono::DateTime<chrono::Local> = time.into();
                            datetime.format("%d/%m %H:%M").to_string()
                        } else {
                            "-".to_string()
                        }
                    } else {
                        "-".to_string()
                    };

                    notes.push(QuickNote {
                        name,
                        path,
                        preview,
                        modified,
                    });
                }
            }
        }

        // Ordenar por fecha de modificación (más reciente primero)
        notes.sort_by(|a, b| b.modified.cmp(&a.modified));
        notes
    }

    /// Crea una nueva quick note con timestamp como nombre
    pub fn create_quick_note(&self) -> Result<QuickNote, String> {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let name = format!("qn_{}", timestamp);
        let path = self.quick_notes_path.join(format!("{}.md", name));

        let initial_content = format!(
            "# Quick Note\n\n> Creada: {}\n\n",
            chrono::Local::now().format("%d/%m/%Y %H:%M")
        );

        std::fs::write(&path, &initial_content)
            .map_err(|e| format!("Error creando quick note: {}", e))?;

        Ok(QuickNote {
            name,
            path,
            preview: initial_content.chars().take(80).collect(),
            modified: chrono::Local::now().format("%d/%m %H:%M").to_string(),
        })
    }

    /// Lee el contenido de una quick note
    pub fn read_quick_note(&self, name: &str) -> Result<String, String> {
        let path = self.quick_notes_path.join(format!("{}.md", name));
        std::fs::read_to_string(&path).map_err(|e| format!("Error leyendo quick note: {}", e))
    }

    /// Guarda el contenido de una quick note
    pub fn save_quick_note(&self, name: &str, content: &str) -> Result<(), String> {
        let path = self.quick_notes_path.join(format!("{}.md", name));
        std::fs::write(&path, content).map_err(|e| format!("Error guardando quick note: {}", e))
    }

    /// Elimina una quick note
    pub fn delete_quick_note(&self, name: &str) -> Result<(), String> {
        let path = self.quick_notes_path.join(format!("{}.md", name));
        std::fs::remove_file(&path).map_err(|e| format!("Error eliminando quick note: {}", e))
    }

    /// Renombra una quick note
    pub fn rename_quick_note(&self, old_name: &str, new_name: &str) -> Result<(), String> {
        let old_path = self.quick_notes_path.join(format!("{}.md", old_name));
        let new_path = self.quick_notes_path.join(format!("{}.md", new_name));
        std::fs::rename(&old_path, &new_path)
            .map_err(|e| format!("Error renombrando quick note: {}", e))
    }
}

/// Ventana flotante de Quick Notes
pub struct QuickNoteWindow {
    window: gtk::Window,
    manager: Rc<RefCell<QuickNoteManager>>,
    current_note: Rc<RefCell<Option<String>>>,
    text_buffer: gtk::TextBuffer,
    notes_list: gtk::ListBox,
    content_stack: gtk::Stack,
    title_label: gtk::Label,
    #[allow(dead_code)]
    i18n: Rc<RefCell<I18n>>,
    has_unsaved_changes: Rc<RefCell<bool>>,
}

// Implementación manual de Debug porque gtk::Window y otros widgets no implementan Debug
impl std::fmt::Debug for QuickNoteWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuickNoteWindow")
            .field("current_note", &self.current_note)
            .field("has_unsaved_changes", &self.has_unsaved_changes)
            .finish_non_exhaustive()
    }
}

impl QuickNoteWindow {
    pub fn new(
        _parent: &impl IsA<gtk::Window>,
        notes_dir: NotesDirectory,
        i18n: Rc<RefCell<I18n>>,
    ) -> Self {
        let manager = Rc::new(RefCell::new(QuickNoteManager::new(notes_dir)));
        let current_note: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let has_unsaved_changes = Rc::new(RefCell::new(false));

        // Obtener traducciones
        let t_title = i18n.borrow().t("quick_notes_title");
        let t_back = i18n.borrow().t("quick_note_back_to_list");
        let t_new = i18n.borrow().t("quick_note_new");
        let t_keep_visible = i18n.borrow().t("quick_note_keep_visible");
        let t_close = i18n.borrow().t("quick_note_close");
        let t_no_notes = i18n.borrow().t("quick_note_no_notes");
        let t_press_to_create = i18n.borrow().t("quick_note_press_to_create");
        let t_saved = i18n.borrow().t("quick_note_saved");
        let t_shortcut_hint = i18n.borrow().t("quick_note_shortcut_hint");

        // Crear ventana flotante
        // Usamos "Quick Note" (singular) para que coincida con las reglas de Hyprland
        let window = gtk::Window::builder()
            .title("Quick Note")
            .default_width(450)
            .default_height(400)
            .modal(false)
            .resizable(true)
            .decorated(true)
            .build();

        // NO usar transient_for para que la ventana se abra en el monitor del ratón
        // window.set_transient_for(Some(parent));

        // Establecer tamaño máximo y mínimo para evitar que se maximice
        window.set_size_request(400, 350);

        // CSS class para estilos personalizados
        window.add_css_class("quick-note-window");

        // === LAYOUT PRINCIPAL ===
        let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        main_box.add_css_class("quick-note-container");

        // Header
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.set_margin_all(12);
        header.add_css_class("quick-note-header");

        let header_icon = gtk::Label::new(Some("📝"));
        header_icon.add_css_class("quick-note-icon");
        header.append(&header_icon);

        let title_label = gtk::Label::new(Some(&t_title));
        title_label.set_hexpand(true);
        title_label.set_xalign(0.0);
        title_label.add_css_class("quick-note-title");
        header.append(&title_label);

        // Botón para volver a la lista
        let back_button = gtk::Button::new();
        back_button.set_icon_name("go-previous-symbolic");
        back_button.set_tooltip_text(Some(&t_back));
        back_button.add_css_class("flat");
        back_button.add_css_class("circular");
        back_button.set_visible(false);
        header.append(&back_button);

        // Botón para crear nueva nota
        let new_button = gtk::Button::new();
        new_button.set_icon_name("list-add-symbolic");
        new_button.set_tooltip_text(Some(&t_new));
        new_button.add_css_class("flat");
        new_button.add_css_class("circular");
        header.append(&new_button);

        // Botón pin (visual, el pin real lo hace Hyprland)
        let pin_button = gtk::ToggleButton::new();
        pin_button.set_icon_name("view-pin-symbolic");
        pin_button.set_tooltip_text(Some(&t_keep_visible));
        pin_button.add_css_class("flat");
        pin_button.add_css_class("circular");
        pin_button.set_active(true);
        header.append(&pin_button);

        // Botón cerrar
        let close_button = gtk::Button::new();
        close_button.set_icon_name("window-close-symbolic");
        close_button.set_tooltip_text(Some(&t_close));
        close_button.add_css_class("flat");
        close_button.add_css_class("circular");
        header.append(&close_button);

        main_box.append(&header);
        main_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

        // === STACK: Lista de notas / Editor ===
        let content_stack = gtk::Stack::new();
        content_stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
        content_stack.set_transition_duration(200);
        content_stack.set_vexpand(true);

        // --- PÁGINA 1: Lista de quick notes ---
        let list_page = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let list_scroll = gtk::ScrolledWindow::new();
        list_scroll.set_vexpand(true);
        list_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

        let notes_list = gtk::ListBox::new();
        notes_list.add_css_class("quick-notes-list");
        notes_list.add_css_class("boxed-list");
        notes_list.set_selection_mode(gtk::SelectionMode::None);
        list_scroll.set_child(Some(&notes_list));

        list_page.append(&list_scroll);

        // Mensaje cuando no hay notas
        let empty_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
        empty_box.set_valign(gtk::Align::Center);
        empty_box.set_halign(gtk::Align::Center);
        empty_box.set_vexpand(true);

        let empty_icon = gtk::Label::new(Some("📋"));
        empty_icon.add_css_class("dim-label");
        empty_icon.set_opacity(0.5);
        empty_box.append(&empty_icon);

        let empty_label = gtk::Label::new(Some(&t_no_notes));
        empty_label.add_css_class("dim-label");
        empty_box.append(&empty_label);

        let empty_hint = gtk::Label::new(Some(&t_press_to_create));
        empty_hint.add_css_class("dim-label");
        empty_hint.set_opacity(0.7);
        empty_box.append(&empty_hint);

        list_page.append(&empty_box);

        content_stack.add_named(&list_page, Some("list"));

        // --- PÁGINA 2: Editor ---
        let editor_page = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let editor_scroll = gtk::ScrolledWindow::new();
        editor_scroll.set_vexpand(true);
        editor_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);

        let text_view = gtk::TextView::new();
        text_view.set_wrap_mode(gtk::WrapMode::WordChar);
        text_view.set_left_margin(16);
        text_view.set_right_margin(16);
        text_view.set_top_margin(12);
        text_view.set_bottom_margin(12);
        text_view.add_css_class("quick-note-editor");

        let text_buffer = text_view.buffer();
        editor_scroll.set_child(Some(&text_view));
        editor_page.append(&editor_scroll);

        // Barra de estado del editor
        let status_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        status_bar.set_margin_all(8);
        status_bar.add_css_class("quick-note-status");

        let save_indicator = gtk::Label::new(Some(&t_saved));
        save_indicator.add_css_class("dim-label");
        save_indicator.set_xalign(0.0);
        status_bar.append(&save_indicator);

        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        status_bar.append(&spacer);

        let shortcut_hint = gtk::Label::new(Some(&t_shortcut_hint));
        shortcut_hint.add_css_class("dim-label");
        shortcut_hint.set_opacity(0.7);
        status_bar.append(&shortcut_hint);

        editor_page.append(&status_bar);

        content_stack.add_named(&editor_page, Some("editor"));

        main_box.append(&content_stack);

        window.set_child(Some(&main_box));

        // === CONECTAR EVENTOS ===

        // Cerrar ventana
        let window_clone = window.clone();
        close_button.connect_clicked(move |_| {
            window_clone.set_visible(false);
        });

        // Escape para cerrar
        let key_controller = gtk::EventControllerKey::new();
        let window_for_key = window.clone();
        let content_stack_for_key = content_stack.clone();
        let back_button_for_key = back_button.clone();
        let new_button_for_key = new_button.clone();
        let title_label_for_key = title_label.clone();
        let current_note_for_key = current_note.clone();
        let manager_for_key = manager.clone();
        let notes_list_for_key = notes_list.clone();
        let text_buffer_for_key = text_buffer.clone();
        let has_unsaved_for_key = has_unsaved_changes.clone();
        let save_indicator_for_key = save_indicator.clone();

        key_controller.connect_key_pressed(move |_, keyval, _, modifiers| {
            let key_name = keyval.name().map(|s| s.to_string()).unwrap_or_default();

            match key_name.as_str() {
                "Escape" => {
                    // Si estamos en el editor, volver a la lista
                    if content_stack_for_key.visible_child_name().as_deref() == Some("editor") {
                        // Guardar antes de salir
                        if *has_unsaved_for_key.borrow() {
                            if let Some(note_name) = current_note_for_key.borrow().as_ref() {
                                let start = text_buffer_for_key.start_iter();
                                let end = text_buffer_for_key.end_iter();
                                let content =
                                    text_buffer_for_key.text(&start, &end, false).to_string();
                                let _ = manager_for_key
                                    .borrow()
                                    .save_quick_note(note_name, &content);
                            }
                        }

                        // Volver a la lista
                        content_stack_for_key.set_visible_child_name("list");
                        back_button_for_key.set_visible(false);
                        new_button_for_key.set_visible(true);
                        title_label_for_key.set_label("Quick Notes");
                        *current_note_for_key.borrow_mut() = None;

                        // Refrescar lista
                        refresh_notes_list(&notes_list_for_key, &manager_for_key.borrow());
                    } else {
                        // En la lista, cerrar ventana
                        window_for_key.set_visible(false);
                    }
                    gtk::glib::Propagation::Stop
                }
                "s" if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) => {
                    // Ctrl+S: Guardar
                    if let Some(note_name) = current_note_for_key.borrow().as_ref() {
                        let start = text_buffer_for_key.start_iter();
                        let end = text_buffer_for_key.end_iter();
                        let content = text_buffer_for_key.text(&start, &end, false).to_string();

                        if manager_for_key
                            .borrow()
                            .save_quick_note(note_name, &content)
                            .is_ok()
                        {
                            *has_unsaved_for_key.borrow_mut() = false;
                            save_indicator_for_key.set_label("💾 Guardado");
                        }
                    }
                    gtk::glib::Propagation::Stop
                }
                _ => gtk::glib::Propagation::Proceed,
            }
        });
        window.add_controller(key_controller);

        // Pin toggle - nota: en GTK4/Wayland el pin real lo maneja Hyprland con windowrules
        // Este botón es solo visual, la funcionalidad real está en la configuración del WM
        pin_button.connect_toggled(move |_btn| {
            // En GTK4 no hay set_keep_above directo
            // La ventana se marca como "always on top" mediante reglas de Hyprland/Sway
            println!(
                "📌 Pin toggle - configurar en Hyprland: windowrulev2 = pin, title:^(Quick Note)$"
            );
        });

        // Nueva quick note
        let manager_for_new = manager.clone();
        let content_stack_for_new = content_stack.clone();
        let text_buffer_for_new = text_buffer.clone();
        let current_note_for_new = current_note.clone();
        let back_button_for_new = back_button.clone();
        let new_button_for_new = new_button.clone();
        let title_label_for_new = title_label.clone();
        let notes_list_for_new = notes_list.clone();
        let text_view_for_new = text_view.clone();

        new_button.connect_clicked(move |_| {
            match manager_for_new.borrow().create_quick_note() {
                Ok(note) => {
                    // Cargar contenido
                    if let Ok(content) = manager_for_new.borrow().read_quick_note(&note.name) {
                        text_buffer_for_new.set_text(&content);
                    }

                    *current_note_for_new.borrow_mut() = Some(note.name.clone());
                    title_label_for_new.set_label(&format!("📝 {}", note.name));

                    // Cambiar a editor
                    content_stack_for_new.set_visible_child_name("editor");
                    back_button_for_new.set_visible(true);
                    new_button_for_new.set_visible(false);

                    // Focus en el editor
                    text_view_for_new.grab_focus();

                    // Refrescar lista para cuando vuelva
                    refresh_notes_list(&notes_list_for_new, &manager_for_new.borrow());
                }
                Err(e) => {
                    eprintln!("❌ Error creando quick note: {}", e);
                }
            }
        });

        // Volver a la lista
        let manager_for_back = manager.clone();
        let content_stack_for_back = content_stack.clone();
        let text_buffer_for_back = text_buffer.clone();
        let current_note_for_back = current_note.clone();
        let new_button_for_back = new_button.clone();
        let title_label_for_back = title_label.clone();
        let notes_list_for_back = notes_list.clone();
        let has_unsaved_for_back = has_unsaved_changes.clone();
        let back_button_clone = back_button.clone();

        back_button.connect_clicked(move |btn| {
            // Guardar antes de salir
            if *has_unsaved_for_back.borrow() {
                if let Some(note_name) = current_note_for_back.borrow().as_ref() {
                    let start = text_buffer_for_back.start_iter();
                    let end = text_buffer_for_back.end_iter();
                    let content = text_buffer_for_back.text(&start, &end, false).to_string();
                    let _ = manager_for_back
                        .borrow()
                        .save_quick_note(note_name, &content);
                }
            }

            // Volver a la lista
            content_stack_for_back.set_visible_child_name("list");
            btn.set_visible(false);
            new_button_for_back.set_visible(true);
            title_label_for_back.set_label("Quick Notes");
            *current_note_for_back.borrow_mut() = None;

            // Refrescar lista
            refresh_notes_list(&notes_list_for_back, &manager_for_back.borrow());
        });

        // Detectar cambios en el buffer
        let has_unsaved_for_change = has_unsaved_changes.clone();
        let save_indicator_for_change = save_indicator.clone();
        let i18n_for_change = i18n.clone();

        text_buffer.connect_changed(move |_| {
            *has_unsaved_for_change.borrow_mut() = true;
            let t = i18n_for_change.borrow().t("quick_note_unsaved");
            save_indicator_for_change.set_label(&t);
        });

        // Auto-guardado cada 5 segundos si hay cambios
        let manager_for_autosave = manager.clone();
        let current_note_for_autosave = current_note.clone();
        let text_buffer_for_autosave = text_buffer.clone();
        let has_unsaved_for_autosave = has_unsaved_changes.clone();
        let save_indicator_for_autosave = save_indicator.clone();
        let i18n_for_autosave = i18n.clone();

        gtk::glib::timeout_add_local(std::time::Duration::from_secs(5), move || {
            if *has_unsaved_for_autosave.borrow() {
                if let Some(note_name) = current_note_for_autosave.borrow().as_ref() {
                    let start = text_buffer_for_autosave.start_iter();
                    let end = text_buffer_for_autosave.end_iter();
                    let content = text_buffer_for_autosave
                        .text(&start, &end, false)
                        .to_string();

                    if manager_for_autosave
                        .borrow()
                        .save_quick_note(note_name, &content)
                        .is_ok()
                    {
                        *has_unsaved_for_autosave.borrow_mut() = false;
                        let t = i18n_for_autosave.borrow().t("quick_note_autosaved");
                        save_indicator_for_autosave.set_label(&t);
                    }
                }
            }
            gtk::glib::ControlFlow::Continue
        });

        let instance = Self {
            window,
            manager,
            current_note,
            text_buffer,
            notes_list,
            content_stack,
            title_label,
            i18n,
            has_unsaved_changes,
        };

        // Manejar close request (cuando Hyprland intenta cerrar la ventana)
        // Guardamos y ocultamos en lugar de destruir
        let manager_for_close = instance.manager.clone();
        let text_buffer_for_close = instance.text_buffer.clone();
        let current_note_for_close = instance.current_note.clone();
        let has_unsaved_for_close = instance.has_unsaved_changes.clone();

        instance.window.connect_close_request(move |win| {
            // Guardar si hay cambios pendientes
            if *has_unsaved_for_close.borrow() {
                if let Some(note_name) = current_note_for_close.borrow().as_ref() {
                    let start = text_buffer_for_close.start_iter();
                    let end = text_buffer_for_close.end_iter();
                    let content = text_buffer_for_close.text(&start, &end, false).to_string();
                    let _ = manager_for_close
                        .borrow()
                        .save_quick_note(note_name, &content);
                }
            }
            // Ocultar en lugar de destruir
            win.set_visible(false);
            // Inhibir el cierre para mantener la ventana viva
            gtk::glib::Propagation::Stop
        });

        // Configurar click en items de la lista
        instance.setup_list_clicks();

        // Cargar lista inicial
        instance.refresh_list();

        instance
    }

    /// Configura los clicks en los items de la lista
    fn setup_list_clicks(&self) {
        let manager = self.manager.clone();
        let content_stack = self.content_stack.clone();
        let text_buffer = self.text_buffer.clone();
        let current_note = self.current_note.clone();
        let title_label = self.title_label.clone();
        let notes_list = self.notes_list.clone();

        // Necesitamos obtener referencias a los botones del header
        // Como no los guardamos, los buscaremos cuando se active un row

        self.notes_list.connect_row_activated(move |list, row| {
            // Obtener el nombre de la nota del row
            let note_name: Option<String> =
                unsafe { row.data::<String>("note_name").map(|d| d.as_ref().clone()) };

            if let Some(name) = note_name {
                // Cargar contenido
                if let Ok(content) = manager.borrow().read_quick_note(&name) {
                    text_buffer.set_text(&content);
                }

                *current_note.borrow_mut() = Some(name.clone());
                title_label.set_label(&format!("📝 {}", name));

                // Cambiar a editor
                content_stack.set_visible_child_name("editor");

                // Buscar y actualizar botones en el header
                if let Some(parent) = list.parent() {
                    if let Some(grandparent) = parent.parent() {
                        if let Some(main_box) = grandparent.parent() {
                            // Buscar el header
                            if let Some(first_child) = main_box.first_child() {
                                // Iterar children del header
                                let mut child = first_child.first_child();
                                while let Some(widget) = child {
                                    if let Ok(btn) = widget.clone().downcast::<gtk::Button>() {
                                        if btn.icon_name().as_deref()
                                            == Some("go-previous-symbolic")
                                        {
                                            btn.set_visible(true);
                                        } else if btn.icon_name().as_deref()
                                            == Some("list-add-symbolic")
                                        {
                                            btn.set_visible(false);
                                        }
                                    }
                                    child = widget.next_sibling();
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    /// Refresca la lista de quick notes
    pub fn refresh_list(&self) {
        refresh_notes_list(&self.notes_list, &self.manager.borrow());
    }

    /// Muestra la ventana (toggle)
    pub fn toggle(&self) {
        if self.window.is_visible() {
            self.window.set_visible(false);
        } else {
            // Refrescar lista antes de mostrar
            self.refresh_list();

            // Mostrar en la lista de notas
            self.content_stack.set_visible_child_name("list");

            self.window.set_visible(true);
            self.window.present();
        }
    }

    /// Muestra la ventana
    pub fn show(&self) {
        self.refresh_list();
        self.content_stack.set_visible_child_name("list");
        self.window.set_visible(true);
        self.window.present();
    }

    /// Oculta la ventana
    pub fn hide(&self) {
        // Guardar si hay cambios pendientes
        if *self.has_unsaved_changes.borrow() {
            if let Some(note_name) = self.current_note.borrow().as_ref() {
                let start = self.text_buffer.start_iter();
                let end = self.text_buffer.end_iter();
                let content = self.text_buffer.text(&start, &end, false).to_string();
                let _ = self.manager.borrow().save_quick_note(note_name, &content);
            }
        }
        self.window.set_visible(false);
    }

    /// Crea una nueva quick note y la abre
    pub fn new_note(&self) {
        if let Ok(note) = self.manager.borrow().create_quick_note() {
            // Cargar contenido
            if let Ok(content) = self.manager.borrow().read_quick_note(&note.name) {
                self.text_buffer.set_text(&content);
            }

            *self.current_note.borrow_mut() = Some(note.name.clone());
            self.title_label.set_label(&format!("📝 {}", note.name));

            // Cambiar a editor
            self.content_stack.set_visible_child_name("editor");

            // Mostrar ventana
            self.window.set_visible(true);
            self.window.present();
        }
    }

    /// Retorna si la ventana está visible
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }
}

/// Helper para refrescar la lista de notas
fn refresh_notes_list(list: &gtk::ListBox, manager: &QuickNoteManager) {
    // Limpiar lista
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let notes = manager.list_quick_notes();

    if notes.is_empty() {
        // Mostrar mensaje de vacío (ya está en la UI como widget separado)
        return;
    }

    for note in notes {
        let row = gtk::ListBoxRow::new();
        row.set_activatable(true);
        row.add_css_class("quick-note-row");

        // Guardar nombre en el row
        unsafe {
            row.set_data("note_name", note.name.clone());
        }

        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        hbox.set_margin_all(12);

        // Icono
        let icon = gtk::Label::new(Some("📄"));
        hbox.append(&icon);

        // Info de la nota
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 4);
        vbox.set_hexpand(true);

        let name_label = gtk::Label::new(Some(&note.name));
        name_label.set_xalign(0.0);
        name_label.add_css_class("heading");
        vbox.append(&name_label);

        if !note.preview.is_empty() {
            let preview_label = gtk::Label::new(Some(&note.preview));
            preview_label.set_xalign(0.0);
            preview_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            preview_label.set_max_width_chars(40);
            preview_label.add_css_class("dim-label");
            vbox.append(&preview_label);
        }

        hbox.append(&vbox);

        // Fecha
        let date_label = gtk::Label::new(Some(&note.modified));
        date_label.add_css_class("dim-label");
        date_label.set_valign(gtk::Align::Center);
        hbox.append(&date_label);

        row.set_child(Some(&hbox));
        list.append(&row);
    }
}
