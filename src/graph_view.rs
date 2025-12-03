//! Vista de grafo interactivo para visualizar relaciones entre propiedades agrupadas
//!
//! Implementa un grafo force-directed con:
//! - Nodos arrastrables
//! - Zoom y pan con gestos
//! - Simulación de fuerzas (springs + repulsión)
//! - Colores por tipo de propiedad

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};
use relm4::gtk;
use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::time::{SystemTime, UNIX_EPOCH};

/// Generador de números pseudo-aleatorios simple (LCG)
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(42);
        Self { state: seed }
    }

    fn next_f64(&mut self) -> f64 {
        // Linear Congruential Generator
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 33) as f64 / (1u64 << 31) as f64
    }
}

thread_local! {
    static RNG: RefCell<SimpleRng> = RefCell::new(SimpleRng::new());
}

fn random_f64() -> f64 {
    RNG.with(|rng| rng.borrow_mut().next_f64())
}

/// Un nodo en el grafo
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub node_type: NodeType,
    pub x: f64,
    pub y: f64,
    pub vx: f64, // velocidad X
    pub vy: f64, // velocidad Y
    pub fixed: bool, // si está siendo arrastrado
    pub radius: f64,
}

/// Tipo de nodo (para colores)
#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    PropertyKey,    // Nombre de propiedad (ej: "autor")
    PropertyValue,  // Valor de propiedad (ej: "Cervantes")
    Note,           // Nota origen
}

impl NodeType {
    /// Colores Catppuccin Mocha para los tipos de nodo
    pub fn color(&self) -> (f64, f64, f64) {
        match self {
            // Catppuccin Mocha Blue: #89b4fa
            NodeType::PropertyKey => (0x89 as f64 / 255.0, 0xb4 as f64 / 255.0, 0xfa as f64 / 255.0),
            // Catppuccin Mocha Peach: #fab387
            NodeType::PropertyValue => (0xfa as f64 / 255.0, 0xb3 as f64 / 255.0, 0x87 as f64 / 255.0),
            // Catppuccin Mocha Green: #a6e3a1
            NodeType::Note => (0xa6 as f64 / 255.0, 0xe3 as f64 / 255.0, 0xa1 as f64 / 255.0),
        }
    }
}

/// Una arista en el grafo
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub label: Option<String>,
}

/// Estado del grafo
pub struct GraphState {
    pub nodes: HashMap<String, GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,
    pub dragging_node: Option<String>,
    pub drag_start_x: f64,
    pub drag_start_y: f64,
    pub is_panning: bool,
    pub pan_start_x: f64,
    pub pan_start_y: f64,
    pub simulation_running: bool,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            dragging_node: None,
            drag_start_x: 0.0,
            drag_start_y: 0.0,
            is_panning: false,
            pan_start_x: 0.0,
            pan_start_y: 0.0,
            simulation_running: true,
        }
    }
}

impl GraphState {
    /// Añadir un nodo al grafo
    pub fn add_node(&mut self, id: &str, label: &str, node_type: NodeType) {
        if !self.nodes.contains_key(id) {
            // Posición inicial aleatoria
            let x = random_f64() * 400.0 + 100.0;
            let y = random_f64() * 400.0 + 100.0;
            
            let radius = match node_type {
                NodeType::PropertyKey => 25.0,
                NodeType::PropertyValue => 20.0,
                NodeType::Note => 30.0,
            };
            
            self.nodes.insert(id.to_string(), GraphNode {
                id: id.to_string(),
                label: label.to_string(),
                node_type,
                x,
                y,
                vx: 0.0,
                vy: 0.0,
                fixed: false,
                radius,
            });
        }
    }

    /// Añadir una arista
    pub fn add_edge(&mut self, source: &str, target: &str, label: Option<&str>) {
        self.edges.push(GraphEdge {
            source: source.to_string(),
            target: target.to_string(),
            label: label.map(|s| s.to_string()),
        });
    }

    /// Aplicar simulación de fuerzas (un paso)
    pub fn simulate_step(&mut self, width: f64, height: f64) {
        if !self.simulation_running {
            return;
        }

        let damping = 0.85;
        let repulsion = 5000.0;
        let spring_length = 100.0;
        let spring_strength = 0.05;
        let center_gravity = 0.01;

        let center_x = width / 2.0;
        let center_y = height / 2.0;

        // Calcular fuerzas de repulsión entre todos los nodos
        let node_ids: Vec<String> = self.nodes.keys().cloned().collect();
        
        for i in 0..node_ids.len() {
            for j in (i + 1)..node_ids.len() {
                let id_i = &node_ids[i];
                let id_j = &node_ids[j];
                
                let (dx, dy) = {
                    let node_i = &self.nodes[id_i];
                    let node_j = &self.nodes[id_j];
                    (node_j.x - node_i.x, node_j.y - node_i.y)
                };
                
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let force = repulsion / (dist * dist);
                let fx = (dx / dist) * force;
                let fy = (dy / dist) * force;
                
                if let Some(node) = self.nodes.get_mut(id_i) {
                    if !node.fixed {
                        node.vx -= fx;
                        node.vy -= fy;
                    }
                }
                if let Some(node) = self.nodes.get_mut(id_j) {
                    if !node.fixed {
                        node.vx += fx;
                        node.vy += fy;
                    }
                }
            }
        }

        // Calcular fuerzas de resorte (springs) para las aristas
        for edge in &self.edges {
            let (dx, dy, dist) = {
                let source = match self.nodes.get(&edge.source) {
                    Some(n) => n,
                    None => continue,
                };
                let target = match self.nodes.get(&edge.target) {
                    Some(n) => n,
                    None => continue,
                };
                let dx = target.x - source.x;
                let dy = target.y - source.y;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                (dx, dy, dist)
            };
            
            let displacement = dist - spring_length;
            let force = displacement * spring_strength;
            let fx = (dx / dist) * force;
            let fy = (dy / dist) * force;
            
            if let Some(node) = self.nodes.get_mut(&edge.source) {
                if !node.fixed {
                    node.vx += fx;
                    node.vy += fy;
                }
            }
            if let Some(node) = self.nodes.get_mut(&edge.target) {
                if !node.fixed {
                    node.vx -= fx;
                    node.vy -= fy;
                }
            }
        }

        // Aplicar gravedad hacia el centro y actualizar posiciones
        for node in self.nodes.values_mut() {
            if node.fixed {
                continue;
            }

            // Gravedad hacia el centro
            node.vx += (center_x - node.x) * center_gravity;
            node.vy += (center_y - node.y) * center_gravity;

            // Aplicar amortiguación
            node.vx *= damping;
            node.vy *= damping;

            // Actualizar posición
            node.x += node.vx;
            node.y += node.vy;

            // Mantener dentro de los límites
            let margin = node.radius + 10.0;
            node.x = node.x.clamp(margin, width - margin);
            node.y = node.y.clamp(margin, height - margin);
        }
    }

    /// Encontrar nodo en una posición (considerando zoom y pan)
    pub fn node_at(&self, x: f64, y: f64) -> Option<String> {
        // Convertir coordenadas de pantalla a coordenadas del grafo
        let graph_x = (x - self.pan_x) / self.zoom;
        let graph_y = (y - self.pan_y) / self.zoom;

        for (id, node) in &self.nodes {
            let dx = graph_x - node.x;
            let dy = graph_y - node.y;
            if dx * dx + dy * dy <= node.radius * node.radius {
                return Some(id.clone());
            }
        }
        None
    }

    /// Limpiar el grafo
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
    }
}

/// Widget GTK4 para el grafo
mod imp {
    use super::*;
    use gtk::subclass::prelude::*;
    use std::sync::Arc;

    #[derive(Default)]
    pub struct GraphView {
        pub state: RefCell<GraphState>,
        pub animation_id: RefCell<Option<glib::SourceId>>,
        pub on_node_click: RefCell<Option<Arc<dyn Fn(&str) + Send + Sync>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GraphView {
        const NAME: &'static str = "NotNativeGraphView";
        type Type = super::GraphView;
        type ParentType = gtk::DrawingArea;
    }

    impl ObjectImpl for GraphView {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            
            // Configurar el draw function
            obj.set_draw_func(|widget, cr, width, height| {
                let graph_view = widget.downcast_ref::<super::GraphView>().unwrap();
                graph_view.draw(cr, width, height);
            });

            // Configurar controladores de gestos
            obj.setup_gestures();
        }

        fn dispose(&self) {
            // Detener animación al destruir
            if let Some(id) = self.animation_id.borrow_mut().take() {
                id.remove();
            }
        }
    }

    impl WidgetImpl for GraphView {}
    impl DrawingAreaImpl for GraphView {}
}

glib::wrapper! {
    pub struct GraphView(ObjectSubclass<imp::GraphView>)
        @extends gtk::DrawingArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for GraphView {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphView {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Obtener el estado del grafo
    pub fn state(&self) -> std::cell::Ref<'_, GraphState> {
        self.imp().state.borrow()
    }

    /// Obtener el estado mutable del grafo
    pub fn state_mut(&self) -> std::cell::RefMut<'_, GraphState> {
        self.imp().state.borrow_mut()
    }

    /// Configurar controladores de gestos
    fn setup_gestures(&self) {
        // Gesture para arrastrar nodos
        let drag = gtk::GestureDrag::new();
        drag.set_button(gdk::BUTTON_PRIMARY);
        
        let widget = self.clone();
        drag.connect_drag_begin(move |gesture, x, y| {
            let mut state = widget.state_mut();
            if let Some(node_id) = state.node_at(x, y) {
                state.dragging_node = Some(node_id.clone());
                state.drag_start_x = x;
                state.drag_start_y = y;
                if let Some(node) = state.nodes.get_mut(&node_id) {
                    node.fixed = true;
                }
            } else {
                // Iniciar panning
                state.is_panning = true;
                state.pan_start_x = state.pan_x;
                state.pan_start_y = state.pan_y;
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
        });

        let widget = self.clone();
        drag.connect_drag_update(move |_, offset_x, offset_y| {
            let mut state = widget.state_mut();
            if let Some(ref node_id) = state.dragging_node.clone() {
                let new_x = (state.drag_start_x + offset_x - state.pan_x) / state.zoom;
                let new_y = (state.drag_start_y + offset_y - state.pan_y) / state.zoom;
                if let Some(node) = state.nodes.get_mut(node_id) {
                    node.x = new_x;
                    node.y = new_y;
                }
            } else if state.is_panning {
                state.pan_x = state.pan_start_x + offset_x;
                state.pan_y = state.pan_start_y + offset_y;
            }
            drop(state);
            widget.queue_draw();
        });

        let widget = self.clone();
        drag.connect_drag_end(move |_, _, _| {
            let mut state = widget.state_mut();
            if let Some(ref node_id) = state.dragging_node.clone() {
                if let Some(node) = state.nodes.get_mut(node_id) {
                    node.fixed = false;
                }
            }
            state.dragging_node = None;
            state.is_panning = false;
        });

        self.add_controller(drag);

        // Gesture para zoom con scroll
        let scroll = gtk::EventControllerScroll::new(
            gtk::EventControllerScrollFlags::VERTICAL
        );
        
        let widget = self.clone();
        scroll.connect_scroll(move |_, _, dy| {
            let mut state = widget.state_mut();
            let zoom_factor = if dy < 0.0 { 1.1 } else { 0.9 };
            state.zoom = (state.zoom * zoom_factor).clamp(0.1, 5.0);
            drop(state);
            widget.queue_draw();
            glib::Propagation::Stop
        });

        self.add_controller(scroll);
        
        // Gesture para doble-clic en nodos
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        
        let widget = self.clone();
        click.connect_released(move |gesture, n_press, x, y| {
            if n_press == 2 {
                // Doble clic - navegar al nodo
                let state = widget.state();
                if let Some(node_id) = state.node_at(x, y) {
                    // Solo para nodos de tipo Note
                    if let Some(node) = state.nodes.get(&node_id) {
                        if node.node_type == NodeType::Note {
                            // Extraer el nombre de la nota del id "note:123"
                            let note_name = node.label.clone();
                            drop(state);
                            
                            // Llamar al callback
                            if let Some(ref callback) = *widget.imp().on_node_click.borrow() {
                                callback(&note_name);
                            }
                        }
                    }
                }
                gesture.set_state(gtk::EventSequenceState::Claimed);
            }
        });
        
        self.add_controller(click);
    }
    
    /// Configurar callback para clic en nodos de notas
    pub fn on_note_click<F: Fn(&str) + Send + Sync + 'static>(&self, callback: F) {
        *self.imp().on_node_click.borrow_mut() = Some(std::sync::Arc::new(callback));
    }

    /// Dibujar el grafo
    fn draw(&self, cr: &gtk::cairo::Context, width: i32, height: i32) {
        let state = self.state();
        
        // Obtener colores del tema GTK usando lookup_color
        let style_context = self.style_context();
        
        // Intentar obtener color de fondo del tema
        let bg_color = style_context.lookup_color("base")
            .or_else(|| style_context.lookup_color("theme_bg_color"))
            .or_else(|| style_context.lookup_color("window_bg_color"));
        
        // Obtener color de texto para determinar si es tema oscuro
        let fg_color = style_context.color();
        let luminance = fg_color.red() * 0.299 + fg_color.green() * 0.587 + fg_color.blue() * 0.114;
        let is_dark = luminance > 0.5;
        
        // Color de fondo - usar el del tema o fallback
        let (bg_r, bg_g, bg_b) = if let Some(bg) = bg_color {
            (bg.red() as f64, bg.green() as f64, bg.blue() as f64)
        } else if is_dark {
            (0x1e as f64 / 255.0, 0x1e as f64 / 255.0, 0x2e as f64 / 255.0)
        } else {
            (0xef as f64 / 255.0, 0xf1 as f64 / 255.0, 0xf5 as f64 / 255.0)
        };
        
        // Fondo con color del tema
        cr.set_source_rgb(bg_r, bg_g, bg_b);
        cr.paint().ok();

        // Aplicar transformaciones (zoom y pan)
        cr.translate(state.pan_x, state.pan_y);
        cr.scale(state.zoom, state.zoom);

        // Color de aristas - obtener del tema o generar basado en fondo
        let border_color = style_context.lookup_color("border")
            .or_else(|| style_context.lookup_color("borders"));
        
        let edge_color = if let Some(border) = border_color {
            (border.red() as f64, border.green() as f64, border.blue() as f64)
        } else if is_dark {
            (0x58 as f64 / 255.0, 0x5b as f64 / 255.0, 0x70 as f64 / 255.0)
        } else {
            (0xac as f64 / 255.0, 0xb0 as f64 / 255.0, 0xbe as f64 / 255.0)
        };

        // Dibujar aristas
        cr.set_line_width(1.5 / state.zoom);
        for edge in &state.edges {
            if let (Some(source), Some(target)) = (
                state.nodes.get(&edge.source),
                state.nodes.get(&edge.target),
            ) {
                cr.set_source_rgba(edge_color.0, edge_color.1, edge_color.2, 0.6);
                cr.move_to(source.x, source.y);
                cr.line_to(target.x, target.y);
                cr.stroke().ok();
            }
        }

        // Dibujar nodos
        for node in state.nodes.values() {
            let (r, g, b) = node.node_type.color();
            
            // Sombra
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.3);
            cr.arc(node.x + 2.0, node.y + 2.0, node.radius, 0.0, 2.0 * PI);
            cr.fill().ok();
            
            // Nodo
            cr.set_source_rgb(r, g, b);
            cr.arc(node.x, node.y, node.radius, 0.0, 2.0 * PI);
            cr.fill().ok();

            // Borde
            cr.set_source_rgb(r * 0.7, g * 0.7, b * 0.7);
            cr.set_line_width(2.0 / state.zoom);
            cr.arc(node.x, node.y, node.radius, 0.0, 2.0 * PI);
            cr.stroke().ok();

            // Etiqueta - usar color del tema
            cr.set_source_rgba(
                fg_color.red() as f64,
                fg_color.green() as f64,
                fg_color.blue() as f64,
                1.0
            );
            let font_size = 11.0 / state.zoom.sqrt();
            cr.set_font_size(font_size);
            
            // Centrar texto en el nodo
            if let Ok(extents) = cr.text_extents(&node.label) {
                let text_x = node.x - extents.width() / 2.0;
                let text_y = node.y + extents.height() / 2.0 - 2.0;
                
                cr.move_to(text_x, text_y);
                cr.show_text(&node.label).ok();
            }
        }
    }

    /// Iniciar la animación de simulación
    pub fn start_simulation(&self) {
        let widget = self.clone();
        
        let id = glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
            let width = widget.width() as f64;
            let height = widget.height() as f64;
            
            if width > 0.0 && height > 0.0 {
                widget.state_mut().simulate_step(width, height);
                widget.queue_draw();
            }
            
            glib::ControlFlow::Continue
        });

        *self.imp().animation_id.borrow_mut() = Some(id);
    }

    /// Detener la animación
    pub fn stop_simulation(&self) {
        if let Some(id) = self.imp().animation_id.borrow_mut().take() {
            id.remove();
        }
    }

    /// Cargar datos desde registros agrupados
    pub fn load_from_grouped_records(&self, records: &[crate::core::GroupedRecord]) {
        let mut state = self.state_mut();
        state.clear();

        for record in records {
            // Añadir nodo de nota
            let note_id = format!("note:{}", record.note_id);
            state.add_node(&note_id, &record.note_name, NodeType::Note);

            // Añadir nodos para cada propiedad del grupo
            for (key, value) in &record.properties {
                let value_id = format!("value:{}:{}", key, value);
                state.add_node(&value_id, value, NodeType::PropertyValue);
                
                // Arista de nota a valor
                state.add_edge(&note_id, &value_id, Some(key));
            }

            // Conectar valores del mismo grupo entre sí
            let value_ids: Vec<String> = record.properties.iter()
                .map(|(k, v)| format!("value:{}:{}", k, v))
                .collect();
            
            for i in 0..value_ids.len() {
                for j in (i + 1)..value_ids.len() {
                    state.add_edge(&value_ids[i], &value_ids[j], None);
                }
            }
        }
    }

    /// Cargar datos filtrados por un valor específico
    pub fn load_filtered(&self, key: &str, value: &str, records: &[crate::core::GroupedRecord]) {
        let mut state = self.state_mut();
        state.clear();

        // Nodo central para el valor buscado
        let center_id = format!("center:{}:{}", key, value);
        state.add_node(&center_id, value, NodeType::PropertyValue);

        for record in records {
            // Añadir nodo de nota
            let note_id = format!("note:{}", record.note_id);
            state.add_node(&note_id, &record.note_name, NodeType::Note);
            
            // Conectar nota al centro
            state.add_edge(&note_id, &center_id, Some(key));

            // Añadir otras propiedades del grupo
            for (k, v) in &record.properties {
                if k != key || v != value {
                    let value_id = format!("value:{}:{}", k, v);
                    state.add_node(&value_id, v, NodeType::PropertyValue);
                    state.add_edge(&note_id, &value_id, Some(k));
                }
            }
        }
    }
}
