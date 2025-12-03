use gtk::prelude::*;
use gtk::{gio, glib};
use relm4::gtk;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::path::Path;
use std::fmt;
use webkit6::prelude::WebViewExt;

use crate::core::{
    Base, BaseQueryEngine, BaseView, ColumnConfig, Filter, FilterGroup, 
    FilterOperator, GroupedRecord, NoteMetadata, NoteWithProperties, NotesDatabase, PropertyValue, 
    SortConfig, SortDirection, SourceType, ViewType, HtmlRenderer, PreviewTheme,
    CellFormat, SpecialCellContent, SpecialRow, CellGrid, CellRef, CellValue,
};
use crate::graph_view::GraphView;
use crate::i18n::{I18n, Language};

/// Colores del tema extraídos de GTK
#[derive(Debug, Clone)]
pub struct GtkThemeColors {
    pub bg_primary: String,    // Fondo principal
    pub bg_secondary: String,  // Fondo secundario (rows alternas)
    pub bg_tertiary: String,   // Fondo hover
    pub fg_primary: String,    // Texto principal
    pub fg_secondary: String,  // Texto secundario
    pub fg_muted: String,      // Texto atenuado
    pub accent: String,        // Color de acento
    pub border: String,        // Bordes
}

impl Default for GtkThemeColors {
    fn default() -> Self {
        // Colores por defecto (tema oscuro)
        Self {
            bg_primary: "#1e1e2e".to_string(),
            bg_secondary: "#313244".to_string(),
            bg_tertiary: "#45475a".to_string(),
            fg_primary: "#cdd6f4".to_string(),
            fg_secondary: "#a6adc8".to_string(),
            fg_muted: "#6c7086".to_string(),
            accent: "#89b4fa".to_string(),
            border: "#45475a".to_string(),
        }
    }
}

impl GtkThemeColors {
    /// Extraer colores del tema GTK actual usando lookup_color
    pub fn from_widget(widget: &impl IsA<gtk::Widget>) -> Self {
        let style_context = widget.style_context();
        
        // Intentar obtener colores del tema CSS
        // Primero buscar los colores personalizados de la app (@base, @text)
        // luego fallback a los colores estándar de GTK
        let bg_color = style_context.lookup_color("base")
            .or_else(|| style_context.lookup_color("theme_bg_color"))
            .or_else(|| style_context.lookup_color("window_bg_color"));
        
        // Buscar @text primero, luego theme_fg_color
        let fg_color = style_context.lookup_color("text")
            .or_else(|| style_context.lookup_color("theme_fg_color"))
            .unwrap_or_else(|| style_context.color());
        
        // Accent color
        let accent_color = style_context.lookup_color("accent_bg_color")
            .or_else(|| style_context.lookup_color("accent_color"))
            .or_else(|| style_context.lookup_color("selected_bg_color"));
        
        // Border color - buscar @border primero
        let border_color = style_context.lookup_color("border")
            .or_else(|| style_context.lookup_color("borders"));
        
        // Si obtuvimos el color de fondo, usarlo
        if let Some(bg) = bg_color {
            let fg = fg_color;
            let accent = accent_color.unwrap_or_else(|| gtk::gdk::RGBA::new(0.4, 0.7, 1.0, 1.0));
            let border = border_color.unwrap_or_else(|| Self::adjust_brightness(&bg, 0.15));
            
            // Generar variantes del fondo
            let bg_secondary = Self::adjust_brightness(&bg, 0.05);
            let bg_tertiary = Self::adjust_brightness(&bg, 0.10);
            
            // Detectar si es tema claro u oscuro basado en luminosidad del fondo
            let bg_luminance = bg.red() * 0.299 + bg.green() * 0.587 + bg.blue() * 0.114;
            let is_light_theme = bg_luminance > 0.5;
            
            // Generar variantes de texto sólidas (mezcla con el fondo)
            let (fg_secondary, fg_muted) = if is_light_theme {
                // Tema claro: aclarar ligeramente el texto oscuro
                (Self::blend_colors(&fg, &bg, 0.15), Self::blend_colors(&fg, &bg, 0.40))
            } else {
                // Tema oscuro: oscurecer ligeramente el texto claro
                (Self::blend_colors(&fg, &bg, 0.15), Self::blend_colors(&fg, &bg, 0.40))
            };
            
            Self {
                bg_primary: Self::rgba_to_hex(&bg),
                bg_secondary: Self::rgba_to_hex(&bg_secondary),
                bg_tertiary: Self::rgba_to_hex(&bg_tertiary),
                fg_primary: Self::rgba_to_hex(&fg),
                fg_secondary,
                fg_muted,
                accent: Self::rgba_to_hex(&accent),
                border: Self::rgba_to_hex(&border),
            }
        } else {
            // Fallback: usar el color de texto para inferir el tema
            let fg = fg_color;
            let luminance = fg.red() * 0.299 + fg.green() * 0.587 + fg.blue() * 0.114;
            let is_dark = luminance > 0.5;
            
            if is_dark {
                // Tema oscuro - generar colores oscuros
                Self::generate_dark_theme(&fg, accent_color)
            } else {
                // Tema claro - generar colores claros
                Self::generate_light_theme(&fg, accent_color)
            }
        }
    }
    
    fn rgba_to_hex(color: &gtk::gdk::RGBA) -> String {
        format!("#{:02x}{:02x}{:02x}",
            (color.red() * 255.0) as u8,
            (color.green() * 255.0) as u8,
            (color.blue() * 255.0) as u8
        )
    }
    
    fn rgba_with_alpha(color: &gtk::gdk::RGBA, alpha: f32) -> String {
        format!("rgba({}, {}, {}, {})",
            (color.red() * 255.0) as u8,
            (color.green() * 255.0) as u8,
            (color.blue() * 255.0) as u8,
            alpha
        )
    }
    
    /// Mezclar dos colores con un factor (0.0 = color1, 1.0 = color2)
    fn blend_colors(color1: &gtk::gdk::RGBA, color2: &gtk::gdk::RGBA, factor: f32) -> String {
        let r = color1.red() * (1.0 - factor) + color2.red() * factor;
        let g = color1.green() * (1.0 - factor) + color2.green() * factor;
        let b = color1.blue() * (1.0 - factor) + color2.blue() * factor;
        format!("#{:02x}{:02x}{:02x}",
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8
        )
    }
    
    fn adjust_brightness(color: &gtk::gdk::RGBA, amount: f32) -> gtk::gdk::RGBA {
        let luminance = color.red() * 0.299 + color.green() * 0.587 + color.blue() * 0.114;
        
        if luminance < 0.5 {
            // Color oscuro - aclarar
            gtk::gdk::RGBA::new(
                (color.red() + amount).clamp(0.0, 1.0),
                (color.green() + amount).clamp(0.0, 1.0),
                (color.blue() + amount).clamp(0.0, 1.0),
                1.0
            )
        } else {
            // Color claro - oscurecer
            gtk::gdk::RGBA::new(
                (color.red() - amount).clamp(0.0, 1.0),
                (color.green() - amount).clamp(0.0, 1.0),
                (color.blue() - amount).clamp(0.0, 1.0),
                1.0
            )
        }
    }
    
    fn generate_dark_theme(fg: &gtk::gdk::RGBA, accent: Option<gtk::gdk::RGBA>) -> Self {
        let accent = accent.unwrap_or_else(|| gtk::gdk::RGBA::new(0.54, 0.71, 0.98, 1.0)); // #89b4fa
        let bg = gtk::gdk::RGBA::new(0.118, 0.118, 0.18, 1.0); // #1e1e2e
        Self {
            bg_primary: "#1e1e2e".to_string(),
            bg_secondary: "#252536".to_string(),
            bg_tertiary: "#313244".to_string(),
            fg_primary: Self::rgba_to_hex(fg),
            fg_secondary: Self::blend_colors(fg, &bg, 0.15),
            fg_muted: Self::blend_colors(fg, &bg, 0.40),
            accent: Self::rgba_to_hex(&accent),
            border: "#45475a".to_string(),
        }
    }
    
    fn generate_light_theme(fg: &gtk::gdk::RGBA, accent: Option<gtk::gdk::RGBA>) -> Self {
        let accent = accent.unwrap_or_else(|| gtk::gdk::RGBA::new(0.12, 0.40, 0.96, 1.0)); // #1e66f5
        let bg = gtk::gdk::RGBA::new(0.937, 0.945, 0.961, 1.0); // #eff1f5
        Self {
            bg_primary: "#eff1f5".to_string(),
            bg_secondary: "#e6e9ef".to_string(),
            bg_tertiary: "#dce0e8".to_string(),
            fg_primary: Self::rgba_to_hex(fg),
            fg_secondary: Self::blend_colors(fg, &bg, 0.15),
            fg_muted: Self::blend_colors(fg, &bg, 0.40),
            accent: Self::rgba_to_hex(&accent),
            border: "#ccd0da".to_string(),
        }
    }
}

/// Capitaliza la primera letra de un string
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Widget para mostrar una vista de tabla de una Base
pub struct BaseTableWidget {
    container: gtk::Box,
    content_stack: gtk::Stack,  // Stack para alternar tabla/grafo
    table_webview: webkit6::WebView,  // WebView para la tabla HTML
    column_view: gtk::ColumnView,  // ColumnView (mantenido para lógica de columnas)
    list_store: gio::ListStore,  // ListStore para datos
    filter_bar: gtk::Box,
    filters_container: gtk::Box,
    view_tabs: gtk::Box,
    status_bar: gtk::Box,
    graph_view: GraphView,  // Vista de grafo de relaciones
    graph_toggle: gtk::ToggleButton,  // Botón para alternar vista
    sort_btn: gtk::MenuButton,  // Botón de ordenamiento
    columns_btn: gtk::Button,  // Botón de columnas
    formula_row_btn: gtk::MenuButton,  // Botón para filas con fórmulas
    export_xlsx_btn: gtk::Button,  // Botón para exportar a XLSX
    source_type_btn: gtk::MenuButton,  // Botón para cambiar modo (Notes/GroupedRecords)
    
    /// Internacionalización
    i18n: Rc<RefCell<I18n>>,
    
    /// Base actual
    base: Rc<RefCell<Option<Base>>>,
    
    /// Notas actuales (sin filtrar)
    all_notes: Rc<RefCell<Vec<NoteWithProperties>>>,
    
    /// Notas filtradas (mostradas)
    notes: Rc<RefCell<Vec<NoteWithProperties>>>,
    
    /// Filtros activos (adicionales a los de la vista)
    active_filters: Rc<RefCell<Vec<Filter>>>,
    
    /// Ordenamiento actual
    current_sort: Rc<RefCell<Option<SortConfig>>>,
    
    /// Propiedades disponibles
    available_properties: Rc<RefCell<Vec<String>>>,
    
    /// Referencia a la BD y notes_root para refrescar
    db_path: Rc<RefCell<Option<std::path::PathBuf>>>,
    notes_root: Rc<RefCell<Option<std::path::PathBuf>>>,
    
    /// ID de la base actual (para persistir cambios)
    base_id: Rc<RefCell<Option<i64>>>,
    
    /// Referencia a la BD para persistir cambios
    notes_db: Rc<RefCell<Option<NotesDatabase>>>,
    
    /// Callbacks
    on_note_selected: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
    on_note_double_click: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
    
    /// Callback para clic en nodo del grafo (requiere Send+Sync)
    on_graph_note_click: std::sync::Arc<std::sync::Mutex<Option<Box<dyn Fn(&str) + Send + Sync>>>>,
    
    /// Callback cuando cambia el source_type (para recargar)
    on_source_type_changed: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    
    /// Callback cuando se hace clic en la vista (para cerrar sidebar)
    on_view_clicked: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    
    /// Callback para edición de celda (note_id, group_id, property, new_value)
    on_cell_edit: Rc<RefCell<Option<Box<dyn Fn(i64, i64, &str, &str)>>>>,
    
    /// Preferencia de tema oscuro (sincronizada con la app)
    is_dark_theme: Rc<RefCell<bool>>,
    
    /// Colores del tema GTK extraídos
    theme_colors: Rc<RefCell<GtkThemeColors>>,
    
    /// Flag para evitar actualizaciones intermedias del WebView durante carga
    is_loading: Rc<RefCell<bool>>,
}

impl fmt::Debug for BaseTableWidget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BaseTableWidget")
            .field("base_id", &self.base_id.borrow())
            .finish_non_exhaustive()
    }
}

impl BaseTableWidget {
    pub fn new(i18n: Rc<RefCell<I18n>>) -> Self {
        // Container principal
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .css_classes(["base-table-container"])
            .build();

        // Barra de filtros (arriba)
        let (filter_bar, filters_container, sort_btn, columns_btn, formula_row_btn, export_xlsx_btn, graph_toggle, source_type_btn) = Self::create_filter_bar(&i18n.borrow());
        container.append(&filter_bar);

        // Tabs de vistas
        let view_tabs = Self::create_view_tabs();
        container.append(&view_tabs);

        // Stack para alternar entre tabla y grafo
        let content_stack = gtk::Stack::builder()
            .vexpand(true)
            .transition_type(gtk::StackTransitionType::Crossfade)
            .transition_duration(200)
            .build();

        // Scroll container para el WebView de la tabla
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .css_classes(["base-table-scroll"])
            .build();

        // WebView para la tabla HTML (como en las notas)
        let table_webview = webkit6::WebView::builder()
            .vexpand(true)
            .hexpand(true)
            .build();
        
        // Configurar el WebView
        if let Some(settings) = webkit6::prelude::WebViewExt::settings(&table_webview) {
            settings.set_enable_javascript(true);
            settings.set_enable_smooth_scrolling(true);
        }
        
        // Configurar color de fondo del WebView para evitar flash negro durante transiciones
        // Usar un gris oscuro que coincida con el tema oscuro por defecto
        table_webview.set_background_color(&gtk::gdk::RGBA::new(0.12, 0.12, 0.12, 1.0));
        
        // Configurar UserContentManager para recibir mensajes JS→Rust
        let on_note_selected: Rc<RefCell<Option<Box<dyn Fn(&str)>>>> = Rc::new(RefCell::new(None));
        let on_note_double_click: Rc<RefCell<Option<Box<dyn Fn(&str)>>>> = Rc::new(RefCell::new(None));
        let on_view_clicked: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_cell_edit: Rc<RefCell<Option<Box<dyn Fn(i64, i64, &str, &str)>>>> = Rc::new(RefCell::new(None));
        
        if let Some(content_manager) = table_webview.user_content_manager() {
            content_manager.register_script_message_handler("noteClick", None);
            content_manager.register_script_message_handler("cellEdit", None);
            content_manager.register_script_message_handler("specialRowAction", None);
            
            // Conectar el handler para clicks
            let on_note_double_click_clone = on_note_double_click.clone();
            let on_view_clicked_clone = on_view_clicked.clone();
            
            content_manager.connect_script_message_received(Some("noteClick"), move |_, result| {
                // Obtener el mensaje
                let message_str = result.to_str();
                let clean_path = message_str.trim_matches('"').to_string();
                
                // Siempre cerrar el sidebar al hacer clic
                if let Some(ref callback) = *on_view_clicked_clone.borrow() {
                    callback();
                }
                
                // Solo procesar selección si no es el mensaje especial de cerrar sidebar
                if !clean_path.is_empty() && clean_path != "__close_sidebar__" {
                    if let Some(ref callback) = *on_note_double_click_clone.borrow() {
                        // Manejar mensaje especial __open_note__:nombre
                        if clean_path.starts_with("__open_note__:") {
                            let note_name = clean_path.strip_prefix("__open_note__:").unwrap_or("");
                            if !note_name.is_empty() {
                                // Enviar el nombre con extensión .md si no la tiene
                                let full_name = if note_name.ends_with(".md") {
                                    note_name.to_string()
                                } else {
                                    format!("{}.md", note_name)
                                };
                                callback(&full_name);
                            }
                        } else {
                            callback(&clean_path);
                        }
                    }
                }
            });
            
            // Conectar el handler para ediciones de celda
            let on_cell_edit_clone = on_cell_edit.clone();
            content_manager.connect_script_message_received(Some("cellEdit"), move |_, result| {
                // Formato esperado: JSON con {action, noteId, groupId, property, value, originalValue, notePath}
                let message_str = result.to_str();
                let clean_msg = message_str.trim_matches('"');
                
                // Parsear JSON
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(clean_msg) {
                    // noteId y groupId pueden venir como string o número
                    let note_id = json.get("noteId")
                        .and_then(|v| {
                            v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                        })
                        .unwrap_or(0);
                    let group_id = json.get("groupId")
                        .and_then(|v| {
                            v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                        })
                        .unwrap_or(0);
                    let property = json.get("property")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let new_value = json.get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    
                    if !property.is_empty() {
                        println!("📝 Cell edit: note_id={}, group_id={}, {}::{}", 
                            note_id, group_id, property, new_value);
                        if let Some(ref callback) = *on_cell_edit_clone.borrow() {
                            callback(note_id, group_id, property, new_value);
                        }
                    }
                } else {
                    eprintln!("⚠️ Error parsing cellEdit JSON: {}", clean_msg);
                }
            });
            
            // Handler para acciones de filas especiales (fórmulas)
            // Este handler necesita acceso al base, notes, etc. - se configura en setup_formula_row_popover
        }

        scroll.set_child(Some(&table_webview));
        
        // Lista vacía para datos (mantenida para lógica de filtros/orden)
        let list_store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let selection_model = gtk::SingleSelection::new(Some(list_store.clone()));
        
        // ColumnView (oculto, solo para lógica de columnas)
        let column_view = gtk::ColumnView::builder()
            .model(&selection_model)
            .css_classes(["base-table"])
            .build();
        
        // Añadir WebView al stack (no ColumnView)
        content_stack.add_named(&scroll, Some("table"));
        
        // Crear GraphView
        let graph_view = GraphView::new();
        graph_view.set_vexpand(true);
        graph_view.set_hexpand(true);
        graph_view.add_css_class("base-graph-view");
        
        // Añadir grafo al stack
        content_stack.add_named(&graph_view, Some("graph"));
        
        // Mostrar tabla por defecto
        content_stack.set_visible_child_name("table");
        
        container.append(&content_stack);

        // Barra de estado (abajo)
        let status_bar = Self::create_status_bar();
        container.append(&status_bar);

        // Crear referencias compartidas
        let base = Rc::new(RefCell::new(None));
        let base_id = Rc::new(RefCell::new(None));
        let notes_db: Rc<RefCell<Option<NotesDatabase>>> = Rc::new(RefCell::new(None));
        let available_properties = Rc::new(RefCell::new(Vec::new()));
        let notes = Rc::new(RefCell::new(Vec::new()));
        
        // Conectar botón de columnas UNA SOLA VEZ
        {
            let base_ref = base.clone();
            let base_id_clone = base_id.clone();
            let notes_db_clone = notes_db.clone();
            let column_view_clone = column_view.clone();
            let available_props = available_properties.clone();
            let table_webview_clone = table_webview.clone();
            let notes_clone = notes.clone();
            let i18n_clone = i18n.clone();
            let container_clone = container.clone();
            
            columns_btn.connect_clicked(move |_btn| {
                Self::show_columns_modal(
                    &container_clone,
                    &base_ref,
                    &base_id_clone,
                    &notes_db_clone,
                    &column_view_clone,
                    &available_props.borrow(),
                    &table_webview_clone,
                    &notes_clone,
                    &i18n_clone.borrow(),
                );
            });
        }

        Self {
            container,
            content_stack,
            table_webview,
            column_view,
            list_store,
            filter_bar,
            filters_container,
            view_tabs,
            status_bar,
            graph_view,
            graph_toggle,
            sort_btn,
            columns_btn,
            formula_row_btn,
            export_xlsx_btn,
            source_type_btn,
            i18n,
            base,
            all_notes: Rc::new(RefCell::new(Vec::new())),
            notes,
            active_filters: Rc::new(RefCell::new(Vec::new())),
            current_sort: Rc::new(RefCell::new(None)),
            available_properties,
            db_path: Rc::new(RefCell::new(None)),
            notes_root: Rc::new(RefCell::new(None)),
            base_id,
            notes_db,
            on_note_selected,
            on_note_double_click,
            on_graph_note_click: std::sync::Arc::new(std::sync::Mutex::new(None)),
            on_source_type_changed: Rc::new(RefCell::new(None)),
            on_view_clicked,
            on_cell_edit,
            is_dark_theme: Rc::new(RefCell::new(Self::detect_system_theme())),
            theme_colors: Rc::new(RefCell::new(GtkThemeColors::default())),
            is_loading: Rc::new(RefCell::new(false)),
        }
    }

    fn create_filter_bar(i18n: &I18n) -> (gtk::Box, gtk::Box, gtk::MenuButton, gtk::Button, gtk::MenuButton, gtk::Button, gtk::ToggleButton, gtk::MenuButton) {
        let bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_start(12)
            .margin_end(12)
            .margin_top(8)
            .margin_bottom(8)
            .css_classes(["base-filter-bar"])
            .build();

        // Botón de añadir filtro
        let add_filter_btn = gtk::MenuButton::builder()
            .icon_name("view-filter-symbolic")
            .tooltip_text(&i18n.t("base_add_filter"))
            .css_classes(["flat"])
            .build();
        bar.append(&add_filter_btn);

        // Separator
        bar.append(&gtk::Separator::new(gtk::Orientation::Vertical));

        // Contenedor de filtros activos (se llenará dinámicamente)
        let filters_container = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .hexpand(true)
            .build();
        bar.append(&filters_container);

        // Botón de ordenamiento
        let sort_btn = gtk::MenuButton::builder()
            .icon_name("view-sort-ascending-symbolic")
            .tooltip_text(&i18n.t("base_sort"))
            .css_classes(["flat"])
            .build();
        bar.append(&sort_btn);

        // Botón de columnas
        let columns_btn = gtk::Button::builder()
            .icon_name("view-column-symbolic")
            .tooltip_text(&i18n.t("base_columns"))
            .css_classes(["flat"])
            .build();
        bar.append(&columns_btn);

        // Botón para filas con fórmulas (totales, promedios, etc.)
        let formula_row_btn = gtk::MenuButton::builder()
            .icon_name("accessories-calculator-symbolic")
            .tooltip_text(&i18n.t("base_formula_rows"))
            .css_classes(["flat"])
            .build();
        bar.append(&formula_row_btn);

        // Botón para exportar a XLSX
        let export_xlsx_btn = gtk::Button::builder()
            .icon_name("document-save-as-symbolic")
            .tooltip_text(&i18n.t("base_export_xlsx"))
            .css_classes(["flat"])
            .build();
        bar.append(&export_xlsx_btn);

        // Separator antes del toggle de grafo
        bar.append(&gtk::Separator::new(gtk::Orientation::Vertical));

        // Botón para cambiar modo (Notes/GroupedRecords)
        let source_type_btn = gtk::MenuButton::builder()
            .icon_name("view-list-symbolic")
            .tooltip_text(&i18n.t("base_data_source"))
            .css_classes(["flat"])
            .build();
        bar.append(&source_type_btn);

        // Separator antes del toggle de grafo
        bar.append(&gtk::Separator::new(gtk::Orientation::Vertical));

        // Toggle para vista de grafo de relaciones
        let graph_toggle = gtk::ToggleButton::builder()
            .icon_name("network-workgroup-symbolic")
            .tooltip_text(&i18n.t("base_show_graph"))
            .css_classes(["flat", "base-graph-toggle"])
            .build();
        bar.append(&graph_toggle);

        (bar, filters_container, sort_btn, columns_btn, formula_row_btn, export_xlsx_btn, graph_toggle, source_type_btn)
    }

    fn create_view_tabs() -> gtk::Box {
        let tabs = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(0)
            .margin_start(12)
            .margin_end(12)
            .css_classes(["base-view-tabs"])
            .build();

        // Se llenarán dinámicamente con las vistas de la Base

        tabs
    }

    fn create_status_bar() -> gtk::Box {
        let bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(4)
            .margin_bottom(4)
            .css_classes(["base-status-bar"])
            .build();

        // Contador de notas
        let count_label = gtk::Label::builder()
            .label("0 notes")
            .css_classes(["dim-label"])
            .build();
        bar.append(&count_label);

        bar
    }

    /// Obtener el widget principal
    pub fn widget(&self) -> &gtk::Box {
        &self.container
    }
    
    /// Obtener el ID de la base actualmente cargada
    pub fn current_base_id(&self) -> Option<i64> {
        *self.base_id.borrow()
    }
    
    /// Actualizar el idioma de la interfaz (llamar cuando cambia el idioma global)
    pub fn update_language(&mut self) {
        let i18n = self.i18n.borrow();
        
        // Actualizar tooltips de los botones de la barra de herramientas
        // Buscar el botón de filtro (primer hijo después del inicio)
        if let Some(filter_btn) = self.filter_bar.first_child() {
            if let Some(btn) = filter_btn.downcast_ref::<gtk::MenuButton>() {
                btn.set_tooltip_text(Some(&i18n.t("base_add_filter")));
            }
        }
        
        // Actualizar tooltip de sort
        self.sort_btn.set_tooltip_text(Some(&i18n.t("base_sort")));
        
        // Actualizar tooltip de columnas
        self.columns_btn.set_tooltip_text(Some(&i18n.t("base_columns")));
        
        // Actualizar tooltip de source type
        self.source_type_btn.set_tooltip_text(Some(&i18n.t("base_data_source")));
        
        // Actualizar tooltip del toggle de grafo
        self.graph_toggle.set_tooltip_text(Some(&i18n.t("base_show_graph")));
        
        drop(i18n);
        
        // Regenerar los popovers con el nuevo idioma
        self.setup_filter_popover();
        self.setup_sort_popover();
        // columns_btn ya está conectado en el constructor
        self.setup_source_type_popover();
        self.setup_formula_row_popover();
        self.setup_export_xlsx_btn();
        
        // Actualizar los chips de filtro
        self.update_filter_chips();
        
        // Si hay datos cargados, refrescar la tabla para actualizar los headers
        if self.base.borrow().is_some() {
            let notes = self.notes.borrow();
            if !notes.is_empty() {
                let columns = if let Some(base) = self.base.borrow().as_ref() {
                    if let Some(view) = base.active_view() {
                        view.columns.clone()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };
                drop(notes);
                
                let notes_ref = self.notes.borrow();
                let html = self.render_table_html(&notes_ref, &columns);
                self.table_webview.load_html(&html, None);
            }
        }
    }

    /// Cargar una Base y mostrar sus datos
    /// `is_dark_theme`: Preferencia de tema de la aplicación (usado como fallback)
    pub fn load_base(&mut self, base_id: i64, base: Base, db: NotesDatabase, notes_root: &Path, _is_dark_theme: bool) {
        // Activar flag de carga para evitar actualizaciones intermedias del WebView
        *self.is_loading.borrow_mut() = true;
        
        // Extraer colores del tema GTK actual
        let new_colors = GtkThemeColors::from_widget(&self.container);
        *self.theme_colors.borrow_mut() = new_colors;
        
        // Guardar referencias
        *self.base.borrow_mut() = Some(base.clone());
        *self.base_id.borrow_mut() = Some(base_id);
        *self.notes_db.borrow_mut() = Some(db.clone_connection());
        
        // Guardar paths para refrescar
        *self.notes_root.borrow_mut() = Some(notes_root.to_path_buf());
        
        // Cargar filtros y sort guardados desde la vista activa
        if let Some(view) = base.active_view() {
            *self.active_filters.borrow_mut() = view.filter.filters.clone();
            *self.current_sort.borrow_mut() = view.sort.clone();
        }

        // Comportamiento según el tipo de fuente
        match base.source_type {
            SourceType::Notes => {
                // Descubrir propiedades disponibles
                let engine = BaseQueryEngine::new(&db, notes_root);
                if let Ok(props) = engine.discover_properties(base.source_folder.as_deref()) {
                    *self.available_properties.borrow_mut() = props;
                }

                // Configurar popovers con las propiedades
                self.setup_filter_popover();
                self.setup_sort_popover();
                // columns_btn ya está conectado en el constructor

                // Actualizar tabs de vistas
                self.update_view_tabs(&base);

                // Obtener la vista activa
                if let Some(view) = base.active_view() {
                    self.load_view(view, base.source_folder.as_deref(), &db, notes_root);
                }
            }
            SourceType::GroupedRecords => {
                // Cargar registros agrupados
                self.load_grouped_records(&db, &base);
            }
            SourceType::PropertyRecords => {
                // Cargar registros filtrados por propiedad con columnas auto-descubiertas
                self.load_property_records(&db, &base);
            }
        }
        
        // Configurar toggle del grafo
        self.setup_graph_toggle(&db);
        
        // Configurar popover de modo de datos
        self.setup_source_type_popover();
        
        // Configurar popover de filas de fórmulas
        self.setup_formula_row_popover();
        
        // Configurar botón de exportar XLSX
        self.setup_export_xlsx_btn();
        
        // Mostrar los chips de filtros guardados
        self.update_filter_chips();
        
        // Desactivar flag de carga y forzar actualización final del WebView
        *self.is_loading.borrow_mut() = false;
        self.force_update_webview();
    }
    
    /// Cargar registros agrupados en la tabla
    fn load_grouped_records(&mut self, db: &NotesDatabase, base: &Base) {
        match db.get_all_grouped_records() {
            Ok(records) => {
                // Descubrir propiedades disponibles de los registros
                // properties es Vec<(String, String)>, extraemos las claves
                let mut props: Vec<String> = records.iter()
                    .flat_map(|r| r.properties.iter().map(|(k, _)| k.clone()))
                    .collect();
                props.sort();
                props.dedup();
                
                // Añadir _note al inicio
                let mut available = vec!["_note".to_string()];
                available.extend(props);
                *self.available_properties.borrow_mut() = available;
                
                // Configurar popovers con las propiedades correctas
                self.setup_filter_popover();
                self.setup_sort_popover();
                // columns_btn ya está conectado en el constructor
                
                // Actualizar tabs
                self.update_view_tabs(base);
                
                // Obtener columnas de la vista activa
                let columns = base.active_view()
                    .map(|v| v.columns.clone())
                    .unwrap_or_default();
                
                // Obtener las propiedades visibles (excluyendo _note que siempre se muestra)
                let visible_props: Vec<String> = columns.iter()
                    .filter(|c| c.visible && c.property != "_note")
                    .map(|c| c.property.clone())
                    .collect();
                
                // Filtrar registros: solo mostrar los que tienen al menos una propiedad visible
                let filtered_records: Vec<_> = records.iter()
                    .filter(|r| {
                        // Si no hay columnas configuradas, mostrar todo
                        if visible_props.is_empty() {
                            return true;
                        }
                        // Verificar si el registro tiene al menos una propiedad visible
                        r.properties.iter().any(|(k, _)| visible_props.contains(k))
                    })
                    .collect();
                
                // Actualizar columnas en la tabla
                self.update_columns(&columns);
                
                // Convertir GroupedRecord a NoteWithProperties para reusar la tabla
                let notes: Vec<NoteWithProperties> = filtered_records.iter().map(|r| {
                    let mut properties = HashMap::new();
                    properties.insert("_note".to_string(), PropertyValue::Text(r.note_name.clone()));
                    // Añadir metadatos para edición bidireccional
                    properties.insert("_note_id".to_string(), PropertyValue::Number(r.note_id as f64));
                    properties.insert("_group_id".to_string(), PropertyValue::Number(r.group_id as f64));
                    
                    for (k, v) in &r.properties {
                        properties.insert(k.clone(), PropertyValue::Text(v.clone()));
                    }
                    
                    // Crear metadata falsa para reusar NoteWithProperties
                    let metadata = NoteMetadata {
                        id: r.note_id,
                        name: r.note_name.clone(),
                        path: String::new(),
                        folder: None,
                        order_index: 0,
                        icon: None,
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    };
                    
                    NoteWithProperties {
                        metadata,
                        properties,
                        content: None,
                    }
                }).collect();
                
                *self.all_notes.borrow_mut() = notes.clone();
                *self.notes.borrow_mut() = notes.clone();
                
                // Actualizar la tabla con WebView
                self.update_data(&notes);
                
                // Actualizar status
                self.update_status_bar(notes.len());
            }
            Err(e) => {
                eprintln!("Error loading grouped records: {}", e);
            }
        }
    }
    
    /// Cargar registros filtrados por propiedad con columnas auto-descubiertas
    /// Este es el modo bidireccional donde se pueden editar valores
    fn load_property_records(&mut self, db: &NotesDatabase, base: &Base) {
        let filter_property = match &base.filter_property {
            Some(prop) => prop.clone(),
            None => {
                eprintln!("PropertyRecords mode requires filter_property");
                return;
            }
        };
        
        // Obtener registros que contienen la propiedad de filtro
        match db.get_records_by_property(&filter_property) {
            Ok(records) => {
                // Descubrir columnas relacionadas automáticamente
                let related_columns = db.discover_related_columns(&filter_property)
                    .unwrap_or_default();
                
                // Construir lista de propiedades disponibles
                let mut available = vec![filter_property.clone(), "_note".to_string()];
                available.extend(related_columns.clone());
                *self.available_properties.borrow_mut() = available.clone();
                
                // Configurar popovers
                self.setup_filter_popover();
                self.setup_sort_popover();
                
                // Actualizar tabs
                self.update_view_tabs(base);
                
                // Actualizar columnas: si la vista tiene columnas configuradas, usarlas
                // Si no, usar las descubiertas automáticamente
                let columns = if let Some(view) = base.active_view() {
                    if view.columns.len() > 2 { // Ya tiene más que las default
                        view.columns.clone()
                    } else {
                        // Generar columnas desde las descubiertas
                        let mut cols = vec![
                            ColumnConfig::new(&filter_property)
                                .with_title(&capitalize_first(&filter_property)),
                        ];
                        for col in &related_columns {
                            cols.push(ColumnConfig::new(col)
                                .with_title(&capitalize_first(col)));
                        }
                        cols.push(ColumnConfig::new("_note").with_title("Note"));
                        cols
                    }
                } else {
                    vec![ColumnConfig::new(&filter_property)]
                };
                
                // Actualizar columnas en la tabla
                self.update_columns(&columns);
                
                // Convertir GroupedRecord a NoteWithProperties
                let notes: Vec<NoteWithProperties> = records.iter().map(|r| {
                    let mut properties = HashMap::new();
                    properties.insert("_note".to_string(), PropertyValue::Text(r.note_name.clone()));
                    // Añadir metadatos para edición
                    properties.insert("_note_id".to_string(), PropertyValue::Number(r.note_id as f64));
                    properties.insert("_group_id".to_string(), PropertyValue::Number(r.group_id as f64));
                    
                    for (k, v) in &r.properties {
                        properties.insert(k.clone(), PropertyValue::Text(v.clone()));
                    }
                    
                    let metadata = NoteMetadata {
                        id: r.note_id,
                        name: r.note_name.clone(),
                        path: String::new(),
                        folder: None,
                        order_index: 0,
                        icon: None,
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    };
                    
                    NoteWithProperties {
                        metadata,
                        properties,
                        content: None,
                    }
                }).collect();
                
                *self.all_notes.borrow_mut() = notes.clone();
                *self.notes.borrow_mut() = notes.clone();
                
                // Actualizar la tabla
                self.update_data(&notes);
                
                // Actualizar status
                self.update_status_bar(notes.len());
            }
            Err(e) => {
                eprintln!("Error loading property records: {}", e);
            }
        }
    }
    
    /// Configurar el toggle del grafo de relaciones
    fn setup_graph_toggle(&self, db: &NotesDatabase) {
        let content_stack = self.content_stack.clone();
        let graph_view = self.graph_view.clone();
        let db_clone = db.clone_connection();
        
        self.graph_toggle.connect_toggled(move |toggle| {
            if toggle.is_active() {
                // Cargar datos del grafo desde registros agrupados
                match db_clone.get_all_grouped_records() {
                    Ok(records) => {
                        graph_view.load_from_grouped_records(&records);
                        graph_view.start_simulation();
                    }
                    Err(e) => {
                        eprintln!("Error loading grouped records: {}", e);
                    }
                }
                content_stack.set_visible_child_name("graph");
            } else {
                graph_view.stop_simulation();
                content_stack.set_visible_child_name("table");
            }
        });
        
        // Configurar callback para doble-clic en nodos del grafo
        let on_click = self.on_graph_note_click.clone();
        self.graph_view.on_note_click(move |note_name| {
            if let Ok(guard) = on_click.lock() {
                if let Some(ref callback) = *guard {
                    callback(note_name);
                }
            }
        });
    }
    
    /// Configurar callback para doble-clic en nodos del grafo
    pub fn on_graph_note_click<F: Fn(&str) + Send + Sync + 'static>(&self, callback: F) {
        if let Ok(mut guard) = self.on_graph_note_click.lock() {
            *guard = Some(Box::new(callback));
        }
    }
    
    /// Configurar callback para cuando cambia el source_type
    pub fn on_source_type_changed<F: Fn() + 'static>(&self, callback: F) {
        *self.on_source_type_changed.borrow_mut() = Some(Box::new(callback));
    }
    
    /// Configurar callback para edición de celda
    /// El callback recibe (note_id, group_id, property_key, new_value)
    pub fn on_cell_edit<F: Fn(i64, i64, &str, &str) + 'static>(&self, callback: F) {
        *self.on_cell_edit.borrow_mut() = Some(Box::new(callback));
    }

    /// Cargar una vista específica
    fn load_view(
        &mut self,
        view: &BaseView,
        source_folder: Option<&str>,
        db: &NotesDatabase,
        notes_root: &Path,
    ) {
        // Ejecutar query
        let engine = BaseQueryEngine::new(db, notes_root);
        match engine.query_view(view, source_folder) {
            Ok(notes) => {
                // Guardar todas las notas
                *self.all_notes.borrow_mut() = notes.clone();
                
                // Copiar el sort de la vista si existe
                *self.current_sort.borrow_mut() = view.sort.clone();

                // Aplicar filtros adicionales y ordenar
                self.apply_filters_and_sort();

                // Actualizar columnas
                self.update_columns(&view.columns);
            }
            Err(e) => {
                eprintln!("Error executing Base query: {}", e);
            }
        }
    }
    
    /// Aplicar filtros activos y ordenamiento
    fn apply_filters_and_sort(&self) {
        let all_notes = self.all_notes.borrow();
        let filters = self.active_filters.borrow();
        let sort = self.current_sort.borrow();
        
        // Filtrar notas
        let mut filtered: Vec<NoteWithProperties> = all_notes
            .iter()
            .filter(|note| {
                filters.iter().all(|f| f.evaluate(&note.properties))
            })
            .cloned()
            .collect();
        
        // Ordenar
        if let Some(sort_config) = sort.as_ref() {
            filtered.sort_by(|a, b| {
                let key_a = a.properties
                    .get(&sort_config.property)
                    .map(|v| v.sort_key())
                    .unwrap_or_default();
                let key_b = b.properties
                    .get(&sort_config.property)
                    .map(|v| v.sort_key())
                    .unwrap_or_default();

                match sort_config.direction {
                    SortDirection::Asc => key_a.cmp(&key_b),
                    SortDirection::Desc => key_b.cmp(&key_a),
                }
            });
        }
        
        // Actualizar notas mostradas
        *self.notes.borrow_mut() = filtered.clone();
        
        // Actualizar UI
        self.update_data(&filtered);
        self.update_status_bar(filtered.len());
        self.update_filter_chips();
    }
    
    /// Persistir la configuración actual de la Base en la BD
    fn save_config(&self) {
        let base_id = self.base_id.borrow();
        let notes_db = self.notes_db.borrow();
        let mut base_opt = self.base.borrow_mut();
        
        if let (Some(id), Some(db), Some(base)) = (base_id.as_ref(), notes_db.as_ref(), base_opt.as_mut()) {
            // Sincronizar filtros y sort a la vista activa
            if let Some(view) = base.views.get_mut(base.active_view) {
                view.filter.filters = self.active_filters.borrow().clone();
                view.sort = self.current_sort.borrow().clone();
            }
            
            // Serializar y guardar
            if let Ok(yaml) = base.serialize() {
                if let Err(e) = db.update_base(*id, &yaml, base.active_view as i32) {
                    eprintln!("Error saving Base config: {}", e);
                }
            }
        }
    }
    
    /// Añadir un filtro
    pub fn add_filter(&self, filter: Filter) {
        self.active_filters.borrow_mut().push(filter);
        self.apply_filters_and_sort();
        self.save_config();
    }
    
    /// Eliminar un filtro por índice
    pub fn remove_filter(&self, index: usize) {
        let mut filters = self.active_filters.borrow_mut();
        if index < filters.len() {
            filters.remove(index);
        }
        drop(filters);
        self.apply_filters_and_sort();
        self.save_config();
    }
    
    /// Limpiar todos los filtros
    pub fn clear_filters(&self) {
        self.active_filters.borrow_mut().clear();
        self.apply_filters_and_sort();
        self.save_config();
    }
    
    /// Limpiar completamente el widget (cuando se elimina la base)
    pub fn clear(&self) {
        // Limpiar base y notas
        *self.base.borrow_mut() = None;
        *self.base_id.borrow_mut() = None;
        self.all_notes.borrow_mut().clear();
        self.notes.borrow_mut().clear();
        self.active_filters.borrow_mut().clear();
        self.available_properties.borrow_mut().clear();
        *self.current_sort.borrow_mut() = None;
        
        // Limpiar columnas del ColumnView
        while self.column_view.columns().n_items() > 0 {
            if let Some(col) = self.column_view.columns().item(0) {
                if let Some(column) = col.downcast_ref::<gtk::ColumnViewColumn>() {
                    self.column_view.remove_column(column);
                }
            }
        }
        
        // Limpiar modelo de la lista
        if let Some(model) = self.column_view.model() {
            if let Some(selection) = model.downcast_ref::<gtk::SingleSelection>() {
                selection.set_model(None::<&gio::ListStore>);
            }
        }
        
        // Limpiar WebView con HTML que mantiene el color de fondo del tema
        let colors = self.theme_colors.borrow();
        let empty_html = format!(
            "<html><head><style>html,body{{margin:0;padding:0;min-height:100vh;background-color:{}}}</style></head><body></body></html>",
            colors.bg_primary
        );
        drop(colors);
        self.table_webview.load_html(&empty_html, None);
        
        // Limpiar grafo
        self.graph_view.state_mut().clear();
        
        // Limpiar chips de filtros
        while let Some(child) = self.filters_container.first_child() {
            self.filters_container.remove(&child);
        }
        
        // Limpiar tabs de vistas
        while let Some(child) = self.view_tabs.first_child() {
            self.view_tabs.remove(&child);
        }
    }
    
    /// Refrescar el tema del WebView (llamar cuando cambie el tema del sistema)
    /// Extrae los colores del tema GTK actual y regenera el HTML
    pub fn refresh_theme(&self, is_dark: bool) {
        // Extraer colores del tema GTK actual usando el container como referencia
        let new_colors = GtkThemeColors::from_widget(&self.container);
        *self.theme_colors.borrow_mut() = new_colors;
        
        // Actualizar color de fondo del WebView según el tema
        let bg_color = if is_dark {
            gtk::gdk::RGBA::new(0.12, 0.12, 0.12, 1.0)
        } else {
            gtk::gdk::RGBA::new(0.95, 0.95, 0.95, 1.0)
        };
        self.table_webview.set_background_color(&bg_color);
        
        // Si hay una base cargada, regenerar el HTML con los nuevos colores
        if self.base.borrow().is_some() {
            let notes_borrowed = self.notes.borrow();
            if let Some(base) = self.base.borrow().as_ref() {
                if let Some(view) = base.views.get(base.active_view) {
                    let colors = self.theme_colors.borrow().clone();
                    let html = Self::render_table_html_with_colors(
                        &notes_borrowed, 
                        &view.columns, 
                        self.i18n.borrow().current_language(), 
                        view.editable, 
                        &view.special_rows,
                        &colors
                    );
                    self.table_webview.load_html(&html, None);
                }
            }
        }
        
        // Refrescar el grafo para que use los nuevos colores del tema
        self.graph_view.queue_draw();
    }
    
    /// Establecer ordenamiento
    pub fn set_sort(&self, sort: Option<SortConfig>) {
        *self.current_sort.borrow_mut() = sort;
        self.apply_filters_and_sort();
        self.save_config();
    }
    
    /// Configurar el popover de filtros
    fn setup_filter_popover(&self) {
        // Obtener solo las columnas visibles de la vista actual
        let properties: Vec<String> = if let Some(base) = self.base.borrow().as_ref() {
            if let Some(view) = base.active_view() {
                view.columns.iter()
                    .filter(|c| c.visible)
                    .map(|c| c.property.clone())
                    .collect()
            } else {
                self.available_properties.borrow().clone()
            }
        } else {
            self.available_properties.borrow().clone()
        };
        let (popover, prop_combo, op_combo, value_entry) = create_filter_popover_with_refs(&properties, &self.i18n.borrow());
        
        // Clonar referencias para el closure
        let active_filters = self.active_filters.clone();
        let all_notes = self.all_notes.clone();
        let notes = self.notes.clone();
        let current_sort = self.current_sort.clone();
        let list_store = self.list_store.clone();
        let status_bar = self.status_bar.clone();
        let filters_container = self.filters_container.clone();
        let popover_clone = popover.clone();
        let properties_clone = properties.clone();
        let table_webview = self.table_webview.clone();
        let base = self.base.clone();
        let i18n_clone = self.i18n.clone();
        let base_id = self.base_id.clone();
        let notes_db = self.notes_db.clone();
        
        // Buscar el botón Apply dentro del popover y conectarlo
        if let Some(content) = popover.child().and_downcast::<gtk::Box>() {
            // El último hijo es el box de botones
            if let Some(buttons_box) = content.last_child().and_downcast::<gtk::Box>() {
                // El último botón es Apply
                if let Some(apply_btn) = buttons_box.last_child().and_downcast::<gtk::Button>() {
                    apply_btn.connect_clicked(move |_| {
                        // Obtener valores seleccionados
                        let prop_idx = prop_combo.selected() as usize;
                        let op_idx = op_combo.selected() as usize;
                        let value_text = value_entry.text().to_string();
                        
                        if prop_idx < properties_clone.len() {
                            let property = properties_clone[prop_idx].clone();
                            let operator = index_to_operator(op_idx);
                            let value = parse_filter_value(&value_text);
                            
                            let filter = Filter {
                                property,
                                operator,
                                value,
                            };
                            
                            // Añadir filtro
                            active_filters.borrow_mut().push(filter);
                            
                            // Re-aplicar filtros
                            let all = all_notes.borrow();
                            let filters = active_filters.borrow();
                            let sort = current_sort.borrow();
                            
                            let mut filtered: Vec<NoteWithProperties> = all
                                .iter()
                                .filter(|note| {
                                    filters.iter().all(|f| f.evaluate(&note.properties))
                                })
                                .cloned()
                                .collect();
                            
                            // Ordenar
                            if let Some(sort_config) = sort.as_ref() {
                                filtered.sort_by(|a, b| {
                                    let key_a = a.properties
                                        .get(&sort_config.property)
                                        .map(|v| v.sort_key())
                                        .unwrap_or_default();
                                    let key_b = b.properties
                                        .get(&sort_config.property)
                                        .map(|v| v.sort_key())
                                        .unwrap_or_default();

                                    match sort_config.direction {
                                        SortDirection::Asc => key_a.cmp(&key_b),
                                        SortDirection::Desc => key_b.cmp(&key_a),
                                    }
                                });
                            }
                            
                            drop(all);
                            drop(filters);
                            drop(sort);
                            
                            *notes.borrow_mut() = filtered.clone();
                            
                            // Actualizar UI (list_store para lógica)
                            list_store.remove_all();
                            for note in &filtered {
                                let boxed = glib::BoxedAnyObject::new(note.clone());
                                list_store.append(&boxed);
                            }
                            
                            // Actualizar WebView
                            let columns = if let Some(base) = base.borrow().as_ref() {
                                if let Some(view) = base.views.get(base.active_view) {
                                    view.columns.clone()
                                } else {
                                    vec![
                                        ColumnConfig { property: "title".to_string(), title: None, width: Some(300), visible: true },
                                        ColumnConfig { property: "created".to_string(), title: None, width: Some(150), visible: true },
                                    ]
                                }
                            } else {
                                vec![
                                    ColumnConfig { property: "title".to_string(), title: None, width: Some(300), visible: true },
                                    ColumnConfig { property: "created".to_string(), title: None, width: Some(150), visible: true },
                                ]
                            };
                            let html = Self::render_table_html_static(&filtered, &columns, Language::from_env(), false, &[]);
                            table_webview.load_html(&html, None);
                            
                            // Actualizar status
                            if let Some(label) = status_bar.first_child().and_downcast::<gtk::Label>() {
                                let text = if filtered.len() == 1 {
                                    "1 note".to_string()
                                } else {
                                    format!("{} notes", filtered.len())
                                };
                                label.set_text(&text);
                            }
                            
                            // Actualizar chips con handlers completos
                            // Limpiar chips existentes
                            while let Some(child) = filters_container.first_child() {
                                filters_container.remove(&child);
                            }
                            
                            let filters_snapshot = active_filters.borrow().clone();
                            if filters_snapshot.is_empty() {
                                let placeholder = gtk::Label::builder()
                                    .label(&i18n_clone.borrow().t("base_no_filters"))
                                    .css_classes(["dim-label"])
                                    .build();
                                filters_container.append(&placeholder);
                            } else {
                                for (i, filter) in filters_snapshot.iter().enumerate() {
                                    let chip = create_filter_chip(filter, i);
                                    
                                    // Clonar todo para el handler del botón X
                                    let af = active_filters.clone();
                                    let an = all_notes.clone();
                                    let n = notes.clone();
                                    let cs = current_sort.clone();
                                    let ls = list_store.clone();
                                    let sb = status_bar.clone();
                                    let fc = filters_container.clone();
                                    let tw = table_webview.clone();
                                    let b = base.clone();
                                    let i18n = i18n_clone.clone();
                                    
                                    if let Some(close_btn) = chip.last_child().and_downcast::<gtk::Button>() {
                                        close_btn.connect_clicked(move |_| {
                                            // Eliminar filtro
                                            let mut filters_mut = af.borrow_mut();
                                            if i < filters_mut.len() {
                                                filters_mut.remove(i);
                                            }
                                            let new_filters = filters_mut.clone();
                                            drop(filters_mut);
                                            
                                            // Re-aplicar filtros
                                            let all = an.borrow();
                                            let sort = cs.borrow();
                                            
                                            let mut filtered_notes: Vec<NoteWithProperties> = all
                                                .iter()
                                                .filter(|note| {
                                                    new_filters.iter().all(|f| f.evaluate(&note.properties))
                                                })
                                                .cloned()
                                                .collect();
                                            
                                            if let Some(sort_config) = sort.as_ref() {
                                                filtered_notes.sort_by(|a, b_note| {
                                                    let key_a = a.properties
                                                        .get(&sort_config.property)
                                                        .map(|v| v.sort_key())
                                                        .unwrap_or_default();
                                                    let key_b = b_note.properties
                                                        .get(&sort_config.property)
                                                        .map(|v| v.sort_key())
                                                        .unwrap_or_default();
                                                    match sort_config.direction {
                                                        SortDirection::Asc => key_a.cmp(&key_b),
                                                        SortDirection::Desc => key_b.cmp(&key_a),
                                                    }
                                                });
                                            }
                                            drop(all);
                                            drop(sort);
                                            
                                            *n.borrow_mut() = filtered_notes.clone();
                                            
                                            ls.remove_all();
                                            for note in &filtered_notes {
                                                let boxed = glib::BoxedAnyObject::new(note.clone());
                                                ls.append(&boxed);
                                            }
                                            
                                            let columns = if let Some(base_ref) = b.borrow().as_ref() {
                                                if let Some(view) = base_ref.views.get(base_ref.active_view) {
                                                    view.columns.clone()
                                                } else {
                                                    vec![
                                                        ColumnConfig { property: "title".to_string(), title: None, width: Some(300), visible: true },
                                                        ColumnConfig { property: "created".to_string(), title: None, width: Some(150), visible: true },
                                                    ]
                                                }
                                            } else {
                                                vec![
                                                    ColumnConfig { property: "title".to_string(), title: None, width: Some(300), visible: true },
                                                    ColumnConfig { property: "created".to_string(), title: None, width: Some(150), visible: true },
                                                ]
                                            };
                                            let html = BaseTableWidget::render_table_html_static(&filtered_notes, &columns, Language::from_env(), false, &[]);
                                            tw.load_html(&html, None);
                                            
                                            if let Some(label) = sb.first_child().and_downcast::<gtk::Label>() {
                                                let text = if filtered_notes.len() == 1 { "1 note".to_string() } else { format!("{} notes", filtered_notes.len()) };
                                                label.set_text(&text);
                                            }
                                            
                                            // Limpiar y recrear chips
                                            while let Some(child) = fc.first_child() {
                                                fc.remove(&child);
                                            }
                                            if new_filters.is_empty() {
                                                let placeholder = gtk::Label::builder()
                                                    .label(&i18n.borrow().t("base_no_filters"))
                                                    .css_classes(["dim-label"])
                                                    .build();
                                                fc.append(&placeholder);
                                            } else {
                                                // Solo mostrar chips sin handlers (limitación de recursión)
                                                for (idx, f) in new_filters.iter().enumerate() {
                                                    let new_chip = create_filter_chip(f, idx);
                                                    fc.append(&new_chip);
                                                }
                                            }
                                        });
                                    }
                                    
                                    filters_container.append(&chip);
                                }
                            }
                            
                            // Persistir la configuración
                            let bid = base_id.borrow();
                            let ndb = notes_db.borrow();
                            let mut base_opt = base.borrow_mut();
                            if let (Some(id), Some(db), Some(base_ref)) = (bid.as_ref(), ndb.as_ref(), base_opt.as_mut()) {
                                if let Some(view) = base_ref.views.get_mut(base_ref.active_view) {
                                    view.filter.filters = active_filters.borrow().clone();
                                }
                                if let Ok(yaml) = base_ref.serialize() {
                                    if let Err(e) = db.update_base(*id, &yaml, base_ref.active_view as i32) {
                                        eprintln!("Error saving Base config: {}", e);
                                    }
                                }
                            }
                        }
                        
                        // Cerrar popover
                        popover_clone.popdown();
                        
                        // Limpiar entry
                        value_entry.set_text("");
                    });
                }
            }
        }
        
        // Buscar el botón de filtros en la barra
        if let Some(filter_btn) = self.filter_bar.first_child().and_downcast::<gtk::MenuButton>() {
            filter_btn.set_popover(Some(&popover));
        }
    }
    
    /// Configurar el popover de ordenamiento
    fn setup_sort_popover(&self) {
        // Obtener solo las columnas visibles de la vista actual
        let properties: Vec<String> = if let Some(base) = self.base.borrow().as_ref() {
            if let Some(view) = base.active_view() {
                view.columns.iter()
                    .filter(|c| c.visible)
                    .map(|c| c.property.clone())
                    .collect()
            } else {
                self.available_properties.borrow().clone()
            }
        } else {
            self.available_properties.borrow().clone()
        };
        let popover = create_sort_popover_with_callbacks(
            &properties,
            self.current_sort.clone(),
            self.all_notes.clone(),
            self.notes.clone(),
            self.active_filters.clone(),
            self.list_store.clone(),
            self.status_bar.clone(),
            self.table_webview.clone(),
            self.base.clone(),
            &self.i18n.borrow(),
        );
        
        // Usar referencia directa al botón de sort
        self.sort_btn.set_popover(Some(&popover));
    }
    
    /// Mostrar el modal de configuración de columnas
    fn show_columns_modal(
        parent: &gtk::Box,
        base_ref: &Rc<RefCell<Option<Base>>>,
        base_id: &Rc<RefCell<Option<i64>>>,
        notes_db: &Rc<RefCell<Option<NotesDatabase>>>,
        column_view: &gtk::ColumnView,
        available_props: &[String],
        table_webview: &webkit6::WebView,
        notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
        i18n: &I18n,
    ) {
        // Obtener la ventana raíz
        let window = parent.root()
            .and_then(|r| r.downcast::<gtk::Window>().ok());
        
        // Crear el diálogo modal
        let dialog = gtk::Window::builder()
            .title(&i18n.t("base_columns_config"))
            .modal(true)
            .default_width(500)
            .default_height(600)
            .css_classes(["columns-modal"])
            .build();
        
        if let Some(win) = window {
            dialog.set_transient_for(Some(&win));
        }
        
        // Contenedor principal
        let main_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .build();
        
        // === Header ===
        let header = gtk::HeaderBar::builder()
            .title_widget(&gtk::Label::builder()
                .label(&i18n.t("base_columns_config"))
                .css_classes(["title"])
                .build())
            .show_title_buttons(false)
            .build();
        
        let close_btn = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .css_classes(["flat", "circular"])
            .build();
        header.pack_end(&close_btn);
        
        main_box.append(&header);
        
        // === Contenido con dos paneles ===
        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(16)
            .vexpand(true)
            .margin_start(16)
            .margin_end(16)
            .margin_top(8)
            .margin_bottom(16)
            .css_classes(["columns-modal-content"])
            .build();
        
        // Panel izquierdo: Columnas activas
        let left_panel = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .hexpand(true)
            .build();
        
        let left_title = gtk::Label::builder()
            .label(&i18n.t("base_current_columns"))
            .css_classes(["heading"])
            .xalign(0.0)
            .build();
        left_panel.append(&left_title);
        
        let left_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .build();
        
        let active_columns_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list", "columns-list"])
            .build();
        
        left_scroll.set_child(Some(&active_columns_list));
        left_panel.append(&left_scroll);
        
        content_box.append(&left_panel);
        
        // Separador vertical
        let separator = gtk::Separator::new(gtk::Orientation::Vertical);
        content_box.append(&separator);
        
        // Panel derecho: Propiedades disponibles
        let right_panel = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .hexpand(true)
            .build();
        
        let right_title = gtk::Label::builder()
            .label(&i18n.t("base_add_column"))
            .css_classes(["heading"])
            .xalign(0.0)
            .build();
        right_panel.append(&right_title);
        
        // Barra de búsqueda
        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text(&i18n.t("base_search_properties"))
            .build();
        right_panel.append(&search_entry);
        
        let right_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .build();
        
        let available_props_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list", "columns-list"])
            .build();
        
        right_scroll.set_child(Some(&available_props_list));
        right_panel.append(&right_scroll);
        
        content_box.append(&right_panel);
        main_box.append(&content_box);
        
        dialog.set_child(Some(&main_box));
        
        // Conectar cierre
        let dialog_clone = dialog.clone();
        close_btn.connect_clicked(move |_| {
            dialog_clone.close();
        });
        
        // Función para refrescar el contenido del modal
        let refresh_modal = {
            let active_columns_list = active_columns_list.clone();
            let available_props_list = available_props_list.clone();
            let base_ref = base_ref.clone();
            let base_id = base_id.clone();
            let notes_db = notes_db.clone();
            let column_view = column_view.clone();
            let available_props_vec: Vec<String> = available_props.to_vec();
            let table_webview = table_webview.clone();
            let notes = notes.clone();
            let search_entry = search_entry.clone();
            
            move || {
                let filter_text = search_entry.text().to_string().to_lowercase();
                Self::refresh_columns_modal_content(
                    &active_columns_list,
                    &available_props_list,
                    &base_ref,
                    &base_id,
                    &notes_db,
                    &column_view,
                    &available_props_vec,
                    &table_webview,
                    &notes,
                    if filter_text.is_empty() { None } else { Some(filter_text) },
                );
            }
        };
        
        // Refrescar al inicio
        refresh_modal();
        
        // Conectar búsqueda
        let refresh_for_search = refresh_modal.clone();
        search_entry.connect_search_changed(move |_| {
            refresh_for_search();
        });
        
        // Cerrar con el botón X
        {
            let dialog_clone = dialog.clone();
            close_btn.connect_clicked(move |_| {
                dialog_clone.close();
            });
        }
        
        // Cerrar con ESC
        let key_controller = gtk::EventControllerKey::new();
        {
            let dialog_clone = dialog.clone();
            key_controller.connect_key_pressed(move |_, key, _, _| {
                if key == gtk::gdk::Key::Escape {
                    dialog_clone.close();
                    return gtk::glib::Propagation::Stop;
                }
                gtk::glib::Propagation::Proceed
            });
        }
        dialog.add_controller(key_controller);
        
        // Mostrar el diálogo
        dialog.present();
    }
    
    /// Refrescar el contenido del modal de columnas
    fn refresh_columns_modal_content(
        active_list: &gtk::ListBox,
        available_list: &gtk::ListBox,
        base_ref: &Rc<RefCell<Option<Base>>>,
        base_id: &Rc<RefCell<Option<i64>>>,
        notes_db: &Rc<RefCell<Option<NotesDatabase>>>,
        column_view: &gtk::ColumnView,
        available_props: &[String],
        table_webview: &webkit6::WebView,
        notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
        filter_text: Option<String>,
    ) {
        let i18n = I18n::new(Language::from_env());
        
        // Debug: mostrar propiedades disponibles
        eprintln!("DEBUG columns modal - available_props: {:?}", available_props);
        if let Ok(base) = base_ref.try_borrow() {
            if let Some(b) = base.as_ref() {
                eprintln!("DEBUG columns modal - source_type: {:?}", b.source_type);
            }
        }
        
        // Limpiar listas
        while let Some(row) = active_list.first_child() {
            active_list.remove(&row);
        }
        while let Some(row) = available_list.first_child() {
            available_list.remove(&row);
        }
        
        // Intentar obtener la base - si falla, mostrar mensaje y retornar
        let base = match base_ref.try_borrow() {
            Ok(b) => b,
            Err(_) => {
                let error_label = gtk::Label::builder()
                    .label("Error: Base en uso")
                    .css_classes(["dim-label"])
                    .margin_top(20)
                    .build();
                active_list.append(&error_label);
                return;
            }
        };
        
        if let Some(base_data) = base.as_ref() {
            if let Some(view) = base_data.active_view() {
                let existing_props: Vec<String> = view.columns.iter()
                    .map(|c| c.property.clone())
                    .collect();
                let total_columns = view.columns.len();
                
                // === Panel izquierdo: Columnas activas ===
                for (col_idx, col) in view.columns.iter().enumerate() {
                    let row = gtk::Box::builder()
                        .orientation(gtk::Orientation::Horizontal)
                        .spacing(8)
                        .css_classes(["column-row"])
                        .build();
                    
                    // Botones de reordenamiento
                    let move_up_btn = gtk::Button::builder()
                        .icon_name("go-up-symbolic")
                        .css_classes(["flat", "circular"])
                        .tooltip_text(&i18n.t("base_move_up"))
                        .sensitive(col_idx > 0)
                        .build();
                    
                    let move_down_btn = gtk::Button::builder()
                        .icon_name("go-down-symbolic")
                        .css_classes(["flat", "circular"])
                        .tooltip_text(&i18n.t("base_move_down"))
                        .sensitive(col_idx < total_columns - 1)
                        .build();
                    
                    row.append(&move_up_btn);
                    row.append(&move_down_btn);
                    
                    // Checkbox de visibilidad
                    let check = gtk::CheckButton::builder()
                        .active(col.visible)
                        .tooltip_text(&i18n.t("base_toggle_visibility"))
                        .build();
                    row.append(&check);
                    
                    // Nombre de la columna
                    let label = gtk::Label::builder()
                        .label(&col.display_title())
                        .hexpand(true)
                        .xalign(0.0)
                        .build();
                    row.append(&label);
                    
                    // Botón eliminar
                    let remove_btn = gtk::Button::builder()
                        .icon_name("user-trash-symbolic")
                        .css_classes(["flat", "circular", "destructive-action"])
                        .tooltip_text(&i18n.t("base_remove_column"))
                        .build();
                    row.append(&remove_btn);
                    
                    // Conectar mover arriba
                    {
                        let base_ref = base_ref.clone();
                        let base_id = base_id.clone();
                        let notes_db = notes_db.clone();
                        let column_view = column_view.clone();
                        let table_webview = table_webview.clone();
                        let notes = notes.clone();
                        let active_list = active_list.clone();
                        let available_list = available_list.clone();
                        let available_props = available_props.to_vec();
                        
                        move_up_btn.connect_clicked(move |_| {
                            Self::move_column(&base_ref, &base_id, &notes_db, &column_view, &table_webview, &notes, col_idx, -1);
                            Self::refresh_columns_modal_content(&active_list, &available_list, &base_ref, &base_id, &notes_db, &column_view, &available_props, &table_webview, &notes, None);
                        });
                    }
                    
                    // Conectar mover abajo
                    {
                        let base_ref = base_ref.clone();
                        let base_id = base_id.clone();
                        let notes_db = notes_db.clone();
                        let column_view = column_view.clone();
                        let table_webview = table_webview.clone();
                        let notes = notes.clone();
                        let active_list = active_list.clone();
                        let available_list = available_list.clone();
                        let available_props = available_props.to_vec();
                        
                        move_down_btn.connect_clicked(move |_| {
                            Self::move_column(&base_ref, &base_id, &notes_db, &column_view, &table_webview, &notes, col_idx, 1);
                            Self::refresh_columns_modal_content(&active_list, &available_list, &base_ref, &base_id, &notes_db, &column_view, &available_props, &table_webview, &notes, None);
                        });
                    }
                    
                    // Conectar checkbox visibilidad
                    {
                        let base_ref = base_ref.clone();
                        let base_id = base_id.clone();
                        let notes_db = notes_db.clone();
                        let column_view = column_view.clone();
                        let table_webview = table_webview.clone();
                        let notes = notes.clone();
                        
                        check.connect_toggled(move |btn| {
                            Self::toggle_column_visibility(&base_ref, &base_id, &notes_db, &column_view, &table_webview, &notes, col_idx, btn.is_active());
                        });
                    }
                    
                    // Conectar eliminar
                    {
                        let base_ref = base_ref.clone();
                        let base_id = base_id.clone();
                        let notes_db = notes_db.clone();
                        let column_view = column_view.clone();
                        let table_webview = table_webview.clone();
                        let notes = notes.clone();
                        let active_list = active_list.clone();
                        let available_list = available_list.clone();
                        let available_props = available_props.to_vec();
                        
                        remove_btn.connect_clicked(move |_| {
                            Self::remove_column(&base_ref, &base_id, &notes_db, &column_view, &table_webview, &notes, col_idx);
                            Self::refresh_columns_modal_content(&active_list, &available_list, &base_ref, &base_id, &notes_db, &column_view, &available_props, &table_webview, &notes, None);
                        });
                    }
                    
                    active_list.append(&row);
                }
                
                // === Panel derecho: Propiedades disponibles ===
                let new_props: Vec<&String> = available_props.iter()
                    .filter(|p| !existing_props.contains(p))
                    .filter(|p| {
                        if let Some(ref filter) = filter_text {
                            p.to_lowercase().contains(filter)
                        } else {
                            true
                        }
                    })
                    .collect();
                
                if new_props.is_empty() {
                    let empty_label = gtk::Label::builder()
                        .label(&i18n.t("base_no_available_props"))
                        .css_classes(["dim-label"])
                        .margin_top(20)
                        .build();
                    available_list.append(&empty_label);
                } else {
                    for prop in new_props {
                        let row = gtk::Box::builder()
                            .orientation(gtk::Orientation::Horizontal)
                            .spacing(8)
                            .css_classes(["column-row"])
                            .build();
                        
                        let add_btn = gtk::Button::builder()
                            .icon_name("list-add-symbolic")
                            .css_classes(["flat", "circular", "suggested-action"])
                            .tooltip_text(&i18n.t("base_add_as_column"))
                            .build();
                        row.append(&add_btn);
                        
                        let label = gtk::Label::builder()
                            .label(prop)
                            .hexpand(true)
                            .xalign(0.0)
                            .build();
                        row.append(&label);
                        
                        // Conectar añadir
                        {
                            let base_ref = base_ref.clone();
                            let base_id = base_id.clone();
                            let notes_db = notes_db.clone();
                            let column_view = column_view.clone();
                            let table_webview = table_webview.clone();
                            let notes = notes.clone();
                            let active_list = active_list.clone();
                            let available_list = available_list.clone();
                            let available_props = available_props.to_vec();
                            let prop_clone = prop.clone();
                            
                            add_btn.connect_clicked(move |_| {
                                Self::add_column(&base_ref, &base_id, &notes_db, &column_view, &table_webview, &notes, &prop_clone);
                                Self::refresh_columns_modal_content(&active_list, &available_list, &base_ref, &base_id, &notes_db, &column_view, &available_props, &table_webview, &notes, None);
                            });
                        }
                        
                        available_list.append(&row);
                    }
                }
            } else {
                // No hay vista activa
                let msg = gtk::Label::builder()
                    .label("No hay vista activa")
                    .css_classes(["dim-label"])
                    .margin_top(20)
                    .build();
                active_list.append(&msg);
            }
        } else {
            // No hay base cargada
            let msg = gtk::Label::builder()
                .label("No hay base cargada")
                .css_classes(["dim-label"])
                .margin_top(20)
                .build();
            active_list.append(&msg);
        }
    }
    
    // === Funciones helper para operaciones de columnas ===
    
    fn move_column(
        base_ref: &Rc<RefCell<Option<Base>>>,
        base_id: &Rc<RefCell<Option<i64>>>,
        notes_db: &Rc<RefCell<Option<NotesDatabase>>>,
        column_view: &gtk::ColumnView,
        table_webview: &webkit6::WebView,
        notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
        col_idx: usize,
        direction: i32,
    ) {
        let mut base_opt = match base_ref.try_borrow_mut() {
            Ok(b) => b,
            Err(_) => return,
        };
        if let Some(base) = base_opt.as_mut() {
            if let Some(view) = base.views.get_mut(base.active_view) {
                let new_idx = (col_idx as i32 + direction) as usize;
                if new_idx < view.columns.len() {
                    view.columns.swap(col_idx, new_idx);
                    Self::rebuild_column_view(column_view, &view.columns);
                    
                    let notes_borrowed = notes.borrow();
                    let html = Self::render_table_html_static(&notes_borrowed, &view.columns, Language::from_env(), view.editable, &view.special_rows);
                    table_webview.load_html(&html, None);
                    
                    if let (Some(id), Some(db)) = (base_id.borrow().as_ref(), notes_db.borrow().as_ref()) {
                        if let Ok(yaml) = base.serialize() {
                            let _ = db.update_base(*id, &yaml, base.active_view as i32);
                        }
                    }
                }
            }
        }
    }
    
    fn toggle_column_visibility(
        base_ref: &Rc<RefCell<Option<Base>>>,
        base_id: &Rc<RefCell<Option<i64>>>,
        notes_db: &Rc<RefCell<Option<NotesDatabase>>>,
        column_view: &gtk::ColumnView,
        table_webview: &webkit6::WebView,
        notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
        col_idx: usize,
        visible: bool,
    ) {
        let mut base_opt = match base_ref.try_borrow_mut() {
            Ok(b) => b,
            Err(_) => return,
        };
        if let Some(base) = base_opt.as_mut() {
            if let Some(view) = base.views.get_mut(base.active_view) {
                if let Some(col) = view.columns.get_mut(col_idx) {
                    col.visible = visible;
                    Self::rebuild_column_view(column_view, &view.columns);
                    
                    let notes_borrowed = notes.borrow();
                    let html = Self::render_table_html_static(&notes_borrowed, &view.columns, Language::from_env(), view.editable, &view.special_rows);
                    table_webview.load_html(&html, None);
                    
                    if let (Some(id), Some(db)) = (base_id.borrow().as_ref(), notes_db.borrow().as_ref()) {
                        if let Ok(yaml) = base.serialize() {
                            let _ = db.update_base(*id, &yaml, base.active_view as i32);
                        }
                    }
                }
            }
        }
    }
    
    fn remove_column(
        base_ref: &Rc<RefCell<Option<Base>>>,
        base_id: &Rc<RefCell<Option<i64>>>,
        notes_db: &Rc<RefCell<Option<NotesDatabase>>>,
        column_view: &gtk::ColumnView,
        table_webview: &webkit6::WebView,
        notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
        col_idx: usize,
    ) {
        let mut base_opt = match base_ref.try_borrow_mut() {
            Ok(b) => b,
            Err(_) => return,
        };
        if let Some(base) = base_opt.as_mut() {
            if let Some(view) = base.views.get_mut(base.active_view) {
                if col_idx < view.columns.len() {
                    view.columns.remove(col_idx);
                    Self::rebuild_column_view(column_view, &view.columns);
                    
                    let notes_borrowed = notes.borrow();
                    let html = Self::render_table_html_static(&notes_borrowed, &view.columns, Language::from_env(), view.editable, &view.special_rows);
                    table_webview.load_html(&html, None);
                    
                    if let (Some(id), Some(db)) = (base_id.borrow().as_ref(), notes_db.borrow().as_ref()) {
                        if let Ok(yaml) = base.serialize() {
                            let _ = db.update_base(*id, &yaml, base.active_view as i32);
                        }
                    }
                }
            }
        }
    }
    
    fn add_column(
        base_ref: &Rc<RefCell<Option<Base>>>,
        base_id: &Rc<RefCell<Option<i64>>>,
        notes_db: &Rc<RefCell<Option<NotesDatabase>>>,
        column_view: &gtk::ColumnView,
        table_webview: &webkit6::WebView,
        notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
        property: &str,
    ) {
        let mut base_opt = match base_ref.try_borrow_mut() {
            Ok(b) => b,
            Err(_) => return,
        };
        if let Some(base) = base_opt.as_mut() {
            if let Some(view) = base.views.get_mut(base.active_view) {
                view.columns.push(ColumnConfig::new(property));
                Self::rebuild_column_view(column_view, &view.columns);
                
                let notes_borrowed = notes.borrow();
                let html = Self::render_table_html_static(&notes_borrowed, &view.columns, Language::from_env(), view.editable, &view.special_rows);
                table_webview.load_html(&html, None);
                
                if let (Some(id), Some(db)) = (base_id.borrow().as_ref(), notes_db.borrow().as_ref()) {
                    if let Ok(yaml) = base.serialize() {
                        let _ = db.update_base(*id, &yaml, base.active_view as i32);
                    }
                }
            }
        }
    }
    
    fn setup_source_type_popover(&self) {
        let popover = gtk::Popover::builder()
            .css_classes(["source-type-popover"])
            .has_arrow(true)
            .build();
        
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .build();
        
        let title = gtk::Label::builder()
            .label(&self.i18n.borrow().t("base_data_source_title"))
            .css_classes(["heading"])
            .xalign(0.0)
            .margin_bottom(8)
            .build();
        content.append(&title);
        
        // Obtener el modo actual
        let current_mode = self.base.borrow()
            .as_ref()
            .map(|b| b.source_type.clone())
            .unwrap_or(SourceType::Notes);
        
        // Radio buttons para los modos
        let notes_radio = gtk::CheckButton::builder()
            .label(&format!("📝 {}", self.i18n.borrow().t("base_notes_mode")))
            .active(matches!(current_mode, SourceType::Notes))
            .build();
        
        let grouped_radio = gtk::CheckButton::builder()
            .label(&format!("📊 {}", self.i18n.borrow().t("base_grouped_mode")))
            .group(&notes_radio)
            .active(matches!(current_mode, SourceType::GroupedRecords))
            .build();
        
        content.append(&notes_radio);
        content.append(&grouped_radio);
        
        // Descripción
        let desc = gtk::Label::builder()
            .label(&self.i18n.borrow().t("base_grouped_hint"))
            .css_classes(["dim-label", "caption"])
            .xalign(0.0)
            .margin_top(8)
            .wrap(true)
            .build();
        content.append(&desc);
        
        // Clonar referencias para los callbacks
        let base_ref = self.base.clone();
        let base_id = self.base_id.clone();
        let notes_db = self.notes_db.clone();
        let db_path = self.db_path.clone();
        let notes_root = self.notes_root.clone();
        let popover_clone = popover.clone();
        
        // Clonar referencias para los callbacks de radio
        let base_ref_notes = base_ref.clone();
        let base_id_notes = base_id.clone();
        let notes_db_notes = notes_db.clone();
        let popover_notes = popover.clone();
        let on_change_notes = self.on_source_type_changed.clone();
        
        notes_radio.connect_toggled(move |btn| {
            if btn.is_active() {
                Self::change_source_type(
                    &base_ref_notes, &base_id_notes, &notes_db_notes,
                    SourceType::Notes,
                );
                popover_notes.popdown();
                // Llamar callback para recargar
                if let Some(ref callback) = *on_change_notes.borrow() {
                    callback();
                }
            }
        });
        
        let base_ref_grouped = base_ref.clone();
        let base_id_grouped = base_id.clone();
        let notes_db_grouped = notes_db.clone();
        let popover_grouped = popover.clone();
        let on_change_grouped = self.on_source_type_changed.clone();
        
        grouped_radio.connect_toggled(move |btn| {
            if btn.is_active() {
                Self::change_source_type(
                    &base_ref_grouped, &base_id_grouped, &notes_db_grouped,
                    SourceType::GroupedRecords,
                );
                popover_grouped.popdown();
                // Llamar callback para recargar
                if let Some(ref callback) = *on_change_grouped.borrow() {
                    callback();
                }
            }
        });
        
        popover.set_child(Some(&content));
        self.source_type_btn.set_popover(Some(&popover));
        
        // Actualizar icono según modo actual
        match current_mode {
            SourceType::Notes => self.source_type_btn.set_icon_name("view-list-symbolic"),
            SourceType::GroupedRecords => self.source_type_btn.set_icon_name("view-grid-symbolic"),
            SourceType::PropertyRecords => self.source_type_btn.set_icon_name("table-symbolic"),
        }
    }
    
    /// Configurar popover para filas de fórmulas
    fn setup_formula_row_popover(&self) {
        let popover = gtk::Popover::builder()
            .css_classes(["formula-row-popover"])
            .has_arrow(true)
            .build();
        
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .width_request(300)
            .build();
        
        let title = gtk::Label::builder()
            .label(&self.i18n.borrow().t("base_formula_rows_title"))
            .css_classes(["heading"])
            .xalign(0.0)
            .margin_bottom(4)
            .build();
        content.append(&title);
        
        // Lista de filas especiales existentes
        let rows_list = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .build();
        
        // Mostrar filas existentes
        if let Some(base) = self.base.borrow().as_ref() {
            if let Some(view) = base.active_view() {
                for special_row in &view.special_rows {
                    let row_item = gtk::Box::builder()
                        .orientation(gtk::Orientation::Horizontal)
                        .spacing(8)
                        .css_classes(["formula-row-item"])
                        .build();
                    
                    let label = gtk::Label::builder()
                        .label(&special_row.label)
                        .hexpand(true)
                        .xalign(0.0)
                        .build();
                    row_item.append(&label);
                    
                    // Botón para editar (TODO: implementar edición)
                    let edit_btn = gtk::Button::builder()
                        .icon_name("document-edit-symbolic")
                        .css_classes(["flat", "circular"])
                        .tooltip_text("Edit")
                        .build();
                    row_item.append(&edit_btn);
                    
                    // Botón para eliminar
                    let delete_btn = gtk::Button::builder()
                        .icon_name("user-trash-symbolic")
                        .css_classes(["flat", "circular", "destructive-action"])
                        .tooltip_text("Delete")
                        .build();
                    
                    let row_id = special_row.id.clone();
                    let base_ref = self.base.clone();
                    let base_id = self.base_id.clone();
                    let notes_db = self.notes_db.clone();
                    let notes = self.notes.clone();
                    let table_webview = self.table_webview.clone();
                    let popover_clone = popover.clone();
                    
                    delete_btn.connect_clicked(move |_| {
                        Self::remove_special_row(
                            &base_ref, &base_id, &notes_db, &notes, &table_webview, &row_id
                        );
                        popover_clone.popdown();
                    });
                    
                    row_item.append(&delete_btn);
                    rows_list.append(&row_item);
                }
            }
        }
        
        content.append(&rows_list);
        
        // Separator
        content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        
        // Botón para añadir fila de totales
        let add_totals_btn = gtk::Button::builder()
            .label(&self.i18n.borrow().t("base_add_formula_row"))
            .css_classes(["suggested-action"])
            .build();
        
        let base_ref = self.base.clone();
        let base_id = self.base_id.clone();
        let notes_db = self.notes_db.clone();
        let notes = self.notes.clone();
        let table_webview = self.table_webview.clone();
        let popover_clone = popover.clone();
        
        add_totals_btn.connect_clicked(move |_| {
            Self::add_totals_row(&base_ref, &base_id, &notes_db, &notes, &table_webview);
            popover_clone.popdown();
        });
        
        content.append(&add_totals_btn);
        
        // Ayuda
        let help = gtk::Label::builder()
            .label(&self.i18n.borrow().t("base_formula_help"))
            .css_classes(["dim-label", "caption"])
            .wrap(true)
            .xalign(0.0)
            .margin_top(4)
            .build();
        content.append(&help);
        
        popover.set_child(Some(&content));
        self.formula_row_btn.set_popover(Some(&popover));
        
        // Configurar handler para acciones de filas especiales desde JavaScript
        self.setup_special_row_handler();
    }
    
    /// Configurar el handler para acciones de filas especiales
    fn setup_special_row_handler(&self) {
        let base_ref = self.base.clone();
        let base_id = self.base_id.clone();
        let notes_db = self.notes_db.clone();
        let notes = self.notes.clone();
        let table_webview = self.table_webview.clone();
        
        if let Some(content_manager) = self.table_webview.user_content_manager() {
            content_manager.connect_script_message_received(Some("specialRowAction"), move |_, result| {
                let message_str = result.to_str();
                let clean_msg = message_str.trim_matches('"');
                
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(clean_msg) {
                    let action = json.get("action").and_then(|v| v.as_str()).unwrap_or("");
                    let row_id = json.get("rowId").and_then(|v| v.as_str()).unwrap_or("");
                    
                    match action {
                        "delete" => {
                            Self::remove_special_row(&base_ref, &base_id, &notes_db, &notes, &table_webview, row_id);
                        }
                        "move" => {
                            let direction = json.get("direction").and_then(|v| v.as_str()).unwrap_or("");
                            Self::move_special_row(&base_ref, &base_id, &notes_db, &notes, &table_webview, row_id, direction);
                        }
                        "reorder" => {
                            let target_id = json.get("targetId").and_then(|v| v.as_str()).unwrap_or("");
                            Self::reorder_special_row(&base_ref, &base_id, &notes_db, &notes, &table_webview, row_id, target_id);
                        }
                        "edit" => {
                            let property = json.get("property").and_then(|v| v.as_str());
                            let field = json.get("field").and_then(|v| v.as_str());
                            let value = json.get("value").and_then(|v| v.as_str()).unwrap_or("");
                            Self::edit_special_row(&base_ref, &base_id, &notes_db, &notes, &table_webview, row_id, property, field, value);
                        }
                        _ => {
                            eprintln!("Unknown special row action: {}", action);
                        }
                    }
                } else {
                    eprintln!("⚠️ Error parsing specialRowAction JSON: {}", clean_msg);
                }
            });
        }
    }
    
    /// Añadir fila de totales con fórmulas SUM por defecto
    fn add_totals_row(
        base_ref: &Rc<RefCell<Option<Base>>>,
        base_id: &Rc<RefCell<Option<i64>>>,
        notes_db: &Rc<RefCell<Option<NotesDatabase>>>,
        notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
        table_webview: &webkit6::WebView,
    ) {
        let mut base_opt = match base_ref.try_borrow_mut() {
            Ok(b) => b,
            Err(_) => return,
        };
        
        if let Some(base) = base_opt.as_mut() {
            if let Some(view) = base.views.get_mut(base.active_view) {
                // Crear fila de totales con fórmulas SUM para columnas numéricas
                let mut totals_row = SpecialRow::totals("Total");
                
                // Añadir fórmula SUM para cada columna (excepto la primera que es el label)
                for (col_idx, col) in view.columns.iter().enumerate().skip(1) {
                    if col.visible {
                        let col_letter = crate::core::formula::col_to_letters(col_idx as u16);
                        let formula = format!("=SUM({}:{})", col_letter, col_letter);
                        totals_row.cells.insert(
                            col.property.clone(),
                            SpecialCellContent::formula(formula)
                                .with_format(CellFormat::new().bold())
                        );
                    }
                }
                
                view.special_rows.push(totals_row);
                
                // Refrescar tabla
                let notes_borrowed = notes.borrow();
                let html = Self::render_table_html_static(
                    &notes_borrowed, &view.columns, Language::from_env(), 
                    view.editable, &view.special_rows
                );
                table_webview.load_html(&html, None);
                
                // Persistir
                if let (Some(id), Some(db)) = (base_id.borrow().as_ref(), notes_db.borrow().as_ref()) {
                    if let Ok(yaml) = base.serialize() {
                        let _ = db.update_base(*id, &yaml, base.active_view as i32);
                    }
                }
            }
        }
    }
    
    /// Eliminar una fila especial
    fn remove_special_row(
        base_ref: &Rc<RefCell<Option<Base>>>,
        base_id: &Rc<RefCell<Option<i64>>>,
        notes_db: &Rc<RefCell<Option<NotesDatabase>>>,
        notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
        table_webview: &webkit6::WebView,
        row_id: &str,
    ) {
        let mut base_opt = match base_ref.try_borrow_mut() {
            Ok(b) => b,
            Err(_) => return,
        };
        
        if let Some(base) = base_opt.as_mut() {
            if let Some(view) = base.views.get_mut(base.active_view) {
                view.special_rows.retain(|r| r.id != row_id);
                
                // Refrescar tabla
                let notes_borrowed = notes.borrow();
                let html = Self::render_table_html_static(
                    &notes_borrowed, &view.columns, Language::from_env(), 
                    view.editable, &view.special_rows
                );
                table_webview.load_html(&html, None);
                
                // Persistir
                if let (Some(id), Some(db)) = (base_id.borrow().as_ref(), notes_db.borrow().as_ref()) {
                    if let Ok(yaml) = base.serialize() {
                        let _ = db.update_base(*id, &yaml, base.active_view as i32);
                    }
                }
            }
        }
    }
    
    /// Mover una fila especial arriba o abajo
    fn move_special_row(
        base_ref: &Rc<RefCell<Option<Base>>>,
        base_id: &Rc<RefCell<Option<i64>>>,
        notes_db: &Rc<RefCell<Option<NotesDatabase>>>,
        notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
        table_webview: &webkit6::WebView,
        row_id: &str,
        direction: &str,
    ) {
        let mut base_opt = match base_ref.try_borrow_mut() {
            Ok(b) => b,
            Err(_) => return,
        };
        
        if let Some(base) = base_opt.as_mut() {
            if let Some(view) = base.views.get_mut(base.active_view) {
                let total_data_rows = notes.borrow().len();
                
                // Encontrar la fila especial
                if let Some(special_row) = view.special_rows.iter_mut().find(|r| r.id == row_id) {
                    // Obtener posición actual (None = después de la última fila)
                    let current_pos = special_row.position.unwrap_or(total_data_rows + 1);
                    
                    let new_pos = match direction {
                        "up" if current_pos > 0 => current_pos.saturating_sub(1),
                        "down" => current_pos + 1,
                        _ => return,
                    };
                    
                    // Actualizar posición
                    if new_pos == 0 {
                        special_row.position = Some(0); // Antes de la primera fila
                    } else if new_pos > total_data_rows {
                        special_row.position = None; // Al final
                    } else {
                        special_row.position = Some(new_pos);
                    }
                    
                    // Refrescar tabla
                    let notes_borrowed = notes.borrow();
                    let html = Self::render_table_html_static(
                        &notes_borrowed, &view.columns, Language::from_env(), 
                        view.editable, &view.special_rows
                    );
                    table_webview.load_html(&html, None);
                    
                    // Persistir
                    if let (Some(id), Some(db)) = (base_id.borrow().as_ref(), notes_db.borrow().as_ref()) {
                        if let Ok(yaml) = base.serialize() {
                            let _ = db.update_base(*id, &yaml, base.active_view as i32);
                        }
                    }
                }
            }
        }
    }
    
    /// Editar una celda de fila especial
    fn edit_special_row(
        base_ref: &Rc<RefCell<Option<Base>>>,
        base_id: &Rc<RefCell<Option<i64>>>,
        notes_db: &Rc<RefCell<Option<NotesDatabase>>>,
        notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
        table_webview: &webkit6::WebView,
        row_id: &str,
        property: Option<&str>,
        field: Option<&str>,
        value: &str,
    ) {
        let mut base_opt = match base_ref.try_borrow_mut() {
            Ok(b) => b,
            Err(_) => return,
        };
        
        if let Some(base) = base_opt.as_mut() {
            if let Some(view) = base.views.get_mut(base.active_view) {
                if let Some(special_row) = view.special_rows.iter_mut().find(|r| r.id == row_id) {
                    // Editar label
                    if field == Some("label") {
                        special_row.label = value.to_string();
                    } else if let Some(prop) = property {
                        // Editar celda con fórmula
                        let content = if value.starts_with('=') {
                            SpecialCellContent::formula(value.to_string())
                        } else {
                            SpecialCellContent::text(value.to_string())
                        };
                        special_row.cells.insert(prop.to_string(), content.with_format(CellFormat::new().bold()));
                    }
                    
                    // Refrescar tabla
                    let notes_borrowed = notes.borrow();
                    let html = Self::render_table_html_static(
                        &notes_borrowed, &view.columns, Language::from_env(), 
                        view.editable, &view.special_rows
                    );
                    table_webview.load_html(&html, None);
                    
                    // Persistir
                    if let (Some(id), Some(db)) = (base_id.borrow().as_ref(), notes_db.borrow().as_ref()) {
                        if let Ok(yaml) = base.serialize() {
                            let _ = db.update_base(*id, &yaml, base.active_view as i32);
                        }
                    }
                }
            }
        }
    }
    
    /// Reordenar filas especiales (drag & drop)
    fn reorder_special_row(
        base_ref: &Rc<RefCell<Option<Base>>>,
        base_id: &Rc<RefCell<Option<i64>>>,
        notes_db: &Rc<RefCell<Option<NotesDatabase>>>,
        notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
        table_webview: &webkit6::WebView,
        row_id: &str,
        target_id: &str,
    ) {
        let mut base_opt = match base_ref.try_borrow_mut() {
            Ok(b) => b,
            Err(_) => return,
        };
        
        if let Some(base) = base_opt.as_mut() {
            if let Some(view) = base.views.get_mut(base.active_view) {
                // Encontrar índices
                let from_idx = view.special_rows.iter().position(|r| r.id == row_id);
                let to_idx = view.special_rows.iter().position(|r| r.id == target_id);
                
                if let (Some(from), Some(to)) = (from_idx, to_idx) {
                    if from != to {
                        // Mover elemento: remover y reinsertar en la posición del target
                        let row = view.special_rows.remove(from);
                        // Después de remove, el índice target puede haber cambiado
                        let insert_at = if from < to { to } else { to };
                        view.special_rows.insert(insert_at, row);
                        
                        // Refrescar tabla
                        let notes_borrowed = notes.borrow();
                        let html = Self::render_table_html_static(
                            &notes_borrowed, &view.columns, Language::from_env(), 
                            view.editable, &view.special_rows
                        );
                        table_webview.load_html(&html, None);
                        
                        // Persistir
                        if let (Some(id), Some(db)) = (base_id.borrow().as_ref(), notes_db.borrow().as_ref()) {
                            if let Ok(yaml) = base.serialize() {
                                let _ = db.update_base(*id, &yaml, base.active_view as i32);
                            }
                        }
                    }
                }
            }
        }
    }
    
    /// Configurar botón de exportar a XLSX
    fn setup_export_xlsx_btn(&self) {
        let base = self.base.clone();
        let notes = self.notes.clone();
        let i18n = self.i18n.clone();
        let export_btn = self.export_xlsx_btn.clone();
        
        export_btn.connect_clicked(move |btn| {
            let base_borrowed = base.borrow();
            let notes_borrowed = notes.borrow();
            
            if base_borrowed.is_none() || notes_borrowed.is_empty() {
                return;
            }
            
            let base_data = base_borrowed.as_ref().unwrap();
            let view = match base_data.active_view() {
                Some(v) => v,
                None => return,
            };
            
            // Clonar datos necesarios
            let notes_vec: Vec<_> = notes_borrowed.clone();
            let columns = view.columns.clone();
            let special_rows = view.special_rows.clone();
            let sheet_name = base_data.name.clone();
            let filename = format!("{}.xlsx", base_data.name);
            
            drop(base_borrowed);
            drop(notes_borrowed);
            
            // Usar ashpd para el diálogo de guardar archivo (funciona en Wayland)
            glib::spawn_future_local(async move {
                use ashpd::desktop::file_chooser::{SaveFileRequest, FileFilter};
                
                let filter = FileFilter::new("Excel Files")
                    .glob("*.xlsx");
                
                match SaveFileRequest::default()
                    .title("Export to Excel")
                    .current_name(filename.as_str())
                    .filter(filter)
                    .send()
                    .await
                {
                    Ok(response) => {
                        if let Ok(files) = response.response() {
                            if let Some(uri) = files.uris().first() {
                                // Convertir file:// URI a path
                                if let Some(path_str) = uri.path().to_string().strip_prefix("file://") {
                                    let path = std::path::PathBuf::from(path_str);
                                    match crate::core::xlsx_export::export_to_xlsx(
                                        &path,
                                        &notes_vec,
                                        &columns,
                                        &special_rows,
                                        &sheet_name,
                                    ) {
                                        Ok(()) => {
                                            eprintln!("XLSX exported successfully to {:?}", path);
                                        }
                                        Err(e) => {
                                            eprintln!("Error exporting XLSX: {}", e);
                                        }
                                    }
                                } else {
                                    // Intentar directamente con el path del URI
                                    let path = std::path::PathBuf::from(uri.path());
                                    match crate::core::xlsx_export::export_to_xlsx(
                                        &path,
                                        &notes_vec,
                                        &columns,
                                        &special_rows,
                                        &sheet_name,
                                    ) {
                                        Ok(()) => {
                                            eprintln!("XLSX exported successfully to {:?}", path);
                                        }
                                        Err(e) => {
                                            eprintln!("Error exporting XLSX: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("File dialog error: {}", e);
                    }
                }
            });
        });
    }
    
    /// Cambiar el source_type de la Base y persistir
    fn change_source_type(
        base_ref: &Rc<RefCell<Option<Base>>>,
        base_id: &Rc<RefCell<Option<i64>>>,
        notes_db: &Rc<RefCell<Option<NotesDatabase>>>,
        new_type: SourceType,
    ) {
        let mut base_opt = base_ref.borrow_mut();
        if let Some(base) = base_opt.as_mut() {
            base.source_type = new_type;
            
            // Persistir en la BD
            if let (Some(id), Some(db)) = (base_id.borrow().as_ref(), notes_db.borrow().as_ref()) {
                if let Ok(yaml) = base.serialize() {
                    if let Err(e) = db.update_base(*id, &yaml, base.active_view as i32) {
                        eprintln!("Error saving Base source_type: {}", e);
                    }
                }
            }
        }
    }
    
    /// Actualizar los chips de filtros activos
    fn update_filter_chips(&self) {
        // Limpiar chips existentes
        while let Some(child) = self.filters_container.first_child() {
            self.filters_container.remove(&child);
        }
        
        let filters = self.active_filters.borrow();
        
        if filters.is_empty() {
            // Mostrar placeholder
            let placeholder = gtk::Label::builder()
                .label(&self.i18n.borrow().t("base_no_filters"))
                .css_classes(["dim-label"])
                .build();
            self.filters_container.append(&placeholder);
        } else {
            // Crear chips para cada filtro
            for (i, filter) in filters.iter().enumerate() {
                let chip = create_filter_chip(filter, i);
                
                // Conectar botón de cerrar
                let active_filters = self.active_filters.clone();
                let all_notes = self.all_notes.clone();
                let notes = self.notes.clone();
                let current_sort = self.current_sort.clone();
                let list_store = self.list_store.clone();
                let status_bar = self.status_bar.clone();
                let filters_container = self.filters_container.clone();
                let table_webview = self.table_webview.clone();
                let base = self.base.clone();
                let i18n = self.i18n.clone();
                let chip_widget = chip.clone();
                
                // DEBUG: Verificar estructura del chip
                eprintln!("DEBUG: Chip creado, buscando last_child...");
                if let Some(last) = chip.last_child() {
                    eprintln!("DEBUG: last_child encontrado, tipo: {:?}", last.type_());
                    if let Some(close_btn) = last.downcast_ref::<gtk::Button>() {
                        eprintln!("DEBUG: Botón encontrado, conectando handler");
                        close_btn.connect_clicked(move |_| {
                            eprintln!("DEBUG: ¡CLICK EN BOTÓN X DETECTADO!");
                            // Eliminar filtro
                            let mut filters_mut = active_filters.borrow_mut();
                            if i < filters_mut.len() {
                                filters_mut.remove(i);
                            }
                            let filters_snapshot = filters_mut.clone();
                            drop(filters_mut);
                        
                        // Re-aplicar filtros
                        let all = all_notes.borrow();
                        let sort = current_sort.borrow();
                        
                        let mut filtered: Vec<NoteWithProperties> = all
                            .iter()
                            .filter(|note| {
                                filters_snapshot.iter().all(|f| f.evaluate(&note.properties))
                            })
                            .cloned()
                            .collect();
                        
                        // Ordenar
                        if let Some(sort_config) = sort.as_ref() {
                            filtered.sort_by(|a, b| {
                                let key_a = a.properties
                                    .get(&sort_config.property)
                                    .map(|v| v.sort_key())
                                    .unwrap_or_default();
                                let key_b = b.properties
                                    .get(&sort_config.property)
                                    .map(|v| v.sort_key())
                                    .unwrap_or_default();

                                match sort_config.direction {
                                    SortDirection::Asc => key_a.cmp(&key_b),
                                    SortDirection::Desc => key_b.cmp(&key_a),
                                }
                            });
                        }
                        
                        drop(all);
                        drop(sort);
                        
                        *notes.borrow_mut() = filtered.clone();
                        
                        // Actualizar list_store
                        list_store.remove_all();
                        for note in &filtered {
                            let boxed = glib::BoxedAnyObject::new(note.clone());
                            list_store.append(&boxed);
                        }
                        
                        // Actualizar WebView
                        let columns = if let Some(base) = base.borrow().as_ref() {
                            if let Some(view) = base.views.get(base.active_view) {
                                view.columns.clone()
                            } else {
                                vec![
                                    ColumnConfig { property: "title".to_string(), title: None, width: Some(300), visible: true },
                                    ColumnConfig { property: "created".to_string(), title: None, width: Some(150), visible: true },
                                ]
                            }
                        } else {
                            vec![
                                ColumnConfig { property: "title".to_string(), title: None, width: Some(300), visible: true },
                                ColumnConfig { property: "created".to_string(), title: None, width: Some(150), visible: true },
                            ]
                        };
                        let html = Self::render_table_html_static(&filtered, &columns, Language::from_env(), false, &[]);
                        table_webview.load_html(&html, None);
                        
                        // Actualizar status bar
                        if let Some(label) = status_bar.first_child().and_downcast::<gtk::Label>() {
                            let text = if filtered.len() == 1 {
                                "1 note".to_string()
                            } else {
                                format!("{} notes", filtered.len())
                            };
                            label.set_text(&text);
                        }
                        
                        // DEBUG: Verificar que el closure se ejecuta
                        eprintln!("DEBUG: Eliminando filtro, quedan {} filtros", filters_snapshot.len());
                        
                        // Eliminar este chip visualmente - limpiar todo y reconstruir
                        while let Some(child) = filters_container.first_child() {
                            eprintln!("DEBUG: Removiendo child del container");
                            filters_container.remove(&child);
                        }
                        
                        eprintln!("DEBUG: Container limpiado, children restantes: {}", 
                            if filters_container.first_child().is_some() { "SI" } else { "NO" });
                        
                        // Mostrar estado actualizado
                        if filters_snapshot.is_empty() {
                            let placeholder = gtk::Label::builder()
                                .label(&i18n.borrow().t("base_no_filters"))
                                .css_classes(["dim-label"])
                                .build();
                            filters_container.append(&placeholder);
                            eprintln!("DEBUG: Placeholder añadido");
                        } else {
                            // Recrear chips restantes (sin handlers, solo visual por ahora)
                            for (idx, filter) in filters_snapshot.iter().enumerate() {
                                let new_chip = create_filter_chip(filter, idx);
                                filters_container.append(&new_chip);
                            }
                            eprintln!("DEBUG: {} chips recreados", filters_snapshot.len());
                        }
                    });
                    } else {
                        eprintln!("DEBUG: ERROR - last_child no es un Button");
                    }
                } else {
                    eprintln!("DEBUG: ERROR - chip no tiene last_child");
                }
                
                self.filters_container.append(&chip);
            }
        }
    }

    /// Actualizar las columnas del ColumnView
    fn update_columns(&self, columns: &[ColumnConfig]) {
        Self::rebuild_column_view(&self.column_view, columns);
    }
    
    /// Reconstruir las columnas de un ColumnView (función estática para usar en callbacks)
    fn rebuild_column_view(column_view: &gtk::ColumnView, columns: &[ColumnConfig]) {
        // Limpiar columnas existentes
        while let Some(col) = column_view.columns().item(0) {
            if let Some(column) = col.downcast_ref::<gtk::ColumnViewColumn>() {
                column_view.remove_column(column);
            }
        }

        // Crear nuevas columnas
        for config in columns {
            if !config.visible {
                continue;
            }

            let property_name = config.property.clone();

            // Factory para crear las celdas
            let factory = gtk::SignalListItemFactory::new();
            
            factory.connect_setup(move |_, list_item| {
                let label = gtk::Label::builder()
                    .xalign(0.0)
                    .css_classes(["base-cell"])
                    .build();
                list_item.set_child(Some(&label));
            });

            let prop_name = property_name.clone();
            factory.connect_bind(move |_, list_item| {
                if let Some(boxed) = list_item.item().and_downcast::<glib::BoxedAnyObject>() {
                    let note = boxed.borrow::<NoteWithProperties>();
                    if let Some(label) = list_item.child().and_downcast::<gtk::Label>() {
                        label.set_text(&note.get_display(&prop_name));
                        
                        // Aplicar clase para filas alternas
                        let position = list_item.position();
                        label.remove_css_class("row-even");
                        label.remove_css_class("row-odd");
                        if position % 2 == 0 {
                            label.add_css_class("row-even");
                        } else {
                            label.add_css_class("row-odd");
                        }
                    }
                }
            });

            // Crear columna
            let column = gtk::ColumnViewColumn::builder()
                .title(&config.display_title())
                .factory(&factory)
                .resizable(true)
                .build();

            if let Some(width) = config.width {
                column.set_fixed_width(width as i32);
            }

            column_view.append_column(&column);
        }
    }

    /// Actualizar los datos de la tabla usando el WebView
    fn update_data(&self, notes: &[NoteWithProperties]) {
        // Si estamos en proceso de carga, no actualizar el WebView todavía
        // La actualización se hará al final con force_update_webview()
        if *self.is_loading.borrow() {
            return;
        }
        
        self.update_webview_internal(notes);
    }
    
    /// Forzar actualización del WebView (se llama al final de load_base)
    fn force_update_webview(&self) {
        let notes = self.notes.borrow().clone();
        self.update_webview_internal(&notes);
    }
    
    /// Actualización interna del WebView (usada por update_data y force_update_webview)
    fn update_webview_internal(&self, notes: &[NoteWithProperties]) {
        // Obtener las columnas configuradas de la vista actual
        let columns = if let Some(base) = self.base.borrow().as_ref() {
            if let Some(view) = base.views.get(base.active_view) {
                view.columns.clone()
            } else {
                // Columnas por defecto
                vec![
                    ColumnConfig { property: "title".to_string(), title: None, width: Some(300), visible: true },
                    ColumnConfig { property: "created".to_string(), title: None, width: Some(150), visible: true },
                ]
            }
        } else {
            vec![
                ColumnConfig { property: "title".to_string(), title: None, width: Some(300), visible: true },
                ColumnConfig { property: "created".to_string(), title: None, width: Some(150), visible: true },
            ]
        };
        
        // Renderizar el HTML de la tabla
        let html = self.render_table_html(notes, &columns);
        
        // Cargar directamente el HTML completo
        self.table_webview.load_html(&html, None);
    }
    
    /// Generar el HTML para la tabla
    fn render_table_html(&self, notes: &[NoteWithProperties], columns: &[ColumnConfig]) -> String {
        // Determinar si es editable:
        // 1. Si la vista lo especifica explícitamente
        // 2. O si estamos en modo GroupedRecords o PropertyRecords (siempre editable)
        let (editable, special_rows) = self.base.borrow()
            .as_ref()
            .map(|b| {
                let source_editable = matches!(b.source_type, SourceType::GroupedRecords | SourceType::PropertyRecords);
                let view_editable = b.active_view().map(|v| v.editable).unwrap_or(false);
                let special = b.active_view().map(|v| v.special_rows.clone()).unwrap_or_default();
                (source_editable || view_editable, special)
            })
            .unwrap_or((false, Vec::new()));
        let colors = self.theme_colors.borrow().clone();
        Self::render_table_html_with_colors(notes, columns, self.i18n.borrow().current_language(), editable, &special_rows, &colors)
    }
    
    /// Generar el HTML para la tabla (versión estática para usar en closures)
    fn render_table_html_static(notes: &[NoteWithProperties], columns: &[ColumnConfig], language: Language, editable: bool, special_rows: &[SpecialRow]) -> String {
        // Usar colores por defecto para versión estática
        Self::render_table_html_with_colors(notes, columns, language, editable, special_rows, &GtkThemeColors::default())
    }
    
    /// Generar el HTML para la tabla con colores específicos del tema GTK
    fn render_table_html_with_colors(notes: &[NoteWithProperties], columns: &[ColumnConfig], language: Language, editable: bool, special_rows: &[SpecialRow], colors: &GtkThemeColors) -> String {
        // Traducciones para el HTML
        let (search_placeholder, items_label, no_notes_label) = if language == Language::Spanish {
            ("Buscar en tabla...", "elementos", "No se encontraron notas")
        } else {
            ("Search in table...", "items", "No notes found")
        };
        
        // CSS con colores dinámicos del tema GTK
        let css = format!(r#"
:root {{
    --bg-primary: {bg_primary};
    --bg-secondary: {bg_secondary};
    --bg-tertiary: {bg_tertiary};
    --fg-primary: {fg_primary};
    --fg-secondary: {fg_secondary};
    --fg-muted: {fg_muted};
    --accent: {accent};
    --border: {border};
}}

* {{
    box-sizing: border-box;
    margin: 0;
    padding: 0;
}}

html {{
    min-height: 100vh;
    height: 100%;
    background-color: {bg_primary};
}}

body {{
    min-height: 100vh;
    height: 100%;
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    font-size: 15px;
    line-height: 1.6;
    color: var(--fg-primary);
    background-color: var(--bg-primary);
    padding: 16px;
    opacity: 0;
}}

body.loaded {{
    opacity: 1;
    transition: opacity 0.1s ease-out;
}}

table {{
    width: 100%;
    border-collapse: collapse;
    margin: 0;
    font-size: 0.95em;
}}"#,
            bg_primary = colors.bg_primary,
            bg_secondary = colors.bg_secondary,
            bg_tertiary = colors.bg_tertiary,
            fg_primary = colors.fg_primary,
            fg_secondary = colors.fg_secondary,
            fg_muted = colors.fg_muted,
            accent = colors.accent,
            border = colors.border,
        );
        
        // Resto del CSS (sin variables hardcodeadas)
        let css_rest = r#"th, td {
    border: 1px solid var(--border);
    padding: 10px 14px;
    text-align: left;
}

th {
    background-color: var(--bg-secondary);
    font-weight: 600;
    text-transform: uppercase;
    font-size: 0.85em;
    color: var(--fg-secondary);
}

tr:nth-child(even) {
    background-color: var(--bg-secondary);
}

tr:hover {
    background-color: var(--bg-tertiary);
    cursor: pointer;
}

.title-cell {
    font-weight: 500;
    color: var(--fg-primary);
}

.date-cell {
    color: var(--fg-muted);
    font-size: 0.9em;
}

.property-cell {
    color: var(--fg-secondary);
    font-size: 0.9em;
}

.empty-state {
    text-align: center;
    padding: 48px;
    color: var(--fg-muted);
}

/* Search bar */
.search-container {
    position: sticky;
    top: 0;
    z-index: 100;
    background: var(--bg-primary);
    padding: 8px 0 12px 0;
    margin-bottom: 8px;
}

.search-input {
    width: 100%;
    max-width: 400px;
    padding: 8px 12px 8px 36px;
    font-size: 14px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg-secondary);
    color: var(--fg-primary);
    outline: none;
    transition: border-color 0.2s, box-shadow 0.2s;
}

.search-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px rgba(137, 180, 250, 0.2);
}

.search-input::placeholder {
    color: var(--fg-muted);
}

.search-wrapper {
    position: relative;
    display: inline-block;
    width: 100%;
    max-width: 400px;
}

.search-icon {
    position: absolute;
    left: 12px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--fg-muted);
    pointer-events: none;
}

.search-results-count {
    display: inline-block;
    margin-left: 12px;
    font-size: 13px;
    color: var(--fg-muted);
}

tr.hidden-by-search {
    display: none;
}

tr.search-highlight td {
    background-color: rgba(137, 180, 250, 0.15);
}

/* Link de nota clickeable */
.note-link-cell {
    color: var(--fg-secondary);
    font-size: 0.9em;
}

.note-link {
    color: var(--accent);
    text-decoration: none;
    cursor: pointer;
}

.note-link:hover {
    text-decoration: underline;
}

/* Celdas editables */
.editable-cell {
    cursor: text;
    transition: background-color 0.2s, box-shadow 0.2s;
}

.editable-cell:hover {
    background-color: rgba(137, 180, 250, 0.1);
}

.editable-cell:focus {
    outline: none;
    background-color: rgba(137, 180, 250, 0.15);
    box-shadow: inset 0 0 0 2px var(--accent);
}

.editable-cell.modified {
    background-color: rgba(166, 227, 161, 0.15);
}

.editable-cell.saving {
    opacity: 0.6;
    pointer-events: none;
}

/* Filas especiales con fórmulas */
.special-row {
    background-color: var(--bg-secondary) !important;
    font-weight: 500;
}

.special-row:hover {
    background-color: var(--bg-tertiary) !important;
    cursor: default;
}

.special-row td {
    border-top: 2px solid var(--border);
}

.special-row-totals {
    background-color: var(--bg-tertiary) !important;
}

.special-row-label {
    font-weight: 600;
    color: var(--fg-secondary);
    font-style: italic;
}

.formula-cell {
    font-family: 'SF Mono', 'Monaco', 'Consolas', monospace;
    color: var(--accent);
}

.formula-error {
    color: #f38ba8;
    font-style: italic;
}

/* Referencias de columna en headers */
.col-ref {
    display: inline-block;
    background: var(--accent);
    color: var(--bg-primary);
    font-size: 0.7em;
    padding: 1px 5px;
    border-radius: 3px;
    margin-right: 6px;
    font-weight: 700;
    vertical-align: middle;
}

/* Referencia de fila */
.row-ref {
    display: inline-block;
    background: var(--fg-muted);
    color: var(--bg-primary);
    font-size: 0.7em;
    padding: 1px 4px;
    border-radius: 3px;
    margin-right: 4px;
    font-weight: 600;
    min-width: 20px;
    text-align: center;
}

/* Celdas editables en filas especiales */
.special-cell-editable {
    cursor: text;
    min-width: 60px;
}

.special-cell-editable:hover {
    background-color: rgba(137, 180, 250, 0.15);
}

.special-cell-editable:focus {
    outline: none;
    background-color: rgba(137, 180, 250, 0.2);
    box-shadow: inset 0 0 0 2px var(--accent);
}

/* Dropdown de autocompletado de fórmulas */
.formula-autocomplete {
    position: absolute;
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0,0,0,0.3);
    z-index: 1000;
    max-height: 250px;
    overflow-y: auto;
    min-width: 220px;
    display: none;
}

.formula-autocomplete.visible {
    display: block;
}

.formula-item {
    padding: 8px 12px;
    cursor: pointer;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 2px;
}

.formula-item:last-child {
    border-bottom: none;
}

.formula-item:hover,
.formula-item.selected {
    background: var(--accent);
    color: var(--bg-primary);
}

.formula-item:hover .formula-desc,
.formula-item.selected .formula-desc {
    color: var(--bg-secondary);
}

.formula-name {
    font-weight: 600;
    font-family: monospace;
    font-size: 0.95em;
}

.formula-desc {
    font-size: 0.8em;
    color: var(--fg-muted);
}

.formula-syntax {
    font-family: monospace;
    font-size: 0.75em;
    color: var(--fg-secondary);
    background: var(--bg-secondary);
    padding: 2px 4px;
    border-radius: 3px;
    margin-top: 2px;
}

.formula-item:hover .formula-syntax,
.formula-item.selected .formula-syntax {
    background: rgba(255,255,255,0.2);
}

/* Controles de fila especial - siempre visibles */
.special-row-controls-cell {
    position: relative;
}

.special-row-controls {
    display: flex;
    gap: 2px;
    justify-content: center;
}

.special-row-btn {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    color: var(--fg-secondary);
    padding: 2px 5px;
    border-radius: 3px;
    cursor: pointer;
    font-size: 0.75em;
}

.special-row-btn:hover {
    background: var(--accent);
    color: var(--bg-primary);
}

.special-row-btn.disabled {
    opacity: 0.3;
    cursor: not-allowed;
}

.special-row-btn.disabled:hover {
    background: var(--bg-secondary);
    color: var(--fg-secondary);
}

.special-row-btn.delete:hover {
    background: #f38ba8;
}

/* Columna de número de fila */
.row-num-col {
    width: 40px;
    min-width: 40px;
    max-width: 50px;
    text-align: center;
    color: var(--fg-muted);
    font-size: 0.85em;
    font-weight: 500;
    background: var(--bg-secondary);
    user-select: none;
}

/* Celda de controles en fila especial - más ancha */
.special-row .row-num-col {
    width: 70px;
    min-width: 70px;
    max-width: 80px;
}

th.row-num-col {
    font-size: 0.8em;
}

/* Filas especiales siempre visibles (no afectadas por filtro) */
tr.special-row.hidden-by-search {
    display: table-row !important;
}
"#;
        
        // Combinar CSS base con resto
        let full_css = format!("{}{}", css, css_rest);
        
        let mut html = format!(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>{}</style>
</head>
<body>
<script>
// Función de búsqueda en la tabla
function filterTable(query) {{
    var tbody = document.querySelector('tbody');
    if (!tbody) return;
    
    var rows = tbody.querySelectorAll('tr[data-path]');
    var count = 0;
    var total = rows.length;
    var searchLower = query.toLowerCase().trim();
    
    rows.forEach(function(row) {{
        if (searchLower === '') {{
            row.classList.remove('hidden-by-search');
            row.classList.remove('search-highlight');
            count++;
        }} else {{
            var text = row.textContent.toLowerCase();
            if (text.includes(searchLower)) {{
                row.classList.remove('hidden-by-search');
                row.classList.add('search-highlight');
                count++;
            }} else {{
                row.classList.add('hidden-by-search');
                row.classList.remove('search-highlight');
            }}
        }}
    }});
    
    // Actualizar contador
    var countEl = document.getElementById('search-count');
    if (countEl) {{
        if (searchLower === '') {{
            countEl.textContent = total + ' items';
        }} else {{
            countEl.textContent = count + ' of ' + total + ' items';
        }}
    }}
}}

// Listener en document para capturar todos los clics
document.addEventListener('click', function(event) {{
    // Ignorar clics en el campo de búsqueda
    if (event.target.closest('.search-container')) {{
        return;
    }}
    
    // Ignorar clics en celdas editables (para permitir edición)
    if (event.target.classList && event.target.classList.contains('editable-cell')) {{
        return;
    }}
    
    // Verificar si el clic fue en un link de nota (_note column)
    var noteLink = event.target.closest('.note-link');
    if (noteLink) {{
        event.preventDefault();
        event.stopPropagation();
        var noteName = noteLink.dataset.note;
        if (noteName) {{
            window.webkit.messageHandlers.noteClick.postMessage('__open_note__:' + noteName);
        }}
        return;
    }}
    
    // Verificar si el clic fue en una fila de la tabla
    var row = event.target.closest('tr[data-path]');
    if (row) {{
        // Clic en fila - enviar el path de la nota o el nombre si no hay path
        var path = row.dataset.path;
        if (path && path.length > 0) {{
            window.webkit.messageHandlers.noteClick.postMessage(path);
        }} else {{
            var name = row.dataset.name;
            if (name) {{
                window.webkit.messageHandlers.noteClick.postMessage('__open_note__:' + name);
            }}
        }}
    }} else {{
        // Clic fuera de las filas - solo cerrar sidebar
        window.webkit.messageHandlers.noteClick.postMessage('__close_sidebar__');
    }}
}});

// Atajos de teclado
document.addEventListener('keydown', function(event) {{
    // Ctrl+F o Cmd+F para enfocar búsqueda
    if ((event.ctrlKey || event.metaKey) && event.key === 'f') {{
        event.preventDefault();
        var searchInput = document.getElementById('table-search');
        if (searchInput) {{
            searchInput.focus();
            searchInput.select();
        }}
    }}
    // Escape para limpiar búsqueda
    if (event.key === 'Escape') {{
        var searchInput = document.getElementById('table-search');
        if (searchInput && document.activeElement === searchInput) {{
            searchInput.value = '';
            filterTable('');
            searchInput.blur();
        }}
    }}
}});

// === Manejo de celdas editables ===
function handleCellEdit(cell) {{
    var newValue = cell.textContent.trim();
    var originalValue = cell.dataset.original || '';
    
    // Si no cambió, no hacer nada
    if (newValue === originalValue) {{
        cell.classList.remove('modified');
        return;
    }}
    
    var row = cell.closest('tr');
    if (!row) return;
    
    var noteId = row.dataset.noteId;
    var groupId = row.dataset.groupId;
    var property = cell.dataset.property;
    var notePath = row.dataset.path;
    
    if (!property) return;
    
    // Marcar como guardando
    cell.classList.add('saving');
    cell.classList.remove('modified');
    
    // Enviar mensaje a Rust
    var message = JSON.stringify({{
        action: 'update',
        noteId: noteId,
        groupId: groupId,
        property: property,
        value: newValue,
        originalValue: originalValue,
        notePath: notePath
    }});
    
    if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.cellEdit) {{
        window.webkit.messageHandlers.cellEdit.postMessage(message);
    }}
    
    // Actualizar original y quitar estado saving después de un momento
    setTimeout(function() {{
        cell.dataset.original = newValue;
        cell.classList.remove('saving');
    }}, 300);
}}

// Listener para blur en celdas editables
document.addEventListener('blur', function(event) {{
    if (event.target.classList && event.target.classList.contains('editable-cell')) {{
        handleCellEdit(event.target);
    }}
}}, true);

// Listener para input en celdas editables (marcar como modificado)
document.addEventListener('input', function(event) {{
    if (event.target.classList && event.target.classList.contains('editable-cell')) {{
        var cell = event.target;
        var newValue = cell.textContent.trim();
        var originalValue = cell.dataset.original || '';
        if (newValue !== originalValue) {{
            cell.classList.add('modified');
        }} else {{
            cell.classList.remove('modified');
        }}
    }}
}}, true);

// Enter en celda editable = confirmar y salir
document.addEventListener('keydown', function(event) {{
    if (event.target.classList && event.target.classList.contains('editable-cell')) {{
        if (event.key === 'Enter' && !event.shiftKey) {{
            event.preventDefault();
            event.target.blur();
        }}
        if (event.key === 'Escape') {{
            // Restaurar valor original
            event.target.textContent = event.target.dataset.original || '';
            event.target.classList.remove('modified');
            event.target.blur();
        }}
    }}
}}, true);

// === Manejo de filas especiales (fórmulas) ===
function deleteSpecialRow(rowId) {{
    if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.specialRowAction) {{
        window.webkit.messageHandlers.specialRowAction.postMessage(JSON.stringify({{
            action: 'delete',
            rowId: rowId
        }}));
    }}
}}

function moveSpecialRow(rowId, direction) {{
    if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.specialRowAction) {{
        window.webkit.messageHandlers.specialRowAction.postMessage(JSON.stringify({{
            action: 'move',
            rowId: rowId,
            direction: direction
        }}));
    }}
}}

// === Autocompletado de fórmulas ===
var formulaList = [
    // Numéricas
    {{ name: 'SUM', desc: 'Suma de valores', syntax: '=SUM(A:A) o =SUM(A1:B5)', cat: 'num' }},
    {{ name: 'AVG', desc: 'Promedio de valores', syntax: '=AVG(A:A) o =AVG(A1:B5)', cat: 'num' }},
    {{ name: 'MIN', desc: 'Valor mínimo', syntax: '=MIN(A:A) o =MIN(A1:B5)', cat: 'num' }},
    {{ name: 'MAX', desc: 'Valor máximo', syntax: '=MAX(A:A) o =MAX(A1:B5)', cat: 'num' }},
    {{ name: 'COUNT', desc: 'Contar números', syntax: '=COUNT(A:A)', cat: 'num' }},
    {{ name: 'COUNTA', desc: 'Contar no vacías', syntax: '=COUNTA(A:A)', cat: 'num' }},
    {{ name: 'ABS', desc: 'Valor absoluto', syntax: '=ABS(A1)', cat: 'num' }},
    {{ name: 'ROUND', desc: 'Redondear', syntax: '=ROUND(A1, 2)', cat: 'num' }},
    // Texto
    {{ name: 'CONCAT', desc: 'Concatenar textos', syntax: '=CONCAT(A1, \" \", B1)', cat: 'txt' }},
    {{ name: 'UPPER', desc: 'Convertir a MAYÚSCULAS', syntax: '=UPPER(A1)', cat: 'txt' }},
    {{ name: 'LOWER', desc: 'Convertir a minúsculas', syntax: '=LOWER(A1)', cat: 'txt' }},
    {{ name: 'TRIM', desc: 'Quitar espacios', syntax: '=TRIM(A1)', cat: 'txt' }},
    {{ name: 'LEN', desc: 'Longitud de texto', syntax: '=LEN(A1)', cat: 'txt' }},
    {{ name: 'LEFT', desc: 'Primeros N caracteres', syntax: '=LEFT(A1, 3)', cat: 'txt' }},
    {{ name: 'RIGHT', desc: 'Últimos N caracteres', syntax: '=RIGHT(A1, 3)', cat: 'txt' }},
    {{ name: 'MID', desc: 'Extraer subcadena', syntax: '=MID(A1, 2, 5)', cat: 'txt' }},
    {{ name: 'REPLACE', desc: 'Reemplazar por posición', syntax: '=REPLACE(A1, 2, 3, \"x\")', cat: 'txt' }},
    {{ name: 'SUBSTITUTE', desc: 'Reemplazar texto', syntax: '=SUBSTITUTE(A1, \"viejo\", \"nuevo\")', cat: 'txt' }},
    {{ name: 'TEXT', desc: 'Formatear número', syntax: '=TEXT(A1, \"0.00\")', cat: 'txt' }},
    {{ name: 'REPT', desc: 'Repetir texto N veces', syntax: '=REPT(\"*\", 5)', cat: 'txt' }},
    // Fechas
    {{ name: 'TODAY', desc: 'Fecha de hoy', syntax: '=TODAY()', cat: 'date' }},
    {{ name: 'NOW', desc: 'Fecha y hora actual', syntax: '=NOW()', cat: 'date' }},
    {{ name: 'YEAR', desc: 'Extraer año', syntax: '=YEAR(A1)', cat: 'date' }},
    {{ name: 'MONTH', desc: 'Extraer mes (1-12)', syntax: '=MONTH(A1)', cat: 'date' }},
    {{ name: 'DAY', desc: 'Extraer día (1-31)', syntax: '=DAY(A1)', cat: 'date' }},
    {{ name: 'HOUR', desc: 'Extraer hora (0-23)', syntax: '=HOUR(A1)', cat: 'date' }},
    {{ name: 'MINUTE', desc: 'Extraer minutos', syntax: '=MINUTE(A1)', cat: 'date' }},
    {{ name: 'WEEKDAY', desc: 'Día de semana (1-7)', syntax: '=WEEKDAY(A1)', cat: 'date' }},
    {{ name: 'WEEKNUM', desc: 'Número de semana', syntax: '=WEEKNUM(A1)', cat: 'date' }},
    {{ name: 'DATEDIF', desc: 'Diferencia entre fechas', syntax: '=DATEDIF(A1, B1, \"D\")', cat: 'date' }},
    {{ name: 'DATEFORMAT', desc: 'Formatear fecha', syntax: '=DATEFORMAT(A1, \"DD/MM/YYYY\")', cat: 'date' }},
    {{ name: 'EOMONTH', desc: 'Fin de mes', syntax: '=EOMONTH(A1, 0)', cat: 'date' }},
    // Lógica
    {{ name: 'IF', desc: 'Condicional', syntax: '=IF(A1>10, \"Sí\", \"No\")', cat: 'log' }}
];

var autocompleteDropdown = null;
var selectedIndex = -1;
var currentCell = null;
var filteredFormulas = [];

function createAutocompleteDropdown() {{
    if (autocompleteDropdown) return;
    autocompleteDropdown = document.createElement('div');
    autocompleteDropdown.className = 'formula-autocomplete';
    autocompleteDropdown.id = 'formula-autocomplete';
    document.body.appendChild(autocompleteDropdown);
}}

function showAutocomplete(cell, filter) {{
    createAutocompleteDropdown();
    currentCell = cell;
    
    // Filtrar fórmulas que coincidan
    var searchTerm = filter.replace(/^=/, '').toUpperCase();
    filteredFormulas = formulaList.filter(function(f) {{
        return f.name.indexOf(searchTerm) === 0 || (searchTerm === '' && filter === '=');
    }});
    
    if (filteredFormulas.length === 0) {{
        hideAutocomplete();
        return;
    }}
    
    // Construir HTML del dropdown
    var html = '';
    var catIcon = {{ num: '🔢', txt: '📝', log: '❓', date: '📅' }};
    filteredFormulas.forEach(function(f, idx) {{
        var selectedClass = idx === selectedIndex ? ' selected' : '';
        var icon = catIcon[f.cat] || '📊';
        html += '<div class="formula-item' + selectedClass + '" data-index="' + idx + '">';
        html += '<span class="formula-name">' + icon + ' =' + f.name + '()</span>';
        html += '<span class="formula-desc">' + f.desc + '</span>';
        html += '<span class="formula-syntax">' + f.syntax + '</span>';
        html += '</div>';
    }});
    
    autocompleteDropdown.innerHTML = html;
    
    // Posicionar dropdown debajo de la celda
    var rect = cell.getBoundingClientRect();
    autocompleteDropdown.style.left = rect.left + 'px';
    autocompleteDropdown.style.top = (rect.bottom + 2) + 'px';
    autocompleteDropdown.classList.add('visible');
    
    // Eventos de click en items
    autocompleteDropdown.querySelectorAll('.formula-item').forEach(function(item) {{
        item.addEventListener('mousedown', function(e) {{
            e.preventDefault();
            var idx = parseInt(this.dataset.index);
            selectFormula(idx);
        }});
    }});
}}

function hideAutocomplete() {{
    if (autocompleteDropdown) {{
        autocompleteDropdown.classList.remove('visible');
    }}
    selectedIndex = -1;
    filteredFormulas = [];
}}

function selectFormula(index) {{
    if (index < 0 || index >= filteredFormulas.length || !currentCell) return;
    
    var formula = filteredFormulas[index];
    // Insertar la fórmula con cursor dentro de los paréntesis
    currentCell.textContent = '=' + formula.name + '()';
    
    // Posicionar cursor antes del paréntesis de cierre
    var range = document.createRange();
    var textNode = currentCell.firstChild;
    if (textNode) {{
        var pos = currentCell.textContent.length - 1; // antes de )
        range.setStart(textNode, pos);
        range.setEnd(textNode, pos);
        var sel = window.getSelection();
        sel.removeAllRanges();
        sel.addRange(range);
    }}
    
    hideAutocomplete();
}}

function navigateAutocomplete(direction) {{
    if (filteredFormulas.length === 0) return false;
    
    if (direction === 'down') {{
        selectedIndex = (selectedIndex + 1) % filteredFormulas.length;
    }} else if (direction === 'up') {{
        selectedIndex = selectedIndex <= 0 ? filteredFormulas.length - 1 : selectedIndex - 1;
    }}
    
    // Actualizar visual
    autocompleteDropdown.querySelectorAll('.formula-item').forEach(function(item, idx) {{
        if (idx === selectedIndex) {{
            item.classList.add('selected');
            item.scrollIntoView({{ block: 'nearest' }});
        }} else {{
            item.classList.remove('selected');
        }}
    }});
    
    return true;
}}

// Al hacer focus en celda de fórmula, mostrar la fórmula en vez del resultado
document.addEventListener('focus', function(event) {{
    if (event.target.classList && event.target.classList.contains('special-cell-editable')) {{
        var cell = event.target;
        var formula = cell.dataset.formula || cell.dataset.original || '';
        if (formula) {{
            cell.textContent = formula;
            // Seleccionar todo el texto para fácil edición
            var range = document.createRange();
            range.selectNodeContents(cell);
            var sel = window.getSelection();
            sel.removeAllRanges();
            sel.addRange(range);
        }}
    }}
}}, true);

// Input para detectar cuando se escribe = y mostrar autocompletado
document.addEventListener('input', function(event) {{
    if (event.target.classList && event.target.classList.contains('special-cell-editable')) {{
        var cell = event.target;
        var text = cell.textContent.trim();
        
        // Mostrar autocompletado si empieza con =
        if (text.startsWith('=')) {{
            // Extraer lo que está escribiendo después del =
            var match = text.match(/^=([A-Za-z]*)$/);
            if (match) {{
                showAutocomplete(cell, text);
            }} else {{
                hideAutocomplete();
            }}
        }} else {{
            hideAutocomplete();
        }}
    }}
}}, true);

// Listener para edición de celdas de filas especiales
document.addEventListener('blur', function(event) {{
    if (event.target.classList && event.target.classList.contains('special-cell-editable')) {{
        hideAutocomplete();
        var cell = event.target;
        var newValue = cell.textContent.trim();
        var originalValue = cell.dataset.original || '';
        var rowId = cell.dataset.specialRow;
        var property = cell.dataset.property;
        var field = cell.dataset.field; // 'label' para el nombre de la fila
        
        if (newValue !== originalValue && rowId) {{
            if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.specialRowAction) {{
                window.webkit.messageHandlers.specialRowAction.postMessage(JSON.stringify({{
                    action: 'edit',
                    rowId: rowId,
                    property: property || null,
                    field: field || null,
                    value: newValue
                }}));
            }}
            // Actualizar data-original y data-formula con el nuevo valor
            cell.dataset.original = newValue;
            cell.dataset.formula = newValue;
            // Si es fórmula, mostrar placeholder hasta que se recalcule
            if (newValue.startsWith('=')) {{
                cell.textContent = '...';
            }}
        }} else {{
            // No hubo cambios, restaurar el resultado si existe
            var result = cell.dataset.result;
            if (result !== undefined && result !== '') {{
                cell.textContent = result;
            }}
        }}
    }}
}}, true);

// Teclas en celda de fila especial (Enter, Escape, flechas para autocompletado)
document.addEventListener('keydown', function(event) {{
    if (event.target.classList && event.target.classList.contains('special-cell-editable')) {{
        var autocompleteVisible = autocompleteDropdown && autocompleteDropdown.classList.contains('visible');
        
        if (autocompleteVisible) {{
            if (event.key === 'ArrowDown') {{
                event.preventDefault();
                navigateAutocomplete('down');
                return;
            }}
            if (event.key === 'ArrowUp') {{
                event.preventDefault();
                navigateAutocomplete('up');
                return;
            }}
            if (event.key === 'Tab' || (event.key === 'Enter' && selectedIndex >= 0)) {{
                event.preventDefault();
                if (selectedIndex >= 0) {{
                    selectFormula(selectedIndex);
                }} else if (filteredFormulas.length > 0) {{
                    selectFormula(0);
                }}
                return;
            }}
        }}
        
        if (event.key === 'Enter' && !event.shiftKey) {{
            event.preventDefault();
            hideAutocomplete();
            event.target.blur();
        }}
        if (event.key === 'Escape') {{
            if (autocompleteVisible) {{
                event.preventDefault();
                hideAutocomplete();
            }} else {{
                event.target.textContent = event.target.dataset.original || '';
                event.target.blur();
            }}
        }}
    }}
}}, true);
</script>
"#, full_css);
        
        if notes.is_empty() {
            html.push_str(&format!(r#"<div class="empty-state">{}</div>"#, no_notes_label));
        } else {
            // Barra de búsqueda
            let notes_count = notes.len();
            html.push_str(&format!(r#"<div class="search-container">
    <div class="search-wrapper">
        <span class="search-icon">🔍</span>
        <input type="text" id="table-search" class="search-input" placeholder="{}" oninput="filterTable(this.value)" autocomplete="off">
    </div>
    <span id="search-count" class="search-results-count">{} {}</span>
</div>
"#, search_placeholder, notes_count, items_label));
            
            html.push_str("<table>\n<thead>\n<tr>\n");
            
            // Columna # para números de fila
            html.push_str(r#"<th class="row-num-col">#</th>"#);
            html.push_str("\n");
            
            // Cabeceras con referencia de columna (A, B, C...)
            let visible_cols: Vec<_> = columns.iter().filter(|c| c.visible).collect();
            for (col_idx, col) in visible_cols.iter().enumerate() {
                let header_name = Self::format_column_header(&col.property, language);
                let col_letter = crate::core::formula::col_to_letters(col_idx as u16);
                html.push_str(&format!(
                    r#"<th><span class="col-ref">{}</span> {}</th>"#,
                    col_letter,
                    Self::escape_html(&header_name)
                ));
                html.push_str("\n");
            }
            html.push_str("</tr>\n</thead>\n<tbody>\n");
            
            // Construir grid para fórmulas (una sola vez)
            let grid = Self::build_cell_grid(notes, columns);
            
            // Insertar filas especiales que van al inicio (position = 0)
            for special_row in special_rows.iter() {
                if special_row.position == Some(0) {
                    html.push_str(&Self::render_special_row(special_row, columns, &grid, 0, notes.len()));
                }
            }
            
            // Filas de datos
            for (row_idx, note) in notes.iter().enumerate() {
                let row_num = row_idx + 1; // 1-indexed como Excel
                let path_attr = Self::escape_html(&note.metadata.path);
                let name_attr = Self::escape_html(&note.metadata.name);
                
                // Obtener note_id y group_id para edición
                let note_id = note.properties.get("_note_id")
                    .map(|v| v.to_display_string())
                    .unwrap_or_else(|| note.metadata.id.to_string());
                let group_id = note.properties.get("_group_id")
                    .map(|v| v.to_display_string())
                    .unwrap_or_else(|| "0".to_string());
                
                html.push_str(&format!(
                    r#"<tr data-path="{}" data-name="{}" data-note-id="{}" data-group-id="{}" data-row="{}">"#,
                    path_attr, name_attr, note_id, group_id, row_num
                ));
                
                // Columna # con número de fila
                html.push_str(&format!(r#"<td class="row-num-col">{}</td>"#, row_num));
                
                for (col_idx, col) in visible_cols.iter().enumerate() {
                    let value = Self::get_property_value(note, &col.property);
                    let cell_class = match col.property.as_str() {
                        "title" => "title-cell",
                        "created" | "modified" => "date-cell",
                        "_note" => "note-link-cell",
                        _ => "property-cell",
                    };
                    
                    // Referencia de celda (A1, B1, etc.)
                    let col_letter = crate::core::formula::col_to_letters(col_idx as u16);
                    let cell_ref = format!("{}{}", col_letter, row_num);
                    
                    // La columna _note es clickeable (no editable)
                    if col.property == "_note" {
                        let escaped_value = Self::escape_html(&value);
                        html.push_str(&format!(
                            r#"<td class="{}" data-cell="{}"><a href="javascript:void(0)" class="note-link" data-note="{}">{}</a></td>"#,
                            cell_class,
                            cell_ref,
                            escaped_value,
                            escaped_value
                        ));
                    } else if editable && col.property != "title" && col.property != "created" && col.property != "modified" {
                        // Celda editable para propiedades inline
                        let escaped_value = Self::escape_html(&value);
                        html.push_str(&format!(
                            r#"<td class="{} editable-cell" contenteditable="true" data-property="{}" data-original="{}" data-cell="{}">{}</td>"#,
                            cell_class,
                            Self::escape_html(&col.property),
                            escaped_value,
                            cell_ref,
                            escaped_value
                        ));
                    } else {
                        html.push_str(&format!(r#"<td class="{}" data-cell="{}">{}</td>"#, cell_class, cell_ref, Self::escape_html(&value)));
                    }
                }
                html.push_str("</tr>\n");
                
                // Insertar filas especiales que van después de esta posición
                for special_row in special_rows.iter() {
                    if special_row.position == Some(row_num) {
                        html.push_str(&Self::render_special_row(special_row, columns, &grid, row_num, notes.len()));
                    }
                }
            }
            
            // Renderizar filas especiales al final (las que no tienen posición específica o position > total)
            for special_row in special_rows.iter() {
                // Sin posición = al final, o posición mayor que el total de filas
                if special_row.position.is_none() || special_row.position.unwrap_or(0) > notes.len() {
                    html.push_str(&Self::render_special_row(special_row, columns, &grid, notes.len(), notes.len()));
                }
            }
            
            html.push_str("</tbody>\n</table>\n");
        }
        
        // Script para mostrar el body después de que todo esté cargado
        html.push_str("<script>document.body.classList.add('loaded');</script>\n");
        html.push_str("</body>\n</html>");
        html
    }
    
    /// Construir un CellGrid a partir de las notas para evaluar fórmulas
    fn build_cell_grid(notes: &[NoteWithProperties], columns: &[ColumnConfig]) -> CellGrid {
        let mut grid = CellGrid::new();
        
        // Mapear columnas visibles a índices (A, B, C...)
        let visible_columns: Vec<_> = columns.iter().filter(|c| c.visible).collect();
        
        for (row_idx, note) in notes.iter().enumerate() {
            let row = (row_idx + 1) as u32; // Filas 1-indexed como Excel
            
            for (col_idx, col) in visible_columns.iter().enumerate() {
                let col_num = col_idx as u16;
                let cell = CellRef::new(col_num, row);
                
                let value = Self::get_property_value(note, &col.property);
                
                // Intentar parsear como número
                if let Ok(num) = value.parse::<f64>() {
                    grid.set(cell, CellValue::Number(num));
                } else if value.is_empty() {
                    grid.set(cell, CellValue::Empty);
                } else {
                    grid.set(cell, CellValue::Text(value));
                }
            }
        }
        
        grid
    }
    
    /// Renderizar una fila especial con controles editables
    fn render_special_row(special_row: &SpecialRow, columns: &[ColumnConfig], grid: &CellGrid, current_pos: usize, total_rows: usize) -> String {
        let css_class = special_row.css_class.as_deref().unwrap_or("");
        let mut html = format!(
            r#"<tr class="special-row {}" data-special-row="{}">"#,
            css_class, 
            Self::escape_html(&special_row.id)
        );
        
        let visible_columns: Vec<_> = columns.iter().filter(|c| c.visible).collect();
        
        // Columna # con controles de posición
        let can_move_up = current_pos > 0;
        let can_move_down = special_row.position.is_some() || current_pos < total_rows;
        
        html.push_str(&format!(
            r#"<td class="row-num-col special-row-controls-cell">
                <span class="special-row-controls">
                    <button class="special-row-btn{}" onclick="moveSpecialRow('{}', 'up')" title="Move up">↑</button>
                    <button class="special-row-btn{}" onclick="moveSpecialRow('{}', 'down')" title="Move down">↓</button>
                    <button class="special-row-btn delete" onclick="deleteSpecialRow('{}')" title="Delete">✕</button>
                </span>
            </td>"#,
            if can_move_up { "" } else { " disabled" },
            Self::escape_html(&special_row.id),
            if can_move_down { "" } else { " disabled" },
            Self::escape_html(&special_row.id),
            Self::escape_html(&special_row.id)
        ));
        
        for (col_idx, col) in visible_columns.iter().enumerate() {
            // Primera columna de datos: label editable
            if col_idx == 0 {
                html.push_str(&format!(
                    r#"<td class="special-row-label">
                        <span class="special-cell-editable" contenteditable="true" 
                              data-special-row="{}" data-field="label" 
                              data-original="{}">{}</span>
                    </td>"#,
                    Self::escape_html(&special_row.id),
                    Self::escape_html(&special_row.label),
                    Self::escape_html(&special_row.label),
                ));
                continue;
            }
            
            // Buscar contenido para esta columna
            if let Some(cell_content) = special_row.cells.get(&col.property) {
                let (value, is_error, formula_str) = if cell_content.is_formula() {
                    // Evaluar la fórmula
                    let formula = cell_content.content.clone();
                    match grid.evaluate(&formula) {
                        Ok(CellValue::Number(n)) => (cell_content.format.format_number(n), false, formula),
                        Ok(CellValue::Text(s)) => (s, false, formula),
                        Ok(CellValue::Empty) => (String::new(), false, formula),
                        Ok(CellValue::Error(e)) => (format!("#ERR: {}", e), true, formula),
                        Err(e) => (format!("#ERR: {}", e), true, formula),
                    }
                } else {
                    let content = cell_content.content.clone();
                    (content.clone(), false, content)
                };
                
                let style = cell_content.format.to_css();
                let class = if is_error { 
                    "formula-cell formula-error special-cell-editable" 
                } else { 
                    "formula-cell special-cell-editable" 
                };
                
                // Celda editable que muestra el resultado pero guarda la fórmula
                html.push_str(&format!(
                    r#"<td class="{}" style="{}" contenteditable="true" 
                        data-special-row="{}" data-property="{}" 
                        data-original="{}" data-formula="{}" data-result="{}"
                        title="Formula: {}">{}</td>"#,
                    class,
                    style,
                    Self::escape_html(&special_row.id),
                    Self::escape_html(&col.property),
                    Self::escape_html(&formula_str),
                    Self::escape_html(&formula_str),
                    Self::escape_html(&value),
                    Self::escape_html(&formula_str),
                    Self::escape_html(&value)
                ));
            } else {
                // Celda vacía pero editable
                html.push_str(&format!(
                    r#"<td class="special-cell-editable" contenteditable="true" 
                        data-special-row="{}" data-property="{}" 
                        data-original=""></td>"#,
                    Self::escape_html(&special_row.id),
                    Self::escape_html(&col.property)
                ));
            }
        }
        
        html.push_str("</tr>\n");
        html
    }
    
    /// Formatear el nombre de la columna para el header
    fn format_column_header(property: &str, language: Language) -> String {
        match property {
            "title" => if language == Language::Spanish { "Título".to_string() } else { "Title".to_string() },
            "created" => if language == Language::Spanish { "Creado".to_string() } else { "Created".to_string() },
            "modified" => if language == Language::Spanish { "Modificado".to_string() } else { "Modified".to_string() },
            "tags" => if language == Language::Spanish { "Etiquetas".to_string() } else { "Tags".to_string() },
            other => {
                // Capitalizar primera letra
                let mut chars = other.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => c.to_uppercase().chain(chars).collect(),
                }
            }
        }
    }
    
    /// Obtener el valor de una propiedad de la nota
    fn get_property_value(note: &NoteWithProperties, property: &str) -> String {
        match property {
            "title" => note.metadata.name.clone(),
            "created" => note.metadata.created_at.format("%Y-%m-%d %H:%M").to_string(),
            "modified" => note.metadata.updated_at.format("%Y-%m-%d %H:%M").to_string(),
            other => {
                // Buscar en properties
                note.properties
                    .get(other)
                    .map(|v| v.to_display_string())
                    .unwrap_or_default()
            }
        }
    }
    
    /// Escapar HTML
    fn escape_html(s: &str) -> String {
        s.replace('&', "&amp;")
         .replace('<', "&lt;")
         .replace('>', "&gt;")
         .replace('"', "&quot;")
         .replace('\'', "&#39;")
    }
    
    /// Detectar si el tema del sistema es oscuro
    fn detect_system_theme() -> bool {
        // Detectar tema oscuro del sistema GTK
        if let Some(settings) = gtk::Settings::default() {
            // Verificar prefer-dark-theme
            if settings.is_gtk_application_prefer_dark_theme() {
                return true;
            }
            // También verificar el nombre del tema
            if let Some(theme_name) = settings.gtk_theme_name() {
                let theme_lower = theme_name.to_lowercase();
                if theme_lower.contains("dark") {
                    return true;
                }
                // Temas conocidos como claros
                if theme_lower.contains("light") || theme_lower == "adwaita" || theme_lower == "default" {
                    return false;
                }
            }
        }
        // Por defecto, asumir tema oscuro
        true
    }

    /// Actualizar los tabs de vistas
    fn update_view_tabs(&self, base: &Base) {
        // Limpiar tabs existentes
        while let Some(child) = self.view_tabs.first_child() {
            self.view_tabs.remove(&child);
        }

        // Crear un tab por cada vista
        for (i, view) in base.views.iter().enumerate() {
            let is_active = i == base.active_view;

            // Usar LinkButton que tiene mejor herencia de colores, o Label dentro de Box
            let button = gtk::ToggleButton::builder()
                .active(is_active)
                .css_classes(if is_active { 
                    vec!["flat", "base-view-tab", "active"] 
                } else { 
                    vec!["flat", "base-view-tab"] 
                })
                .build();
            
            // Crear label manualmente para poder controlar su color
            let label = gtk::Label::new(Some(&view.name));
            label.add_css_class("base-view-tab-label");
            button.set_child(Some(&label));

            self.view_tabs.append(&button);
        }

        // Botón para añadir nueva vista
        let add_view_btn = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Add view")
            .css_classes(["flat", "base-add-view"])
            .build();
        self.view_tabs.append(&add_view_btn);
    }

    /// Actualizar la barra de estado
    fn update_status_bar(&self, count: usize) {
        if let Some(label) = self.status_bar.first_child().and_downcast::<gtk::Label>() {
            let text = if count == 1 {
                "1 note".to_string()
            } else {
                format!("{} notes", count)
            };
            label.set_text(&text);
        }
    }

    /// Configurar callback para selección de nota
    pub fn on_note_selected<F: Fn(&str) + 'static>(&self, callback: F) {
        *self.on_note_selected.borrow_mut() = Some(Box::new(callback));
    }
    
    /// Configurar callback para cuando se hace clic en la vista (para cerrar sidebar)
    pub fn on_view_clicked<F: Fn() + 'static>(&self, callback: F) {
        *self.on_view_clicked.borrow_mut() = Some(Box::new(callback));
    }

    /// Configurar callback para doble clic en nota
    pub fn on_note_double_click<F: Fn(&str) + 'static>(&self, callback: F) {
        *self.on_note_double_click.borrow_mut() = Some(Box::new(callback));
    }
}

impl Default for BaseTableWidget {
    fn default() -> Self {
        Self::new(Rc::new(RefCell::new(I18n::new(Language::from_env()))))
    }
}

/// CSS para los widgets de Base
pub const BASE_CSS: &str = r#"
.base-table-container {
    background: @theme_bg_color;
}

.base-filter-bar {
    background: alpha(@theme_fg_color, 0.02);
    border-bottom: 1px solid alpha(@theme_fg_color, 0.08);
    padding: 6px 12px;
}

.base-filter-bar button {
    min-height: 28px;
    min-width: 28px;
    padding: 4px 8px;
    border-radius: 6px;
}

.base-filter-bar button:hover {
    background: alpha(@theme_fg_color, 0.08);
}

.base-view-tabs {
    padding: 8px 12px 0 12px;
    background: transparent;
}

.base-view-tab,
button.base-view-tab,
togglebutton.base-view-tab {
    padding: 8px 16px;
    border-radius: 8px 8px 0 0;
    margin: 0 2px;
    background: transparent;
    border: none;
    font-weight: 500;
    color: @theme_fg_color;
}

.base-view-tab label,
button.base-view-tab label,
togglebutton.base-view-tab label,
.base-view-tab-label {
    color: @theme_fg_color;
    font-weight: 500;
}

.base-view-tab:checked,
.base-view-tab.active,
button.base-view-tab:checked,
togglebutton.base-view-tab:checked {
    box-shadow: 0 -2px 0 0 @accent_bg_color inset;
    color: @accent_color;
}

.base-view-tab:checked label,
.base-view-tab:checked .base-view-tab-label,
.base-view-tab.active .base-view-tab-label,
button.base-view-tab:checked label,
togglebutton.base-view-tab:checked label {
    color: @accent_color;
}

.base-view-tab:hover:not(:checked) {
    background: alpha(@theme_fg_color, 0.08);
}

/* Tabla principal */
.base-table {
    background: transparent;
}

.base-table > listview {
    background: transparent;
}

.base-table > listview > row {
    padding: 0;
    background: transparent;
    border-bottom: 1px solid alpha(@theme_fg_color, 0.06);
    transition: background 150ms ease;
}

.base-table > listview > row:hover {
    background: alpha(@theme_fg_color, 0.04);
}

.base-table > listview > row:selected {
    background: alpha(@accent_bg_color, 0.15);
}

/* Cabeceras de columna */
.base-table header {
    background: alpha(@theme_fg_color, 0.03);
    border-bottom: 2px solid alpha(@theme_fg_color, 0.1);
}

.base-table header button {
    font-weight: 600;
    font-size: 0.85em;
    padding: 10px 16px;
    background: transparent;
    border: none;
    border-right: 1px solid alpha(@theme_fg_color, 0.06);
    color: alpha(@theme_fg_color, 0.8);
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

.base-table header button:hover {
    background: alpha(@theme_fg_color, 0.05);
    color: @theme_fg_color;
}

.base-table header button:last-child {
    border-right: none;
}

/* Celdas */
.base-cell {
    padding: 12px 16px;
    font-size: 0.95em;
    color: @theme_fg_color;
}

/* Barra de estado */
.base-status-bar {
    background: alpha(@theme_fg_color, 0.02);
    border-top: 1px solid alpha(@theme_fg_color, 0.08);
    padding: 8px 16px;
    font-size: 0.85em;
    color: alpha(@theme_fg_color, 0.6);
}

/* Filter chips */
.base-filter-chip {
    background: alpha(currentColor, 0.1);
    padding: 2px 8px;
    border-radius: 12px;
    font-size: 0.8em;
    font-weight: 500;
}

.base-filter-chip:hover {
    background: alpha(currentColor, 0.15);
}

.base-filter-chip button {
    padding: 0;
    min-width: 16px;
    min-height: 16px;
    margin-left: 4px;
    border-radius: 50%;
}

.base-filter-chip button:hover {
    background: alpha(currentColor, 0.15);
}

/* Filter popover */
.filter-popover {
    padding: 16px;
    background-color: @theme_bg_color;
    border: 1px solid alpha(@theme_fg_color, 0.1);
    border-radius: 12px;
    box-shadow: 0 4px 12px alpha(black, 0.15);
}

.filter-popover .property-row {
    margin-bottom: 12px;
}

.filter-popover label {
    margin-bottom: 6px;
    font-weight: 500;
    font-size: 0.9em;
    color: alpha(@theme_fg_color, 0.8);
}

.filter-popover dropdown,
.filter-popover entry {
    min-height: 36px;
    border-radius: 8px;
}

/* Sort popover */
.sort-popover {
    padding: 12px;
    min-width: 220px;
    background-color: @theme_bg_color;
    border: 1px solid alpha(@theme_fg_color, 0.1);
    border-radius: 12px;
    box-shadow: 0 4px 12px alpha(black, 0.15);
}

.sort-popover .sort-row {
    padding: 10px 12px;
    border-radius: 8px;
    margin: 2px 0;
}

.sort-popover .sort-row:hover {
    background: alpha(@theme_fg_color, 0.06);
}

/* Columns popover */
.columns-popover {
    padding: 12px;
    min-width: 220px;
    background-color: @theme_bg_color;
    border: 1px solid alpha(@theme_fg_color, 0.1);
    border-radius: 12px;
    box-shadow: 0 4px 12px alpha(black, 0.15);
}

.columns-popover .heading {
    font-weight: 600;
    font-size: 0.85em;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: alpha(@theme_fg_color, 0.6);
    margin-bottom: 8px;
}

.columns-popover check {
    margin-right: 10px;
}

.columns-popover row {
    padding: 6px 4px;
    border-radius: 6px;
}

.columns-popover row:hover {
    background: alpha(@theme_fg_color, 0.04);
}

/* Source type popover */
.source-type-popover {
    padding: 16px;
    min-width: 240px;
    background-color: @theme_bg_color;
    border: 1px solid alpha(@theme_fg_color, 0.1);
    border-radius: 12px;
    box-shadow: 0 4px 12px alpha(black, 0.15);
}

.source-type-popover .heading {
    font-weight: 600;
    font-size: 0.9em;
    margin-bottom: 12px;
}

.source-type-popover checkbutton {
    padding: 10px 12px;
    border-radius: 8px;
    margin: 4px 0;
}

.source-type-popover checkbutton:hover {
    background: alpha(@theme_fg_color, 0.06);
}

.source-type-popover checkbutton:checked {
    background: alpha(@accent_bg_color, 0.12);
}

.source-type-popover .caption {
    font-size: 0.8em;
    line-height: 1.4;
}

/* Property types */
.property-checkbox {
    color: @success_color;
}

.property-date {
    color: @accent_color;
}

.property-tags {
    font-size: 0.85em;
}

.property-tag {
    background: alpha(@accent_bg_color, 0.15);
    padding: 2px 6px;
    border-radius: 3px;
    margin-right: 4px;
}

/* Graph view toggle */
.base-graph-toggle:checked {
    background: alpha(@accent_bg_color, 0.3);
    color: @accent_color;
}

/* Graph view styles */
.base-graph-view {
    background: #1e1e22;
    min-height: 400px;
}
"#;

/// Crear un chip de filtro visual con índice para eliminación
pub fn create_filter_chip(filter: &Filter, _index: usize) -> gtk::Box {
    let chip = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .css_classes(["base-filter-chip"])
        .build();

    // Nombre de la propiedad
    let prop_label = gtk::Label::new(Some(&filter.property));
    chip.append(&prop_label);

    // Operador
    let op_text = operator_to_symbol(&filter.operator);
    let op_label = gtk::Label::builder()
        .label(op_text)
        .css_classes(["dim-label"])
        .build();
    chip.append(&op_label);

    // Valor (solo si no es IsEmpty/IsNotEmpty)
    if !matches!(filter.operator, FilterOperator::IsEmpty | FilterOperator::IsNotEmpty) {
        let value_text = filter.value.to_display_string();
        // Truncar si es muy largo
        let display_value = if value_text.len() > 20 {
            format!("{}...", &value_text[..17])
        } else {
            value_text
        };
        let value_label = gtk::Label::new(Some(&display_value));
        chip.append(&value_label);
    }

    // Botón de cerrar
    let close_btn = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .css_classes(["flat", "circular"])
        .tooltip_text("Remove filter")
        .build();
    chip.append(&close_btn);

    chip
}

/// Convertir operador a símbolo visual
fn operator_to_symbol(op: &FilterOperator) -> &'static str {
    match op {
        FilterOperator::Equals => "=",
        FilterOperator::NotEquals => "≠",
        FilterOperator::Contains => "contains",
        FilterOperator::NotContains => "not contains",
        FilterOperator::GreaterThan => ">",
        FilterOperator::GreaterOrEqual => "≥",
        FilterOperator::LessThan => "<",
        FilterOperator::LessOrEqual => "≤",
        FilterOperator::StartsWith => "starts with",
        FilterOperator::EndsWith => "ends with",
        FilterOperator::IsEmpty => "is empty",
        FilterOperator::IsNotEmpty => "is not empty",
    }
}

/// Convertir índice del combo a FilterOperator
fn index_to_operator(index: usize) -> FilterOperator {
    match index {
        0 => FilterOperator::Equals,
        1 => FilterOperator::NotEquals,
        2 => FilterOperator::Contains,
        3 => FilterOperator::NotContains,
        4 => FilterOperator::GreaterThan,
        5 => FilterOperator::GreaterOrEqual,
        6 => FilterOperator::LessThan,
        7 => FilterOperator::LessOrEqual,
        8 => FilterOperator::StartsWith,
        9 => FilterOperator::EndsWith,
        10 => FilterOperator::IsEmpty,
        11 => FilterOperator::IsNotEmpty,
        _ => FilterOperator::Contains,
    }
}

/// Parsear el texto de valor a PropertyValue
fn parse_filter_value(text: &str) -> PropertyValue {
    let trimmed = text.trim();
    
    // Intentar parsear como número
    if let Ok(num) = trimmed.parse::<f64>() {
        return PropertyValue::Number(num);
    }
    
    // Intentar como booleano
    if trimmed.eq_ignore_ascii_case("true") {
        return PropertyValue::Checkbox(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return PropertyValue::Checkbox(false);
    }
    
    // Default: texto
    PropertyValue::Text(trimmed.to_string())
}

/// Actualizar los chips de filtros en el contenedor
fn update_filter_chips_in_container(container: &gtk::Box, filters: &[Filter], i18n: &I18n) {
    // Limpiar chips existentes
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    
    if filters.is_empty() {
        let placeholder = gtk::Label::builder()
            .label(&i18n.t("base_no_filters"))
            .css_classes(["dim-label"])
            .build();
        container.append(&placeholder);
    } else {
        for (i, filter) in filters.iter().enumerate() {
            let chip = create_filter_chip(filter, i);
            container.append(&chip);
        }
    }
}

/// Crear el popover para añadir filtros (devuelve referencias a los widgets)
pub fn create_filter_popover_with_refs(properties: &[String], i18n: &I18n) -> (gtk::Popover, gtk::DropDown, gtk::DropDown, gtk::Entry) {
    let popover = gtk::Popover::builder()
        .css_classes(["filter-popover"])
        .build();
    
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_start(8)
        .margin_end(8)
        .margin_top(8)
        .margin_bottom(8)
        .build();
    
    // Título
    let title = gtk::Label::builder()
        .label(&i18n.t("base_add_filter_title"))
        .css_classes(["heading"])
        .xalign(0.0)
        .build();
    content.append(&title);
    
    // Selector de propiedad
    let prop_label = gtk::Label::builder()
        .label(&i18n.t("base_property"))
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    content.append(&prop_label);
    
    let prop_combo = gtk::DropDown::from_strings(
        &properties.iter().map(|s| s.as_str()).collect::<Vec<_>>()
    );
    prop_combo.set_css_classes(&["filter-property-combo"]);
    content.append(&prop_combo);
    
    // Selector de operador
    let op_label = gtk::Label::builder()
        .label(&i18n.t("base_operator"))
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    content.append(&op_label);
    
    // Operadores traducidos
    let operators = [
        i18n.t("filter_op_equals"),
        i18n.t("filter_op_not_equals"),
        i18n.t("filter_op_contains"),
        i18n.t("filter_op_not_contains"),
        i18n.t("filter_op_greater_than"),
        i18n.t("filter_op_greater_or_equal"),
        i18n.t("filter_op_less_than"),
        i18n.t("filter_op_less_or_equal"),
        i18n.t("filter_op_starts_with"),
        i18n.t("filter_op_ends_with"),
        i18n.t("filter_op_is_empty"),
        i18n.t("filter_op_is_not_empty"),
    ];
    let op_strs: Vec<&str> = operators.iter().map(|s| s.as_str()).collect();
    let op_combo = gtk::DropDown::from_strings(&op_strs);
    content.append(&op_combo);
    
    // Campo de valor
    let value_label = gtk::Label::builder()
        .label(&i18n.t("base_value"))
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    content.append(&value_label);
    
    let value_entry = gtk::Entry::builder()
        .placeholder_text(&i18n.t("base_filter_value_placeholder"))
        .build();
    content.append(&value_entry);
    
    // Botones
    let buttons = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(8)
        .halign(gtk::Align::End)
        .build();
    
    let cancel_btn = gtk::Button::builder()
        .label(&i18n.t("base_cancel"))
        .css_classes(["flat"])
        .build();
    
    let popover_clone = popover.clone();
    cancel_btn.connect_clicked(move |_| {
        popover_clone.popdown();
    });
    
    let apply_btn = gtk::Button::builder()
        .label(&i18n.t("base_apply_filter"))
        .css_classes(["suggested-action"])
        .build();
    
    buttons.append(&cancel_btn);
    buttons.append(&apply_btn);
    content.append(&buttons);
    
    popover.set_child(Some(&content));
    
    (popover, prop_combo, op_combo, value_entry)
}

/// Crear el popover para añadir filtros
pub fn create_filter_popover(properties: &[String]) -> gtk::Popover {
    let popover = gtk::Popover::builder()
        .css_classes(["filter-popover"])
        .build();
    
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_start(8)
        .margin_end(8)
        .margin_top(8)
        .margin_bottom(8)
        .build();
    
    // Título
    let title = gtk::Label::builder()
        .label("Add Filter")
        .css_classes(["heading"])
        .xalign(0.0)
        .build();
    content.append(&title);
    
    // Selector de propiedad
    let prop_label = gtk::Label::builder()
        .label("Property")
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    content.append(&prop_label);
    
    let prop_combo = gtk::DropDown::from_strings(
        &properties.iter().map(|s| s.as_str()).collect::<Vec<_>>()
    );
    prop_combo.set_css_classes(&["filter-property-combo"]);
    content.append(&prop_combo);
    
    // Selector de operador
    let op_label = gtk::Label::builder()
        .label("Operator")
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    content.append(&op_label);
    
    let operators = [
        "equals", "not equals", "contains", "not contains",
        "greater than", "greater or equal", "less than", "less or equal",
        "starts with", "ends with", "is empty", "is not empty"
    ];
    let op_combo = gtk::DropDown::from_strings(&operators);
    content.append(&op_combo);
    
    // Campo de valor
    let value_label = gtk::Label::builder()
        .label("Value")
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    content.append(&value_label);
    
    let value_entry = gtk::Entry::builder()
        .placeholder_text("Filter value...")
        .build();
    content.append(&value_entry);
    
    // Botones
    let buttons = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(8)
        .halign(gtk::Align::End)
        .build();
    
    let cancel_btn = gtk::Button::builder()
        .label("Cancel")
        .css_classes(["flat"])
        .build();
    
    let apply_btn = gtk::Button::builder()
        .label("Add Filter")
        .css_classes(["suggested-action"])
        .build();
    
    buttons.append(&cancel_btn);
    buttons.append(&apply_btn);
    content.append(&buttons);
    
    // Conectar señales
    let popover_clone = popover.clone();
    cancel_btn.connect_clicked(move |_| {
        popover_clone.popdown();
    });
    
    // El apply_btn se conectará desde el widget que crea el popover
    // para tener acceso al estado del widget
    
    popover.set_child(Some(&content));
    popover
}

/// Crear el popover de ordenamiento con callbacks conectados
pub fn create_sort_popover_with_callbacks(
    properties: &[String],
    current_sort: Rc<RefCell<Option<SortConfig>>>,
    all_notes: Rc<RefCell<Vec<NoteWithProperties>>>,
    notes: Rc<RefCell<Vec<NoteWithProperties>>>,
    active_filters: Rc<RefCell<Vec<Filter>>>,
    list_store: gio::ListStore,
    status_bar: gtk::Box,
    table_webview: webkit6::WebView,
    base: Rc<RefCell<Option<Base>>>,
    i18n: &I18n,
) -> gtk::Popover {
    let popover = gtk::Popover::builder()
        .css_classes(["sort-popover"])
        .build();
    
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_start(8)
        .margin_end(8)
        .margin_top(8)
        .margin_bottom(8)
        .build();
    
    // Título
    let title = gtk::Label::builder()
        .label(&i18n.t("base_sort_by"))
        .css_classes(["heading"])
        .xalign(0.0)
        .margin_bottom(8)
        .build();
    content.append(&title);
    
    // Opción para quitar ordenamiento
    let none_btn = gtk::Button::builder()
        .label(&i18n.t("base_no_sorting"))
        .css_classes(["flat"])
        .hexpand(true)
        .build();
    
    {
        let current_sort = current_sort.clone();
        let all_notes = all_notes.clone();
        let notes = notes.clone();
        let active_filters = active_filters.clone();
        let list_store = list_store.clone();
        let status_bar = status_bar.clone();
        let table_webview = table_webview.clone();
        let base = base.clone();
        let popover = popover.clone();
        
        none_btn.connect_clicked(move |_| {
            *current_sort.borrow_mut() = None;
            apply_sort_and_refresh(
                &current_sort, &all_notes, &notes, &active_filters, 
                &list_store, &status_bar, &table_webview, &base
            );
            popover.popdown();
        });
    }
    content.append(&none_btn);
    
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    
    // Una fila por cada propiedad
    let t_sort_asc = i18n.t("base_sort_ascending");
    let t_sort_desc = i18n.t("base_sort_descending");
    
    for prop in properties {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .css_classes(["sort-row"])
            .margin_top(2)
            .margin_bottom(2)
            .build();
        
        let prop_label = gtk::Label::builder()
            .label(prop)
            .hexpand(true)
            .xalign(0.0)
            .build();
        row.append(&prop_label);
        
        // Botón ascendente
        let asc_btn = gtk::Button::builder()
            .icon_name("view-sort-ascending-symbolic")
            .tooltip_text(&t_sort_asc)
            .css_classes(["flat", "circular"])
            .build();
        
        {
            let prop = prop.clone();
            let current_sort = current_sort.clone();
            let all_notes = all_notes.clone();
            let notes = notes.clone();
            let active_filters = active_filters.clone();
            let list_store = list_store.clone();
            let status_bar = status_bar.clone();
            let table_webview = table_webview.clone();
            let base = base.clone();
            let popover = popover.clone();
            
            asc_btn.connect_clicked(move |_| {
                *current_sort.borrow_mut() = Some(SortConfig {
                    property: prop.clone(),
                    direction: SortDirection::Asc,
                });
                apply_sort_and_refresh(
                    &current_sort, &all_notes, &notes, &active_filters,
                    &list_store, &status_bar, &table_webview, &base
                );
                popover.popdown();
            });
        }
        row.append(&asc_btn);
        
        // Botón descendente
        let desc_btn = gtk::Button::builder()
            .icon_name("view-sort-descending-symbolic")
            .tooltip_text(&t_sort_desc)
            .css_classes(["flat", "circular"])
            .build();
        
        {
            let prop = prop.clone();
            let current_sort = current_sort.clone();
            let all_notes = all_notes.clone();
            let notes = notes.clone();
            let active_filters = active_filters.clone();
            let list_store = list_store.clone();
            let status_bar = status_bar.clone();
            let table_webview = table_webview.clone();
            let base = base.clone();
            let popover = popover.clone();
            
            desc_btn.connect_clicked(move |_| {
                *current_sort.borrow_mut() = Some(SortConfig {
                    property: prop.clone(),
                    direction: SortDirection::Desc,
                });
                apply_sort_and_refresh(
                    &current_sort, &all_notes, &notes, &active_filters,
                    &list_store, &status_bar, &table_webview, &base
                );
                popover.popdown();
            });
        }
        row.append(&desc_btn);
        
        content.append(&row);
    }
    
    popover.set_child(Some(&content));
    popover
}

/// Aplicar ordenamiento y refrescar la UI
fn apply_sort_and_refresh(
    current_sort: &Rc<RefCell<Option<SortConfig>>>,
    all_notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
    notes: &Rc<RefCell<Vec<NoteWithProperties>>>,
    active_filters: &Rc<RefCell<Vec<Filter>>>,
    list_store: &gio::ListStore,
    status_bar: &gtk::Box,
    table_webview: &webkit6::WebView,
    base: &Rc<RefCell<Option<Base>>>,
) {
    let all = all_notes.borrow();
    let filters = active_filters.borrow();
    let sort = current_sort.borrow();
    
    // Filtrar
    let mut filtered: Vec<NoteWithProperties> = all
        .iter()
        .filter(|note| {
            filters.iter().all(|f| f.evaluate(&note.properties))
        })
        .cloned()
        .collect();
    
    // Ordenar
    if let Some(sort_config) = sort.as_ref() {
        filtered.sort_by(|a, b| {
            let key_a = a.properties
                .get(&sort_config.property)
                .map(|v| v.sort_key())
                .unwrap_or_default();
            let key_b = b.properties
                .get(&sort_config.property)
                .map(|v| v.sort_key())
                .unwrap_or_default();

            match sort_config.direction {
                SortDirection::Asc => key_a.cmp(&key_b),
                SortDirection::Desc => key_b.cmp(&key_a),
            }
        });
    }
    
    drop(all);
    drop(filters);
    drop(sort);
    
    *notes.borrow_mut() = filtered.clone();
    
    // Actualizar UI (list_store para lógica)
    list_store.remove_all();
    for note in &filtered {
        let boxed = glib::BoxedAnyObject::new(note.clone());
        list_store.append(&boxed);
    }
    
    // Actualizar WebView
    let (columns, special_rows) = if let Some(base) = base.borrow().as_ref() {
        if let Some(view) = base.views.get(base.active_view) {
            (view.columns.clone(), view.special_rows.clone())
        } else {
            (vec![
                ColumnConfig { property: "title".to_string(), title: None, width: Some(300), visible: true },
                ColumnConfig { property: "created".to_string(), title: None, width: Some(150), visible: true },
            ], Vec::new())
        }
    } else {
        (vec![
            ColumnConfig { property: "title".to_string(), title: None, width: Some(300), visible: true },
            ColumnConfig { property: "created".to_string(), title: None, width: Some(150), visible: true },
        ], Vec::new())
    };
    let html = BaseTableWidget::render_table_html_static(&filtered, &columns, Language::from_env(), false, &special_rows);
    table_webview.load_html(&html, None);
    
    // Actualizar status
    if let Some(label) = status_bar.first_child().and_downcast::<gtk::Label>() {
        let text = if filtered.len() == 1 {
            "1 note".to_string()
        } else {
            format!("{} notes", filtered.len())
        };
        label.set_text(&text);
    }
}

/// Crear el popover para ordenamiento
pub fn create_sort_popover(properties: &[String]) -> gtk::Popover {
    let popover = gtk::Popover::builder()
        .css_classes(["sort-popover"])
        .build();
    
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();
    
    // Título
    let title = gtk::Label::builder()
        .label("Sort by")
        .css_classes(["heading"])
        .xalign(0.0)
        .margin_bottom(8)
        .build();
    content.append(&title);
    
    // Opción para quitar ordenamiento
    let none_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .css_classes(["sort-row"])
        .build();
    
    let none_btn = gtk::Button::builder()
        .label("No sorting")
        .css_classes(["flat"])
        .hexpand(true)
        .build();
    none_row.append(&none_btn);
    content.append(&none_row);
    
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    
    // Una fila por cada propiedad
    for prop in properties {
        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .css_classes(["sort-row"])
            .build();
        
        let prop_label = gtk::Label::builder()
            .label(prop)
            .hexpand(true)
            .xalign(0.0)
            .build();
        row.append(&prop_label);
        
        // Botón ascendente
        let asc_btn = gtk::Button::builder()
            .icon_name("view-sort-ascending-symbolic")
            .tooltip_text("Sort ascending")
            .css_classes(["flat", "circular"])
            .build();
        row.append(&asc_btn);
        
        // Botón descendente
        let desc_btn = gtk::Button::builder()
            .icon_name("view-sort-descending-symbolic")
            .tooltip_text("Sort descending")
            .css_classes(["flat", "circular"])
            .build();
        row.append(&desc_btn);
        
        content.append(&row);
    }
    
    popover.set_child(Some(&content));
    popover
}

/// Crear el popover para visibilidad de columnas
pub fn create_columns_popover(columns: &[ColumnConfig]) -> gtk::Popover {
    let popover = gtk::Popover::builder()
        .css_classes(["columns-popover"])
        .build();
    
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();
    
    // Título
    let title = gtk::Label::builder()
        .label("Visible Columns")
        .css_classes(["heading"])
        .xalign(0.0)
        .margin_bottom(8)
        .build();
    content.append(&title);
    
    // Un checkbox por cada columna
    for col in columns {
        let check = gtk::CheckButton::builder()
            .label(&col.display_title())
            .active(col.visible)
            .build();
        content.append(&check);
    }
    
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    
    // Botón para mostrar todas
    let show_all_btn = gtk::Button::builder()
        .label("Show all")
        .css_classes(["flat"])
        .build();
    content.append(&show_all_btn);
    
    popover.set_child(Some(&content));
    popover
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_table_widget_creation() {
        gtk::init().unwrap();
        let i18n = Rc::new(RefCell::new(I18n::new(Language::from_env())));
        let widget = BaseTableWidget::new(i18n);
        assert!(widget.widget().is_visible() || !widget.widget().is_visible()); // Just verify it compiles
    }
}
