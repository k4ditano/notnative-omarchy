use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FrontmatterError {
    #[error("YAML parse error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("No frontmatter found")]
    NoFrontmatter,

    #[error("Invalid frontmatter format")]
    InvalidFormat,
}

pub type Result<T> = std::result::Result<T, FrontmatterError>;

/// Estructura para el frontmatter YAML de una nota
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    /// Tags de la nota
    #[serde(default)]
    pub tags: Vec<String>,

    /// Título opcional (si es diferente al nombre del archivo)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Fecha opcional
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,

    /// Autor opcional
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Campos personalizados adicionales
    #[serde(flatten)]
    pub custom: HashMap<String, serde_yaml::Value>,
}

impl Frontmatter {
    /// Parsear frontmatter desde el contenido de una nota markdown
    ///
    /// Formato esperado:
    /// ```markdown
    /// ---
    /// tags: [tag1, tag2, tag3]
    /// title: Mi título
    /// date: 2025-11-01
    /// ---
    ///
    /// # Contenido de la nota...
    /// ```
    ///
    /// Retorna el frontmatter parseado y el contenido restante (sin el frontmatter)
    pub fn parse(content: &str) -> Result<(Self, String)> {
        let content = content.trim();

        // Verificar si empieza con ---
        if !content.starts_with("---") {
            return Err(FrontmatterError::NoFrontmatter);
        }

        // Encontrar el segundo ---
        let rest = &content[3..];
        if let Some(end_pos) = rest.find("\n---") {
            let yaml_content = &rest[..end_pos].trim();
            let remaining_content = &rest[end_pos + 4..].trim_start();

            // Parsear YAML
            let frontmatter: Frontmatter = serde_yaml::from_str(yaml_content)?;

            Ok((frontmatter, remaining_content.to_string()))
        } else {
            Err(FrontmatterError::InvalidFormat)
        }
    }

    /// Intenta parsear frontmatter, pero si falla devuelve frontmatter vacío y todo el contenido
    pub fn parse_or_empty(content: &str) -> (Self, String) {
        Self::parse(content).unwrap_or_else(|_| (Self::default(), content.to_string()))
    }

    /// Serializar frontmatter a formato YAML
    pub fn serialize(&self) -> Result<String> {
        let yaml = serde_yaml::to_string(self)?;
        Ok(format!("---\n{}---\n", yaml))
    }

    /// Crear contenido completo con frontmatter + contenido markdown
    pub fn to_markdown(&self, content: &str) -> Result<String> {
        let frontmatter_str = self.serialize()?;
        Ok(format!("{}\n{}", frontmatter_str, content))
    }

    /// Verificar si tiene tags
    pub fn has_tags(&self) -> bool {
        !self.tags.is_empty()
    }

    /// Añadir un tag
    pub fn add_tag(&mut self, tag: String) {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// Remover un tag
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
    }

    /// Limpiar tags duplicados y ordenar
    pub fn normalize_tags(&mut self) {
        self.tags.sort();
        self.tags.dedup();
    }
}

/// Extraer tags inline del contenido (patrón #palabra)
///
/// Detecta patrones como #rust #programming pero NO:
/// - # Heading (# seguido de espacio)
/// - ## Heading (múltiples #)
/// - URLs con #anchor
pub fn extract_inline_tags(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for line in lines {
        // Ignorar headings (# seguido de espacio)
        if line.trim_start().starts_with('#') && line.trim_start().chars().nth(1) == Some(' ') {
            continue;
        }

        // Buscar patrones #palabra
        let mut chars = line.chars().peekable();
        let mut current_tag = String::new();
        let mut in_tag = false;
        let mut prev_char = ' ';

        while let Some(ch) = chars.next() {
            if ch == '#' && (prev_char.is_whitespace() || prev_char == '(' || prev_char == '[') {
                // Inicio de un posible tag
                in_tag = true;
                current_tag.clear();
            } else if in_tag {
                if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                    current_tag.push(ch);
                } else {
                    // Fin del tag
                    if !current_tag.is_empty() {
                        tags.push(current_tag.trim().to_lowercase());
                        current_tag.clear();
                    }
                    in_tag = false;
                }
            }
            prev_char = ch;
        }

        // Si la línea termina con un tag
        if in_tag && !current_tag.is_empty() {
            tags.push(current_tag.trim().to_lowercase());
        }
    }

    // Eliminar duplicados
    tags.sort();
    tags.dedup();

    tags
}

/// Extraer tags de una nota (parseando el frontmatter)
pub fn extract_tags(content: &str) -> Vec<String> {
    match Frontmatter::parse(content) {
        Ok((frontmatter, _)) => frontmatter.tags,
        Err(_) => Vec::new(),
    }
}

/// Extraer todos los tags: frontmatter + inline
pub fn extract_all_tags(content: &str) -> Vec<String> {
    let mut all_tags = extract_tags(content);
    let inline_tags = extract_inline_tags(content);

    // Combinar ambas fuentes
    for tag in inline_tags {
        if !all_tags.contains(&tag) {
            all_tags.push(tag);
        }
    }

    all_tags.sort();
    all_tags.dedup();
    all_tags
}

/// Actualizar tags en el contenido de una nota
///
/// Si ya tiene frontmatter, actualiza los tags.
/// Si no tiene frontmatter, lo crea con los tags.
pub fn update_tags(content: &str, new_tags: Vec<String>) -> Result<String> {
    let (mut frontmatter, markdown_content) = Frontmatter::parse_or_empty(content);

    frontmatter.tags = new_tags;
    frontmatter.normalize_tags();

    if frontmatter.has_tags() || frontmatter.title.is_some() || !frontmatter.custom.is_empty() {
        frontmatter.to_markdown(&markdown_content)
    } else {
        // Si no hay nada en el frontmatter, no lo añadimos
        Ok(markdown_content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
tags: [rust, gtk, notas]
title: Mi Nota
date: 2025-11-01
---

# Contenido

Este es el contenido de la nota.
"#;

        let (frontmatter, body) = Frontmatter::parse(content).unwrap();

        assert_eq!(frontmatter.tags, vec!["rust", "gtk", "notas"]);
        assert_eq!(frontmatter.title, Some("Mi Nota".to_string()));
        assert_eq!(frontmatter.date, Some("2025-11-01".to_string()));
        assert!(body.starts_with("# Contenido"));
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "# Just a note\n\nWithout frontmatter.";

        let result = Frontmatter::parse(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_or_empty() {
        let content = "# Just a note\n\nWithout frontmatter.";

        let (frontmatter, body) = Frontmatter::parse_or_empty(content);

        assert!(frontmatter.tags.is_empty());
        assert_eq!(body, content);
    }

    #[test]
    fn test_serialize() {
        let mut frontmatter = Frontmatter::default();
        frontmatter.tags = vec!["rust".to_string(), "gtk".to_string()];
        frontmatter.title = Some("Test".to_string());

        let yaml = frontmatter.serialize().unwrap();

        assert!(yaml.contains("tags:"));
        assert!(yaml.contains("rust"));
        assert!(yaml.contains("gtk"));
        assert!(yaml.contains("title: Test"));
    }

    #[test]
    fn test_roundtrip() {
        let original = r#"---
tags: [rust, gtk]
title: Test Note
---

# Content
"#;

        let (frontmatter, body) = Frontmatter::parse(original).unwrap();
        let recreated = frontmatter.to_markdown(&body).unwrap();

        // Re-parsear para verificar
        let (frontmatter2, body2) = Frontmatter::parse(&recreated).unwrap();

        assert_eq!(frontmatter.tags, frontmatter2.tags);
        assert_eq!(frontmatter.title, frontmatter2.title);
        assert_eq!(body.trim(), body2.trim());
    }

    #[test]
    fn test_add_remove_tag() {
        let mut frontmatter = Frontmatter::default();

        frontmatter.add_tag("rust".to_string());
        frontmatter.add_tag("gtk".to_string());
        frontmatter.add_tag("rust".to_string()); // Duplicado

        assert_eq!(frontmatter.tags.len(), 2);

        frontmatter.remove_tag("gtk");
        assert_eq!(frontmatter.tags, vec!["rust"]);
    }

    #[test]
    fn test_normalize_tags() {
        let mut frontmatter = Frontmatter::default();
        frontmatter.tags = vec![
            "zebra".to_string(),
            "apple".to_string(),
            "zebra".to_string(),
            "banana".to_string(),
        ];

        frontmatter.normalize_tags();

        assert_eq!(frontmatter.tags, vec!["apple", "banana", "zebra"]);
    }

    #[test]
    fn test_extract_tags() {
        let content = r#"---
tags: [rust, gtk, markdown]
---

# Note content
"#;

        let tags = extract_tags(content);
        assert_eq!(tags, vec!["rust", "gtk", "markdown"]);
    }

    #[test]
    fn test_update_tags() {
        let content = r#"---
tags: [old, tags]
title: My Note
---

# Content
"#;

        let new_tags = vec!["new".to_string(), "tags".to_string()];
        let updated = update_tags(content, new_tags).unwrap();

        let (frontmatter, _) = Frontmatter::parse(&updated).unwrap();

        assert_eq!(frontmatter.tags, vec!["new", "tags"]);
        assert_eq!(frontmatter.title, Some("My Note".to_string()));
    }

    #[test]
    fn test_update_tags_no_frontmatter() {
        let content = "# Just content\n\nNo frontmatter.";

        let new_tags = vec!["rust".to_string(), "notes".to_string()];
        let updated = update_tags(content, new_tags).unwrap();

        let (frontmatter, body) = Frontmatter::parse(&updated).unwrap();

        assert_eq!(frontmatter.tags, vec!["notes", "rust"]); // Normalizados (sorted)
        assert!(body.contains("Just content"));
    }
}
