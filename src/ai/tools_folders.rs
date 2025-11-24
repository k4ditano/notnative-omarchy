//! Herramientas de gestión de carpetas para el agente RIG

use crate::ai::tools::ToolError;
use crate::core::database::NotesDatabase;
use anyhow::Result;
use rig::tool::Tool;
use serde::Deserialize;
use std::path::PathBuf;

// ==================== LIST FOLDERS ====================

#[derive(Deserialize)]
pub struct ListFoldersArgs {}

pub struct ListFolders {
    pub db_path: PathBuf,
    pub notes_dir: PathBuf,
}

impl Tool for ListFolders {
    const NAME: &'static str = "list_folders";

    type Args = ListFoldersArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "list_folders".to_string(),
            description: "List all folders in the notes directory with note counts".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("🔧 [ListFolders] Calling tool...");
        let db_path = self.db_path.clone();
        let notes_dir = self.notes_dir.clone();

        let folders_output = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            // 1. Get note counts from DB
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
            let all_notes = db.list_notes(None).map_err(|e| anyhow::anyhow!(e))?;

            let mut folder_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for note in all_notes {
                if let Some(folder) = note.folder {
                    *folder_counts.entry(folder).or_insert(0) += 1;
                }
            }

            // 2. Scan filesystem for ALL folders (including empty ones)
            let mut all_folders = std::collections::HashSet::new();

            // Add folders known to DB
            for k in folder_counts.keys() {
                all_folders.insert(k.clone());
            }

            // Add folders from disk
            let mut stack = vec![notes_dir.clone()];
            while let Some(current_dir) = stack.pop() {
                if let Ok(entries) = std::fs::read_dir(&current_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            // Ignore hidden folders (starting with .)
                            if path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.starts_with('.'))
                                .unwrap_or(false)
                            {
                                continue;
                            }

                            // Get relative path
                            if let Ok(relative) = path.strip_prefix(&notes_dir) {
                                if let Some(s) = relative.to_str() {
                                    all_folders.insert(s.to_string());
                                    stack.push(path);
                                }
                            }
                        }
                    }
                }
            }

            // 3. Generate sorted output
            let mut sorted_folders: Vec<_> = all_folders.into_iter().collect();
            sorted_folders.sort();

            if sorted_folders.is_empty() {
                return Ok("No folders found. All notes are in the root directory.".to_string());
            }

            let mut output = format!("Folders ({} total):\n", sorted_folders.len());
            for folder in &sorted_folders {
                let count = folder_counts.get(folder).unwrap_or(&0);

                // Check if it has subfolders
                let folder_slash = format!("{}/", folder);
                let has_subfolders = sorted_folders.iter().any(|f| f.starts_with(&folder_slash));

                let sub_info = if has_subfolders {
                    " [HAS SUBFOLDERS]"
                } else {
                    ""
                };

                output.push_str(&format!("- {} ({} notes){}\n", folder, count, sub_info));
            }

            Ok(output)
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(folders_output)
    }
}

impl ListFolders {
    pub fn new(db_path: PathBuf, notes_dir: PathBuf) -> Self {
        Self { db_path, notes_dir }
    }
}

// ==================== CREATE FOLDER ====================

#[derive(Deserialize)]
pub struct CreateFolderArgs {
    pub path: String,
}

pub struct CreateFolder {
    pub notes_dir: PathBuf,
}

impl Tool for CreateFolder {
    const NAME: &'static str = "create_folder";

    type Args = CreateFolderArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "create_folder".to_string(),
            description: "Create a new folder in the notes directory. Supports nested paths like 'Projects/Work/2024'".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The folder path to create (e.g., 'Work' or 'Projects/Personal')"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!("🔧 [CreateFolder] Creating folder: {}", args.path);
        let notes_dir = self.notes_dir.clone();

        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let folder_path = notes_dir.join(&args.path);

            if folder_path.exists() {
                return Err(anyhow::anyhow!("Folder '{}' already exists", args.path));
            }

            std::fs::create_dir_all(&folder_path)
                .map_err(|e| anyhow::anyhow!("Failed to create folder: {}", e))?;

            Ok(format!("Folder '{}' created successfully", args.path))
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(result)
    }
}

impl CreateFolder {
    pub fn new(notes_dir: PathBuf) -> Self {
        Self { notes_dir }
    }
}

// ==================== MOVE NOTE ====================

#[derive(Deserialize)]
pub struct MoveNoteArgs {
    pub name: String,
    pub folder: String,
}

pub struct MoveNote {
    pub db_path: PathBuf,
    pub notes_dir: PathBuf,
}

impl Tool for MoveNote {
    const NAME: &'static str = "move_note";

    type Args = MoveNoteArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "move_note".to_string(),
            description: "Move a note to a different folder. Use empty string for root folder."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name of the note to move"
                    },
                    "folder": {
                        "type": "string",
                        "description": "The destination folder path (use '' for root)"
                    }
                },
                "required": ["name", "folder"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "🔧 [MoveNote] Moving note '{}' to '{}'",
            args.name, args.folder
        );
        let db_path = self.db_path.clone();
        let notes_dir = self.notes_dir.clone();

        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
            let metadata = db.get_note(&args.name).map_err(|e| anyhow::anyhow!(e))?;

            if let Some(meta) = metadata {
                let new_dir = if args.folder.is_empty() {
                    notes_dir.clone()
                } else {
                    notes_dir.join(&args.folder)
                };

                // Create folder if it doesn't exist
                if !new_dir.exists() {
                    std::fs::create_dir_all(&new_dir)
                        .map_err(|e| anyhow::anyhow!("Failed to create folder: {}", e))?;
                }

                let new_path = new_dir.join(format!("{}.md", args.name));

                if new_path.exists() {
                    return Err(anyhow::anyhow!(
                        "A note named '{}' already exists in folder '{}'",
                        args.name,
                        args.folder
                    ));
                }

                // Move the file
                std::fs::rename(&meta.path, &new_path)
                    .map_err(|e| anyhow::anyhow!("Failed to move note: {}", e))?;

                // Update database
                let content = std::fs::read_to_string(&new_path).map_err(|e| anyhow::anyhow!(e))?;

                let new_folder = if args.folder.is_empty() {
                    None
                } else {
                    Some(args.folder.as_str())
                };
                db.index_note(&args.name, new_path.to_str().unwrap(), &content, new_folder)
                    .map_err(|e| anyhow::anyhow!(e))?;

                Ok(format!(
                    "Note '{}' moved to folder '{}'",
                    args.name, args.folder
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

impl MoveNote {
    pub fn new(db_path: PathBuf, notes_dir: PathBuf) -> Self {
        Self { db_path, notes_dir }
    }
}

// ==================== RENAME NOTE ====================

#[derive(Deserialize)]
pub struct RenameNoteArgs {
    pub old_name: String,
    pub new_name: String,
}

pub struct RenameNote {
    pub db_path: PathBuf,
}

impl Tool for RenameNote {
    const NAME: &'static str = "rename_note";

    type Args = RenameNoteArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "rename_note".to_string(),
            description: "Rename a note. The note stays in the same folder.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "old_name": {
                        "type": "string",
                        "description": "Current name of the note"
                    },
                    "new_name": {
                        "type": "string",
                        "description": "New name for the note"
                    }
                },
                "required": ["old_name", "new_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let db_path = self.db_path.clone();

        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
            let metadata = db
                .get_note(&args.old_name)
                .map_err(|e| anyhow::anyhow!(e))?;

            if let Some(meta) = metadata {
                // Check if new name already exists
                if db
                    .get_note(&args.new_name)
                    .map_err(|e| anyhow::anyhow!(e))?
                    .is_some()
                {
                    return Err(anyhow::anyhow!(
                        "A note named '{}' already exists",
                        args.new_name
                    ));
                }

                let note_path = PathBuf::from(&meta.path);
                let parent_dir = note_path
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
                let new_path = parent_dir.join(format!("{}.md", args.new_name));

                // Rename the file
                std::fs::rename(&meta.path, &new_path)
                    .map_err(|e| anyhow::anyhow!("Failed to rename note: {}", e))?;

                // Update database - delete old, insert new
                db.delete_note(&args.old_name)
                    .map_err(|e| anyhow::anyhow!(e))?;

                let content = std::fs::read_to_string(&new_path).map_err(|e| anyhow::anyhow!(e))?;

                db.index_note(
                    &args.new_name,
                    new_path.to_str().unwrap(),
                    &content,
                    meta.folder.as_deref(),
                )
                .map_err(|e| anyhow::anyhow!(e))?;

                Ok(format!(
                    "Note renamed from '{}' to '{}'",
                    args.old_name, args.new_name
                ))
            } else {
                Err(anyhow::anyhow!("Note '{}' not found", args.old_name))
            }
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(result)
    }
}

impl RenameNote {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

// ==================== BATCH RENAME NOTES ====================

#[derive(Deserialize)]
pub struct RenamePair {
    pub old_name: String,
    pub new_name: String,
}

#[derive(Deserialize)]
pub struct BatchRenameNotesArgs {
    pub renames: Vec<RenamePair>,
}

pub struct BatchRenameNotes {
    pub db_path: PathBuf,
}

impl Tool for BatchRenameNotes {
    const NAME: &'static str = "batch_rename_notes";

    type Args = BatchRenameNotesArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "batch_rename_notes".to_string(),
            description: "Rename multiple notes at once.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "renames": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "old_name": { "type": "string", "description": "Current name of the note" },
                                "new_name": { "type": "string", "description": "New name for the note" }
                            },
                            "required": ["old_name", "new_name"]
                        },
                        "description": "List of rename pairs"
                    }
                },
                "required": ["renames"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "🔧 [BatchRenameNotes] Renaming {} notes",
            args.renames.len()
        );
        let db_path = self.db_path.clone();

        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;

            let mut renamed_count = 0;
            let mut errors = Vec::new();

            for pair in args.renames {
                let old_name = pair.old_name;
                let new_name = pair.new_name;

                // Check if new name already exists
                if let Ok(Some(_)) = db.get_note(&new_name) {
                    errors.push(format!(
                        "Cannot rename '{}' to '{}': Target already exists",
                        old_name, new_name
                    ));
                    continue;
                }

                if let Ok(Some(meta)) = db.get_note(&old_name) {
                    let note_path = PathBuf::from(&meta.path);
                    let parent_dir = match note_path.parent() {
                        Some(p) => p,
                        None => {
                            errors.push(format!("Invalid path for note '{}'", old_name));
                            continue;
                        }
                    };
                    let new_path = parent_dir.join(format!("{}.md", new_name));

                    // Rename the file
                    if let Err(e) = std::fs::rename(&meta.path, &new_path) {
                        errors.push(format!("Failed to rename '{}': {}", old_name, e));
                        continue;
                    }

                    // Update database - delete old, insert new
                    if let Err(e) = db.delete_note(&old_name) {
                        errors.push(format!(
                            "Failed to delete old index for '{}': {}",
                            old_name, e
                        ));
                        // Try to revert file rename? Too risky/complex for now.
                        continue;
                    }

                    let content = match std::fs::read_to_string(&new_path) {
                        Ok(c) => c,
                        Err(e) => {
                            errors.push(format!("Failed to read new file '{}': {}", new_name, e));
                            continue;
                        }
                    };

                    if let Err(e) = db.index_note(
                        &new_name,
                        new_path.to_str().unwrap(),
                        &content,
                        meta.folder.as_deref(),
                    ) {
                        errors.push(format!("Failed to index new note '{}': {}", new_name, e));
                    } else {
                        renamed_count += 1;
                    }
                } else {
                    errors.push(format!("Note '{}' not found", old_name));
                }
            }

            let mut output = format!("Successfully renamed {} notes.", renamed_count);
            if !errors.is_empty() {
                output.push_str("\n\nErrors encountered:\n");
                for error in errors {
                    output.push_str(&format!("- {}\n", error));
                }
            }

            Ok(output)
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(result)
    }
}

impl BatchRenameNotes {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

// ==================== BATCH MOVE NOTES ====================

#[derive(Deserialize)]
pub struct BatchMoveNotesArgs {
    pub notes: Vec<String>,
    pub folder: String,
}

pub struct BatchMoveNotes {
    pub db_path: PathBuf,
    pub notes_dir: PathBuf,
}

impl Tool for BatchMoveNotes {
    const NAME: &'static str = "batch_move_notes";

    type Args = BatchMoveNotesArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "batch_move_notes".to_string(),
            description: "Move multiple notes to a specific folder at once.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "notes": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of note names to move"
                    },
                    "folder": {
                        "type": "string",
                        "description": "The destination folder path (use '' for root)"
                    }
                },
                "required": ["notes", "folder"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "🔧 [BatchMoveNotes] Moving {} notes to '{}'",
            args.notes.len(),
            args.folder
        );
        let db_path = self.db_path.clone();
        let notes_dir = self.notes_dir.clone();

        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;

            let new_dir = if args.folder.is_empty() {
                notes_dir.clone()
            } else {
                notes_dir.join(&args.folder)
            };

            // Create folder if it doesn't exist
            if !new_dir.exists() {
                std::fs::create_dir_all(&new_dir)
                    .map_err(|e| anyhow::anyhow!("Failed to create folder: {}", e))?;
            }

            let mut moved_count = 0;
            let mut errors = Vec::new();

            for note_name in args.notes {
                if let Ok(Some(meta)) = db.get_note(&note_name) {
                    let new_path = new_dir.join(format!("{}.md", note_name));

                    if new_path.exists() {
                        errors.push(format!("Note '{}' already exists in target folder", note_name));
                        continue;
                    }

                    // Move the file
                    if let Err(e) = std::fs::rename(&meta.path, &new_path) {
                        errors.push(format!("Failed to move '{}': {}", note_name, e));
                        continue;
                    }

                    // Update database
                    if let Ok(content) = std::fs::read_to_string(&new_path) {
                        let new_folder = if args.folder.is_empty() {
                            None
                        } else {
                            Some(args.folder.as_str())
                        };

                        if let Err(e) = db.index_note(&note_name, new_path.to_str().unwrap(), &content, new_folder) {
                            errors.push(format!("Failed to update DB for '{}': {}", note_name, e));
                        } else {
                            moved_count += 1;
                        }
                    } else {
                        errors.push(format!("Failed to read moved note '{}'", note_name));
                    }
                } else {
                    errors.push(format!("Note '{}' not found", note_name));
                }
            }

            let mut output = format!("Successfully moved {} notes to '{}'.", moved_count, args.folder);
            if !errors.is_empty() {
                output.push_str("\n\nErrors encountered:\n");
                for error in errors {
                    output.push_str(&format!("- {}\n", error));
                }
            } else {
                output.push_str("\n\nNEXT STEP: If you have finished organizing, provide a final summary of your actions.");
            }

            Ok(output)
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(result)
    }
}

impl BatchMoveNotes {
    pub fn new(db_path: PathBuf, notes_dir: PathBuf) -> Self {
        Self { db_path, notes_dir }
    }
}

// ==================== BATCH CREATE FOLDERS ====================

#[derive(Deserialize)]
pub struct BatchCreateFoldersArgs {
    pub folders: Vec<String>,
}

pub struct BatchCreateFolders {
    pub notes_dir: PathBuf,
}

impl Tool for BatchCreateFolders {
    const NAME: &'static str = "batch_create_folders";

    type Args = BatchCreateFoldersArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "batch_create_folders".to_string(),
            description: "Create multiple folders at once.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "folders": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of folder paths to create"
                    }
                },
                "required": ["folders"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "🔧 [BatchCreateFolders] Creating {} folders",
            args.folders.len()
        );
        let notes_dir = self.notes_dir.clone();

        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let mut created_count = 0;
            let mut errors = Vec::new();

            for folder in args.folders {
                let folder_path = notes_dir.join(&folder);

                if folder_path.exists() {
                    // Not an error, just skip
                    continue;
                }

                if let Err(e) = std::fs::create_dir_all(&folder_path) {
                    errors.push(format!("Failed to create '{}': {}", folder, e));
                } else {
                    created_count += 1;
                }
            }

            let mut output = format!("Successfully created {} folders.", created_count);
            if !errors.is_empty() {
                output.push_str("\n\nErrors encountered:\n");
                for error in errors {
                    output.push_str(&format!("- {}\n", error));
                }
            } else {
                output.push_str("\n\nNEXT STEP: Now you MUST proceed to move the notes using `batch_move_notes`.");
            }

            Ok(output)
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(result)
    }
}

impl BatchCreateFolders {
    pub fn new(notes_dir: PathBuf) -> Self {
        Self { notes_dir }
    }
}

// ==================== DELETE FOLDER ====================

#[derive(Deserialize)]
pub struct DeleteFolderArgs {
    pub path: String,
    pub recursive: bool,
    pub force: Option<bool>,
}

pub struct DeleteFolder {
    pub db_path: PathBuf,
    pub notes_dir: PathBuf,
}

impl Tool for DeleteFolder {
    const NAME: &'static str = "delete_folder";

    type Args = DeleteFolderArgs;
    type Output = String;
    type Error = ToolError;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: "delete_folder".to_string(),
            description: "Delete a folder. Use recursive=true to delete non-empty folders."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The folder path to delete"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Whether to delete the folder even if it contains files/subfolders"
                    },
                    "force": {
                        "type": "boolean",
                        "description": "Force deletion even if the folder contains notes (default: false)"
                    }
                },
                "required": ["path", "recursive"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "🔧 [DeleteFolder] Deleting folder '{}' (recursive: {}, force: {:?})",
            args.path, args.recursive, args.force
        );
        let db_path = self.db_path.clone();
        let notes_dir = self.notes_dir.clone();

        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let folder_path = notes_dir.join(&args.path);

            if !folder_path.exists() {
                return Err(anyhow::anyhow!("Folder '{}' does not exist", args.path));
            }

            if args.recursive {
                // SAFETY CHECK: Check for notes in this folder or subfolders
                let db = NotesDatabase::new(&db_path).map_err(|e| anyhow::anyhow!(e))?;
                let all_notes = db.list_notes(None).map_err(|e| anyhow::anyhow!(e))?;

                let folder_prefix = if args.path.ends_with('/') {
                    args.path.clone()
                } else {
                    format!("{}/", args.path)
                };

                let notes_in_folder_count = all_notes.iter().filter(|n| {
                    if let Some(folder) = &n.folder {
                        folder == &args.path || folder.starts_with(&folder_prefix)
                    } else {
                        false
                    }
                }).count();

                if notes_in_folder_count > 0 && !args.force.unwrap_or(false) {
                    return Err(anyhow::anyhow!(
                        "SAFETY ERROR: Folder '{}' contains {} notes in subfolders. Use force=true to delete anyway.",
                        args.path,
                        notes_in_folder_count
                    ));
                }

                // If we are here, either no notes or force=true.
                // We should delete notes from DB if we are forcing.
                if notes_in_folder_count > 0 {
                     for note in all_notes {
                        if let Some(folder) = &note.folder {
                            if folder == &args.path || folder.starts_with(&folder_prefix) {
                                db.delete_note(&note.name).map_err(|e| anyhow::anyhow!(e))?;
                            }
                        }
                    }
                }

                std::fs::remove_dir_all(&folder_path)
                    .map_err(|e| anyhow::anyhow!("Failed to delete folder recursively: {}", e))?;

                Ok(format!("Folder '{}' deleted recursively. Removed {} notes from index.", args.path, notes_in_folder_count))
            } else {
                std::fs::remove_dir(&folder_path)
                    .map_err(|e| anyhow::anyhow!("Failed to delete folder (not empty?): {}", e))?;
                Ok(format!("Folder '{}' deleted successfully.", args.path))
            }
        })
        .await
        .map_err(|e| ToolError(e.to_string()))??;

        Ok(result)
    }
}

impl DeleteFolder {
    pub fn new(db_path: PathBuf, notes_dir: PathBuf) -> Self {
        Self { db_path, notes_dir }
    }
}
