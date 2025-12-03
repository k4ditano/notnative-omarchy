use serde::{Deserialize, Serialize};
use std::fmt;

/// Tipos de propiedades soportados (similar a Obsidian)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum PropertyValue {
    /// Texto plano
    Text(String),

    /// Número (entero o decimal)
    Number(f64),

    /// Booleano (checkbox)
    Checkbox(bool),

    /// Fecha en formato ISO 8601 (YYYY-MM-DD)
    Date(String),

    /// Fecha y hora en formato ISO 8601
    DateTime(String),

    /// Lista de valores de texto
    List(Vec<String>),

    /// Lista de tags (#tag)
    Tags(Vec<String>),

    /// Enlaces a otras notas ([[nota]])
    Links(Vec<String>),

    /// Enlace a una sola nota (@nota) - para propiedades inline
    Link(String),

    /// Valor nulo o vacío
    Null,
}

impl PropertyValue {
    /// Inferir el tipo de un valor YAML
    pub fn from_yaml(value: &serde_yaml::Value) -> Self {
        match value {
            serde_yaml::Value::Null => PropertyValue::Null,
            serde_yaml::Value::Bool(b) => PropertyValue::Checkbox(*b),
            serde_yaml::Value::Number(n) => {
                PropertyValue::Number(n.as_f64().unwrap_or(0.0))
            }
            serde_yaml::Value::String(s) => {
                // Intentar detectar si es una fecha
                if Self::is_date(s) {
                    PropertyValue::Date(s.clone())
                } else if Self::is_datetime(s) {
                    PropertyValue::DateTime(s.clone())
                } else {
                    PropertyValue::Text(s.clone())
                }
            }
            serde_yaml::Value::Sequence(seq) => {
                let items: Vec<String> = seq
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();

                // Detectar si son tags o links
                if items.iter().all(|s| s.starts_with('#')) {
                    PropertyValue::Tags(items.iter().map(|s| s.trim_start_matches('#').to_string()).collect())
                } else if items.iter().all(|s| s.starts_with("[[") && s.ends_with("]]")) {
                    PropertyValue::Links(items.iter().map(|s| {
                        s.trim_start_matches("[[").trim_end_matches("]]").to_string()
                    }).collect())
                } else {
                    PropertyValue::List(items)
                }
            }
            serde_yaml::Value::Mapping(_) => {
                // Los mappings anidados se convierten en texto JSON
                PropertyValue::Text(serde_json::to_string(value).unwrap_or_default())
            }
            serde_yaml::Value::Tagged(_) => PropertyValue::Null,
        }
    }

    /// Verificar si un string es una fecha ISO 8601
    fn is_date(s: &str) -> bool {
        // Formato: YYYY-MM-DD
        if s.len() != 10 {
            return false;
        }
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return false;
        }
        parts[0].len() == 4 && parts[1].len() == 2 && parts[2].len() == 2 &&
        parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
    }

    /// Verificar si un string es un datetime ISO 8601
    fn is_datetime(s: &str) -> bool {
        // Formato: YYYY-MM-DDTHH:MM:SS o similar
        s.contains('T') && s.len() >= 19 && Self::is_date(&s[..10])
    }

    /// Obtener el nombre del tipo
    pub fn type_name(&self) -> &'static str {
        match self {
            PropertyValue::Text(_) => "text",
            PropertyValue::Number(_) => "number",
            PropertyValue::Checkbox(_) => "checkbox",
            PropertyValue::Date(_) => "date",
            PropertyValue::DateTime(_) => "datetime",
            PropertyValue::List(_) => "list",
            PropertyValue::Tags(_) => "tags",
            PropertyValue::Links(_) => "links",
            PropertyValue::Link(_) => "link",
            PropertyValue::Null => "null",
        }
    }

    /// Convertir a string para mostrar
    pub fn to_display_string(&self) -> String {
        match self {
            PropertyValue::Text(s) => s.clone(),
            PropertyValue::Number(n) => {
                if n.fract() == 0.0 {
                    format!("{}", *n as i64)
                } else {
                    format!("{:.2}", n)
                }
            }
            PropertyValue::Checkbox(b) => if *b { "✓" } else { "✗" }.to_string(),
            PropertyValue::Date(d) => Self::format_date_friendly(d),
            PropertyValue::DateTime(dt) => Self::format_datetime_friendly(dt),
            PropertyValue::List(items) => items.join(", "),
            PropertyValue::Tags(tags) => tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" "),
            PropertyValue::Links(links) => links.iter().map(|l| format!("[[{}]]", l)).collect::<Vec<_>>().join(", "),
            PropertyValue::Link(note) => format!("@{}", note),
            PropertyValue::Null => "—".to_string(),
        }
    }
    
    /// Formatear fecha de forma amigable
    fn format_date_friendly(date_str: &str) -> String {
        // Intentar parsear varios formatos
        if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            return date.format("%d %b %Y").to_string();
        }
        // Si no se puede parsear, devolver original
        date_str.to_string()
    }
    
    /// Formatear datetime de forma amigable
    fn format_datetime_friendly(dt_str: &str) -> String {
        let dt_str = dt_str.trim();
        
        // Intentar parsear formato ISO con timezone (2025-12-02T19:30:53+00:00)
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(dt_str) {
            let local = dt.with_timezone(&chrono::Local);
            return local.format("%d %b %Y, %H:%M").to_string();
        }
        
        // Intentar ISO sin timezone (2025-12-02T19:30:53)
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(dt_str, "%Y-%m-%dT%H:%M:%S") {
            return dt.format("%d %b %Y, %H:%M").to_string();
        }
        
        // Intentar con milisegundos
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(dt_str, "%Y-%m-%dT%H:%M:%S%.f") {
            return dt.format("%d %b %Y, %H:%M").to_string();
        }
        
        // Intentar formato simple (2025-12-02 19:30:53)
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(dt_str, "%Y-%m-%d %H:%M:%S") {
            return dt.format("%d %b %Y, %H:%M").to_string();
        }
        
        // Intentar formato sin segundos
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(dt_str, "%Y-%m-%d %H:%M") {
            return dt.format("%d %b %Y, %H:%M").to_string();
        }
        
        // Si no se puede parsear, devolver original
        dt_str.to_string()
    }

    /// Convertir a valor para ordenamiento
    pub fn sort_key(&self) -> String {
        match self {
            PropertyValue::Text(s) => s.to_lowercase(),
            PropertyValue::Number(n) => format!("{:020.6}", n + 1e15), // Padding para orden numérico
            PropertyValue::Checkbox(b) => if *b { "1" } else { "0" }.to_string(),
            PropertyValue::Date(d) => d.clone(),
            PropertyValue::DateTime(dt) => dt.clone(),
            PropertyValue::List(items) => items.first().cloned().unwrap_or_default().to_lowercase(),
            PropertyValue::Tags(tags) => tags.first().cloned().unwrap_or_default().to_lowercase(),
            PropertyValue::Links(links) => links.first().cloned().unwrap_or_default().to_lowercase(),
            PropertyValue::Link(note) => note.to_lowercase(),
            PropertyValue::Null => String::new(),
        }
    }

    /// Verificar si es vacío/nulo
    pub fn is_empty(&self) -> bool {
        match self {
            PropertyValue::Text(s) => s.is_empty(),
            PropertyValue::Number(_) => false,
            PropertyValue::Checkbox(_) => false,
            PropertyValue::Date(d) => d.is_empty(),
            PropertyValue::DateTime(dt) => dt.is_empty(),
            PropertyValue::List(items) => items.is_empty(),
            PropertyValue::Tags(tags) => tags.is_empty(),
            PropertyValue::Links(links) => links.is_empty(),
            PropertyValue::Link(note) => note.is_empty(),
            PropertyValue::Null => true,
        }
    }

    /// Convertir a valor YAML
    pub fn to_yaml(&self) -> serde_yaml::Value {
        match self {
            PropertyValue::Text(s) => serde_yaml::Value::String(s.clone()),
            PropertyValue::Number(n) => {
                serde_yaml::Value::Number(serde_yaml::Number::from(*n))
            }
            PropertyValue::Checkbox(b) => serde_yaml::Value::Bool(*b),
            PropertyValue::Date(d) => serde_yaml::Value::String(d.clone()),
            PropertyValue::DateTime(dt) => serde_yaml::Value::String(dt.clone()),
            PropertyValue::List(items) => {
                serde_yaml::Value::Sequence(
                    items.iter().map(|s| serde_yaml::Value::String(s.clone())).collect()
                )
            }
            PropertyValue::Tags(tags) => {
                serde_yaml::Value::Sequence(
                    tags.iter().map(|t| serde_yaml::Value::String(format!("#{}", t))).collect()
                )
            }
            PropertyValue::Links(links) => {
                serde_yaml::Value::Sequence(
                    links.iter().map(|l| serde_yaml::Value::String(format!("[[{}]]", l))).collect()
                )
            }
            PropertyValue::Link(note) => serde_yaml::Value::String(format!("@{}", note)),
            PropertyValue::Null => serde_yaml::Value::Null,
        }
    }
}

impl fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}

impl Default for PropertyValue {
    fn default() -> Self {
        PropertyValue::Null
    }
}

/// Una propiedad con su nombre y valor tipado
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    pub key: String,
    pub value: PropertyValue,
}

impl Property {
    pub fn new(key: impl Into<String>, value: PropertyValue) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }

    pub fn text(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(key, PropertyValue::Text(value.into()))
    }

    pub fn number(key: impl Into<String>, value: f64) -> Self {
        Self::new(key, PropertyValue::Number(value))
    }

    pub fn checkbox(key: impl Into<String>, value: bool) -> Self {
        Self::new(key, PropertyValue::Checkbox(value))
    }

    pub fn date(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(key, PropertyValue::Date(value.into()))
    }

    pub fn list(key: impl Into<String>, items: Vec<String>) -> Self {
        Self::new(key, PropertyValue::List(items))
    }

    pub fn tags(key: impl Into<String>, tags: Vec<String>) -> Self {
        Self::new(key, PropertyValue::Tags(tags))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_value_from_yaml() {
        // Text
        let yaml = serde_yaml::Value::String("hello".to_string());
        assert!(matches!(PropertyValue::from_yaml(&yaml), PropertyValue::Text(s) if s == "hello"));

        // Number
        let yaml = serde_yaml::Value::Number(42.into());
        assert!(matches!(PropertyValue::from_yaml(&yaml), PropertyValue::Number(n) if n == 42.0));

        // Checkbox
        let yaml = serde_yaml::Value::Bool(true);
        assert!(matches!(PropertyValue::from_yaml(&yaml), PropertyValue::Checkbox(true)));

        // Date
        let yaml = serde_yaml::Value::String("2025-11-28".to_string());
        assert!(matches!(PropertyValue::from_yaml(&yaml), PropertyValue::Date(d) if d == "2025-11-28"));

        // List
        let yaml = serde_yaml::Value::Sequence(vec![
            serde_yaml::Value::String("a".to_string()),
            serde_yaml::Value::String("b".to_string()),
        ]);
        assert!(matches!(PropertyValue::from_yaml(&yaml), PropertyValue::List(items) if items.len() == 2));
    }

    #[test]
    fn test_sort_key() {
        let num1 = PropertyValue::Number(10.0);
        let num2 = PropertyValue::Number(2.0);
        
        // Verificar que el orden numérico funciona
        assert!(num2.sort_key() < num1.sort_key());
    }

    #[test]
    fn test_display() {
        assert_eq!(PropertyValue::Checkbox(true).to_display_string(), "✓");
        assert_eq!(PropertyValue::Number(42.0).to_display_string(), "42");
        assert_eq!(PropertyValue::Number(3.14159).to_display_string(), "3.14");
    }
}
