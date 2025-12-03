use std::collections::HashMap;
use std::path::Path;

use super::base::{Base, BaseView, FilterGroup, SortConfig, SortDirection};
use super::database::{NotesDatabase, NoteMetadata, Result as DbResult};
use super::property::PropertyValue;

/// Resultado de una nota con sus propiedades extraídas
#[derive(Debug, Clone)]
pub struct NoteWithProperties {
    /// Metadata de la nota desde la BD
    pub metadata: NoteMetadata,
    
    /// Propiedades extraídas del frontmatter + metadata
    pub properties: HashMap<String, PropertyValue>,
    
    /// Contenido de la nota (opcional, para vistas que lo necesiten)
    pub content: Option<String>,
}

impl NoteWithProperties {
    /// Obtener una propiedad por nombre
    pub fn get(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }

    /// Obtener el valor de display de una propiedad
    pub fn get_display(&self, key: &str) -> String {
        self.properties
            .get(key)
            .map(|v| v.to_display_string())
            .unwrap_or_else(|| "—".to_string())
    }
}

/// Motor de queries para Bases
pub struct BaseQueryEngine<'a> {
    db: &'a NotesDatabase,
    notes_root: &'a Path,
}

impl<'a> BaseQueryEngine<'a> {
    pub fn new(db: &'a NotesDatabase, notes_root: &'a Path) -> Self {
        Self { db, notes_root }
    }

    /// Ejecutar una query de Base y devolver las notas que coinciden
    pub fn query(&self, base: &Base) -> DbResult<Vec<NoteWithProperties>> {
        let view = base.active_view().ok_or_else(|| {
            super::database::DatabaseError::NoteNotFound("No active view".to_string())
        })?;

        self.query_view(view, base.source_folder.as_deref())
    }

    /// Ejecutar una query para una vista específica
    pub fn query_view(
        &self,
        view: &BaseView,
        source_folder: Option<&str>,
    ) -> DbResult<Vec<NoteWithProperties>> {
        // 1. Obtener todas las notas (opcionalmente filtradas por carpeta)
        let notes = self.db.list_notes(source_folder)?;

        // 2. Cargar propiedades de cada nota y filtrar
        let mut results: Vec<NoteWithProperties> = notes
            .into_iter()
            .filter_map(|note| {
                // Cargar contenido y extraer propiedades
                let props = self.load_note_properties(&note).ok()?;
                
                // Aplicar filtros
                if view.filter.evaluate(&props.properties) {
                    Some(props)
                } else {
                    None
                }
            })
            .collect();

        // 3. Ordenar resultados
        if let Some(sort) = &view.sort {
            self.sort_results(&mut results, sort);
        }

        Ok(results)
    }

    /// Cargar propiedades de una nota desde la BD (propiedades inline indexadas)
    fn load_note_properties(&self, note: &NoteMetadata) -> DbResult<NoteWithProperties> {
        let mut properties = HashMap::new();

        // Propiedades built-in desde metadata
        properties.insert(
            "title".to_string(),
            PropertyValue::Text(note.name.clone()),
        );
        properties.insert(
            "name".to_string(),
            PropertyValue::Text(note.name.clone()),
        );
        properties.insert(
            "path".to_string(),
            PropertyValue::Text(note.path.clone()),
        );
        
        if let Some(folder) = &note.folder {
            properties.insert(
                "folder".to_string(),
                PropertyValue::Text(folder.clone()),
            );
        }

        properties.insert(
            "created_at".to_string(),
            PropertyValue::DateTime(note.created_at.to_rfc3339()),
        );
        properties.insert(
            "updated_at".to_string(),
            PropertyValue::DateTime(note.updated_at.to_rfc3339()),
        );

        // Cargar tags de la BD
        if let Ok(tags) = self.db.get_note_tags(note.id) {
            let tag_names: Vec<String> = tags.into_iter().map(|t| t.name).collect();
            if !tag_names.is_empty() {
                properties.insert("tags".to_string(), PropertyValue::Tags(tag_names));
            }
        }

        // Cargar propiedades inline desde la tabla inline_properties
        if let Ok(inline_props) = self.db.get_inline_properties(note.id) {
            for prop in inline_props {
                let value = prop.to_property_value();
                let key = prop.key.clone();
                
                // Si ya existe la misma key, combinar valores
                if properties.contains_key(&key) {
                    let existing = properties.remove(&key).unwrap();
                    let new_value = match (existing, &value) {
                        (PropertyValue::List(mut list), PropertyValue::Text(s)) => {
                            list.push(s.clone());
                            PropertyValue::List(list)
                        }
                        (PropertyValue::Text(s1), PropertyValue::Text(s2)) => {
                            PropertyValue::List(vec![s1, s2.clone()])
                        }
                        (_, new) => new.clone(),
                    };
                    properties.insert(key, new_value);
                } else {
                    properties.insert(key, value);
                }
            }
        }

        // Cargar contenido si es necesario (solo el path, no parseamos frontmatter)
        let content = if Path::new(&note.path).exists() {
            std::fs::read_to_string(&note.path).ok()
        } else {
            None
        };

        Ok(NoteWithProperties {
            metadata: note.clone(),
            properties,
            content,
        })
    }

    /// Ordenar resultados según configuración
    fn sort_results(&self, results: &mut Vec<NoteWithProperties>, sort: &SortConfig) {
        results.sort_by(|a, b| {
            let key_a = a.properties
                .get(&sort.property)
                .map(|v| v.sort_key())
                .unwrap_or_default();
            let key_b = b.properties
                .get(&sort.property)
                .map(|v| v.sort_key())
                .unwrap_or_default();

            match sort.direction {
                SortDirection::Asc => key_a.cmp(&key_b),
                SortDirection::Desc => key_b.cmp(&key_a),
            }
        });
    }

    /// Agrupar resultados por una propiedad (para vistas Board/Gallery)
    pub fn group_by(
        &self,
        results: Vec<NoteWithProperties>,
        property: &str,
    ) -> HashMap<String, Vec<NoteWithProperties>> {
        let mut groups: HashMap<String, Vec<NoteWithProperties>> = HashMap::new();

        for note in results {
            let group_key = note
                .properties
                .get(property)
                .map(|v| v.to_display_string())
                .unwrap_or_else(|| "—".to_string());

            groups.entry(group_key).or_default().push(note);
        }

        groups
    }

    /// Obtener todas las propiedades únicas encontradas en las notas
    /// Descubrir propiedades disponibles para el modo Notes (solo propiedades de notas)
    pub fn discover_properties(&self, _source_folder: Option<&str>) -> DbResult<Vec<String>> {
        // Solo propiedades built-in de notas - NO incluir propiedades inline
        let mut property_names = std::collections::HashSet::new();
        property_names.insert("title".to_string());
        property_names.insert("tags".to_string());
        property_names.insert("folder".to_string());
        property_names.insert("created_at".to_string());
        property_names.insert("updated_at".to_string());

        let mut names: Vec<String> = property_names.into_iter().collect();
        names.sort();
        Ok(names)
    }
    
    /// Descubrir propiedades inline para el modo Inline Data
    pub fn discover_inline_properties(&self) -> DbResult<Vec<String>> {
        let mut property_names = std::collections::HashSet::new();
        
        // _note es especial - referencia a la nota origen
        property_names.insert("_note".to_string());
        
        // Descubrir propiedades inline desde la BD
        if let Ok(keys) = self.db.get_all_property_keys() {
            for key in keys {
                property_names.insert(key);
            }
        }

        let mut names: Vec<String> = property_names.into_iter().collect();
        names.sort();
        Ok(names)
    }

    /// Contar notas por valor de una propiedad (para estadísticas/summaries)
    pub fn count_by_property(
        &self,
        results: &[NoteWithProperties],
        property: &str,
    ) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();

        for note in results {
            let value = note
                .properties
                .get(property)
                .map(|v| v.to_display_string())
                .unwrap_or_else(|| "—".to_string());

            *counts.entry(value).or_insert(0) += 1;
        }

        counts
    }

    /// Calcular suma de una propiedad numérica
    pub fn sum_property(&self, results: &[NoteWithProperties], property: &str) -> f64 {
        results
            .iter()
            .filter_map(|note| {
                note.properties.get(property).and_then(|v| {
                    if let PropertyValue::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
            })
            .sum()
    }

    /// Calcular promedio de una propiedad numérica
    pub fn avg_property(&self, results: &[NoteWithProperties], property: &str) -> Option<f64> {
        let values: Vec<f64> = results
            .iter()
            .filter_map(|note| {
                note.properties.get(property).and_then(|v| {
                    if let PropertyValue::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
            })
            .collect();

        if values.is_empty() {
            None
        } else {
            Some(values.iter().sum::<f64>() / values.len() as f64)
        }
    }
    
    /// Calcular el valor mínimo de una propiedad numérica
    pub fn min_property(&self, results: &[NoteWithProperties], property: &str) -> Option<f64> {
        results
            .iter()
            .filter_map(|note| {
                note.properties.get(property).and_then(|v| {
                    if let PropertyValue::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
            })
            .fold(None, |acc, val| {
                Some(acc.map_or(val, |a: f64| a.min(val)))
            })
    }
    
    /// Calcular el valor máximo de una propiedad numérica
    pub fn max_property(&self, results: &[NoteWithProperties], property: &str) -> Option<f64> {
        results
            .iter()
            .filter_map(|note| {
                note.properties.get(property).and_then(|v| {
                    if let PropertyValue::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
            })
            .fold(None, |acc, val| {
                Some(acc.map_or(val, |a: f64| a.max(val)))
            })
    }
    
    /// Contar valores no vacíos de una propiedad
    pub fn count_non_empty(&self, results: &[NoteWithProperties], property: &str) -> usize {
        results
            .iter()
            .filter(|note| {
                note.properties.get(property)
                    .map(|v| !v.is_empty())
                    .unwrap_or(false)
            })
            .count()
    }
    
    /// Calcular todas las agregaciones para una propiedad numérica
    pub fn aggregate_property(&self, results: &[NoteWithProperties], property: &str) -> PropertyAggregation {
        PropertyAggregation {
            sum: self.sum_property(results, property),
            avg: self.avg_property(results, property),
            min: self.min_property(results, property),
            max: self.max_property(results, property),
            count: self.count_non_empty(results, property),
            total: results.len(),
        }
    }
}

/// Resultado de agregaciones sobre una propiedad
#[derive(Debug, Clone, Default)]
pub struct PropertyAggregation {
    pub sum: f64,
    pub avg: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub count: usize,
    pub total: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::base::{Filter, FilterGroup, FilterOperator};

    fn make_test_note(name: &str, props: HashMap<String, PropertyValue>) -> NoteWithProperties {
        NoteWithProperties {
            metadata: NoteMetadata {
                id: 1,
                name: name.to_string(),
                path: format!("/test/{}.md", name),
                folder: None,
                order_index: 0,
                icon: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            properties: props,
            content: None,
        }
    }

    #[test]
    fn test_filter_group_evaluation() {
        let filter = FilterGroup::new(vec![
            Filter::has_tag("rust"),
        ]);

        let mut props = HashMap::new();
        props.insert("tags".to_string(), PropertyValue::Tags(vec!["rust".to_string(), "gtk".to_string()]));
        props.insert("title".to_string(), PropertyValue::Text("Test Note".to_string()));

        assert!(filter.evaluate(&props));
    }

    #[test]
    fn test_note_with_properties() {
        let mut props = HashMap::new();
        props.insert("status".to_string(), PropertyValue::Text("done".to_string()));
        props.insert("priority".to_string(), PropertyValue::Number(1.0));

        let note = make_test_note("test", props);

        assert_eq!(note.get_display("status"), "done");
        assert_eq!(note.get_display("priority"), "1");
        assert_eq!(note.get_display("nonexistent"), "—");
    }
}
