//! Herramientas nativas para el agente RIG

use crate::ai::memory::NoteMemory;
use crate::core::database::NotesDatabase;
use anyhow::Result;
use rig::embeddings::EmbeddingModel;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("Tool error: {0}")]
pub struct ToolError(pub String);

impl From<anyhow::Error> for ToolError {
    fn from(err: anyhow::Error) -> Self {
        ToolError(err.to_string())
    }
}

// --- SearchNotes (FTS) ---

#[derive(Deserialize)]
pub struct SearchArgs {
    pub query: String,
}

pub struct SearchNotes {
    pub db_path: PathBuf,
}

impl Tool for SearchNotes {
    const NAME: &'static str = "search_notes";

    type Args = SearchArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "search_notes".to_string(),
            description: "Search for notes using full-text search (keywords)".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query (keywords)"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_path = self.db_path.clone();
        // Run blocking DB operation in a blocking task
        let results = tokio::task::spawn_blocking(move || {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
            db.search_notes(&args.query).map_err(|e| anyhow::anyhow!(e))
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        if results.is_empty() {
            return Ok("No notes found matching the query.".to_string());
        }

        let mut output = String::new();
        for res in results {
            output.push_str(&format!(
                "- {} (ID: {}): {}\n",
                res.note_name, res.note_id, res.snippet
            ));
        }
        Ok(output)
    }
}

impl SearchNotes {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

// --- SemanticSearch ---

#[derive(Deserialize)]
pub struct SemanticSearchArgs {
    pub query: String,
}

pub struct SemanticSearch<M: EmbeddingModel + Sync + Send + 'static> {
    pub memory: Arc<NoteMemory<M>>,
}

impl<M: EmbeddingModel + Sync + Send + 'static> Tool for SemanticSearch<M> {
    const NAME: &'static str = "semantic_search";

    type Args = SemanticSearchArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "semantic_search".to_string(),
            description: "Search for notes by meaning/similarity (vector search)".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The semantic query"
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let results = self
            .memory
            .search(&args.query, 5)
            .await
            .map_err(|e| ToolError(e.to_string()))?;

        if results.is_empty() {
            return Ok("No semantically similar notes found.".to_string());
        }

        let mut output = String::new();
        for (score, id, _metadata, content) in results {
            // Limpiar saltos de línea excesivos para el snippet
            let clean_content = content.replace('\n', " ");
            let snippet = if clean_content.len() > 300 {
                format!("{}...", &clean_content[..300])
            } else {
                clean_content
            };

            output.push_str(&format!(
                "- Note: {}\n  Score: {:.2}\n  Snippet: {}\n\n",
                id, score, snippet
            ));
        }
        Ok(output)
    }
}

// --- ReadNote ---

#[derive(Deserialize)]
pub struct ReadNoteArgs {
    pub name: String,
}

pub struct ReadNote {
    pub db_path: PathBuf,
}

impl Tool for ReadNote {
    const NAME: &'static str = "read_note";

    type Args = ReadNoteArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "read_note".to_string(),
            description: "Read the content of a specific note by name".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The exact name of the note"
                    }
                },
                "required": ["name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_path = self.db_path.clone();
        let content = tokio::task::spawn_blocking(move || {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
            let metadata = db.get_note(&args.name).map_err(|e| anyhow::anyhow!(e))?;

            if let Some(meta) = metadata {
                std::fs::read_to_string(&meta.path).map_err(|e| anyhow::anyhow!(e))
            } else {
                Err(anyhow::anyhow!("Note not found"))
            }
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(content)
    }
}

impl ReadNote {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

// --- CreateNote ---

#[derive(Deserialize, Clone)]
pub struct CreateNoteArgs {
    pub name: String,
    pub content: String,
    pub folder: Option<String>,
}

pub struct CreateNote<M: EmbeddingModel + Sync + Send + Clone + 'static> {
    pub db_path: PathBuf,
    pub notes_dir: PathBuf,
    pub memory: Option<Arc<NoteMemory<M>>>,
}

impl<M: EmbeddingModel + Sync + Send + Clone + 'static> Tool for CreateNote<M> {
    const NAME: &'static str = "create_note";

    type Args = CreateNoteArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "create_note".to_string(),
            description: "Create a new note with the given content".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name of the note (without extension)"
                    },
                    "content": {
                        "type": "string",
                        "description": "The markdown content of the note"
                    },
                    "folder": {
                        "type": "string",
                        "description": "Optional folder path relative to notes root"
                    }
                },
                "required": ["name", "content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_path = self.db_path.clone();
        let notes_dir = self.notes_dir.clone();
        let memory = self.memory.clone();

        // Clone args for use inside the closure
        let args_for_closure = args.clone();

        let result = tokio::task::spawn_blocking(move || {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;

            // Determine file path
            let mut file_path = notes_dir.clone();
            if let Some(folder) = &args_for_closure.folder {
                file_path.push(folder);
                std::fs::create_dir_all(&file_path).map_err(|e| anyhow::anyhow!(e))?;
            }
            file_path.push(format!("{}.md", args_for_closure.name));

            // Write file
            std::fs::write(&file_path, &args_for_closure.content)
                .map_err(|e| anyhow::anyhow!(e))?;

            // Index in DB
            let path_str = file_path.to_string_lossy().to_string();
            db.index_note(
                &args_for_closure.name,
                &path_str,
                &args_for_closure.content,
                args_for_closure.folder.as_deref(),
            )
            .map_err(|e| anyhow::anyhow!(e))?;

            Ok::<String, anyhow::Error>(format!(
                "Note '{}' created successfully at {}. Link: [{}]( {})",
                args_for_closure.name, path_str, args_for_closure.name, args_for_closure.name
            ))
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        // Index in Memory (Semantic Search) if available
        if let Some(mem) = memory {
            let metadata = serde_json::json!({
                "name": args.name,
                "folder": args.folder
            });
            if let Err(e) = mem.index_note(&args.name, &args.content, metadata).await {
                eprintln!("Failed to index note in vector store: {}", e);
                // Don't fail the tool call, just log the error
            }
        }

        Ok(result)
    }
}

impl<M: EmbeddingModel + Sync + Send + Clone + 'static> CreateNote<M> {
    pub fn new(db_path: PathBuf, notes_dir: PathBuf, memory: Option<Arc<NoteMemory<M>>>) -> Self {
        Self {
            db_path,
            notes_dir,
            memory,
        }
    }
}

// --- ListNotes ---

#[derive(Deserialize)]
pub struct ListNotesArgs {
    pub folder: Option<String>,
}

pub struct ListNotes {
    pub db_path: PathBuf,
}

impl Tool for ListNotes {
    const NAME: &'static str = "list_notes";

    type Args = ListNotesArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "list_notes".to_string(),
            description: "List all notes, optionally filtered by folder".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "folder": {
                        "type": "string",
                        "description": "Optional folder to filter by"
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_path = self.db_path.clone();

        let notes = tokio::task::spawn_blocking(move || {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
            db.list_notes(args.folder.as_deref())
                .map_err(|e| anyhow::anyhow!(e))
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        if notes.is_empty() {
            return Ok("No notes found.".to_string());
        }

        let mut output = String::new();
        for note in notes.iter().take(50) {
            // Limit to 50 to avoid context overflow
            let folder_info = note.folder.as_deref().unwrap_or("root");
            output.push_str(&format!("- {} (Folder: {})\n", note.name, folder_info));
        }
        if notes.len() > 50 {
            output.push_str(&format!("... and {} more notes.", notes.len() - 50));
        }
        Ok(output)
    }
}

impl ListNotes {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

// --- IndexAllNotes ---

#[derive(Deserialize)]
pub struct IndexAllNotesArgs {}

pub struct IndexAllNotes<M: EmbeddingModel + Sync + Send + Clone + 'static> {
    pub db_path: PathBuf,
    pub memory: Arc<NoteMemory<M>>,
}

impl<M: EmbeddingModel + Sync + Send + Clone + 'static> Tool for IndexAllNotes<M> {
    const NAME: &'static str = "index_all_notes";

    type Args = IndexAllNotesArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "index_all_notes".to_string(),
            description: "Index all existing notes into the vector store for semantic search"
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!("🚀 [IndexAllNotes] Iniciando reindexación completa (versión corregida)...");
        let db_path = self.db_path.clone();
        let memory = self.memory.clone();

        // Clear all existing indexes first
        if let Err(e) = memory.clear_all().await {
            return Err(ToolError(format!(
                "Failed to clear existing indexes: {}",
                e
            )));
        }

        // Get all notes from DB
        let notes = tokio::task::spawn_blocking(move || {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
            let notes = db.list_notes(None).map_err(|e| anyhow::anyhow!(e))?;

            // Read content for each note
            let mut notes_with_content = Vec::new();
            for note in notes {
                if let Ok(content) = std::fs::read_to_string(&note.path) {
                    notes_with_content.push((note, content));
                }
            }
            Ok::<Vec<(crate::core::database::NoteMetadata, String)>, anyhow::Error>(
                notes_with_content,
            )
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        let total = notes.len();
        let mut indexed = 0;
        let mut errors = 0;

        for (note, content) in notes {
            let metadata = serde_json::json!({
                "name": note.name,
                "folder": note.folder
            });

            match memory.index_note(&note.name, &content, metadata).await {
                Ok(_) => indexed += 1,
                Err(e) => {
                    eprintln!("Failed to index note '{}': {}", note.name, e);
                    errors += 1;
                }
            }
        }

        Ok(format!(
            "Indexed {} notes successfully. {} errors.",
            indexed, errors
        ))
    }
}

impl<M: EmbeddingModel + Sync + Send + Clone + 'static> IndexAllNotes<M> {
    pub fn new(db_path: PathBuf, memory: Arc<NoteMemory<M>>) -> Self {
        Self { db_path, memory }
    }
}
