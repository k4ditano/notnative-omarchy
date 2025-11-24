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
    pub similarity: Option<f32>, // Para búsqueda semántica
}

/// Embedding de un chunk de nota
#[derive(Debug, Clone)]
pub struct NoteEmbedding {
    pub id: i64,
    pub chunk_index: usize,
    pub chunk_text: String,
    pub embedding: Vec<f32>,
}

/// Embedding global con path de nota
#[derive(Debug, Clone)]
pub struct GlobalEmbedding {
    pub note_path: String,
    pub chunk_index: usize,
    pub chunk_text: String,
    pub embedding: Vec<f32>,
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
    const SCHEMA_VERSION: i32 = 4;

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

        // Primero crear tabla de versión si no existe
        db.ensure_version_table()?;

        // Verificar versión actual y migrar si es necesario
        db.migrate_if_needed()?;

        // Inicializar esquema completo
        db.initialize_schema()?;

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

    /// Obtiene el path de la base de datos
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Asegurar que existe la tabla de versión
    fn ensure_version_table(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            );
            "#,
        )?;

        // Verificar si hay una versión registrada
        let version_exists: bool = self
            .conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |row| {
                row.get::<_, i64>(0)
            })
            .map(|count| count > 0)?;

        // Si no hay versión, insertar la versión 1 (asumimos base de datos nueva o antigua)
        if !version_exists {
            self.conn
                .execute("INSERT INTO schema_version (version) VALUES (1)", [])?;
        }

        Ok(())
    }

    /// Inicializar esquema de base de datos
    fn initialize_schema(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
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

            -- Tabla de embeddings para búsqueda semántica (v2)
            CREATE TABLE IF NOT EXISTS note_embeddings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                note_path TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                chunk_text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                token_count INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(note_path, chunk_index)
            );

            -- Índices para embeddings
            CREATE INDEX IF NOT EXISTS idx_embeddings_note ON note_embeddings(note_path);
            CREATE INDEX IF NOT EXISTS idx_embeddings_updated ON note_embeddings(updated_at DESC);
            "#,
        )?;

        Ok(())
    }

    /// Verificar y ejecutar migraciones si es necesario
    fn migrate_if_needed(&mut self) -> Result<()> {
        let current_version: i32 =
            self.conn
                .query_row("SELECT version FROM schema_version", [], |row| row.get(0))?;

        if current_version < Self::SCHEMA_VERSION {
            // Solo mostrar mensaje si realmente necesitamos migrar
            println!(
                "Migrando base de datos de v{} a v{}",
                current_version,
                Self::SCHEMA_VERSION
            );

            // Migración v1 -> v2: Agregar tabla de embeddings
            if current_version < 2 {
                self.migrate_to_v2()?;
            }

            // Migración v2 -> v3: Agregar tabla de caché de queries
            if current_version < 3 {
                self.migrate_to_v3()?;
            }

            // Migración v3 -> v4: Agregar tabla de recordatorios
            if current_version < 4 {
                self.migrate_to_v4()?;
            }

            println!(
                "✅ Migraciones completadas - BD actualizada a v{}",
                Self::SCHEMA_VERSION
            );
        }

        Ok(())
    }

    /// Migración a versión 2: Agregar tabla de embeddings
    fn migrate_to_v2(&mut self) -> Result<()> {
        // Verificar si la tabla ya existe
        let table_exists: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='note_embeddings'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)?;

        if !table_exists {
            println!("Aplicando migración v2: Agregando tabla de embeddings");
        }

        self.conn.execute_batch(
            r#"
            -- Tabla de embeddings para búsqueda semántica
            CREATE TABLE IF NOT EXISTS note_embeddings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                note_path TEXT NOT NULL,
                chunk_index INTEGER NOT NULL,
                chunk_text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                token_count INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(note_path, chunk_index)
            );

            -- Índices para mejorar performance
            CREATE INDEX IF NOT EXISTS idx_embeddings_note ON note_embeddings(note_path);
            CREATE INDEX IF NOT EXISTS idx_embeddings_updated ON note_embeddings(updated_at DESC);
            "#,
        )?;

        // Actualizar versión usando REPLACE (elimina e inserta)
        self.conn
            .execute("REPLACE INTO schema_version (version) VALUES (2)", [])?;

        Ok(())
    }

    /// Migración a versión 3: Agregar tabla de caché de queries
    fn migrate_to_v3(&mut self) -> Result<()> {
        // Verificar si la tabla ya existe
        let table_exists: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='query_cache'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)?;

        if !table_exists {
            println!("Aplicando migración v3: Agregando tabla de caché de queries");
        }

        self.conn.execute_batch(
            r#"
            -- Tabla de caché para queries de búsqueda semántica
            CREATE TABLE IF NOT EXISTS query_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query_text TEXT NOT NULL,
                query_hash TEXT NOT NULL UNIQUE,
                embedding BLOB NOT NULL,
                hits INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                last_used_at INTEGER NOT NULL
            );

            -- Índice para búsquedas rápidas por hash
            CREATE INDEX IF NOT EXISTS idx_query_hash ON query_cache(query_hash);
            CREATE INDEX IF NOT EXISTS idx_query_last_used ON query_cache(last_used_at DESC);
            "#,
        )?;

        // Actualizar versión
        self.conn
            .execute("REPLACE INTO schema_version (version) VALUES (3)", [])?;

        Ok(())
    }

    /// Migración a versión 4: Agregar tabla de recordatorios
    fn migrate_to_v4(&mut self) -> Result<()> {
        // Verificar si la tabla ya existe
        let table_exists: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='reminders'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)?;

        if !table_exists {
            println!("Aplicando migración v4: Agregando tabla de recordatorios");
        }

        self.conn.execute_batch(
            r#"
            -- Tabla de recordatorios
            CREATE TABLE IF NOT EXISTS reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                note_id INTEGER,
                title TEXT NOT NULL,
                description TEXT,
                due_date INTEGER NOT NULL,
                priority INTEGER DEFAULT 1,
                status INTEGER DEFAULT 0,
                snooze_until INTEGER,
                repeat_pattern INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE SET NULL
            );

            CREATE INDEX IF NOT EXISTS idx_reminders_due_date ON reminders(due_date);
            CREATE INDEX IF NOT EXISTS idx_reminders_status ON reminders(status);
            CREATE INDEX IF NOT EXISTS idx_reminders_note_id ON reminders(note_id);
            "#,
        )?;

        // Actualizar versión
        self.conn
            .execute("REPLACE INTO schema_version (version) VALUES (4)", [])?;

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

        // Insertar o actualizar nota (manejar conflictos tanto en name como en path)
        self.conn.execute(
            r#"
            INSERT INTO notes (name, path, folder, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(path) DO UPDATE SET
                name = excluded.name,
                folder = excluded.folder,
                updated_at = excluded.updated_at
            "#,
            params![name, path, folder, now, now],
        )?;

        // Obtener el ID de la nota (puede ser nueva o existente)
        let note_id: i64 = self.conn.query_row(
            "SELECT id FROM notes WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )?;

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
        let note_data: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT id, path FROM notes WHERE name = ?1",
                params![name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if let Some((id, path)) = note_data {
            // Eliminar de tabla principal
            self.conn
                .execute("DELETE FROM notes WHERE id = ?1", params![id])?;

            // Eliminar de FTS
            self.conn
                .execute("DELETE FROM notes_fts WHERE rowid = ?1", params![id])?;

            // Eliminar embeddings asociados
            self.conn.execute(
                "DELETE FROM note_embeddings WHERE note_path = ?1",
                params![path],
            )?;

            println!("🗑️ Nota '{}' eliminada de BD (incluidos embeddings)", name);
        }

        Ok(())
    }

    /// Limpiar notas huérfanas (que están en BD pero no existen en el filesystem)
    pub fn cleanup_orphaned_notes(&self, existing_paths: &[String]) -> Result<usize> {
        // Obtener todas las notas en BD
        let all_notes: Vec<(String, String)> = self
            .conn
            .prepare("SELECT name, path FROM notes")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut deleted_count = 0;

        for (name, path) in all_notes {
            // Verificar si el path existe en la lista de archivos actuales
            if !existing_paths.contains(&path) {
                println!("🧹 Limpiando nota huérfana: '{}' (path: {})", name, path);
                self.delete_note(&name)?;
                deleted_count += 1;
            }
        }

        if deleted_count > 0 {
            println!(
                "✅ Limpieza completada: {} notas huérfanas eliminadas",
                deleted_count
            );
        }

        Ok(deleted_count)
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

    /// Obtener una nota por su ruta
    pub fn get_note_by_path(&self, path: &str) -> Result<Option<NoteMetadata>> {
        let result = self
            .conn
            .query_row(
                r#"
            SELECT id, name, path, folder, order_index, created_at, updated_at
            FROM notes WHERE path = ?1
            "#,
                params![path],
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
                    similarity: None,
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

        // Si la búsqueda empieza con #, buscar por tag exacto en lugar de contenido
        if query_text.trim().starts_with('#') {
            let tag_name = query_text.trim()[1..].trim().to_lowercase();

            if tag_name.is_empty() {
                return Ok(vec![]);
            }

            // Buscar notas que tengan exactamente este tag
            let mut stmt = self.conn.prepare(
                r#"
                SELECT DISTINCT
                    notes.id,
                    notes.name,
                    notes.path,
                    '' as snippet,
                    1.0 as relevance
                FROM notes
                JOIN note_tags ON notes.id = note_tags.note_id
                JOIN tags ON note_tags.tag_id = tags.id
                WHERE LOWER(tags.name) = ?1
                  AND (notes.folder IS NULL OR (
                      notes.folder NOT LIKE '.trash%' AND 
                      notes.folder NOT LIKE '.history%'
                  ))
                ORDER BY notes.name
                LIMIT 50
                "#,
            )?;

            let results = stmt
                .query_map([&tag_name], |row| {
                    Ok(SearchResult {
                        note_id: row.get(0)?,
                        note_name: row.get(1)?,
                        note_path: row.get(2)?,
                        snippet: row.get(3)?,
                        relevance: row.get::<_, f64>(4)? as f32,
                        matched_tags: vec![tag_name.clone()],
                        similarity: None,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            return Ok(results);
        }

        // Búsqueda normal por contenido usando FTS5
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
              AND (notes.folder IS NULL OR (
                  notes.folder NOT LIKE '.trash%' AND 
                  notes.folder NOT LIKE '.history%'
              ))
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
                    similarity: None,
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
            INSERT INTO chat_context_notes (session_id, note_id, added_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(session_id, note_id) DO UPDATE SET added_at = ?3
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

    // ============================================================================
    // EMBEDDING MANAGEMENT
    // ============================================================================

    /// Almacenar el embedding de un chunk de nota
    pub fn insert_embedding(
        &self,
        note_path: &str,
        chunk_index: usize,
        chunk_text: &str,
        embedding: &[f32],
        token_count: usize,
    ) -> Result<()> {
        let now = Utc::now().timestamp();

        // Convertir el vector de f32 a bytes
        let embedding_bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        self.conn.execute(
            r#"
            INSERT INTO note_embeddings (note_path, chunk_index, chunk_text, embedding, token_count, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(note_path, chunk_index) DO UPDATE SET
                chunk_text = excluded.chunk_text,
                embedding = excluded.embedding,
                token_count = excluded.token_count,
                updated_at = excluded.updated_at
            "#,
            params![note_path, chunk_index as i64, chunk_text, embedding_bytes, token_count as i64, now, now],
        )?;

        Ok(())
    }

    /// Obtener todos los embeddings de una nota específica
    pub fn get_embeddings_by_note(&self, note_path: &str) -> Result<Vec<NoteEmbedding>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, chunk_index, chunk_text, embedding FROM note_embeddings WHERE note_path = ?1 ORDER BY chunk_index"
        )?;

        let embeddings = stmt
            .query_map(params![note_path], |row| {
                let id: i64 = row.get(0)?;
                let chunk_index: i64 = row.get(1)?;
                let chunk_text: String = row.get(2)?;
                let embedding_bytes: Vec<u8> = row.get(3)?;

                // Convertir bytes a vector de f32
                let embedding: Vec<f32> = embedding_bytes
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();

                Ok(NoteEmbedding {
                    id,
                    chunk_index: chunk_index as usize,
                    chunk_text,
                    embedding,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(embeddings)
    }

    /// Obtener todos los embeddings de todas las notas (para búsqueda semántica)
    pub fn get_all_embeddings(&self) -> Result<Vec<GlobalEmbedding>> {
        let mut stmt = self.conn.prepare(
            "SELECT note_path, chunk_index, chunk_text, embedding FROM note_embeddings ORDER BY note_path, chunk_index"
        )?;

        let embeddings = stmt
            .query_map([], |row| {
                let note_path: String = row.get(0)?;
                let chunk_index: i64 = row.get(1)?;
                let chunk_text: String = row.get(2)?;
                let embedding_bytes: Vec<u8> = row.get(3)?;

                // Convertir bytes a vector de f32
                let embedding: Vec<f32> = embedding_bytes
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();

                Ok(GlobalEmbedding {
                    note_path,
                    chunk_index: chunk_index as usize,
                    chunk_text,
                    embedding,
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(embeddings)
    }

    /// Eliminar todos los embeddings de una nota
    pub fn delete_embeddings_by_note(&self, note_path: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM note_embeddings WHERE note_path = ?1",
            params![note_path],
        )?;
        Ok(())
    }

    /// Obtener la fecha de actualización del embedding de una nota
    pub fn get_embedding_timestamp(&self, note_path: &str) -> Result<Option<DateTime<Utc>>> {
        let result = self.conn.query_row(
            "SELECT MAX(updated_at) FROM note_embeddings WHERE note_path = ?1",
            params![note_path],
            |row| row.get::<_, Option<i64>>(0),
        );

        match result {
            Ok(Some(ts)) => Ok(Some(DateTime::from_timestamp(ts, 0).unwrap())),
            Ok(None) => Ok(None),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Contar el número total de embeddings almacenados
    pub fn count_embeddings(&self) -> Result<usize> {
        let count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM note_embeddings", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Contar el número de notas con embeddings
    pub fn count_notes_with_embeddings(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT note_path) FROM note_embeddings",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Obtener estadísticas de embeddings
    pub fn get_embedding_stats(&self) -> Result<(usize, usize, usize)> {
        let total_notes = self.count_notes_with_embeddings()?;
        let total_chunks = self.count_embeddings()?;
        let total_tokens: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(token_count), 0) FROM note_embeddings",
            [],
            |row| row.get(0),
        )?;

        Ok((total_notes, total_chunks, total_tokens as usize))
    }

    // ===== Query Cache Methods =====

    /// Obtener embedding de query desde caché
    pub fn get_cached_query_embedding(&self, query_text: &str) -> Result<Option<Vec<f32>>> {
        use sha2::{Digest, Sha256};

        // Calcular hash de la query
        let mut hasher = Sha256::new();
        hasher.update(query_text.as_bytes());
        let query_hash = format!("{:x}", hasher.finalize());

        // Buscar en caché
        let result: Option<(Vec<u8>, i64)> = self
            .conn
            .query_row(
                "SELECT embedding, id FROM query_cache WHERE query_hash = ?1",
                params![query_hash],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if let Some((embedding_blob, cache_id)) = result {
            // Incrementar contador de hits
            let now = Utc::now().timestamp();
            self.conn.execute(
                "UPDATE query_cache SET hits = hits + 1, last_used_at = ?1 WHERE id = ?2",
                params![now, cache_id],
            )?;

            // Deserializar embedding
            let embedding: Vec<f32> = bincode::deserialize(&embedding_blob)
                .map_err(|e| DatabaseError::Sqlite(rusqlite::Error::InvalidQuery))?;

            Ok(Some(embedding))
        } else {
            Ok(None)
        }
    }

    /// Guardar embedding de query en caché
    pub fn cache_query_embedding(&self, query_text: &str, embedding: &[f32]) -> Result<()> {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(query_text.as_bytes());
        let query_hash = format!("{:x}", hasher.finalize());

        // Serializar embedding
        let embedding_blob = bincode::serialize(embedding)
            .map_err(|e| DatabaseError::Sqlite(rusqlite::Error::InvalidQuery))?;

        let now = Utc::now().timestamp();

        // Insertar o actualizar caché
        self.conn.execute(
            r#"
            INSERT INTO query_cache (query_text, query_hash, embedding, hits, created_at, last_used_at)
            VALUES (?1, ?2, ?3, 1, ?4, ?5)
            ON CONFLICT(query_hash) DO UPDATE SET
                hits = hits + 1,
                last_used_at = excluded.last_used_at
            "#,
            params![query_text, query_hash, embedding_blob, now, now],
        )?;

        Ok(())
    }

    /// Limpiar queries antiguas del caché
    pub fn clean_old_cache(&self, days: i64) -> Result<usize> {
        let cutoff = Utc::now().timestamp() - (days * 24 * 60 * 60);

        let deleted = self.conn.execute(
            "DELETE FROM query_cache WHERE last_used_at < ?1",
            params![cutoff],
        )?;

        Ok(deleted)
    }

    /// Obtener estadísticas del caché
    pub fn get_cache_stats(&self) -> Result<(usize, usize, f64)> {
        let total_queries: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM query_cache", [], |row| row.get(0))?;

        let total_hits: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(hits), 0) FROM query_cache",
            [],
            |row| row.get(0),
        )?;

        let hit_rate = if total_queries > 0 {
            total_hits as f64 / total_queries as f64
        } else {
            0.0
        };

        Ok((total_queries as usize, total_hits as usize, hit_rate))
    }

    /// Obtener expansión de query desde caché
    pub fn get_cached_query_expansion(&self, query_text: &str) -> Result<Option<String>> {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(format!("expansion:{}", query_text).as_bytes());
        let query_hash = format!("{:x}", hasher.finalize());

        let result: Option<String> = self
            .conn
            .query_row(
                "SELECT query_text FROM query_cache WHERE query_hash = ?1 AND embedding IS NULL",
                params![query_hash],
                |row| row.get(0),
            )
            .optional()?;

        if result.is_some() {
            // Actualizar last_used
            let now = Utc::now().timestamp();
            self.conn.execute(
                "UPDATE query_cache SET hits = hits + 1, last_used_at = ?1 WHERE query_hash = ?2",
                params![now, query_hash],
            )?;
        }

        Ok(result)
    }

    /// Guardar expansión de query en caché
    pub fn cache_query_expansion(&self, original_query: &str, expanded_query: &str) -> Result<()> {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(format!("expansion:{}", original_query).as_bytes());
        let query_hash = format!("{:x}", hasher.finalize());

        let now = Utc::now().timestamp();

        // Guardar la expansión en el campo query_text, embedding NULL
        self.conn.execute(
            r#"
            INSERT INTO query_cache (query_text, query_hash, embedding, hits, created_at, last_used_at)
            VALUES (?1, ?2, NULL, 1, ?3, ?4)
            ON CONFLICT(query_hash) DO UPDATE SET
                hits = hits + 1,
                last_used_at = excluded.last_used_at
            "#,
            params![expanded_query, query_hash, now, now],
        )?;

        Ok(())
    }

    /// Eliminar todas las notas de una carpeta de la BD
    pub fn delete_notes_in_folder(&self, folder_path: &str) -> Result<usize> {
        // Obtener todas las notas en esa carpeta
        let notes: Vec<(i64, String, String)> = self
            .conn
            .prepare("SELECT id, name, path FROM notes WHERE folder = ?1 OR folder LIKE ?2")?
            .query_map(params![folder_path, format!("{}/%", folder_path)], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        let count = notes.len();

        for (id, name, path) in notes {
            // Eliminar de tabla principal
            self.conn
                .execute("DELETE FROM notes WHERE id = ?1", params![id])?;

            // Eliminar de FTS
            self.conn
                .execute("DELETE FROM notes_fts WHERE rowid = ?1", params![id])?;

            // Eliminar embeddings asociados
            self.conn.execute(
                "DELETE FROM note_embeddings WHERE note_path = ?1",
                params![path],
            )?;

            println!(
                "🗑️ Nota '{}' eliminada de BD (carpeta: {})",
                name, folder_path
            );
        }

        if count > 0 {
            println!(
                "🗑️ Total de {} notas eliminadas de la carpeta '{}'",
                count, folder_path
            );
        }

        Ok(count)
    }

    /// Actualizar el campo folder de todas las notas en una carpeta (para rename/move)
    /// Actualizar el campo folder y path de todas las notas en una carpeta (para rename/move)
    pub fn update_notes_folder(
        &self,
        old_folder: &str,
        new_folder: &str,
        root_path: &str,
    ) -> Result<usize> {
        let old_prefix_path = Path::new(root_path)
            .join(old_folder)
            .to_string_lossy()
            .to_string();
        let new_prefix_path = Path::new(root_path)
            .join(new_folder)
            .to_string_lossy()
            .to_string();

        // Asegurar que terminen en separador para evitar reemplazos parciales incorrectos
        let old_prefix_str = if old_prefix_path.ends_with(std::path::MAIN_SEPARATOR) {
            old_prefix_path
        } else {
            format!("{}{}", old_prefix_path, std::path::MAIN_SEPARATOR)
        };

        let new_prefix_str = if new_prefix_path.ends_with(std::path::MAIN_SEPARATOR) {
            new_prefix_path
        } else {
            format!("{}{}", new_prefix_path, std::path::MAIN_SEPARATOR)
        };

        // 1. Actualizar notas directamente en la carpeta
        // Necesitamos actualizar folder Y path
        // SQLite no tiene replace() en todas las versiones, pero rusqlite suele incluirlo.
        // Sin embargo, para mayor seguridad, iteramos y actualizamos.

        let mut total_updated = 0;

        // Buscar todas las notas afectadas (directas y en subcarpetas)
        let pattern = format!("{}%", old_folder); // folder LIKE 'old%'

        let notes_to_update: Vec<(i64, String, String)> = self
            .conn
            .prepare("SELECT id, folder, path FROM notes WHERE folder = ?1 OR folder LIKE ?2")?
            .query_map(params![old_folder, format!("{}/%", old_folder)], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        for (id, current_folder, current_path) in notes_to_update {
            // Calcular nuevo folder
            let new_note_folder = if current_folder == old_folder {
                new_folder.to_string()
            } else if let Some(suffix) = current_folder.strip_prefix(old_folder) {
                // Es una subcarpeta: old/sub -> new/sub
                // suffix empieza con /, ej: "/sub"
                format!("{}{}", new_folder, suffix)
            } else {
                continue; // No debería pasar por el WHERE
            };

            // Calcular nuevo path
            // Reemplazar el prefijo del path absoluto
            let new_note_path = if current_path.starts_with(&old_prefix_str) {
                current_path.replace(&old_prefix_str, &new_prefix_str)
            } else {
                // Fallback si el path no coincide exactamente (raro)
                // Intentar construirlo desde el root
                let filename = Path::new(&current_path).file_name().unwrap_or_default();
                Path::new(root_path)
                    .join(&new_note_folder)
                    .join(filename)
                    .to_string_lossy()
                    .to_string()
            };

            // Actualizar notes
            self.conn.execute(
                "UPDATE notes SET folder = ?1, path = ?2, updated_at = ?3 WHERE id = ?4",
                params![new_note_folder, new_note_path, Utc::now().timestamp(), id],
            )?;

            // Actualizar embeddings
            self.conn.execute(
                "UPDATE note_embeddings SET note_path = ?1 WHERE note_path = ?2",
                params![new_note_path, current_path],
            )?;

            total_updated += 1;
        }

        if total_updated > 0 {
            println!(
                "📝 {} notas actualizadas: carpeta '{}' → '{}' (incluidos paths y embeddings)",
                total_updated, old_folder, new_folder
            );
        }

        Ok(total_updated)
    }

    /// Renombrar una nota actualizando todas las tablas (notes, fts, embeddings)
    pub fn rename_note(
        &self,
        old_name: &str,
        new_name: &str,
        new_path: &str,
        new_folder: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().timestamp();

        // Obtener datos de la nota original
        let note_data: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT id, path FROM notes WHERE name = ?1",
                params![old_name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if let Some((id, old_path)) = note_data {
            // 1. Actualizar tabla principal
            self.conn.execute(
                "UPDATE notes SET name = ?1, path = ?2, folder = ?3, updated_at = ?4 WHERE id = ?5",
                params![new_name, new_path, new_folder, now, id],
            )?;

            // 2. Actualizar FTS (solo el nombre, el contenido se actualiza por separado si cambió)
            self.conn.execute(
                "UPDATE notes_fts SET name = ?1 WHERE rowid = ?2",
                params![new_name, id],
            )?;

            // 3. Actualizar paths en embeddings
            self.conn.execute(
                "UPDATE note_embeddings SET note_path = ?1 WHERE note_path = ?2",
                params![new_path, old_path],
            )?;

            println!(
                "📝 Nota renombrada: '{}' -> '{}' (incluidos embeddings)",
                old_name, new_name
            );
        } else {
            return Err(DatabaseError::NoteNotFound(old_name.to_string()));
        }

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

    #[test]
    fn test_embeddings() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_notes_embeddings.db");

        let db = NotesDatabase::new(&db_path).unwrap();

        // Crear un embedding de prueba (dimensión 1024 como qwen)
        let embedding: Vec<f32> = (0..1024).map(|i| (i as f32) / 1024.0).collect();

        // Insertar embedding
        db.insert_embedding(
            "/path/to/note.md",
            0,
            "Este es el primer chunk de texto",
            &embedding,
            50,
        )
        .unwrap();

        // Insertar segundo chunk
        let embedding2: Vec<f32> = (0..1024).map(|i| ((i + 512) as f32) / 1024.0).collect();
        db.insert_embedding(
            "/path/to/note.md",
            1,
            "Este es el segundo chunk de texto",
            &embedding2,
            45,
        )
        .unwrap();

        // Verificar que se insertaron correctamente
        let embeddings = db.get_embeddings_by_note("/path/to/note.md").unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].chunk_index, 0); // chunk_index
        assert_eq!(embeddings[0].chunk_text, "Este es el primer chunk de texto");
        assert_eq!(embeddings[0].embedding.len(), 1024); // dimensión del embedding
        assert_eq!(embeddings[1].chunk_index, 1);

        // Verificar timestamp
        let timestamp = db.get_embedding_timestamp("/path/to/note.md").unwrap();
        assert!(timestamp.is_some());

        // Verificar estadísticas
        let (notes_count, chunks_count, tokens_count) = db.get_embedding_stats().unwrap();
        assert_eq!(notes_count, 1);
        assert_eq!(chunks_count, 2);
        assert_eq!(tokens_count, 95); // 50 + 45

        // Obtener todos los embeddings
        let all_embeddings = db.get_all_embeddings().unwrap();
        assert_eq!(all_embeddings.len(), 2);

        // Eliminar embeddings
        db.delete_embeddings_by_note("/path/to/note.md").unwrap();
        let embeddings_after = db.get_embeddings_by_note("/path/to/note.md").unwrap();
        assert_eq!(embeddings_after.len(), 0);

        // Cleanup
        std::fs::remove_file(db_path).ok();
    }

    #[test]
    fn test_embedding_update() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_notes_embeddings_update.db");

        let db = NotesDatabase::new(&db_path).unwrap();

        let embedding1: Vec<f32> = (0..1024).map(|_| 0.5).collect();
        let embedding2: Vec<f32> = (0..1024).map(|_| 0.9).collect();

        // Insertar embedding inicial
        db.insert_embedding("/path/to/note.md", 0, "Texto original", &embedding1, 30)
            .unwrap();

        // Actualizar el mismo chunk (misma nota y chunk_index)
        db.insert_embedding("/path/to/note.md", 0, "Texto actualizado", &embedding2, 35)
            .unwrap();

        // Verificar que se actualizó y no se duplicó
        let embeddings = db.get_embeddings_by_note("/path/to/note.md").unwrap();
        assert_eq!(embeddings.len(), 1);
        assert_eq!(embeddings[0].chunk_text, "Texto actualizado");
        assert_eq!(embeddings[0].embedding[0], 0.9); // Verificar que el embedding cambió

        // Cleanup
        std::fs::remove_file(db_path).ok();
    }
}
