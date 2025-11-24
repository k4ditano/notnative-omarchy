//! Herramientas extendidas para el agente RIG
//!
//! Este módulo contiene implementaciones adicionales de herramientas MCP
//! como herramientas nativas de RIG para mejor rendimiento.

use crate::ai::tools::ToolError;
use crate::core::database::NotesDatabase;
use anyhow::Result;
use rig::tool::Tool;
use serde::Deserialize;
use std::path::PathBuf;

// ==================== GESTIÓN DE NOTAS ====================

#[derive(Deserialize)]
pub struct UpdateNoteArgs {
    pub name: String,
    pub content: String,
}

pub struct UpdateNote {
    pub db_path: PathBuf,
}

impl Tool for UpdateNote {
    const NAME: &'static str = "update_note";

    type Args = UpdateNoteArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "update_note".to_string(),
            description: "Update or overwrite the content of an existing note".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name of the note to update"
                    },
                    "content": {
                        "type": "string",
                        "description": "The new complete content"
                    }
                },
                "required": ["name", "content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_path = self.db_path.clone();

        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
            let metadata = db.get_note(&args.name).map_err(|e| anyhow::anyhow!(e))?;

            if let Some(meta) = metadata {
                std::fs::write(&meta.path, &args.content).map_err(|e| anyhow::anyhow!(e))?;

                // Update in DB
                db.index_note(
                    &args.name,
                    &meta.path,
                    &args.content,
                    meta.folder.as_deref(),
                )
                .map_err(|e| anyhow::anyhow!(e))?;

                Ok(format!(
                    "Note '{}' updated successfully. Link: [{}]( {})",
                    args.name, args.name, args.name
                ))
            } else {
                Err(anyhow::anyhow!("Note '{}' not found", args.name))
            }
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(result)
    }
}

impl UpdateNote {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

// ==================== APPEND TO NOTE ====================

#[derive(Deserialize)]
pub struct AppendToNoteArgs {
    pub name: String,
    pub content: String,
}

pub struct AppendToNote {
    pub db_path: PathBuf,
}

impl Tool for AppendToNote {
    const NAME: &'static str = "append_to_note";

    type Args = AppendToNoteArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "append_to_note".to_string(),
            description: "Append content to the end of an existing note without deleting what's already there".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name of the note"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to append at the end"
                    }
                },
                "required": ["name", "content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_path = self.db_path.clone();

        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
            let metadata = db.get_note(&args.name).map_err(|e| anyhow::anyhow!(e))?;

            if let Some(meta) = metadata {
                let mut current_content =
                    std::fs::read_to_string(&meta.path).map_err(|e| anyhow::anyhow!(e))?;

                current_content.push_str(&args.content);

                std::fs::write(&meta.path, &current_content).map_err(|e| anyhow::anyhow!(e))?;

                // Update in DB
                db.index_note(
                    &args.name,
                    &meta.path,
                    &current_content,
                    meta.folder.as_deref(),
                )
                .map_err(|e| anyhow::anyhow!(e))?;

                Ok(format!(
                    "Content appended to note '{}'. Link: [{}]( {})",
                    args.name, args.name, args.name
                ))
            } else {
                Err(anyhow::anyhow!("Note '{}' not found", args.name))
            }
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(result)
    }
}

impl AppendToNote {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

// ==================== DELETE NOTE ====================

#[derive(Deserialize)]
pub struct DeleteNoteArgs {
    pub name: String,
}

pub struct DeleteNote {
    pub db_path: PathBuf,
}

impl Tool for DeleteNote {
    const NAME: &'static str = "delete_note";

    type Args = DeleteNoteArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "delete_note".to_string(),
            description: "Permanently delete a note. Be careful, this action cannot be undone!"
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name of the note to delete"
                    }
                },
                "required": ["name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_path = self.db_path.clone();

        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
            let metadata = db.get_note(&args.name).map_err(|e| anyhow::anyhow!(e))?;

            if let Some(meta) = metadata {
                std::fs::remove_file(&meta.path).map_err(|e| anyhow::anyhow!(e))?;

                // Remove from DB
                db.delete_note(&args.name).map_err(|e| anyhow::anyhow!(e))?;

                Ok(format!("Note '{}' deleted successfully", args.name))
            } else {
                Err(anyhow::anyhow!("Note '{}' not found", args.name))
            }
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(result)
    }
}

impl DeleteNote {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

// ==================== GET TAGS ====================

#[derive(Deserialize)]
pub struct GetNotesWithTagArgs {
    pub tag: String,
}

pub struct GetNotesWithTag {
    pub db_path: PathBuf,
}

impl Tool for GetNotesWithTag {
    const NAME: &'static str = "get_notes_with_tag";

    type Args = GetNotesWithTagArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "get_notes_with_tag".to_string(),
            description: "Find all notes that have a specific tag".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "tag": {
                        "type": "string",
                        "description": "The tag to search for (without #)"
                    }
                },
                "required": ["tag"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_path = self.db_path.clone();
        let tag_clone = args.tag.clone();

        let notes = tokio::task::spawn_blocking(
            move || -> anyhow::Result<Vec<crate::core::database::NoteMetadata>> {
                let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
                let all_notes = db.list_notes(None).map_err(|e| anyhow::anyhow!(e))?;

                // Filter notes that have the tag
                let filtered: Vec<_> = all_notes
                    .into_iter()
                    .filter(|note| {
                        if let Ok(content) = std::fs::read_to_string(&note.path) {
                            use crate::core::frontmatter::extract_all_tags;
                            let tags = extract_all_tags(&content);
                            tags.iter().any(|t| t == &tag_clone)
                        } else {
                            false
                        }
                    })
                    .collect();
                Ok(filtered)
            },
        )
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        if notes.is_empty() {
            return Ok(format!("No notes found with tag '{}'", args.tag));
        }

        let mut output = format!("Notes with tag '{}':\n", args.tag);
        for note in notes.iter().take(20) {
            output.push_str(&format!("- {}\n", note.name));
        }

        if notes.len() > 20 {
            output.push_str(&format!("... and {} more notes.", notes.len() - 20));
        }

        Ok(output)
    }
}

impl GetNotesWithTag {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

// ==================== GET ALL TAGS ====================

#[derive(Deserialize)]
pub struct GetAllTagsArgs {}

pub struct GetAllTags {
    pub db_path: PathBuf,
}

impl Tool for GetAllTags {
    const NAME: &'static str = "get_all_tags";

    type Args = GetAllTagsArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "get_all_tags".to_string(),
            description: "Get all unique tags used across all notes".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_path = self.db_path.clone();

        let tags = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<String>> {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
            let tag_objects = db.get_tags().map_err(|e| anyhow::anyhow!(e))?;
            Ok(tag_objects.into_iter().map(|t| t.name).collect::<Vec<_>>())
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        if tags.is_empty() {
            return Ok("No tags found in the system.".to_string());
        }

        let mut output = format!("All tags ({} total):\n", tags.len());
        for tag in tags.iter().take(50) {
            output.push_str(&format!("- #{}\n", tag));
        }

        if tags.len() > 50 {
            output.push_str(&format!("... and {} more tags.", tags.len() - 50));
        }

        Ok(output)
    }
}

impl GetAllTags {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

// ==================== GET RECENT NOTES ====================

#[derive(Deserialize)]
pub struct GetRecentNotesArgs {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    10
}

pub struct GetRecentNotes {
    pub db_path: PathBuf,
}

impl Tool for GetRecentNotes {
    const NAME: &'static str = "get_recent_notes";

    type Args = GetRecentNotesArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "get_recent_notes".to_string(),
            description: "Get the most recently modified notes".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of notes to return (default: 10)"
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_path = self.db_path.clone();

        let notes = tokio::task::spawn_blocking(
            move || -> anyhow::Result<Vec<crate::core::database::NoteMetadata>> {
                let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
                let mut all_notes = db.list_notes(None).map_err(|e| anyhow::anyhow!(e))?;
                all_notes.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                all_notes.truncate(args.limit);
                Ok(all_notes)
            },
        )
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        if notes.is_empty() {
            return Ok("No notes found.".to_string());
        }

        let mut output = format!("Recent notes ({}):\n", notes.len());
        for note in notes {
            output.push_str(&format!("- {}\n", note.name));
        }

        Ok(output)
    }
}

impl GetRecentNotes {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}
