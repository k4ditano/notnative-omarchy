//! Escritor bidireccional para Bases
//! 
//! Este módulo permite modificar propiedades inline en las notas originales
//! desde la interfaz de Base de datos. Los cambios en la tabla se propagan
//! a los archivos .md correspondientes.

use std::path::Path;
use std::fs;
use thiserror::Error;

use super::database::NotesDatabase;
use super::inline_property::InlinePropertyParser;

#[derive(Debug, Error)]
pub enum BaseWriterError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Note not found: {0}")]
    NoteNotFound(String),
    
    #[error("Group not found: note_id={0}, group_id={1}")]
    GroupNotFound(i64, i64),
    
    #[error("Property not found in group: {0}")]
    PropertyNotFound(String),
    
    #[error("Database error: {0}")]
    Database(String),
}

pub type Result<T> = std::result::Result<T, BaseWriterError>;

/// Escritor bidireccional para modificar propiedades inline en notas
pub struct BaseWriter<'a> {
    db: &'a NotesDatabase,
}

impl<'a> BaseWriter<'a> {
    pub fn new(db: &'a NotesDatabase) -> Self {
        Self { db }
    }
    
    /// Actualizar el valor de una propiedad existente en un grupo
    /// 
    /// # Argumentos
    /// * `note_id` - ID de la nota en la BD
    /// * `group_id` - ID del grupo dentro de la nota
    /// * `property_key` - Nombre de la propiedad a modificar
    /// * `new_value` - Nuevo valor
    /// 
    /// NOTA: Si hay múltiples grupos IDÉNTICOS en la misma nota (exactamente las mismas propiedades),
    /// TODOS serán actualizados. Esto asegura que los duplicados deduplicados se mantengan sincronizados.
    pub fn update_property_value(
        &self,
        note_id: i64,
        group_id: i64,
        property_key: &str,
        new_value: &str,
    ) -> Result<()> {
        // Obtener path de la nota
        let note_path = self.db
            .get_note_path_by_id(note_id)
            .map_err(|e| BaseWriterError::Database(e.to_string()))?
            .ok_or_else(|| BaseWriterError::NoteNotFound(format!("ID: {}", note_id)))?;
        
        // Obtener el valor actual de la propiedad en el grupo especificado
        let current_value = self.db
            .get_property_value(note_id, group_id, property_key)
            .map_err(|e| BaseWriterError::Database(e.to_string()))?
            .ok_or_else(|| BaseWriterError::PropertyNotFound(property_key.to_string()))?;
        
        // Si el valor es el mismo, no hacer nada
        if current_value == new_value {
            return Ok(());
        }
        
        // Obtener TODOS los grupos IDÉNTICOS (mismas propiedades con mismos valores)
        // Esto permite actualizar todas las ocurrencias duplicadas a la vez
        let identical_groups = self.db
            .get_identical_groups(note_id, group_id)
            .map_err(|e| BaseWriterError::Database(e.to_string()))?;
        
        if identical_groups.is_empty() {
            return Err(BaseWriterError::GroupNotFound(note_id, group_id));
        }
        
        // Leer contenido actual
        let mut content = fs::read_to_string(&note_path)?;
        
        // Procesar todos los grupos de fin a inicio (para no afectar las posiciones)
        // Los grupos ya vienen ordenados por char_start DESC
        for (_gid, char_start, char_end) in identical_groups {
            let char_start = char_start as usize;
            let char_end = char_end as usize;
            
            if char_end > content.len() || char_start >= char_end {
                eprintln!("⚠️ Posiciones inválidas para grupo: {}..{} en contenido de {} bytes", 
                    char_start, char_end, content.len());
                continue;
            }
            
            // Extraer el grupo completo [prop1::val1, prop2::val2]
            let group_text = &content[char_start..char_end];
            
            // Verificar que es un grupo válido (empieza con [ y termina con ])
            if !group_text.starts_with('[') || !group_text.ends_with(']') {
                eprintln!("⚠️ Grupo inválido en posición {}..{}: '{}'", char_start, char_end, group_text);
                continue;
            }
            
            // Modificar el valor de la propiedad en este grupo
            match self.replace_property_in_group(group_text, property_key, new_value) {
                Ok(new_group) => {
                    // Reconstruir el contenido
                    let mut new_content = String::with_capacity(content.len());
                    new_content.push_str(&content[..char_start]);
                    new_content.push_str(&new_group);
                    new_content.push_str(&content[char_end..]);
                    content = new_content;
                }
                Err(e) => {
                    eprintln!("⚠️ Error al reemplazar propiedad en grupo: {:?}", e);
                    continue;
                }
            }
        }
        
        // Escribir de vuelta
        fs::write(&note_path, &content)?;
        
        // Re-indexar la nota en la BD
        self.reindex_note(note_id, &note_path)?;
        
        Ok(())
    }
    
    /// Añadir una propiedad nueva a un grupo existente
    /// 
    /// Transforma [juego::Minecraft] en [juego::Minecraft, horas::100]
    pub fn add_property_to_group(
        &self,
        note_id: i64,
        group_id: i64,
        property_key: &str,
        value: &str,
    ) -> Result<()> {
        // Obtener path de la nota
        let note_path = self.db
            .get_note_path_by_id(note_id)
            .map_err(|e| BaseWriterError::Database(e.to_string()))?
            .ok_or_else(|| BaseWriterError::NoteNotFound(format!("ID: {}", note_id)))?;
        
        // Obtener ubicación del grupo (posiciones absolutas en el archivo)
        let (_line_num, char_start, char_end) = self.db
            .get_group_location(note_id, group_id)
            .map_err(|e| BaseWriterError::Database(e.to_string()))?
            .ok_or_else(|| BaseWriterError::GroupNotFound(note_id, group_id))?;
        
        // Leer contenido actual
        let content = fs::read_to_string(&note_path)?;
        
        let char_start = char_start as usize;
        let char_end = char_end as usize;
        
        if char_end > content.len() || char_start >= char_end {
            return Err(BaseWriterError::GroupNotFound(note_id, group_id));
        }
        
        let group_text = &content[char_start..char_end];
        
        // Verificar que es un grupo válido
        if !group_text.starts_with('[') || !group_text.ends_with(']') {
            return Err(BaseWriterError::GroupNotFound(note_id, group_id));
        }
        
        // Añadir la nueva propiedad al grupo
        let new_group = self.append_property_to_group(group_text, property_key, value);
        
        // Reconstruir el contenido
        let mut new_content = String::with_capacity(content.len());
        new_content.push_str(&content[..char_start]);
        new_content.push_str(&new_group);
        new_content.push_str(&content[char_end..]);
        
        // Escribir de vuelta
        fs::write(&note_path, &new_content)?;
        
        // Re-indexar la nota
        self.reindex_note(note_id, &note_path)?;
        
        Ok(())
    }
    
    /// Expandir una propiedad individual a un grupo
    /// 
    /// Transforma [juego::Minecraft] (sin group_id) en [juego::Minecraft, precio::20€]
    /// Nota: char_start y char_end son posiciones absolutas en el archivo
    pub fn expand_individual_to_group(
        &self,
        note_id: i64,
        _line_number: i64,  // No usado, mantenido por compatibilidad
        char_start: i64,
        char_end: i64,
        new_property_key: &str,
        new_value: &str,
    ) -> Result<()> {
        // Obtener path de la nota
        let note_path = self.db
            .get_note_path_by_id(note_id)
            .map_err(|e| BaseWriterError::Database(e.to_string()))?
            .ok_or_else(|| BaseWriterError::NoteNotFound(format!("ID: {}", note_id)))?;
        
        // Leer contenido actual
        let content = fs::read_to_string(&note_path)?;
        
        let char_start = char_start as usize;
        let char_end = char_end as usize;
        
        if char_end > content.len() || char_start >= char_end {
            return Err(BaseWriterError::NoteNotFound(format!(
                "Positions {}..{} out of bounds", char_start, char_end
            )));
        }
        
        let prop_text = &content[char_start..char_end];
        
        // Verificar que es una propiedad válida
        if !prop_text.starts_with('[') || !prop_text.ends_with(']') {
            return Err(BaseWriterError::NoteNotFound(format!(
                "Invalid property at {}..{}: '{}'", char_start, char_end, prop_text
            )));
        }
        
        // Expandir añadiendo la nueva propiedad
        let new_group = self.append_property_to_group(prop_text, new_property_key, new_value);
        
        // Reconstruir el contenido
        let mut new_content = String::with_capacity(content.len());
        new_content.push_str(&content[..char_start]);
        new_content.push_str(&new_group);
        new_content.push_str(&content[char_end..]);
        
        // Escribir de vuelta
        fs::write(&note_path, &new_content)?;
        
        // Re-indexar la nota
        self.reindex_note(note_id, &note_path)?;
        
        Ok(())
    }
    
    /// Reemplazar el valor de una propiedad dentro de un texto de grupo
    /// 
    /// Ejemplo: "[juego::Minecraft, precio::10€]" con key="precio", value="20€"
    /// Resultado: "[juego::Minecraft, precio::20€]"
    fn replace_property_in_group(
        &self,
        group_text: &str,
        property_key: &str,
        new_value: &str,
    ) -> Result<String> {
        // Parsear el grupo para encontrar la propiedad
        let props = InlinePropertyParser::parse(group_text);
        
        let mut found = false;
        let mut result = group_text.to_string();
        
        for prop in props.iter().rev() { // Iteramos en reversa para no afectar los índices
            if prop.key == property_key {
                found = true;
                // Calcular posición del valor dentro del grupo
                // El formato es [key::value] o [key::value, ...]
                let key_pattern = format!("{}::", property_key);
                if let Some(key_pos) = group_text.find(&key_pattern) {
                    let value_start = key_pos + key_pattern.len();
                    // Encontrar el fin del valor (hasta , o ])
                    let remaining = &group_text[value_start..];
                    let value_end = remaining
                        .find(|c| c == ',' || c == ']')
                        .unwrap_or(remaining.len());
                    
                    let old_value = &remaining[..value_end];
                    result = result.replacen(
                        &format!("{}::{}", property_key, old_value),
                        &format!("{}::{}", property_key, new_value),
                        1,
                    );
                }
                break;
            }
        }
        
        if !found {
            return Err(BaseWriterError::PropertyNotFound(property_key.to_string()));
        }
        
        Ok(result)
    }
    
    /// Añadir una propiedad al final de un grupo
    /// 
    /// Ejemplo: "[juego::Minecraft]" + key="precio", value="20€"
    /// Resultado: "[juego::Minecraft, precio::20€]"
    fn append_property_to_group(&self, group_text: &str, key: &str, value: &str) -> String {
        // Encontrar el ] final y añadir antes de él
        if let Some(bracket_pos) = group_text.rfind(']') {
            let before = &group_text[..bracket_pos];
            format!("{}, {}::{}]", before, key, value)
        } else {
            // No debería pasar, pero por si acaso
            format!("{}, {}::{}", group_text, key, value)
        }
    }
    
    /// Re-indexar una nota después de modificarla
    fn reindex_note(&self, note_id: i64, note_path: &str) -> Result<()> {
        let content = fs::read_to_string(note_path)?;
        
        // Sincronizar propiedades inline
        self.db
            .sync_inline_properties(note_id, &content)
            .map_err(|e| BaseWriterError::Database(e.to_string()))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    /// Función auxiliar para testear la lógica pura de append_property_to_group
    /// sin necesidad de una instancia de BaseWriter
    fn append_property_to_group_pure(group_text: &str, key: &str, value: &str) -> String {
        if let Some(bracket_pos) = group_text.rfind(']') {
            let before = &group_text[..bracket_pos];
            format!("{}, {}::{}]", before, key, value)
        } else {
            format!("{}, {}::{}", group_text, key, value)
        }
    }
    
    #[test]
    fn test_append_property_to_group() {
        // Testear la lógica pura sin necesidad de DB
        let group = "[juego::Minecraft]";
        let result = append_property_to_group_pure(group, "precio", "20€");
        assert_eq!(result, "[juego::Minecraft, precio::20€]");
        
        let group2 = "[juego::Minecraft, horas::100]";
        let result2 = append_property_to_group_pure(group2, "rating", "5");
        assert_eq!(result2, "[juego::Minecraft, horas::100, rating::5]");
    }
}
