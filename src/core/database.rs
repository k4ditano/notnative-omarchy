use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Result as SqliteResult, params};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Note not found: {0}")]
    NoteNotFound(String),

    #[error("Tag not found: {0}")]
    TagNotFound(String),
}

pub type Result<T> = std::result::Result<T, DatabaseError>;

/// Metadata de una nota almacenada en la base de datos
#[derive(Debug, Clone)]
pub struct NoteMetadata {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub folder: Option<String>,
    pub order_index: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Un tag con información de uso
#[derive(Debug, Clone)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub usage_count: i32,
}

/// Resultado de una búsqueda con snippet y relevancia
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub note_id: i64,
    pub note_name: String,
    pub note_path: String,
    pub snippet: String,
    pub relevance: f32,
    pub matched_tags: Vec<String>,
}

/// Query de búsqueda con filtros opcionales
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub tags: Vec<String>,
    pub folder: Option<String>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
}

/// Base de datos SQLite para indexar notas
pub struct NotesDatabase {
    conn: Connection,
    path: PathBuf,
}

impl std::fmt::Debug for NotesDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotesDatabase")
            .field("path", &self.path)
            .finish()
    }
}

impl NotesDatabase {
    /// Versión actual del esquema
    const SCHEMA_VERSION: i32 = 1;

    /// Crear o abrir base de datos en la ruta especificada
    pub fn new(path: &Path) -> Result<Self> {
        // Crear directorio si no existe
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;

        let mut db = Self {
            conn,
            path: path.to_path_buf(),
        };

        db.initialize_schema()?;
        db.migrate_if_needed()?;

        Ok(db)
    }

    /// Clona la conexión abriendo una nueva conexión a la misma base de datos
    pub fn clone_connection(&self) -> Self {
        let conn =
            Connection::open(&self.path).expect("No se pudo clonar la conexión a la base de datos");
        Self {
            conn,
            path: self.path.clone(),
        }
    }

    /// Inicializar esquema de base de datos
    fn initialize_schema(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            -- Tabla de versión del esquema
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            );
            
            -- Tabla principal de notas
            CREATE TABLE IF NOT EXISTS notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                path TEXT NOT NULL UNIQUE,
                folder TEXT,
                order_index INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            
            -- Tabla de tags
            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                color TEXT,
                usage_count INTEGER DEFAULT 0
            );
            
            -- Relación many-to-many entre notas y tags
            CREATE TABLE IF NOT EXISTS note_tags (
                note_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                PRIMARY KEY (note_id, tag_id),
                FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            );
            
            -- Tabla virtual para full-text search
            CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
                name,
                content,
                tokenize = 'porter unicode61'
            );
            
            -- Índices para mejorar performance
            CREATE INDEX IF NOT EXISTS idx_notes_folder ON notes(folder);
            CREATE INDEX IF NOT EXISTS idx_notes_updated ON notes(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_notes_order ON notes(order_index);
            CREATE INDEX IF NOT EXISTS idx_tags_usage ON tags(usage_count DESC);
            CREATE INDEX IF NOT EXISTS idx_note_tags_note ON note_tags(note_id);
            CREATE INDEX IF NOT EXISTS idx_note_tags_tag ON note_tags(tag_id);
            
            -- Tabla de sesiones de chat
            CREATE TABLE IF NOT EXISTS chat_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                model TEXT NOT NULL,
                provider TEXT NOT NULL,
                temperature REAL DEFAULT 0.7,
                max_tokens INTEGER DEFAULT 2000
            );
            
            -- Tabla de mensajes de chat
            CREATE TABLE IF NOT EXISTS chat_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL,
                role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
            );
            
            -- Tabla de notas adjuntas al contexto del chat
            CREATE TABLE IF NOT EXISTS chat_context_notes (
                session_id INTEGER NOT NULL,
                note_id INTEGER NOT NULL,
                added_at INTEGER NOT NULL,
                PRIMARY KEY (session_id, note_id),
                FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE,
                FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
            );
            
            -- Índices para chat
            CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id);
            CREATE INDEX IF NOT EXISTS idx_chat_messages_created ON chat_messages(created_at);
            CREATE INDEX IF NOT EXISTS idx_chat_context_session ON chat_context_notes(session_id);
            "#,
        )?;

        // Insertar versión del esquema si no existe (por separado porque execute_batch no soporta params)
        self.conn.execute(
            "INSERT OR IGNORE INTO schema_version (version) VALUES (?1)",
            params![Self::SCHEMA_VERSION],
        )?;

        self.conn.execute(
            "UPDATE schema_version SET version = ?1",
            params![Self::SCHEMA_VERSION],
        )?;

        Ok(())
    }

    /// Verificar y ejecutar migraciones si es necesario
    fn migrate_if_needed(&mut self) -> Result<()> {
        let current_version: i32 =
            self.conn
                .query_row("SELECT version FROM schema_version", [], |row| row.get(0))?;

        if current_version < Self::SCHEMA_VERSION {
            println!(
                "Migrando base de datos de v{} a v{}",
                current_version,
                Self::SCHEMA_VERSION
            );
            // Aquí irían las migraciones futuras
        }

        Ok(())
    }

    /// Indexar una nota en la base de datos
    pub fn index_note(
        &self,
        name: &str,
        path: &str,
        content: &str,
        folder: Option<&str>,
    ) -> Result<i64> {
        let now = Utc::now().timestamp();

        // Insertar o actualizar nota
        self.conn.execute(
            r#"
            INSERT INTO notes (name, path, folder, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(name) DO UPDATE SET
                path = excluded.path,
                folder = excluded.folder,
                updated_at = excluded.updated_at
            "#,
            params![name, path, folder, now, now],
        )?;

        let note_id = self.conn.last_insert_rowid();

        // Indexar en FTS5
        self.conn.execute(
            "INSERT OR REPLACE INTO notes_fts (rowid, name, content) VALUES (?1, ?2, ?3)",
            params![note_id, name, content],
        )?;

        Ok(note_id)
    }

    /// Actualizar una nota existente
    pub fn update_note(&self, name: &str, content: &str) -> Result<()> {
        let now = Utc::now().timestamp();

        // Obtener ID de la nota
        let note_id: i64 = self
            .conn
            .query_row(
                "SELECT id FROM notes WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| DatabaseError::NoteNotFound(name.to_string()))?;

        // Actualizar timestamp
        self.conn.execute(
            "UPDATE notes SET updated_at = ?1 WHERE id = ?2",
            params![now, note_id],
        )?;

        // Actualizar FTS5
        self.conn.execute(
            "UPDATE notes_fts SET content = ?1 WHERE rowid = ?2",
            params![content, note_id],
        )?;

        Ok(())
    }

    /// Eliminar una nota de la base de datos
    pub fn delete_note(&self, name: &str) -> Result<()> {
        let note_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM notes WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = note_id {
            self.conn
                .execute("DELETE FROM notes WHERE id = ?1", params![id])?;
            self.conn
                .execute("DELETE FROM notes_fts WHERE rowid = ?1", params![id])?;
        }

        Ok(())
    }

    /// Obtener metadata de una nota
    pub fn get_note(&self, name: &str) -> Result<Option<NoteMetadata>> {
        let result = self
            .conn
            .query_row(
                r#"
            SELECT id, name, path, folder, order_index, created_at, updated_at
            FROM notes WHERE name = ?1
            "#,
                params![name],
                |row| {
                    Ok(NoteMetadata {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: row.get(2)?,
                        folder: row.get(3)?,
                        order_index: row.get(4)?,
                        created_at: DateTime::from_timestamp(row.get(5)?, 0).unwrap(),
                        updated_at: DateTime::from_timestamp(row.get(6)?, 0).unwrap(),
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    /// Listar todas las notas, opcionalmente filtradas por carpeta
    pub fn list_notes(&self, folder: Option<&str>) -> Result<Vec<NoteMetadata>> {
        let mut stmt = if folder.is_some() {
            self.conn.prepare(
                "SELECT id, name, path, folder, order_index, created_at, updated_at 
                 FROM notes WHERE folder = ?1 ORDER BY order_index, name",
            )?
        } else {
            self.conn.prepare(
                "SELECT id, name, path, folder, order_index, created_at, updated_at 
                 FROM notes ORDER BY order_index, name",
            )?
        };

        let notes = if let Some(f) = folder {
            stmt.query_map(params![f], Self::row_to_note_metadata)?
        } else {
            stmt.query_map([], Self::row_to_note_metadata)?
        };

        notes.collect::<SqliteResult<Vec<_>>>().map_err(Into::into)
    }

    /// Convertir fila SQL a NoteMetadata
    fn row_to_note_metadata(row: &rusqlite::Row) -> SqliteResult<NoteMetadata> {
        Ok(NoteMetadata {
            id: row.get(0)?,
            name: row.get(1)?,
            path: row.get(2)?,
            folder: row.get(3)?,
            order_index: row.get(4)?,
            created_at: DateTime::from_timestamp(row.get(5)?, 0).unwrap(),
            updated_at: DateTime::from_timestamp(row.get(6)?, 0).unwrap(),
        })
    }

    /// Buscar notas usando FTS5 y filtros opcionales
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        if query.text.is_none() && query.tags.is_empty() {
            // Sin filtros, devolver todas las notas
            let notes = self
                .list_notes(query.folder.as_deref())?
                .into_iter()
                .map(|note| SearchResult {
                    note_id: note.id,
                    note_name: note.name,
                    note_path: note.path,
                    snippet: String::new(),
                    relevance: 0.0,
                    matched_tags: vec![],
                })
                .collect();
            return Ok(notes);
        }

        // TODO: Implementar búsqueda FTS5 completa con snippets
        Ok(vec![])
    }

    /// Construye una query FTS5 inteligente desde el texto del usuario
    fn build_fts_query(query_text: &str) -> String {
        let trimmed = query_text.trim();

        // Si el usuario usa comillas, buscar literalmente
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 2 {
            let literal = &trimmed[1..trimmed.len() - 1];
            return format!("\"{}\"", literal.replace('"', "\"\""));
        }

        // Separar por palabras y construir query con OR
        let words: Vec<&str> = trimmed.split_whitespace().collect();

        if words.is_empty() {
            return String::new();
        }

        // Si es una sola palabra, usar wildcard para prefijos
        if words.len() == 1 {
            let word = Self::sanitize_fts_word(words[0]);
            if word.is_empty() {
                return String::new();
            }
            return format!("{}*", word);
        }

        // Para múltiples palabras, buscar todas con AND (deben estar todas presentes)
        let sanitized_words: Vec<String> = words
            .iter()
            .map(|w| Self::sanitize_fts_word(w))
            .filter(|w| !w.is_empty())
            .map(|w| format!("{}*", w))
            .collect();

        if sanitized_words.is_empty() {
            return String::new();
        }

        sanitized_words.join(" ")
    }

    /// Sanitiza una palabra individual para FTS5
    fn sanitize_fts_word(word: &str) -> String {
        // Caracteres especiales de FTS5 que necesitan ser escapados o removidos
        let mut result = String::new();

        for ch in word.chars() {
            match ch {
                // Operadores FTS5 que removemos
                '"' | '*' | '(' | ')' | '{' | '}' | '[' | ']' | '^' | ':' | '#' | '+' | '-'
                | '!' | '&' | '|' | '~' | '.' | ',' | ';' | '=' | '<' | '>' | '/' | '\\' | '?'
                | '@' | '%' | '$' => {}
                // Caracteres válidos (letras, números, _, espacios)
                _ => result.push(ch),
            }
        }

        result
    }

    /// Búsqueda simple por texto usando FTS5
    pub fn search_notes(&self, query_text: &str) -> Result<Vec<SearchResult>> {
        if query_text.trim().is_empty() {
            return Ok(vec![]);
        }

        // Construir query FTS5 inteligente
        let fts_query = Self::build_fts_query(query_text);

        // Si después de sanitizar no queda nada válido, retornar vacío
        if fts_query.trim().is_empty() {
            return Ok(vec![]);
        }

        let mut stmt = self.conn.prepare(
            r#"
            SELECT 
                notes.id,
                notes.name,
                notes.path,
                snippet(notes_fts, -1, '<mark>', '</mark>', '...', 32) as snippet,
                rank as relevance
            FROM notes_fts
            JOIN notes ON notes_fts.rowid = notes.id
            WHERE notes_fts MATCH ?1
            ORDER BY rank
            LIMIT 50
            "#,
        )?;

        let results = stmt
            .query_map([&fts_query], |row| {
                Ok(SearchResult {
                    note_id: row.get(0)?,
                    note_name: row.get(1)?,
                    note_path: row.get(2)?,
                    snippet: row.get(3)?,
                    relevance: row.get::<_, f64>(4)? as f32,
                    matched_tags: vec![],
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Obtener todos los tags ordenados por uso
    pub fn get_tags(&self) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, color, usage_count FROM tags ORDER BY usage_count DESC, name",
        )?;

        let tags = stmt.query_map([], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                usage_count: row.get(3)?,
            })
        })?;

        tags.collect::<SqliteResult<Vec<_>>>().map_err(Into::into)
    }

    /// Añadir un tag a una nota
    pub fn add_tag(&self, note_id: i64, tag_name: &str) -> Result<()> {
        // Crear tag si no existe
        self.conn.execute(
            "INSERT OR IGNORE INTO tags (name, usage_count) VALUES (?1, 0)",
            params![tag_name],
        )?;

        let tag_id: i64 = self.conn.query_row(
            "SELECT id FROM tags WHERE name = ?1",
            params![tag_name],
            |row| row.get(0),
        )?;

        // Añadir relación
        self.conn.execute(
            "INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
            params![note_id, tag_id],
        )?;

        // Incrementar contador de uso
        self.conn.execute(
            "UPDATE tags SET usage_count = usage_count + 1 WHERE id = ?1",
            params![tag_id],
        )?;

        Ok(())
    }

    /// Remover un tag de una nota
    pub fn remove_tag(&self, note_id: i64, tag_name: &str) -> Result<()> {
        let tag_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM tags WHERE name = ?1",
                params![tag_name],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(tid) = tag_id {
            self.conn.execute(
                "DELETE FROM note_tags WHERE note_id = ?1 AND tag_id = ?2",
                params![note_id, tid],
            )?;

            // Decrementar contador
            self.conn.execute(
                "UPDATE tags SET usage_count = MAX(0, usage_count - 1) WHERE id = ?1",
                params![tid],
            )?;
        }

        Ok(())
    }

    /// Obtener tags de una nota específica
    pub fn get_note_tags(&self, note_id: i64) -> Result<Vec<Tag>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.id, t.name, t.color, t.usage_count
            FROM tags t
            INNER JOIN note_tags nt ON t.id = nt.tag_id
            WHERE nt.note_id = ?1
            ORDER BY t.name
            "#,
        )?;

        let tags = stmt.query_map(params![note_id], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                usage_count: row.get(3)?,
            })
        })?;

        tags.collect::<SqliteResult<Vec<_>>>().map_err(Into::into)
    }

    /// Actualizar el orden de una nota
    pub fn update_note_order(&self, note_id: i64, new_order: i32) -> Result<()> {
        self.conn.execute(
            "UPDATE notes SET order_index = ?1 WHERE id = ?2",
            params![new_order, note_id],
        )?;
        Ok(())
    }

    /// Mover una nota a una carpeta diferente
    pub fn move_note_to_folder(
        &self,
        note_id: i64,
        new_folder: Option<&str>,
        new_path: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE notes SET folder = ?1, path = ?2 WHERE id = ?3",
            params![new_folder, new_path, note_id],
        )?;
        Ok(())
    }

    // === Chat History Methods ===

    /// Crear una nueva sesión de chat
    pub fn create_chat_session(
        &self,
        model: &str,
        provider: &str,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<i64> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            r#"
            INSERT INTO chat_sessions (created_at, updated_at, model, provider, temperature, max_tokens)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![now, now, model, provider, temperature, max_tokens as i64],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Guardar un mensaje en una sesión
    pub fn save_chat_message(&self, session_id: i64, role: &str, content: &str) -> Result<i64> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            r#"
            INSERT INTO chat_messages (session_id, role, content, created_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![session_id, role, content, now],
        )?;

        // Actualizar timestamp de la sesión
        self.conn.execute(
            "UPDATE chat_sessions SET updated_at = ?1 WHERE id = ?2",
            params![now, session_id],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Obtener mensajes de una sesión
    pub fn get_chat_messages(
        &self,
        session_id: i64,
    ) -> Result<Vec<(String, String, DateTime<Utc>)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT role, content, created_at
            FROM chat_messages
            WHERE session_id = ?1
            ORDER BY created_at ASC
            "#,
        )?;

        let messages = stmt
            .query_map(params![session_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    DateTime::from_timestamp(row.get::<_, i64>(2)?, 0).unwrap(),
                ))
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(messages)
    }

    /// Adjuntar una nota al contexto de una sesión
    pub fn attach_note_to_chat(&self, session_id: i64, note_id: i64) -> Result<()> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            r#"
            INSERT OR IGNORE INTO chat_context_notes (session_id, note_id, added_at)
            VALUES (?1, ?2, ?3)
            "#,
            params![session_id, note_id, now],
        )?;

        Ok(())
    }

    /// Obtener notas adjuntas al contexto de una sesión
    pub fn get_chat_context_notes(&self, session_id: i64) -> Result<Vec<NoteMetadata>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT n.id, n.name, n.path, n.folder, n.order_index, n.created_at, n.updated_at
            FROM notes n
            INNER JOIN chat_context_notes ccn ON n.id = ccn.note_id
            WHERE ccn.session_id = ?1
            ORDER BY ccn.added_at ASC
            "#,
        )?;

        let notes = stmt
            .query_map(params![session_id], Self::row_to_note_metadata)?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(notes)
    }

    /// Obtener la última sesión de chat
    pub fn get_latest_chat_session(&self) -> Result<Option<i64>> {
        let session_id = self
            .conn
            .query_row(
                "SELECT id FROM chat_sessions ORDER BY updated_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;

        Ok(session_id)
    }

    /// Eliminar una sesión de chat y todos sus mensajes
    pub fn delete_chat_session(&self, session_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM chat_sessions WHERE id = ?1",
            params![session_id],
        )?;
        // Los mensajes y contexto se eliminan por CASCADE
        Ok(())
    }

    /// Eliminar todo el historial de chat
    pub fn clear_all_chat_history(&self) -> Result<()> {
        self.conn.execute("DELETE FROM chat_sessions", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_database() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_notes.db");

        let db = NotesDatabase::new(&db_path).unwrap();
        assert!(db_path.exists());

        // Cleanup
        std::fs::remove_file(db_path).ok();
    }

    #[test]
    fn test_index_note() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_notes_index.db");

        let db = NotesDatabase::new(&db_path).unwrap();

        let note_id = db
            .index_note(
                "test-note",
                "/path/to/test-note.md",
                "# Test Note\n\nSome content here.",
                None,
            )
            .unwrap();

        assert!(note_id > 0);

        let note = db.get_note("test-note").unwrap();
        assert!(note.is_some());

        // Cleanup
        std::fs::remove_file(db_path).ok();
    }

    #[test]
    fn test_tags() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_notes_tags.db");

        let db = NotesDatabase::new(&db_path).unwrap();

        let note_id = db
            .index_note("tagged-note", "/path/to/tagged-note.md", "Content", None)
            .unwrap();

        db.add_tag(note_id, "rust").unwrap();
        db.add_tag(note_id, "gtk").unwrap();

        let tags = db.get_note_tags(note_id).unwrap();
        assert_eq!(tags.len(), 2);

        let all_tags = db.get_tags().unwrap();
        assert_eq!(all_tags.len(), 2);

        // Cleanup
        std::fs::remove_file(db_path).ok();
    }
}
