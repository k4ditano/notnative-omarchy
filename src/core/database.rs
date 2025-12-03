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
    pub icon: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Metadata de una carpeta almacenada en la base de datos
#[derive(Debug, Clone)]
pub struct FolderMetadata {
    pub id: i64,
    pub path: String,
    pub icon: Option<String>,
    pub color: Option<String>,
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

/// Fila de propiedad inline de la base de datos
#[derive(Debug, Clone)]
pub struct InlinePropertyRow {
    pub id: i64,
    pub key: String,
    pub property_type: String,
    pub value_text: Option<String>,
    pub value_number: Option<f64>,
    pub value_bool: Option<i64>,
    pub line_number: i64,
    pub char_start: i64,
    pub char_end: i64,
    pub linked_note_id: Option<i64>,
    /// ID de grupo para propiedades agrupadas [a::1, b::2]
    /// NULL = propiedad individual, número = grupo de propiedades relacionadas
    pub group_id: Option<i64>,
}

impl InlinePropertyRow {
    /// Convertir a PropertyValue
    pub fn to_property_value(&self) -> super::property::PropertyValue {
        use super::property::PropertyValue;

        match self.property_type.as_str() {
            "text" => PropertyValue::Text(self.value_text.clone().unwrap_or_default()),
            "number" => PropertyValue::Number(self.value_number.unwrap_or(0.0)),
            "checkbox" => PropertyValue::Checkbox(self.value_bool.unwrap_or(0) != 0),
            "date" => PropertyValue::Date(self.value_text.clone().unwrap_or_default()),
            "datetime" => PropertyValue::DateTime(self.value_text.clone().unwrap_or_default()),
            "list" => PropertyValue::List(
                self.value_text.clone().unwrap_or_default()
                    .split(',')
                    .map(|s| s.to_string())
                    .collect()
            ),
            "tags" => PropertyValue::Tags(
                self.value_text.clone().unwrap_or_default()
                    .split(',')
                    .map(|s| s.to_string())
                    .collect()
            ),
            "links" => PropertyValue::Links(
                self.value_text.clone().unwrap_or_default()
                    .split(',')
                    .map(|s| s.to_string())
                    .collect()
            ),
            "link" => PropertyValue::Link(self.value_text.clone().unwrap_or_default()),
            _ => PropertyValue::Null,
        }
    }
}

/// Un registro agrupado de propiedades [campo1::val1, campo2::val2]
#[derive(Debug, Clone)]
pub struct GroupedRecord {
    /// ID de la nota que contiene el registro
    pub note_id: i64,
    /// Nombre de la nota
    pub note_name: String,
    /// ID del grupo dentro de la nota
    pub group_id: i64,
    /// Propiedades del registro como pares (clave, valor)
    pub properties: Vec<(String, String)>,
}

impl GroupedRecord {
    /// Obtener el valor de una propiedad por clave
    pub fn get(&self, key: &str) -> Option<&str> {
        self.properties.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
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
    const SCHEMA_VERSION: i32 = 10;

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

        // Inicializar esquema completo (antes de migraciones para que existan las tablas base)
        db.initialize_schema()?;

        // Verificar versión actual y migrar si es necesario
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

    /// Obtiene el path de la base de datos
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Inicia una transacción para operaciones batch (mejora rendimiento)
    pub fn begin_transaction(&self) -> Result<()> {
        self.conn.execute("BEGIN TRANSACTION", [])?;
        Ok(())
    }

    /// Confirma una transacción batch
    pub fn commit_transaction(&self) -> Result<()> {
        self.conn.execute("COMMIT", [])?;
        Ok(())
    }

    /// Revierte una transacción batch
    pub fn rollback_transaction(&self) -> Result<()> {
        self.conn.execute("ROLLBACK", [])?;
        Ok(())
    }

    /// Verifica si una nota necesita re-indexarse basándose en el timestamp del archivo
    /// Retorna true si el archivo fue modificado después del último indexado
    pub fn needs_reindex(&self, path: &str, file_mtime: i64) -> Result<bool> {
        let db_mtime: Option<i64> = self.conn.query_row(
            "SELECT updated_at FROM notes WHERE path = ?1",
            params![path],
            |row| row.get(0),
        ).optional()?;

        match db_mtime {
            Some(mtime) => Ok(file_mtime > mtime),
            None => Ok(true), // Nota no existe en DB, necesita indexarse
        }
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

            -- Tabla virtual para full-text search (unicode61 sin porter para búsqueda por prefijo exacta)
            CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
                name,
                content,
                tokenize = 'unicode61'
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

            // Migración v4 -> v5: Recrear tabla FTS sin Porter (mejor búsqueda por prefijo)
            if current_version < 5 {
                self.migrate_to_v5()?;
            }

            // Migración v5 -> v6: Agregar iconos a notas y tabla de carpetas
            if current_version < 6 {
                self.migrate_to_v6()?;
            }

            // Migración v6 -> v7: Agregar icon_color a notas y carpetas
            if current_version < 7 {
                self.migrate_to_v7()?;
            }

            // Migración v7 -> v8: Agregar tablas para Bases (vistas de base de datos)
            if current_version < 8 {
                self.migrate_to_v8()?;
            }

            // Migración v8 -> v9: Nuevo sistema de propiedades inline
            if current_version < 9 {
                self.migrate_to_v9()?;
            }

            // Migración v9 -> v10: Propiedades agrupadas con group_id
            if current_version < 10 {
                self.migrate_to_v10()?;
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

    /// Migración a versión 5: Recrear tabla FTS sin tokenizer Porter
    /// El tokenizer Porter causa problemas con búsqueda por prefijo (ej: "key" no encuentra "keybindings")
    fn migrate_to_v5(&mut self) -> Result<()> {
        println!(
            "Aplicando migración v5: Recreando tabla FTS sin Porter para mejor búsqueda por prefijo"
        );

        // Verificar si la tabla notes_fts existe
        let fts_exists: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='notes_fts'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)?;

        // 1. Obtener todos los datos actuales de la tabla FTS (solo si existe)
        let notes_data: Vec<(i64, String, String)> = if fts_exists {
            let mut stmt = self
                .conn
                .prepare("SELECT rowid, name, content FROM notes_fts")?;
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            Vec::new()
        };

        println!("  📦 Respaldando {} entradas de FTS", notes_data.len());

        // 2. Eliminar la tabla FTS antigua (si existe)
        self.conn.execute("DROP TABLE IF EXISTS notes_fts", [])?;

        // 3. Crear la nueva tabla FTS con unicode61 (sin Porter)
        self.conn.execute_batch(
            r#"
            CREATE VIRTUAL TABLE notes_fts USING fts5(
                name,
                content,
                tokenize = 'unicode61'
            );
            "#,
        )?;

        // 4. Reinsertar los datos
        for (rowid, name, content) in &notes_data {
            self.conn.execute(
                "INSERT INTO notes_fts (rowid, name, content) VALUES (?1, ?2, ?3)",
                params![rowid, name, content],
            )?;
        }

        println!("  ✅ Tabla FTS recreada con {} entradas", notes_data.len());

        // Actualizar versión
        self.conn
            .execute("REPLACE INTO schema_version (version) VALUES (5)", [])?;

        Ok(())
    }

    /// Migración a versión 6: Agregar iconos a notas y tabla de carpetas
    fn migrate_to_v6(&mut self) -> Result<()> {
        println!("Aplicando migración v6: Agregando soporte de iconos personalizados");

        // Verificar si la tabla notes existe
        let notes_exists: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='notes'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)?;

        // 1. Agregar columna icon a la tabla notes (si existe y no tiene la columna)
        if notes_exists {
            let has_icon_column: bool = self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name='icon'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map(|count| count > 0)?;

            if !has_icon_column {
                self.conn
                    .execute("ALTER TABLE notes ADD COLUMN icon TEXT", [])?;
                println!("  📦 Columna 'icon' agregada a tabla notes");
            }
        }

        // 2. Crear tabla de carpetas para iconos y metadatos
        self.conn.execute_batch(
            r#"
            -- Tabla de carpetas con iconos personalizados
            CREATE TABLE IF NOT EXISTS folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                icon TEXT,
                color TEXT,
                order_index INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            -- Índices para carpetas
            CREATE INDEX IF NOT EXISTS idx_folders_path ON folders(path);
            CREATE INDEX IF NOT EXISTS idx_folders_order ON folders(order_index);
            "#,
        )?;
        println!("  📁 Tabla 'folders' creada");

        // Actualizar versión
        self.conn
            .execute("REPLACE INTO schema_version (version) VALUES (6)", [])?;

        Ok(())
    }

    /// Migración a versión 7: Agregar icon_color a notas y carpetas
    fn migrate_to_v7(&mut self) -> Result<()> {
        println!("Aplicando migración v7: Agregando soporte de color para iconos");

        // Verificar si la tabla notes existe
        let notes_exists: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='notes'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)?;

        // Verificar si la tabla folders existe
        let folders_exists: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='folders'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count > 0)?;

        // 1. Agregar columna icon_color a la tabla notes (si existe y no tiene la columna)
        if notes_exists {
            let has_icon_color_in_notes: bool = self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name='icon_color'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map(|count| count > 0)?;

            if !has_icon_color_in_notes {
                self.conn
                    .execute("ALTER TABLE notes ADD COLUMN icon_color TEXT", [])?;
                println!("  🎨 Columna 'icon_color' agregada a tabla notes");
            }
        }

        // 2. Agregar columna icon_color a la tabla folders (si existe y no tiene la columna)
        if folders_exists {
            let has_icon_color_in_folders: bool = self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('folders') WHERE name='icon_color'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map(|count| count > 0)?;

            if !has_icon_color_in_folders {
                self.conn
                    .execute("ALTER TABLE folders ADD COLUMN icon_color TEXT", [])?;
                println!("  🎨 Columna 'icon_color' agregada a tabla folders");
            }
        }

        // Actualizar versión
        self.conn
            .execute("REPLACE INTO schema_version (version) VALUES (7)", [])?;

        Ok(())
    }

    /// Migración a versión 8: Agregar tablas para Bases (vistas tipo Obsidian)
    fn migrate_to_v8(&mut self) -> Result<()> {
        println!("Aplicando migración v8: Agregando tablas para Bases");

        self.conn.execute_batch(
            r#"
            -- Tabla de propiedades indexadas de notas
            -- Permite queries rápidas sobre propiedades del frontmatter
            CREATE TABLE IF NOT EXISTS note_properties (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                note_id INTEGER NOT NULL,
                property_key TEXT NOT NULL,
                property_type TEXT NOT NULL,
                value_text TEXT,
                value_number REAL,
                value_bool INTEGER,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
                UNIQUE(note_id, property_key)
            );

            -- Índices para búsquedas rápidas de propiedades
            CREATE INDEX IF NOT EXISTS idx_note_props_note ON note_properties(note_id);
            CREATE INDEX IF NOT EXISTS idx_note_props_key ON note_properties(property_key);
            CREATE INDEX IF NOT EXISTS idx_note_props_type ON note_properties(property_type);
            CREATE INDEX IF NOT EXISTS idx_note_props_value_text ON note_properties(value_text);
            CREATE INDEX IF NOT EXISTS idx_note_props_value_number ON note_properties(value_number);

            -- Tabla de Bases (colecciones de vistas sobre notas)
            CREATE TABLE IF NOT EXISTS bases (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                source_folder TEXT,
                config_yaml TEXT NOT NULL,
                active_view INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            -- Índices para bases
            CREATE INDEX IF NOT EXISTS idx_bases_name ON bases(name);
            CREATE INDEX IF NOT EXISTS idx_bases_folder ON bases(source_folder);
            "#,
        )?;

        println!("  📊 Tablas 'note_properties' y 'bases' creadas");

        // Actualizar versión
        self.conn
            .execute("REPLACE INTO schema_version (version) VALUES (8)", [])?;

        Ok(())
    }

    /// Migración a versión 9: Nuevo sistema de propiedades inline [campo::valor]
    fn migrate_to_v9(&mut self) -> Result<()> {
        println!("Aplicando migración v9: Sistema de propiedades inline");

        // Eliminar la tabla vieja note_properties (basada en frontmatter, nunca usada)
        self.conn.execute("DROP TABLE IF EXISTS note_properties", [])?;

        self.conn.execute_batch(
            r#"
            -- Nueva tabla para propiedades inline [campo::valor]
            -- Permite múltiples propiedades con la misma clave en una nota
            CREATE TABLE IF NOT EXISTS inline_properties (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                note_id INTEGER NOT NULL,
                property_key TEXT NOT NULL,
                property_type TEXT NOT NULL,
                value_text TEXT,
                value_number REAL,
                value_bool INTEGER,
                line_number INTEGER NOT NULL,
                char_start INTEGER NOT NULL,
                char_end INTEGER NOT NULL,
                linked_note_id INTEGER,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
                FOREIGN KEY (linked_note_id) REFERENCES notes(id) ON DELETE SET NULL
            );

            -- Índices para búsquedas rápidas
            CREATE INDEX IF NOT EXISTS idx_inline_props_note ON inline_properties(note_id);
            CREATE INDEX IF NOT EXISTS idx_inline_props_key ON inline_properties(property_key);
            CREATE INDEX IF NOT EXISTS idx_inline_props_type ON inline_properties(property_type);
            CREATE INDEX IF NOT EXISTS idx_inline_props_value_text ON inline_properties(value_text);
            CREATE INDEX IF NOT EXISTS idx_inline_props_value_number ON inline_properties(value_number);
            CREATE INDEX IF NOT EXISTS idx_inline_props_linked ON inline_properties(linked_note_id);
            "#,
        )?;

        println!("  🏷️ Tabla 'inline_properties' creada (reemplaza note_properties)");

        // Actualizar versión
        self.conn
            .execute("REPLACE INTO schema_version (version) VALUES (9)", [])?;

        Ok(())
    }

    /// Migración a versión 10: Propiedades agrupadas [campo1::val1, campo2::val2]
    fn migrate_to_v10(&mut self) -> Result<()> {
        println!("Aplicando migración v10: Propiedades agrupadas con group_id");

        // Verificar si la columna group_id ya existe
        let column_exists: bool = self.conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('inline_properties') WHERE name = 'group_id'",
            [],
            |row| row.get(0),
        ).unwrap_or(false);

        if !column_exists {
            // Añadir columna group_id para agrupar propiedades relacionadas
            // NULL significa propiedad individual, un número agrupa propiedades del mismo "registro"
            self.conn.execute(
                "ALTER TABLE inline_properties ADD COLUMN group_id INTEGER",
                [],
            )?;
            println!("  🔗 Columna 'group_id' añadida a inline_properties");
        } else {
            println!("  ℹ️ Columna 'group_id' ya existe, saltando...");
        }

        // Índice para consultas por grupo (IF NOT EXISTS es seguro)
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_inline_props_group ON inline_properties(note_id, group_id)",
            [],
        )?;

        // Actualizar versión
        self.conn
            .execute("REPLACE INTO schema_version (version) VALUES (10)", [])?;

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

        // Sincronizar propiedades inline del contenido
        self.sync_inline_properties(note_id, content)?;

        // Sincronizar tags del contenido (frontmatter + inline #tags)
        self.sync_note_tags(note_id, content)?;

        Ok(note_id)
    }

    /// Sincronizar tags de una nota (elimina antiguos y añade nuevos)
    fn sync_note_tags(&self, note_id: i64, content: &str) -> Result<()> {
        use super::frontmatter::extract_all_tags;
        
        // Extraer tags del contenido (frontmatter + inline)
        let tags = extract_all_tags(content);
        
        // Obtener tags actuales de la nota
        let current_tags: Vec<String> = self.get_note_tags(note_id)?
            .into_iter()
            .map(|t| t.name)
            .collect();
        
        // Tags a eliminar (están en current pero no en tags)
        for tag in &current_tags {
            if !tags.iter().any(|t| t.to_lowercase() == tag.to_lowercase()) {
                let _ = self.remove_tag(note_id, tag);
            }
        }
        
        // Tags a añadir (están en tags pero no en current)
        for tag in &tags {
            if !current_tags.iter().any(|t| t.to_lowercase() == tag.to_lowercase()) {
                let _ = self.add_tag(note_id, tag);
            }
        }
        
        Ok(())
    }

    /// Sincronizar propiedades inline [campo::valor] de una nota
    pub fn sync_inline_properties(&self, note_id: i64, content: &str) -> Result<()> {
        use super::inline_property::InlinePropertyParser;
        use super::property::PropertyValue;

        let now = Utc::now().timestamp();

        // Parsear propiedades del contenido
        let properties = InlinePropertyParser::parse(content);

        // Eliminar propiedades anteriores de esta nota
        self.conn.execute(
            "DELETE FROM inline_properties WHERE note_id = ?1",
            params![note_id],
        )?;

        // Insertar nuevas propiedades
        for prop in properties {
            let (value_text, value_number, value_bool) = match &prop.value {
                PropertyValue::Text(s) => (Some(s.clone()), None, None),
                PropertyValue::Number(n) => (None, Some(*n), None),
                PropertyValue::Checkbox(b) => (None, None, Some(*b as i64)),
                PropertyValue::Date(d) => (Some(d.clone()), None, None),
                PropertyValue::DateTime(dt) => (Some(dt.clone()), None, None),
                PropertyValue::List(items) => (Some(items.join(",")), None, None),
                PropertyValue::Tags(tags) => (Some(tags.join(",")), None, None),
                PropertyValue::Links(links) => (Some(links.join(",")), None, None),
                PropertyValue::Link(note) => (Some(note.clone()), None, None),
                PropertyValue::Null => (None, None, None),
            };

            // Resolver linked_note_id si es un Link
            let linked_note_id: Option<i64> = if let Some(ref note_name) = prop.linked_note {
                self.conn
                    .query_row(
                        "SELECT id FROM notes WHERE name = ?1",
                        params![note_name],
                        |row| row.get(0),
                    )
                    .optional()?
            } else {
                None
            };

            self.conn.execute(
                r#"
                INSERT INTO inline_properties 
                    (note_id, property_key, property_type, value_text, value_number, value_bool,
                     line_number, char_start, char_end, linked_note_id, group_id, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                "#,
                params![
                    note_id,
                    prop.key,
                    prop.value.type_name(),
                    value_text,
                    value_number,
                    value_bool,
                    prop.line_number as i64,
                    prop.char_start as i64,
                    prop.char_end as i64,
                    linked_note_id,
                    prop.group_id.map(|g| g as i64),
                    now,
                    now,
                ],
            )?;
        }

        Ok(())
    }

    /// Obtener todas las propiedades inline de una nota
    pub fn get_inline_properties(&self, note_id: i64) -> Result<Vec<InlinePropertyRow>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, property_key, property_type, value_text, value_number, value_bool,
                   line_number, char_start, char_end, linked_note_id, group_id
            FROM inline_properties
            WHERE note_id = ?1
            ORDER BY line_number, char_start
            "#,
        )?;

        let rows = stmt.query_map(params![note_id], |row| {
            Ok(InlinePropertyRow {
                id: row.get(0)?,
                key: row.get(1)?,
                property_type: row.get(2)?,
                value_text: row.get(3)?,
                value_number: row.get(4)?,
                value_bool: row.get(5)?,
                line_number: row.get(6)?,
                char_start: row.get(7)?,
                char_end: row.get(8)?,
                linked_note_id: row.get(9)?,
                group_id: row.get(10)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// Obtener notas que tienen una propiedad específica
    pub fn get_notes_with_property(&self, property_key: &str) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT n.id, n.name
            FROM notes n
            INNER JOIN inline_properties ip ON n.id = ip.note_id
            WHERE ip.property_key = ?1
            ORDER BY n.name
            "#,
        )?;

        let rows = stmt.query_map(params![property_key], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// Obtener todos los nombres de propiedades únicos
    pub fn get_all_property_keys(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT property_key FROM inline_properties ORDER BY property_key",
        )?;

        let rows = stmt.query_map([], |row| row.get(0))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// Obtener notas que enlazan a una nota específica
    pub fn get_notes_linking_to(&self, note_id: i64) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT n.id, n.name
            FROM notes n
            INNER JOIN inline_properties ip ON n.id = ip.note_id
            WHERE ip.linked_note_id = ?1
            ORDER BY n.name
            "#,
        )?;

        let rows = stmt.query_map(params![note_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// Obtener valores distintos de una propiedad (para autocompletado)
    pub fn get_distinct_values(&self, property_key: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT value_text 
            FROM inline_properties 
            WHERE property_key = ?1 AND value_text IS NOT NULL
            ORDER BY value_text
            "#,
        )?;

        let rows = stmt.query_map(params![property_key], |row| row.get(0))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// Obtener la estructura completa de un registro dado un valor de propiedad.
    /// Usado para autocompletado: cuando el usuario selecciona "Foreger" para "juego",
    /// devuelve todas las propiedades que normalmente acompañan a ese registro.
    /// Ejemplo: get_complete_record_structure("juego", "Foreger") 
    ///          -> Some(vec![("juego", "Foreger"), ("comprado", ""), ("horas", "")])
    pub fn get_complete_record_structure(&self, property_key: &str, property_value: &str) -> Result<Option<Vec<(String, String)>>> {
        // Buscar un registro que tenga este key::value y obtener su estructura completa
        let mut stmt = self.conn.prepare(
            r#"
            SELECT ip2.property_key, ip2.value_text
            FROM inline_properties ip1
            JOIN inline_properties ip2 
              ON ip1.note_id = ip2.note_id 
              AND ip1.group_id = ip2.group_id
            JOIN notes n ON ip1.note_id = n.id
            WHERE ip1.property_key = ?1
              AND ip1.value_text = ?2
              AND ip1.group_id IS NOT NULL
              AND n.name NOT LIKE '.history/%' 
              AND n.name NOT LIKE '.trash/%'
              AND n.path NOT LIKE '%/.history/%'
              AND n.path NOT LIKE '%/.trash/%'
            ORDER BY ip2.property_key
            LIMIT 20
            "#,
        )?;

        let rows: Vec<(String, Option<String>)> = stmt
            .query_map(params![property_key, property_value], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        if rows.is_empty() {
            return Ok(None);
        }

        // Convertir a formato [(key, value)]
        // Para la propiedad principal, usar el valor dado
        // Para las demás, usar valor vacío (para que el usuario lo complete)
        let mut result: Vec<(String, String)> = Vec::new();
        let mut seen_keys = std::collections::HashSet::new();
        
        for (key, value) in rows {
            if seen_keys.contains(&key) {
                continue;
            }
            seen_keys.insert(key.clone());
            
            if key == property_key {
                result.push((key, property_value.to_string()));
            } else {
                // Para otras propiedades, usar cadena vacía (el usuario las completará)
                result.push((key, value.unwrap_or_default()));
            }
        }

        // Asegurar que la propiedad principal esté primero
        result.sort_by(|a, b| {
            if a.0 == property_key { std::cmp::Ordering::Less }
            else if b.0 == property_key { std::cmp::Ordering::Greater }
            else { a.0.cmp(&b.0) }
        });

        Ok(Some(result))
    }

    /// Descubrir columnas relacionadas con una propiedad
    /// Busca todas las propiedades que co-ocurren con la propiedad dada en grupos
    /// Ejemplo: si 'precio' aparece junto a 'juego' y 'pelicula' en diferentes grupos,
    /// devuelve ['juego', 'pelicula', 'horas', ...] 
    pub fn discover_related_columns(&self, property_key: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT ip2.property_key
            FROM inline_properties ip1
            JOIN inline_properties ip2 
              ON ip1.note_id = ip2.note_id 
              AND ip1.group_id = ip2.group_id
            JOIN notes n ON ip1.note_id = n.id
            WHERE ip1.property_key = ?1
              AND ip2.property_key != ?1
              AND ip1.group_id IS NOT NULL
              AND n.name NOT LIKE '.history/%' 
              AND n.name NOT LIKE '.trash/%'
              AND n.path NOT LIKE '%/.history/%'
              AND n.path NOT LIKE '%/.trash/%'
            ORDER BY ip2.property_key
            "#,
        )?;

        let rows = stmt.query_map(params![property_key, property_key], |row| row.get(0))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// Obtener todos los registros que contienen una propiedad específica
    /// Devuelve registros completos (con todas sus propiedades) donde aparece la key
    /// Útil para Bases: filtrar por 'juego' devuelve todos los grupos con juego::X
    pub fn get_records_by_property(&self, property_key: &str) -> Result<Vec<GroupedRecord>> {
        // Obtener todos los grupos que contienen la propiedad
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT ip.note_id, ip.group_id, n.name as note_name
            FROM inline_properties ip
            JOIN notes n ON ip.note_id = n.id
            WHERE ip.property_key = ?1
              AND ip.group_id IS NOT NULL
              AND n.name NOT LIKE '.history/%' 
              AND n.name NOT LIKE '.trash/%'
              AND n.path NOT LIKE '%/.history/%'
              AND n.path NOT LIKE '%/.trash/%'
            ORDER BY n.name, ip.group_id
            "#,
        )?;

        let groups: Vec<(i64, i64, String)> = stmt
            .query_map(params![property_key], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut records = Vec::new();
        for (note_id, group_id, note_name) in groups {
            let props = self.get_group_properties(note_id, group_id)?;
            records.push(GroupedRecord {
                note_id,
                note_name,
                group_id,
                properties: props,
            });
        }

        Ok(records)
    }

    /// Obtener todas las propiedades de un grupo específico
    /// Devuelve Vec<(property_key, value_text)> ordenadas alfabéticamente
    pub fn get_group_properties(&self, note_id: i64, group_id: i64) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT property_key, 
                   COALESCE(value_text, CAST(value_number AS TEXT), 
                            CASE WHEN value_bool = 1 THEN 'true' WHEN value_bool = 0 THEN 'false' ELSE NULL END,
                            '') as value
            FROM inline_properties
            WHERE note_id = ?1 AND group_id = ?2
            ORDER BY property_key
            "#,
        )?;

        let rows = stmt.query_map(params![note_id, group_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// Obtener todos los grupos que son IDÉNTICOS al grupo especificado
    /// (tienen exactamente las mismas propiedades con los mismos valores)
    /// Devuelve Vec<(group_id, char_start, char_end)> ordenados por char_start descendente
    pub fn get_identical_groups(
        &self,
        note_id: i64,
        group_id: i64,
    ) -> Result<Vec<(i64, i64, i64)>> {
        // Primero obtener las propiedades del grupo original
        let original_props = self.get_group_properties(note_id, group_id)?;
        
        if original_props.is_empty() {
            return Ok(vec![]);
        }
        
        // Crear una "firma" del grupo original (propiedades ordenadas)
        let original_signature: String = original_props
            .iter()
            .map(|(k, v)| format!("{}::{}", k, v))
            .collect::<Vec<_>>()
            .join("|");
        
        // Obtener todos los grupos de la nota
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT group_id
            FROM inline_properties
            WHERE note_id = ?1 AND group_id >= 0
            "#,
        )?;
        
        let all_groups: Vec<i64> = stmt
            .query_map(params![note_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        
        // Filtrar los grupos que tienen la misma firma
        let mut matching_groups = Vec::new();
        
        for gid in all_groups {
            let group_props = self.get_group_properties(note_id, gid)?;
            let group_signature: String = group_props
                .iter()
                .map(|(k, v)| format!("{}::{}", k, v))
                .collect::<Vec<_>>()
                .join("|");
            
            if group_signature == original_signature {
                // Obtener ubicación del grupo
                if let Some((_, char_start, char_end)) = self.get_group_location(note_id, gid)? {
                    matching_groups.push((gid, char_start, char_end));
                }
            }
        }
        
        // Ordenar por char_start descendente (para procesar de fin a inicio)
        matching_groups.sort_by(|a, b| b.1.cmp(&a.1));
        
        Ok(matching_groups)
    }

    /// Obtener el valor actual de una propiedad en un grupo específico
    pub fn get_property_value(&self, note_id: i64, group_id: i64, property_key: &str) -> Result<Option<String>> {
        let value: Option<String> = self.conn.query_row(
            r#"
            SELECT value_text
            FROM inline_properties
            WHERE note_id = ?1 AND group_id = ?2 AND property_key = ?3
            "#,
            params![note_id, group_id, property_key],
            |row| row.get(0),
        ).optional()?;

        Ok(value)
    }

    /// Obtener la ubicación exacta de un grupo en una nota (para edición)
    /// Devuelve (line_number, char_start, char_end) del grupo completo
    pub fn get_group_location(&self, note_id: i64, group_id: i64) -> Result<Option<(i64, i64, i64)>> {
        let result: Option<(i64, i64, i64)> = self.conn.query_row(
            r#"
            SELECT MIN(line_number), MIN(char_start), MAX(char_end)
            FROM inline_properties
            WHERE note_id = ?1 AND group_id = ?2
            "#,
            params![note_id, group_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).optional()?;

        Ok(result)
    }

    /// Obtener path de la nota por ID
    pub fn get_note_path_by_id(&self, note_id: i64) -> Result<Option<String>> {
        let path: Option<String> = self.conn.query_row(
            "SELECT path FROM notes WHERE id = ?1",
            params![note_id],
            |row| row.get(0),
        ).optional()?;

        Ok(path)
    }

    /// Obtener registros agrupados que contienen un valor específico en cualquier campo
    /// Ejemplo: buscar "Cervantes" en cualquier campo devuelve todos los registros
    /// donde aparece ese valor junto con los demás campos del grupo
    pub fn get_grouped_records_by_value(
        &self,
        search_key: &str,
        search_value: &str,
    ) -> Result<Vec<GroupedRecord>> {
        // Buscar todos los grupos que contienen el valor buscado
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT ip1.note_id, ip1.group_id, n.name as note_name
            FROM inline_properties ip1
            JOIN notes n ON ip1.note_id = n.id
            WHERE ip1.property_key = ?1 
              AND ip1.value_text = ?2
              AND ip1.group_id IS NOT NULL
            ORDER BY n.name
            "#,
        )?;

        let groups: Vec<(i64, i64, String)> = stmt
            .query_map(params![search_key, search_value], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Para cada grupo encontrado, obtener todas las propiedades del grupo
        let mut records = Vec::new();
        for (note_id, group_id, note_name) in groups {
            let props = self.get_group_properties(note_id, group_id)?;
            records.push(GroupedRecord {
                note_id,
                note_name,
                group_id,
                properties: props,
            });
        }

        Ok(records)
    }

    /// Obtener todos los registros agrupados de todas las notas
    /// Útil para construir vistas de Base con relaciones
    /// Excluye notas de .history y .trash
    /// Incluye tanto propiedades agrupadas [a::1, b::2] como individuales [a::1]
    /// IMPORTANTE: Deduplica registros idénticos dentro de la misma nota
    pub fn get_all_grouped_records(&self) -> Result<Vec<GroupedRecord>> {
        let mut records = Vec::new();
        
        // 1. Primero obtener propiedades con group_id (agrupadas)
        // Excluir notas en .history o .trash (por folder, name o path)
        let mut stmt_grouped = self.conn.prepare(
            r#"
            SELECT DISTINCT ip.note_id, ip.group_id, n.name as note_name
            FROM inline_properties ip
            JOIN notes n ON ip.note_id = n.id
            WHERE ip.group_id IS NOT NULL
              AND n.name NOT LIKE '.history/%' 
              AND n.name NOT LIKE '.trash/%'
              AND n.path NOT LIKE '%/.history/%'
              AND n.path NOT LIKE '%/.trash/%'
              AND (n.folder IS NULL OR (n.folder NOT LIKE '.history%' AND n.folder NOT LIKE '.trash%'))
            ORDER BY n.name, ip.group_id
            "#,
        )?;

        let groups: Vec<(i64, i64, String)> = stmt_grouped
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (note_id, group_id, note_name) in groups {
            let props = self.get_group_properties(note_id, group_id)?;
            records.push(GroupedRecord {
                note_id,
                note_name,
                group_id,
                properties: props,
            });
        }
        
        // 2. Ahora obtener propiedades SIN group_id (individuales)
        // Cada propiedad individual se convierte en un registro separado
        let mut stmt_individual = self.conn.prepare(
            r#"
            SELECT ip.note_id, ip.id, n.name as note_name, ip.property_key,
                   COALESCE(ip.value_text, CAST(ip.value_number AS TEXT), 
                   CASE ip.value_bool WHEN 1 THEN 'true' ELSE 'false' END, '') as value
            FROM inline_properties ip
            JOIN notes n ON ip.note_id = n.id
            WHERE ip.group_id IS NULL
              AND n.name NOT LIKE '.history/%' 
              AND n.name NOT LIKE '.trash/%'
              AND n.path NOT LIKE '%/.history/%'
              AND n.path NOT LIKE '%/.trash/%'
              AND (n.folder IS NULL OR (n.folder NOT LIKE '.history%' AND n.folder NOT LIKE '.trash%'))
            ORDER BY n.name, ip.char_start
            "#,
        )?;
        
        let individual_props: Vec<(i64, i64, String, String, String)> = stmt_individual
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        
        // Cada propiedad individual es su propio "grupo" (con group_id negativo para diferenciar)
        for (note_id, prop_id, note_name, key, value) in individual_props {
            records.push(GroupedRecord {
                note_id,
                note_name,
                group_id: -(prop_id), // ID negativo para propiedades individuales
                properties: vec![(key, value)],
            });
        }

        // 3. Fusionar registros de la misma nota que comparten AL MENOS UNA propiedad
        // con el mismo valor (excepto propiedades muy comunes como "comprado")
        // Por ejemplo: [juego::Novalands] y [juego:::Novalands, comprado::Si] 
        // se fusionan porque ambos tienen juego=Novalands
        let mut merged_records: Vec<GroupedRecord> = Vec::new();
        
        // Agrupar por note_id primero
        let mut by_note: std::collections::HashMap<i64, Vec<GroupedRecord>> = std::collections::HashMap::new();
        for record in records {
            by_note.entry(record.note_id).or_default().push(record);
        }
        
        // Propiedades que NO deberían usarse como clave de fusión (son muy genéricas)
        let non_key_props: std::collections::HashSet<&str> = ["comprado", "completado", "status", "estado", "leido", "visto"]
            .iter().cloned().collect();
        
        for (_note_id, note_records) in by_note {
            let mut remaining: Vec<GroupedRecord> = note_records;
            
            while !remaining.is_empty() {
                let mut current = remaining.remove(0);
                let mut merged_any = true;
                
                while merged_any {
                    merged_any = false;
                    let mut i = 0;
                    
                    while i < remaining.len() {
                        // Buscar si comparten alguna propiedad "clave" (no genérica) con el mismo valor
                        let shares_key_prop = current.properties.iter().any(|(ck, cv)| {
                            // Solo considerar propiedades que no sean genéricas y tengan valor
                            if non_key_props.contains(ck.to_lowercase().as_str()) || cv.is_empty() {
                                return false;
                            }
                            // Buscar si el otro registro tiene la misma propiedad con el mismo valor
                            remaining[i].properties.iter().any(|(ok, ov)| {
                                ck == ok && cv == ov && !ov.is_empty()
                            })
                        });
                        
                        if shares_key_prop {
                            // Fusionar
                            let other = remaining.remove(i);
                            for (key, value) in other.properties {
                                let existing_idx = current.properties.iter().position(|(k, _)| k == &key);
                                match existing_idx {
                                    Some(idx) => {
                                        if current.properties[idx].1.is_empty() && !value.is_empty() {
                                            current.properties[idx].1 = value;
                                        }
                                    }
                                    None => {
                                        current.properties.push((key, value));
                                    }
                                }
                            }
                            // Preferir group_id positivo para edición
                            if current.group_id < 0 && other.group_id >= 0 {
                                current.group_id = other.group_id;
                            }
                            merged_any = true;
                        } else {
                            i += 1;
                        }
                    }
                }
                
                merged_records.push(current);
            }
        }
        
        // 4. Deduplicar registros completamente idénticos (por si quedaron duplicados exactos)
        // Usamos hash directo en lugar de format! para mejor rendimiento
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
        merged_records.retain(|record| {
            let mut props_sorted: Vec<_> = record.properties.iter().collect();
            props_sorted.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
            
            // Calcular hash directamente en lugar de serializar a String
            let mut hasher = DefaultHasher::new();
            record.note_id.hash(&mut hasher);
            for (k, v) in &props_sorted {
                k.hash(&mut hasher);
                v.hash(&mut hasher);
            }
            let key = hasher.finish();
            seen.insert(key)
        });

        Ok(merged_records)
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

        // Sincronizar propiedades inline
        self.sync_inline_properties(note_id, content)?;

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
            // Eliminar propiedades inline asociadas
            self.conn
                .execute("DELETE FROM inline_properties WHERE note_id = ?1", params![id])?;
            
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
    
    /// Limpiar propiedades inline huérfanas (cuya nota ya no existe)
    pub fn cleanup_orphaned_inline_properties(&self) -> Result<usize> {
        let deleted = self.conn.execute(
            r#"
            DELETE FROM inline_properties 
            WHERE note_id NOT IN (SELECT id FROM notes)
            "#,
            [],
        )?;
        
        if deleted > 0 {
            println!("🧹 Limpiadas {} propiedades inline huérfanas", deleted);
        }
        
        Ok(deleted)
    }

    /// Obtener metadata de una nota
    pub fn get_note(&self, name: &str) -> Result<Option<NoteMetadata>> {
        let result = self
            .conn
            .query_row(
                r#"
            SELECT id, name, path, folder, order_index, icon, created_at, updated_at
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
                        icon: row.get(5)?,
                        created_at: DateTime::from_timestamp(row.get(6)?, 0).unwrap(),
                        updated_at: DateTime::from_timestamp(row.get(7)?, 0).unwrap(),
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
            SELECT id, name, path, folder, order_index, icon, created_at, updated_at
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
                        icon: row.get(5)?,
                        created_at: DateTime::from_timestamp(row.get(6)?, 0).unwrap(),
                        updated_at: DateTime::from_timestamp(row.get(7)?, 0).unwrap(),
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    /// Listar todas las notas, opcionalmente filtradas por carpeta
    /// Excluye notas de .history y .trash
    pub fn list_notes(&self, folder: Option<&str>) -> Result<Vec<NoteMetadata>> {
        let mut stmt = if folder.is_some() {
            self.conn.prepare(
                "SELECT id, name, path, folder, order_index, icon, created_at, updated_at
                 FROM notes 
                 WHERE folder = ?1 
                   AND (folder IS NULL OR (folder NOT LIKE '.history%' AND folder NOT LIKE '.trash%'))
                   AND name NOT LIKE '.history/%' AND name NOT LIKE '.trash/%'
                 ORDER BY order_index, name",
            )?
        } else {
            self.conn.prepare(
                "SELECT id, name, path, folder, order_index, icon, created_at, updated_at
                 FROM notes 
                 WHERE (folder IS NULL OR (folder NOT LIKE '.history%' AND folder NOT LIKE '.trash%'))
                   AND name NOT LIKE '.history/%' AND name NOT LIKE '.trash/%'
                 ORDER BY order_index, name",
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
            icon: row.get(5)?,
            created_at: DateTime::from_timestamp(row.get(6)?, 0).unwrap(),
            updated_at: DateTime::from_timestamp(row.get(7)?, 0).unwrap(),
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
                snippet(notes_fts, -1, '<mark>', '</mark>', '...', 16) as snippet,
                rank as relevance
            FROM notes_fts
            JOIN notes ON notes_fts.rowid = notes.id
            WHERE notes_fts MATCH ?1
              AND (notes.folder IS NULL OR (
                  notes.folder NOT LIKE '.trash%' AND 
                  notes.folder NOT LIKE '.history%'
              ))
            ORDER BY rank
            LIMIT 20
            "#,
        )?;

        let results: Vec<SearchResult> = stmt
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

        // Si FTS5 no encontró resultados, intentar búsqueda LIKE como fallback
        if results.is_empty() && query_text.len() >= 2 {
            let like_pattern = format!("%{}%", query_text.to_lowercase());
            let mut fallback_stmt = self.conn.prepare(
                r#"
                SELECT
                    notes.id,
                    notes.name,
                    notes.path,
                    substr(notes.content, 1, 100) as snippet,
                    1.0 as relevance
                FROM notes
                WHERE (LOWER(notes.name) LIKE ?1 OR LOWER(notes.content) LIKE ?1)
                  AND (notes.folder IS NULL OR (
                      notes.folder NOT LIKE '.trash%' AND 
                      notes.folder NOT LIKE '.history%'
                  ))
                ORDER BY notes.name
                LIMIT 20
                "#,
            )?;

            let fallback_results = fallback_stmt
                .query_map([&like_pattern], |row| {
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

            return Ok(fallback_results);
        }

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

    // ==================== FUNCIONES DE ICONOS ====================

    /// Establecer el icono personalizado de una nota
    pub fn set_note_icon(&self, note_name: &str, icon: Option<&str>) -> Result<()> {
        let now = Utc::now().timestamp();
        let rows = self.conn.execute(
            "UPDATE notes SET icon = ?1, updated_at = ?2 WHERE name = ?3",
            params![icon, now, note_name],
        )?;

        if rows == 0 {
            return Err(DatabaseError::NoteNotFound(note_name.to_string()));
        }

        Ok(())
    }

    /// Obtener el icono de una nota
    pub fn get_note_icon(&self, note_name: &str) -> Result<Option<String>> {
        let result = self
            .conn
            .query_row(
                "SELECT icon FROM notes WHERE name = ?1",
                params![note_name],
                |row| row.get(0),
            )
            .optional()?;

        Ok(result.flatten())
    }

    /// Establecer el icono personalizado de una carpeta
    pub fn set_folder_icon(&self, folder_path: &str, icon: Option<&str>) -> Result<()> {
        let now = Utc::now().timestamp();

        // Usar UPSERT para crear o actualizar
        self.conn.execute(
            r#"
            INSERT INTO folders (path, icon, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(path) DO UPDATE SET
                icon = excluded.icon,
                updated_at = excluded.updated_at
            "#,
            params![folder_path, icon, now, now],
        )?;

        Ok(())
    }

    /// Establecer el color de una carpeta
    pub fn set_folder_color(&self, folder_path: &str, color: Option<&str>) -> Result<()> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            r#"
            INSERT INTO folders (path, color, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(path) DO UPDATE SET
                color = excluded.color,
                updated_at = excluded.updated_at
            "#,
            params![folder_path, color, now, now],
        )?;

        Ok(())
    }

    /// Obtener el icono de una carpeta
    pub fn get_folder_icon(&self, folder_path: &str) -> Result<Option<String>> {
        let result = self
            .conn
            .query_row(
                "SELECT icon FROM folders WHERE path = ?1",
                params![folder_path],
                |row| row.get(0),
            )
            .optional()?;

        Ok(result.flatten())
    }

    /// Obtener metadata de una carpeta
    pub fn get_folder(&self, folder_path: &str) -> Result<Option<FolderMetadata>> {
        let result = self
            .conn
            .query_row(
                r#"
                SELECT id, path, icon, color, order_index, created_at, updated_at
                FROM folders WHERE path = ?1
                "#,
                params![folder_path],
                |row| {
                    Ok(FolderMetadata {
                        id: row.get(0)?,
                        path: row.get(1)?,
                        icon: row.get(2)?,
                        color: row.get(3)?,
                        order_index: row.get(4)?,
                        created_at: DateTime::from_timestamp(row.get(5)?, 0).unwrap(),
                        updated_at: DateTime::from_timestamp(row.get(6)?, 0).unwrap(),
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    /// Listar todas las carpetas con sus iconos
    pub fn list_folders_with_icons(&self) -> Result<Vec<FolderMetadata>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, icon, color, order_index, created_at, updated_at
             FROM folders ORDER BY order_index, path",
        )?;

        let folders = stmt
            .query_map([], |row| {
                Ok(FolderMetadata {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    icon: row.get(2)?,
                    color: row.get(3)?,
                    order_index: row.get(4)?,
                    created_at: DateTime::from_timestamp(row.get(5)?, 0).unwrap(),
                    updated_at: DateTime::from_timestamp(row.get(6)?, 0).unwrap(),
                })
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(folders)
    }

    /// Obtener lista de todas las carpetas (nombres únicos) desde las notas
    pub fn get_all_folders(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT folder FROM notes WHERE folder IS NOT NULL AND folder != '' ORDER BY folder"
        )?;

        let folders = stmt
            .query_map([], |row| row.get(0))?
            .collect::<SqliteResult<Vec<String>>>()?;

        Ok(folders)
    }

    /// Obtener un mapa rápido de path -> icono para todas las carpetas
    pub fn get_all_folder_icons(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, icon FROM folders WHERE icon IS NOT NULL")?;

        let icons = stmt
            .query_map([], |row| {
                let path: String = row.get(0)?;
                let icon: String = row.get(1)?;
                Ok((path, icon))
            })?
            .collect::<SqliteResult<std::collections::HashMap<_, _>>>()?;

        Ok(icons)
    }

    /// Obtener un mapa rápido de nombre -> icono para todas las notas con iconos personalizados
    pub fn get_all_note_icons(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, icon FROM notes WHERE icon IS NOT NULL")?;

        let icons = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let icon: String = row.get(1)?;
                Ok((name, icon))
            })?
            .collect::<SqliteResult<std::collections::HashMap<_, _>>>()?;

        Ok(icons)
    }

    /// Establecer el color del icono de una nota
    pub fn set_note_icon_color(&self, note_name: &str, color: Option<&str>) -> Result<()> {
        let now = Utc::now().timestamp();
        let rows = self.conn.execute(
            "UPDATE notes SET icon_color = ?1, updated_at = ?2 WHERE name = ?3",
            params![color, now, note_name],
        )?;

        if rows == 0 {
            return Err(DatabaseError::NoteNotFound(note_name.to_string()));
        }

        Ok(())
    }

    /// Establecer el color del icono de una carpeta
    pub fn set_folder_icon_color(&self, folder_path: &str, color: Option<&str>) -> Result<()> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            r#"
            INSERT INTO folders (path, icon_color, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(path) DO UPDATE SET
                icon_color = excluded.icon_color,
                updated_at = excluded.updated_at
            "#,
            params![folder_path, color, now, now],
        )?;

        Ok(())
    }

    /// Obtener un mapa de nombre -> (icono, color) para todas las notas con iconos
    pub fn get_all_note_icons_with_colors(
        &self,
    ) -> Result<std::collections::HashMap<String, (String, Option<String>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, icon, icon_color FROM notes WHERE icon IS NOT NULL")?;

        let icons = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let icon: String = row.get(1)?;
                let color: Option<String> = row.get(2)?;
                Ok((name, (icon, color)))
            })?
            .collect::<SqliteResult<std::collections::HashMap<_, _>>>()?;

        Ok(icons)
    }

    /// Obtener un mapa de path -> (icono, color) para todas las carpetas con iconos
    pub fn get_all_folder_icons_with_colors(
        &self,
    ) -> Result<std::collections::HashMap<String, (String, Option<String>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, icon, icon_color FROM folders WHERE icon IS NOT NULL")?;

        let icons = stmt
            .query_map([], |row| {
                let path: String = row.get(0)?;
                let icon: String = row.get(1)?;
                let color: Option<String> = row.get(2)?;
                Ok((path, (icon, color)))
            })?
            .collect::<SqliteResult<std::collections::HashMap<_, _>>>()?;

        Ok(icons)
    }

    // ==================== FUNCIONES DE PROPIEDADES ====================

    /// Guardar/actualizar una propiedad de una nota
    pub fn set_note_property(
        &self,
        note_id: i64,
        key: &str,
        prop_type: &str,
        value_text: Option<&str>,
        value_number: Option<f64>,
        value_bool: Option<bool>,
    ) -> Result<()> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            r#"
            INSERT INTO note_properties (note_id, property_key, property_type, value_text, value_number, value_bool, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(note_id, property_key) DO UPDATE SET
                property_type = excluded.property_type,
                value_text = excluded.value_text,
                value_number = excluded.value_number,
                value_bool = excluded.value_bool,
                updated_at = excluded.updated_at
            "#,
            params![note_id, key, prop_type, value_text, value_number, value_bool.map(|b| if b { 1 } else { 0 }), now, now],
        )?;

        Ok(())
    }

    /// Obtener todas las propiedades de una nota
    pub fn get_note_properties(&self, note_id: i64) -> Result<Vec<(String, String, Option<String>, Option<f64>, Option<bool>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT property_key, property_type, value_text, value_number, value_bool FROM note_properties WHERE note_id = ?1"
        )?;

        let props = stmt
            .query_map(params![note_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<f64>>(3)?,
                    row.get::<_, Option<i64>>(4)?.map(|v| v != 0),
                ))
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(props)
    }

    /// Eliminar una propiedad de una nota
    pub fn delete_note_property(&self, note_id: i64, key: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM note_properties WHERE note_id = ?1 AND property_key = ?2",
            params![note_id, key],
        )?;
        Ok(())
    }

    /// Eliminar todas las propiedades de una nota
    pub fn delete_all_note_properties(&self, note_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM note_properties WHERE note_id = ?1",
            params![note_id],
        )?;
        Ok(())
    }

    // ==================== FUNCIONES DE BASES ====================

    /// Crear una nueva Base
    pub fn create_base(&self, name: &str, description: Option<&str>, source_folder: Option<&str>, config_yaml: &str) -> Result<i64> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            r#"
            INSERT INTO bases (name, description, source_folder, config_yaml, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![name, description, source_folder, config_yaml, now, now],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Actualizar una Base existente
    pub fn update_base(&self, id: i64, config_yaml: &str, active_view: i32) -> Result<()> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            "UPDATE bases SET config_yaml = ?1, active_view = ?2, updated_at = ?3 WHERE id = ?4",
            params![config_yaml, active_view, now, id],
        )?;

        Ok(())
    }

    /// Obtener una Base por ID
    pub fn get_base(&self, id: i64) -> Result<Option<(i64, String, Option<String>, Option<String>, String, i32)>> {
        let result = self
            .conn
            .query_row(
                "SELECT id, name, description, source_folder, config_yaml, active_view FROM bases WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .optional()?;

        Ok(result)
    }

    /// Obtener una Base por nombre
    pub fn get_base_by_name(&self, name: &str) -> Result<Option<(i64, String, Option<String>, Option<String>, String, i32)>> {
        let result = self
            .conn
            .query_row(
                "SELECT id, name, description, source_folder, config_yaml, active_view FROM bases WHERE name = ?1",
                params![name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .optional()?;

        Ok(result)
    }

    /// Listar todas las Bases
    pub fn list_bases(&self) -> Result<Vec<(i64, String, Option<String>, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, description, source_folder FROM bases ORDER BY name"
        )?;

        let bases = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(bases)
    }

    /// Eliminar una Base
    pub fn delete_base(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM bases WHERE id = ?1", params![id])?;
        Ok(())
    }
    
    /// Renombrar una Base
    pub fn rename_base(&self, id: i64, new_name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE bases SET name = ?1 WHERE id = ?2",
            params![new_name, id]
        )?;
        Ok(())
    }

    /// Buscar notas que tengan una propiedad con un valor específico
    pub fn find_notes_by_property(&self, key: &str, value: &str) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT note_id FROM note_properties WHERE property_key = ?1 AND value_text = ?2"
        )?;

        let ids = stmt
            .query_map(params![key, value], |row| row.get(0))?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(ids)
    }

    /// Buscar notas que tengan una propiedad numérica en un rango
    pub fn find_notes_by_property_range(&self, key: &str, min: f64, max: f64) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT note_id FROM note_properties WHERE property_key = ?1 AND value_number >= ?2 AND value_number <= ?3"
        )?;

        let ids = stmt
            .query_map(params![key, min, max], |row| row.get(0))?
            .collect::<SqliteResult<Vec<_>>>()?;

        Ok(ids)
    }

    /// Obtener todas las notas de una carpeta (para una Base) con sus propiedades
    /// Retorna: Vec<(note_id, note_name, note_path, properties_map)>
    pub fn get_notes_for_base(&self, source_folder: Option<&str>) -> Result<Vec<(i64, String, String, std::collections::HashMap<String, String>)>> {
        // Primero obtener las notas según el source_folder
        let notes: Vec<(i64, String, String)> = if let Some(folder) = source_folder {
            let mut stmt = self.conn.prepare(
                "SELECT id, name, path FROM notes WHERE folder = ?1 OR folder LIKE ?2 ORDER BY name"
            )?;
            let pattern = format!("{}/%", folder);
            stmt.query_map(params![folder, pattern], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?.collect::<SqliteResult<Vec<_>>>()?
        } else {
            // Si no hay source_folder, obtener todas las notas
            let mut stmt = self.conn.prepare(
                "SELECT id, name, path FROM notes ORDER BY name"
            )?;
            stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?.collect::<SqliteResult<Vec<_>>>()?
        };

        // Para cada nota, obtener sus propiedades
        let mut result = Vec::new();
        for (note_id, note_name, note_path) in notes {
            let props = self.get_note_properties(note_id)?;
            let mut props_map = std::collections::HashMap::new();
            
            for (key, prop_type, value_text, value_number, value_bool) in props {
                let display_value = match prop_type.as_str() {
                    "text" | "select" | "multi_select" | "url" | "email" | "phone" => {
                        value_text.unwrap_or_default()
                    }
                    "number" => {
                        value_number.map(|n| n.to_string()).unwrap_or_default()
                    }
                    "checkbox" => {
                        if value_bool.unwrap_or(false) { "✓".to_string() } else { "✗".to_string() }
                    }
                    "date" => {
                        value_text.unwrap_or_default()
                    }
                    _ => value_text.unwrap_or_default()
                };
                props_map.insert(key, display_value);
            }
            
            result.push((note_id, note_name, note_path, props_map));
        }

        Ok(result)
    }

    /// Obtener todas las claves de propiedades usadas en notas de una carpeta
    pub fn get_property_keys_for_folder(&self, source_folder: Option<&str>) -> Result<Vec<(String, String)>> {
        let keys: Vec<(String, String)> = if let Some(folder) = source_folder {
            let pattern = format!("{}/%", folder);
            let mut stmt = self.conn.prepare(
                r#"
                SELECT DISTINCT np.property_key, np.property_type 
                FROM note_properties np
                JOIN notes n ON np.note_id = n.id
                WHERE n.folder = ?1 OR n.folder LIKE ?2
                ORDER BY np.property_key
                "#
            )?;
            stmt.query_map(params![folder, pattern], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?.collect::<SqliteResult<Vec<_>>>()?
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT DISTINCT property_key, property_type FROM note_properties ORDER BY property_key"
            )?;
            stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?.collect::<SqliteResult<Vec<_>>>()?
        };

        Ok(keys)
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
