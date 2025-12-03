use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::property::PropertyValue;

// ============================================================================
// FORMATO DE CELDAS Y FILAS ESPECIALES (Sistema de Fórmulas)
// ============================================================================

/// Formato visual de una celda
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CellFormat {
    /// Número de decimales para números (None = auto)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    
    /// Prefijo (ej: "€", "$")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    
    /// Sufijo (ej: "%", " kg")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    
    /// Texto en negrita
    #[serde(default)]
    pub bold: bool,
    
    /// Color de texto (CSS color)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    
    /// Color de fondo (CSS color)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
}

impl CellFormat {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn with_decimals(mut self, decimals: u8) -> Self {
        self.decimals = Some(decimals);
        self
    }
    
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }
    
    pub fn with_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }
    
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
    
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }
    
    pub fn with_background(mut self, bg: impl Into<String>) -> Self {
        self.background = Some(bg.into());
        self
    }
    
    /// Formatear un valor numérico según el formato
    pub fn format_number(&self, value: f64) -> String {
        let mut result = match self.decimals {
            Some(d) => format!("{:.1$}", value, d as usize),
            None => {
                // Auto: quitar decimales innecesarios
                if value.fract() == 0.0 {
                    format!("{}", value as i64)
                } else {
                    format!("{:.2}", value)
                }
            }
        };
        
        if let Some(ref prefix) = self.prefix {
            result = format!("{}{}", prefix, result);
        }
        
        if let Some(ref suffix) = self.suffix {
            result = format!("{}{}", result, suffix);
        }
        
        result
    }
    
    /// Generar CSS inline para la celda
    pub fn to_css(&self) -> String {
        let mut styles = Vec::new();
        
        if self.bold {
            styles.push("font-weight: bold".to_string());
        }
        
        if let Some(ref color) = self.color {
            styles.push(format!("color: {}", color));
        }
        
        if let Some(ref bg) = self.background {
            styles.push(format!("background-color: {}", bg));
        }
        
        styles.join("; ")
    }
}

/// Contenido de una celda en una fila especial
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialCellContent {
    /// Fórmula (ej: "=SUM(C1:C10)") o texto estático
    pub content: String,
    
    /// Formato de la celda
    #[serde(default)]
    pub format: CellFormat,
}

impl SpecialCellContent {
    pub fn formula(formula: impl Into<String>) -> Self {
        Self {
            content: formula.into(),
            format: CellFormat::default(),
        }
    }
    
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: text.into(),
            format: CellFormat::default(),
        }
    }
    
    pub fn with_format(mut self, format: CellFormat) -> Self {
        self.format = format;
        self
    }
    
    /// ¿Es una fórmula?
    pub fn is_formula(&self) -> bool {
        self.content.starts_with('=')
    }
}

/// Una fila especial (fórmulas, totales, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialRow {
    /// ID único de la fila
    pub id: String,
    
    /// Etiqueta de la fila (primera columna, ej: "Total", "Promedio")
    pub label: String,
    
    /// Contenido de cada columna (clave = nombre de propiedad/columna)
    #[serde(default)]
    pub cells: HashMap<String, SpecialCellContent>,
    
    /// Posición: índice de fila después de la cual insertar (0 = inicio)
    /// None = al final de la tabla
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<usize>,
    
    /// CSS class adicional para la fila
    #[serde(skip_serializing_if = "Option::is_none")]
    pub css_class: Option<String>,
}

impl SpecialRow {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            cells: HashMap::new(),
            position: None,
            css_class: None,
        }
    }
    
    /// Crear fila de totales al final
    pub fn totals(label: impl Into<String>) -> Self {
        Self {
            id: "totals".to_string(),
            label: label.into(),
            cells: HashMap::new(),
            position: None,
            css_class: Some("special-row-totals".to_string()),
        }
    }
    
    /// Añadir contenido a una columna
    pub fn with_cell(mut self, column: impl Into<String>, content: SpecialCellContent) -> Self {
        self.cells.insert(column.into(), content);
        self
    }
    
    /// Añadir fórmula a una columna
    pub fn with_formula(mut self, column: impl Into<String>, formula: impl Into<String>) -> Self {
        self.cells.insert(column.into(), SpecialCellContent::formula(formula));
        self
    }
    
    /// Establecer posición
    pub fn at_position(mut self, pos: usize) -> Self {
        self.position = Some(pos);
        self
    }
}

// ============================================================================
// ERRORES Y TIPOS BASE
// ============================================================================

#[derive(Debug, Error)]
pub enum BaseError {
    #[error("YAML parse error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Base not found: {0}")]
    NotFound(String),

    #[error("Invalid filter: {0}")]
    InvalidFilter(String),
}

pub type Result<T> = std::result::Result<T, BaseError>;

/// Operadores de comparación para filtros
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOperator {
    /// Igual a
    Equals,
    /// No igual a
    NotEquals,
    /// Contiene (para texto o listas)
    Contains,
    /// No contiene
    NotContains,
    /// Empieza con
    StartsWith,
    /// Termina con
    EndsWith,
    /// Mayor que (números y fechas)
    GreaterThan,
    /// Mayor o igual que
    GreaterOrEqual,
    /// Menor que
    LessThan,
    /// Menor o igual que
    LessOrEqual,
    /// Está vacío
    IsEmpty,
    /// No está vacío
    IsNotEmpty,
}

impl FilterOperator {
    /// Evalúa el operador contra dos valores
    pub fn evaluate(&self, property_value: &PropertyValue, filter_value: &PropertyValue) -> bool {
        match self {
            FilterOperator::Equals => self.check_equals(property_value, filter_value),
            FilterOperator::NotEquals => !self.check_equals(property_value, filter_value),
            FilterOperator::Contains => self.check_contains(property_value, filter_value),
            FilterOperator::NotContains => !self.check_contains(property_value, filter_value),
            FilterOperator::StartsWith => self.check_starts_with(property_value, filter_value),
            FilterOperator::EndsWith => self.check_ends_with(property_value, filter_value),
            FilterOperator::GreaterThan => self.check_greater_than(property_value, filter_value, false),
            FilterOperator::GreaterOrEqual => self.check_greater_than(property_value, filter_value, true),
            FilterOperator::LessThan => self.check_less_than(property_value, filter_value, false),
            FilterOperator::LessOrEqual => self.check_less_than(property_value, filter_value, true),
            FilterOperator::IsEmpty => property_value.is_empty(),
            FilterOperator::IsNotEmpty => !property_value.is_empty(),
        }
    }

    fn check_equals(&self, a: &PropertyValue, b: &PropertyValue) -> bool {
        match (a, b) {
            (PropertyValue::Text(s1), PropertyValue::Text(s2)) => s1.to_lowercase() == s2.to_lowercase(),
            (PropertyValue::Number(n1), PropertyValue::Number(n2)) => (n1 - n2).abs() < f64::EPSILON,
            (PropertyValue::Checkbox(b1), PropertyValue::Checkbox(b2)) => b1 == b2,
            (PropertyValue::Date(d1), PropertyValue::Date(d2)) => d1 == d2,
            (PropertyValue::DateTime(dt1), PropertyValue::DateTime(dt2)) => dt1 == dt2,
            (PropertyValue::Tags(t1), PropertyValue::Tags(t2)) => t1 == t2,
            // Comparar Tags con Text: verificar si algún tag es igual al texto (sin #)
            (PropertyValue::Tags(tags), PropertyValue::Text(s)) => {
                let filter_normalized = s.to_lowercase().trim_start_matches('#').to_string();
                tags.iter().any(|t| t.to_lowercase() == filter_normalized)
            },
            (PropertyValue::List(l1), PropertyValue::List(l2)) => l1 == l2,
            (PropertyValue::Null, PropertyValue::Null) => true,
            _ => false,
        }
    }

    fn check_contains(&self, property: &PropertyValue, filter: &PropertyValue) -> bool {
        let filter_str = filter.to_display_string().to_lowercase();
        
        match property {
            PropertyValue::Text(s) => s.to_lowercase().contains(&filter_str),
            PropertyValue::List(items) => items.iter().any(|i| i.to_lowercase().contains(&filter_str)),
            PropertyValue::Tags(tags) => {
                // Para tags, eliminar # del filtro si existe (los tags se guardan sin #)
                let filter_normalized = filter_str.trim_start_matches('#');
                tags.iter().any(|t| t.to_lowercase().contains(filter_normalized))
            },
            PropertyValue::Links(links) => links.iter().any(|l| l.to_lowercase().contains(&filter_str)),
            _ => property.to_display_string().to_lowercase().contains(&filter_str),
        }
    }

    fn check_starts_with(&self, property: &PropertyValue, filter: &PropertyValue) -> bool {
        let filter_str = filter.to_display_string().to_lowercase();
        property.to_display_string().to_lowercase().starts_with(&filter_str)
    }

    fn check_ends_with(&self, property: &PropertyValue, filter: &PropertyValue) -> bool {
        let filter_str = filter.to_display_string().to_lowercase();
        property.to_display_string().to_lowercase().ends_with(&filter_str)
    }

    fn check_greater_than(&self, a: &PropertyValue, b: &PropertyValue, or_equal: bool) -> bool {
        match (a, b) {
            (PropertyValue::Number(n1), PropertyValue::Number(n2)) => {
                if or_equal { n1 >= n2 } else { n1 > n2 }
            }
            (PropertyValue::Date(d1), PropertyValue::Date(d2)) => {
                if or_equal { d1 >= d2 } else { d1 > d2 }
            }
            (PropertyValue::DateTime(dt1), PropertyValue::DateTime(dt2)) => {
                if or_equal { dt1 >= dt2 } else { dt1 > dt2 }
            }
            _ => false,
        }
    }

    fn check_less_than(&self, a: &PropertyValue, b: &PropertyValue, or_equal: bool) -> bool {
        match (a, b) {
            (PropertyValue::Number(n1), PropertyValue::Number(n2)) => {
                if or_equal { n1 <= n2 } else { n1 < n2 }
            }
            (PropertyValue::Date(d1), PropertyValue::Date(d2)) => {
                if or_equal { d1 <= d2 } else { d1 < d2 }
            }
            (PropertyValue::DateTime(dt1), PropertyValue::DateTime(dt2)) => {
                if or_equal { dt1 <= dt2 } else { dt1 < dt2 }
            }
            _ => false,
        }
    }
}

/// Un filtro individual que compara una propiedad con un valor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    /// Nombre de la propiedad a filtrar (ej: "status", "date", "tags")
    pub property: String,
    
    /// Operador de comparación
    pub operator: FilterOperator,
    
    /// Valor a comparar
    pub value: PropertyValue,
}

impl Filter {
    pub fn new(property: impl Into<String>, operator: FilterOperator, value: PropertyValue) -> Self {
        Self {
            property: property.into(),
            operator,
            value,
        }
    }

    /// Filtro rápido: propiedad equals valor
    pub fn equals(property: impl Into<String>, value: PropertyValue) -> Self {
        Self::new(property, FilterOperator::Equals, value)
    }

    /// Filtro rápido: tiene tag
    pub fn has_tag(tag: impl Into<String>) -> Self {
        Self::new("tags", FilterOperator::Contains, PropertyValue::Text(tag.into()))
    }

    /// Filtro rápido: propiedad contiene texto
    pub fn contains(property: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(property, FilterOperator::Contains, PropertyValue::Text(text.into()))
    }

    /// Filtro rápido: no está vacío
    pub fn is_not_empty(property: impl Into<String>) -> Self {
        Self::new(property, FilterOperator::IsNotEmpty, PropertyValue::Null)
    }

    /// Evaluar el filtro contra un conjunto de propiedades
    pub fn evaluate(&self, properties: &HashMap<String, PropertyValue>) -> bool {
        // Caso especial: propiedades built-in
        let prop_value = match self.property.as_str() {
            "name" | "title" => properties.get("title").or(properties.get("name")),
            "folder" => properties.get("folder"),
            "created" | "created_at" => properties.get("created_at"),
            "updated" | "updated_at" => properties.get("updated_at"),
            _ => properties.get(&self.property),
        };

        match prop_value {
            Some(value) => self.operator.evaluate(value, &self.value),
            None => {
                // Si la propiedad no existe, solo IsEmpty es true
                matches!(self.operator, FilterOperator::IsEmpty)
            }
        }
    }
}

/// Grupo de filtros con operador lógico (AND/OR)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilterGroup {
    /// Filtros individuales
    pub filters: Vec<Filter>,
    
    /// Operador lógico entre filtros (default: AND)
    #[serde(default)]
    pub logic: FilterLogic,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FilterLogic {
    #[default]
    And,
    Or,
}

impl FilterGroup {
    pub fn new(filters: Vec<Filter>) -> Self {
        Self {
            filters,
            logic: FilterLogic::And,
        }
    }

    pub fn with_or(filters: Vec<Filter>) -> Self {
        Self {
            filters,
            logic: FilterLogic::Or,
        }
    }

    pub fn evaluate(&self, properties: &HashMap<String, PropertyValue>) -> bool {
        if self.filters.is_empty() {
            return true;
        }

        match self.logic {
            FilterLogic::And => self.filters.iter().all(|f| f.evaluate(properties)),
            FilterLogic::Or => self.filters.iter().any(|f| f.evaluate(properties)),
        }
    }
}

/// Dirección de ordenamiento
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

/// Configuración de ordenamiento
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortConfig {
    /// Propiedad por la cual ordenar
    pub property: String,
    
    /// Dirección del ordenamiento
    #[serde(default)]
    pub direction: SortDirection,
}

impl SortConfig {
    pub fn asc(property: impl Into<String>) -> Self {
        Self {
            property: property.into(),
            direction: SortDirection::Asc,
        }
    }

    pub fn desc(property: impl Into<String>) -> Self {
        Self {
            property: property.into(),
            direction: SortDirection::Desc,
        }
    }
}

/// Configuración de una columna en la vista de tabla
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnConfig {
    /// Nombre de la propiedad
    pub property: String,
    
    /// Título a mostrar (default: nombre de propiedad capitalizado)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    
    /// Ancho de la columna en píxeles (default: auto)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    
    /// Si la columna es visible (default: true)
    #[serde(default = "default_true")]
    pub visible: bool,
}

fn default_true() -> bool { true }

impl ColumnConfig {
    pub fn new(property: impl Into<String>) -> Self {
        Self {
            property: property.into(),
            title: None,
            width: None,
            visible: true,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn display_title(&self) -> String {
        self.title.clone().unwrap_or_else(|| {
            // Capitalizar primera letra
            let mut chars = self.property.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
    }
}

/// Tipo de vista de la Base
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewType {
    #[default]
    Table,
    List,
    Board,  // Kanban-style
    Gallery,
}

/// Tipo de fuente de datos de la Base
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    /// Notas (comportamiento tradicional)
    #[default]
    Notes,
    /// Registros agrupados de propiedades inline [campo1::val1, campo2::val2]
    GroupedRecords,
    /// Registros filtrados por una propiedad específica (BD bidireccional)
    PropertyRecords,
}

/// Una vista dentro de una Base
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseView {
    /// Nombre de la vista
    pub name: String,
    
    /// Tipo de vista
    #[serde(default)]
    pub view_type: ViewType,
    
    /// Filtros aplicados a esta vista
    #[serde(default)]
    pub filter: FilterGroup,
    
    /// Columnas a mostrar (en orden)
    #[serde(default)]
    pub columns: Vec<ColumnConfig>,
    
    /// Ordenamiento
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<SortConfig>,
    
    /// Propiedad por la cual agrupar (para Board/Gallery)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_by: Option<String>,
    
    /// Si la vista es editable (permite modificar datos en las notas)
    #[serde(default)]
    pub editable: bool,
    
    /// Filas especiales con fórmulas (totales, promedios, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub special_rows: Vec<SpecialRow>,
}

impl BaseView {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            view_type: ViewType::Table,
            filter: FilterGroup::new(vec![]),
            columns: vec![
                ColumnConfig::new("title"),
                ColumnConfig::new("tags"),
                ColumnConfig::new("updated_at").with_title("Modified"),
            ],
            sort: Some(SortConfig::desc("updated_at")),
            group_by: None,
            editable: false,
            special_rows: Vec::new(),
        }
    }

    pub fn table(name: impl Into<String>) -> Self {
        let mut view = Self::new(name);
        view.view_type = ViewType::Table;
        view
    }

    pub fn list(name: impl Into<String>) -> Self {
        let mut view = Self::new(name);
        view.view_type = ViewType::List;
        view
    }

    pub fn board(name: impl Into<String>, group_by: impl Into<String>) -> Self {
        let mut view = Self::new(name);
        view.view_type = ViewType::Board;
        view.group_by = Some(group_by.into());
        view
    }
    
    /// Crear una vista para registros agrupados (sin columnas por defecto)
    pub fn grouped_records(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            view_type: ViewType::Table,
            filter: FilterGroup::new(vec![]),
            columns: vec![
                ColumnConfig::new("_note").with_title("Note"),
            ],
            sort: None,
            group_by: None,
            editable: false,
            special_rows: Vec::new(),
        }
    }
    
    /// Crear una vista para registros filtrados por propiedad (editable)
    /// La columna principal es la propiedad de filtro
    pub fn property_records(name: impl Into<String>, filter_property: &str) -> Self {
        Self {
            name: name.into(),
            view_type: ViewType::Table,
            filter: FilterGroup::new(vec![]),
            columns: vec![
                ColumnConfig::new(filter_property).with_title(&capitalize(filter_property)),
                ColumnConfig::new("_note").with_title("Note"),
            ],
            sort: None,
            group_by: None,
            editable: true,
            special_rows: Vec::new(),
        }
    }
}

/// Capitaliza la primera letra de un string
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Una Base: colección de vistas sobre notas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Base {
    /// Nombre de la Base
    pub name: String,
    
    /// Descripción opcional
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// Tipo de fuente de datos (notas o registros agrupados)
    #[serde(default)]
    pub source_type: SourceType,
    
    /// Carpeta fuente (opcional - si no, busca en todas las notas)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_folder: Option<String>,
    
    /// Propiedad de filtro para SourceType::PropertyRecords
    /// Ejemplo: "juego" filtra todos los registros que contienen [juego::X]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_property: Option<String>,
    
    /// Vistas disponibles
    pub views: Vec<BaseView>,
    
    /// Índice de la vista activa
    #[serde(default)]
    pub active_view: usize,
    
    /// Fecha de creación (timestamp)
    #[serde(default)]
    pub created_at: i64,
    
    /// Fecha de última modificación (timestamp)
    #[serde(default)]
    pub updated_at: i64,
}

impl Base {
    pub fn new(name: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            name: name.into(),
            description: None,
            source_type: SourceType::Notes,
            source_folder: None,
            filter_property: None,
            views: vec![BaseView::new("Default")],
            active_view: 0,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Crear una Base de registros agrupados
    pub fn grouped_records(name: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            name: name.into(),
            description: None,
            source_type: SourceType::GroupedRecords,
            source_folder: None,
            filter_property: None,
            views: vec![BaseView::grouped_records("Default")],
            active_view: 0,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Crear una Base de registros filtrados por propiedad (BD bidireccional)
    /// Las columnas se descubren automáticamente
    pub fn property_records(name: impl Into<String>, filter_property: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp();
        let filter_prop = filter_property.into();
        Self {
            name: name.into(),
            description: None,
            source_type: SourceType::PropertyRecords,
            source_folder: None,
            filter_property: Some(filter_prop.clone()),
            views: vec![BaseView::property_records("Default", &filter_prop)],
            active_view: 0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Crear una Base con una vista inicial
    pub fn with_view(name: impl Into<String>, view: BaseView) -> Self {
        let mut base = Self::new(name);
        base.views = vec![view];
        base
    }

    /// Obtener la vista activa
    pub fn active_view(&self) -> Option<&BaseView> {
        self.views.get(self.active_view)
    }

    /// Obtener la vista activa mutable
    pub fn active_view_mut(&mut self) -> Option<&mut BaseView> {
        self.views.get_mut(self.active_view)
    }

    /// Añadir una nueva vista
    pub fn add_view(&mut self, view: BaseView) {
        self.views.push(view);
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Cambiar a una vista por índice
    pub fn set_active_view(&mut self, index: usize) -> bool {
        if index < self.views.len() {
            self.active_view = index;
            true
        } else {
            false
        }
    }

    /// Parsear desde contenido YAML de un archivo .base
    pub fn parse(content: &str) -> Result<Self> {
        let base: Base = serde_yaml::from_str(content)?;
        Ok(base)
    }

    /// Serializar a YAML para guardar como archivo .base
    pub fn serialize(&self) -> Result<String> {
        let yaml = serde_yaml::to_string(self)?;
        Ok(yaml)
    }

    /// Cargar desde un archivo .base
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Guardar a un archivo .base
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = self.serialize()?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl Default for Base {
    fn default() -> Self {
        Self::new("New Base")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_operator_equals() {
        let op = FilterOperator::Equals;
        
        assert!(op.evaluate(
            &PropertyValue::Text("hello".to_string()),
            &PropertyValue::Text("HELLO".to_string())
        ));
        
        assert!(op.evaluate(
            &PropertyValue::Number(42.0),
            &PropertyValue::Number(42.0)
        ));
        
        assert!(!op.evaluate(
            &PropertyValue::Number(42.0),
            &PropertyValue::Number(43.0)
        ));
    }

    #[test]
    fn test_filter_contains() {
        let filter = Filter::contains("title", "rust");
        
        let mut props = HashMap::new();
        props.insert("title".to_string(), PropertyValue::Text("Learning Rust".to_string()));
        
        assert!(filter.evaluate(&props));
        
        props.insert("title".to_string(), PropertyValue::Text("Learning Python".to_string()));
        assert!(!filter.evaluate(&props));
    }

    #[test]
    fn test_filter_has_tag() {
        let filter = Filter::has_tag("rust");
        
        let mut props = HashMap::new();
        props.insert("tags".to_string(), PropertyValue::Tags(vec!["rust".to_string(), "gtk".to_string()]));
        
        assert!(filter.evaluate(&props));
        
        props.insert("tags".to_string(), PropertyValue::Tags(vec!["python".to_string()]));
        assert!(!filter.evaluate(&props));
    }

    #[test]
    fn test_filter_group_and() {
        let group = FilterGroup::new(vec![
            Filter::has_tag("rust"),
            Filter::contains("title", "learning"),
        ]);
        
        let mut props = HashMap::new();
        props.insert("tags".to_string(), PropertyValue::Tags(vec!["rust".to_string()]));
        props.insert("title".to_string(), PropertyValue::Text("Learning Rust".to_string()));
        
        assert!(group.evaluate(&props));
        
        // Si falta un filtro, AND falla
        props.insert("title".to_string(), PropertyValue::Text("Advanced Rust".to_string()));
        assert!(!group.evaluate(&props));
    }

    #[test]
    fn test_filter_group_or() {
        let group = FilterGroup::with_or(vec![
            Filter::has_tag("rust"),
            Filter::has_tag("python"),
        ]);
        
        let mut props = HashMap::new();
        props.insert("tags".to_string(), PropertyValue::Tags(vec!["rust".to_string()]));
        
        assert!(group.evaluate(&props));
        
        props.insert("tags".to_string(), PropertyValue::Tags(vec!["python".to_string()]));
        assert!(group.evaluate(&props));
        
        props.insert("tags".to_string(), PropertyValue::Tags(vec!["javascript".to_string()]));
        assert!(!group.evaluate(&props));
    }

    #[test]
    fn test_base_serialization() {
        let mut base = Base::new("My Tasks");
        base.description = Some("Task tracking".to_string());
        base.source_folder = Some("projects".to_string());
        
        let yaml = base.serialize().unwrap();
        let parsed = Base::parse(&yaml).unwrap();
        
        assert_eq!(parsed.name, "My Tasks");
        assert_eq!(parsed.description, Some("Task tracking".to_string()));
        assert_eq!(parsed.source_folder, Some("projects".to_string()));
    }

    #[test]
    fn test_base_view_columns() {
        let view = BaseView::table("Tasks")
            ;
        
        assert_eq!(view.columns.len(), 3);
        assert_eq!(view.columns[0].property, "title");
    }
}
