use anyhow::Result;
use serde_json::json;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::core::{NotesDatabase, NotesDirectory};
use crate::mcp::tools::{MCPToolCall, MCPToolResult};

/// Ejecutor de herramientas MCP
#[derive(Debug)]
pub struct MCPToolExecutor {
    notes_dir: NotesDirectory,
    notes_db: Rc<RefCell<NotesDatabase>>,
}

impl MCPToolExecutor {
    pub fn new(notes_dir: NotesDirectory, notes_db: Rc<RefCell<NotesDatabase>>) -> Self {
        Self {
            notes_dir,
            notes_db,
        }
    }

    /// Ejecuta una llamada de herramienta y devuelve el resultado
    pub fn execute(&self, tool: MCPToolCall) -> Result<MCPToolResult> {
        match tool {
            // === Gestión de notas ===
            MCPToolCall::CreateNote { name, content, folder } => {
                self.create_note(&name, &content, folder.as_deref())
            }
            MCPToolCall::ReadNote { name } => {
                self.read_note(&name)
            }
            MCPToolCall::UpdateNote { name, content } => {
                self.update_note(&name, &content)
            }
            MCPToolCall::AppendToNote { name, content } => {
                self.append_to_note(&name, &content)
            }
            MCPToolCall::DeleteNote { name } => {
                self.delete_note(&name)
            }
            MCPToolCall::ListNotes { folder } => {
                self.list_notes(folder.as_deref())
            }
            MCPToolCall::RenameNote { old_name, new_name } => {
                self.rename_note(&old_name, &new_name)
            }
            MCPToolCall::DuplicateNote { name, new_name } => {
                self.duplicate_note(&name, &new_name)
            }

            // === Búsqueda ===
            MCPToolCall::SearchNotes { query } => {
                self.search_notes(&query)
            }
            MCPToolCall::GetNotesWithTag { tag } => {
                self.get_notes_with_tag(&tag)
            }
            MCPToolCall::FuzzySearch { query, limit } => {
                self.fuzzy_search(&query, limit)
            }
            MCPToolCall::GetRecentNotes { limit } => {
                self.get_recent_notes(limit)
            }

            // === Análisis ===
            MCPToolCall::AnalyzeNoteStructure { name } => {
                self.analyze_note_structure(&name)
            }
            MCPToolCall::GetWordCount { name } => {
                self.get_word_count(&name)
            }
            MCPToolCall::SuggestRelatedNotes { name, limit } => {
                self.suggest_related_notes(&name, limit)
            }
            MCPToolCall::GetAllTags => {
                self.get_all_tags()
            }

            // === Transformaciones ===
            MCPToolCall::GenerateTableOfContents { name, max_level } => {
                self.generate_table_of_contents(&name, max_level)
            }
            MCPToolCall::ExtractCodeBlocks { name, language } => {
                self.extract_code_blocks(&name, language.as_deref())
            }
            MCPToolCall::MergeNotes { note_names, output_name } => {
                self.merge_notes(&note_names, &output_name)
            }

            // === Organización ===
            MCPToolCall::CreateFolder { name, parent } => {
                self.create_folder(&name, parent.as_deref())
            }
            MCPToolCall::ListFolders => {
                self.list_folders()
            }
            MCPToolCall::MoveNote { name, folder } => {
                self.move_note(&name, &folder)
            }

            // === Automatización ===
            MCPToolCall::CreateDailyNote { template } => {
                self.create_daily_note(template.as_deref())
            }
            MCPToolCall::FindAndReplace { find, replace, note_names } => {
                self.find_and_replace(&find, &replace, note_names.as_deref())
            }

            // === Sistema ===
            MCPToolCall::GetAppInfo => {
                self.get_app_info()
            }
            MCPToolCall::GetWorkspacePath => {
                self.get_workspace_path()
            }

            // === UI - Estos necesitan comunicación con la app ===
            MCPToolCall::OpenNote { .. }
            | MCPToolCall::ShowNotification { .. }
            | MCPToolCall::HighlightNote { .. }
            | MCPToolCall::ToggleSidebar
            | MCPToolCall::SwitchMode { .. }
            | MCPToolCall::RefreshSidebar
            | MCPToolCall::FocusSearch => {
                Ok(MCPToolResult::error(
                    "Herramienta de UI requiere canal de comunicación con la app (pendiente de implementar)".to_string()
                ))
            }

            // === No implementadas aún ===
            _ => Ok(MCPToolResult::error(
                "Herramienta no implementada todavía".to_string()
            )),
        }
    }

    // ==================== Implementaciones ====================

    fn create_note(
        &self,
        name: &str,
        content: &str,
        folder: Option<&str>,
    ) -> Result<MCPToolResult> {
        // Quitar extensión .md si ya existe (create_note la agrega automáticamente)
        let clean_name = name.strip_suffix(".md").unwrap_or(name);

        match self.notes_dir.create_note(clean_name, content) {
            Ok(note) => {
                // Indexar en BD
                if let Err(e) = self.notes_db.borrow().index_note(
                    clean_name,
                    note.path().to_str().unwrap_or(""),
                    content,
                    folder,
                ) {
                    eprintln!("Error indexando nota: {}", e);
                }

                Ok(MCPToolResult::success(json!({
                    "note_name": note.name(),
                    "message": format!("✓ Nota '{}' creada exitosamente", note.name()),
                    "path": note.path().to_str().unwrap_or("")
                })))
            }
            Err(e) => Ok(MCPToolResult::error(format!(
                "Error creando nota '{}': {}",
                name, e
            ))),
        }
    }

    fn read_note(&self, name: &str) -> Result<MCPToolResult> {
        match self.notes_dir.find_note(name) {
            Ok(Some(note)) => match note.read() {
                Ok(content) => Ok(MCPToolResult::success(json!({
                    "name": name,
                    "content": content,
                    "size": content.len(),
                    "message": format!("✓ Nota '{}' leída correctamente", name)
                }))),
                Err(e) => Ok(MCPToolResult::error(format!(
                    "Error leyendo nota '{}': {}",
                    name, e
                ))),
            },
            Ok(None) => Ok(MCPToolResult::error(format!(
                "Nota '{}' no encontrada",
                name
            ))),
            Err(e) => Ok(MCPToolResult::error(format!(
                "Error buscando nota '{}': {}",
                name, e
            ))),
        }
    }

    fn update_note(&self, name: &str, content: &str) -> Result<MCPToolResult> {
        match self.notes_dir.find_note(name) {
            Ok(Some(note)) => {
                match note.write(content) {
                    Ok(_) => {
                        // Reindexar en BD
                        if let Err(e) = self.notes_db.borrow().index_note(
                            name,
                            note.path().to_str().unwrap_or(""),
                            content,
                            None,
                        ) {
                            eprintln!("Error reindexando nota: {}", e);
                        }

                        Ok(MCPToolResult::success(json!({
                            "note_name": name,
                            "message": format!("✓ Nota '{}' actualizada exitosamente", name),
                            "size": content.len()
                        })))
                    }
                    Err(e) => Ok(MCPToolResult::error(format!(
                        "Error escribiendo nota '{}': {}",
                        name, e
                    ))),
                }
            }
            Ok(None) => Ok(MCPToolResult::error(format!(
                "Nota '{}' no encontrada",
                name
            ))),
            Err(e) => Ok(MCPToolResult::error(format!(
                "Error buscando nota '{}': {}",
                name, e
            ))),
        }
    }

    fn append_to_note(&self, name: &str, content: &str) -> Result<MCPToolResult> {
        match self.notes_dir.find_note(name) {
            Ok(Some(note)) => {
                // Leer contenido actual
                match note.read() {
                    Ok(current_content) => {
                        // Agregar nuevo contenido al final
                        let new_content = if current_content.is_empty() {
                            content.to_string()
                        } else {
                            format!("{}\n\n{}", current_content, content)
                        };

                        // Escribir contenido actualizado
                        match note.write(&new_content) {
                            Ok(_) => {
                                // Reindexar en BD
                                if let Err(e) = self.notes_db.borrow().index_note(
                                    name,
                                    note.path().to_str().unwrap_or(""),
                                    &new_content,
                                    None,
                                ) {
                                    eprintln!("Error reindexando nota: {}", e);
                                }

                                Ok(MCPToolResult::success(json!({
                                    "note_name": name,
                                    "message": format!("✓ Contenido agregado a '{}' exitosamente", name),
                                    "new_size": new_content.len(),
                                    "appended_chars": content.len()
                                })))
                            }
                            Err(e) => Ok(MCPToolResult::error(format!(
                                "Error escribiendo nota '{}': {}",
                                name, e
                            ))),
                        }
                    }
                    Err(e) => Ok(MCPToolResult::error(format!(
                        "Error leyendo nota '{}': {}",
                        name, e
                    ))),
                }
            }
            Ok(None) => Ok(MCPToolResult::error(format!(
                "Nota '{}' no encontrada",
                name
            ))),
            Err(e) => Ok(MCPToolResult::error(format!(
                "Error buscando nota '{}': {}",
                name, e
            ))),
        }
    }

    fn delete_note(&self, name: &str) -> Result<MCPToolResult> {
        match self.notes_dir.find_note(name) {
            Ok(Some(note)) => {
                match std::fs::remove_file(note.path()) {
                    Ok(_) => {
                        // Eliminar de BD
                        if let Err(e) = self.notes_db.borrow().delete_note(name) {
                            eprintln!("Error eliminando nota de BD: {}", e);
                        }

                        Ok(MCPToolResult::success(json!({
                            "note_name": name,
                            "message": format!("✓ Nota '{}' eliminada exitosamente", name)
                        })))
                    }
                    Err(e) => Ok(MCPToolResult::error(format!(
                        "Error eliminando nota '{}': {}",
                        name, e
                    ))),
                }
            }
            Ok(None) => Ok(MCPToolResult::error(format!(
                "Nota '{}' no encontrada",
                name
            ))),
            Err(e) => Ok(MCPToolResult::error(format!(
                "Error buscando nota '{}': {}",
                name, e
            ))),
        }
    }

    fn list_notes(&self, folder: Option<&str>) -> Result<MCPToolResult> {
        let notes = self.notes_dir.list_notes()?;

        let note_names: Vec<String> = notes
            .into_iter()
            .map(|note| note.name().to_string())
            .filter(|name| {
                if let Some(folder_name) = folder {
                    name.starts_with(&format!("{}/", folder_name))
                } else {
                    true
                }
            })
            .collect();

        Ok(MCPToolResult::success(json!({
            "notes": note_names,
            "count": note_names.len(),
            "message": format!("✓ {} notas encontradas", note_names.len())
        })))
    }

    fn search_notes(&self, query: &str) -> Result<MCPToolResult> {
        match self.notes_db.borrow().search_notes(query) {
            Ok(results) => {
                let note_names: Vec<String> = results.iter().map(|m| m.note_name.clone()).collect();

                Ok(MCPToolResult::success(json!({
                    "results": note_names,
                    "count": results.len(),
                    "query": query,
                    "message": format!("✓ {} resultados para '{}'", results.len(), query)
                })))
            }
            Err(e) => Ok(MCPToolResult::error(format!("Error buscando notas: {}", e))),
        }
    }

    fn get_notes_with_tag(&self, tag: &str) -> Result<MCPToolResult> {
        // Buscar usando el tag como query
        let query = format!("#{}", tag);
        match self.notes_db.borrow().search_notes(&query) {
            Ok(results) => {
                let note_names: Vec<String> = results.iter().map(|m| m.note_name.clone()).collect();

                Ok(MCPToolResult::success(json!({
                    "notes": note_names,
                    "count": results.len(),
                    "tag": tag,
                    "message": format!("✓ {} notas con tag #{}", results.len(), tag)
                })))
            }
            Err(e) => Ok(MCPToolResult::error(format!(
                "Error obteniendo notas con tag: {}",
                e
            ))),
        }
    }

    fn get_all_tags(&self) -> Result<MCPToolResult> {
        match self.notes_db.borrow().get_tags() {
            Ok(tags) => {
                let tag_names: Vec<String> = tags.iter().map(|t| t.name.clone()).collect();

                Ok(MCPToolResult::success(json!({
                    "tags": tag_names,
                    "count": tag_names.len(),
                    "message": format!("✓ {} tags encontrados", tag_names.len())
                })))
            }
            Err(e) => Ok(MCPToolResult::error(format!(
                "Error obteniendo tags: {}",
                e
            ))),
        }
    }

    fn create_folder(&self, name: &str, parent: Option<&str>) -> Result<MCPToolResult> {
        let folder_path = if let Some(parent_name) = parent {
            self.notes_dir.root().join(parent_name).join(name)
        } else {
            self.notes_dir.root().join(name)
        };

        match std::fs::create_dir_all(&folder_path) {
            Ok(_) => Ok(MCPToolResult::success(json!({
                "folder_name": name,
                "path": folder_path.to_str().unwrap_or(""),
                "message": format!("✓ Carpeta '{}' creada exitosamente", name)
            }))),
            Err(e) => Ok(MCPToolResult::error(format!(
                "Error creando carpeta '{}': {}",
                name, e
            ))),
        }
    }

    fn list_folders(&self) -> Result<MCPToolResult> {
        let base_path = self.notes_dir.root();
        let mut folders = Vec::new();

        if let Ok(entries) = std::fs::read_dir(base_path) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            folders.push(name.to_string());
                        }
                    }
                }
            }
        }

        folders.sort();

        Ok(MCPToolResult::success(json!({
            "folders": folders,
            "count": folders.len(),
            "message": format!("✓ {} carpetas encontradas", folders.len())
        })))
    }

    // === Nuevas funciones ===

    fn rename_note(&self, old_name: &str, new_name: &str) -> Result<MCPToolResult> {
        let note = self
            .notes_dir
            .find_note(old_name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;
        let old_path = note.path();

        let new_path = if let Some(parent) = old_path.parent() {
            parent.join(new_name)
        } else {
            PathBuf::from(new_name)
        };

        std::fs::rename(&old_path, &new_path)?;

        Ok(MCPToolResult::success(json!({
            "message": format!("Nota renombrada de '{}' a '{}'", old_name, new_name),
            "old_name": old_name,
            "new_name": new_name
        })))
    }

    fn duplicate_note(&self, name: &str, new_name: &str) -> Result<MCPToolResult> {
        let note = self
            .notes_dir
            .find_note(name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;
        let content = note.read()?;

        self.create_note(new_name, &content, None)
    }

    fn fuzzy_search(&self, query: &str, limit: Option<i32>) -> Result<MCPToolResult> {
        let limit = limit.unwrap_or(10) as usize;
        let query_lower = query.to_lowercase();

        let mut results: Vec<_> = self
            .notes_dir
            .list_notes()?
            .iter()
            .filter_map(|note| {
                let name = note.name().to_lowercase();
                let matches: usize = query_lower.chars().filter(|c| name.contains(*c)).count();

                if matches > 0 {
                    Some((note.name().to_string(), matches))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.truncate(limit);

        let results: Vec<String> = results.into_iter().map(|(name, _)| name).collect();

        Ok(MCPToolResult::success(json!({
            "query": query,
            "results": results,
            "count": results.len()
        })))
    }

    fn get_recent_notes(&self, limit: Option<i32>) -> Result<MCPToolResult> {
        let limit = limit.unwrap_or(10) as usize;

        let mut notes: Vec<_> = self
            .notes_dir
            .list_notes()?
            .iter()
            .filter_map(|note| {
                let metadata = std::fs::metadata(note.path()).ok()?;
                let modified = metadata.modified().ok()?;
                Some((note.name().to_string(), modified))
            })
            .collect();

        notes.sort_by(|a, b| b.1.cmp(&a.1));
        notes.truncate(limit);

        let results: Vec<String> = notes.into_iter().map(|(name, _)| name).collect();

        Ok(MCPToolResult::success(json!({
            "notes": results,
            "count": results.len()
        })))
    }

    fn analyze_note_structure(&self, name: &str) -> Result<MCPToolResult> {
        let note = self
            .notes_dir
            .find_note(name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;
        let content = note.read()?;

        let lines: Vec<&str> = content.lines().collect();
        let words: usize = content.split_whitespace().count();
        let chars = content.len();

        let headings: Vec<&str> = lines
            .iter()
            .filter(|line| line.starts_with('#'))
            .copied()
            .collect();

        let code_blocks = content.matches("```").count() / 2;
        let links = content.matches("](").count();

        Ok(MCPToolResult::success(json!({
            "name": name,
            "lines": lines.len(),
            "words": words,
            "chars": chars,
            "headings": headings.len(),
            "heading_preview": headings.iter().take(5).collect::<Vec<_>>(),
            "code_blocks": code_blocks,
            "links": links
        })))
    }

    fn get_word_count(&self, name: &str) -> Result<MCPToolResult> {
        let note = self
            .notes_dir
            .find_note(name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;
        let content = note.read()?;
        let word_count = content.split_whitespace().count();

        Ok(MCPToolResult::success(json!({
            "name": name,
            "word_count": word_count,
            "char_count": content.len(),
            "line_count": content.lines().count()
        })))
    }

    fn suggest_related_notes(&self, name: &str, limit: Option<i32>) -> Result<MCPToolResult> {
        let limit = limit.unwrap_or(5) as usize;
        let note = self
            .notes_dir
            .find_note(name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;
        let content = note.read()?;

        let keywords: Vec<String> = content
            .split_whitespace()
            .filter(|w| w.len() > 4)
            .map(|w| w.to_lowercase())
            .collect();

        if keywords.is_empty() {
            return Ok(MCPToolResult::success(json!({
                "related_notes": []
            })));
        }

        let notes = self.notes_dir.list_notes()?;
        let mut scores: Vec<_> = notes
            .iter()
            .filter(|n| n.name() != name)
            .filter_map(|other_note| {
                let other_content = other_note.read().ok()?;
                let other_lower = other_content.to_lowercase();

                let matches = keywords
                    .iter()
                    .filter(|kw| other_lower.contains(kw.as_str()))
                    .count();

                if matches > 0 {
                    Some((other_note.name(), matches))
                } else {
                    None
                }
            })
            .collect();

        scores.sort_by(|a, b| b.1.cmp(&a.1));
        scores.truncate(limit);

        let related: Vec<String> = scores
            .into_iter()
            .map(|(name, _)| name.to_string())
            .collect();

        Ok(MCPToolResult::success(json!({
            "note": name,
            "related_notes": related
        })))
    }

    fn generate_table_of_contents(
        &self,
        name: &str,
        max_level: Option<i32>,
    ) -> Result<MCPToolResult> {
        let max_level = max_level.unwrap_or(3);
        let note = self
            .notes_dir
            .find_note(name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;
        let content = note.read()?;

        let mut toc = String::new();
        toc.push_str("## Tabla de Contenidos\n\n");

        for line in content.lines() {
            if line.starts_with('#') {
                let level = line.chars().take_while(|c| *c == '#').count();
                if level as i32 <= max_level {
                    let title = line.trim_start_matches('#').trim();
                    let indent = "  ".repeat(level - 1);
                    let anchor = title.to_lowercase().replace(' ', "-");
                    toc.push_str(&format!("{}- [{}](#{})\n", indent, title, anchor));
                }
            }
        }

        Ok(MCPToolResult::success(json!({
            "name": name,
            "toc": toc
        })))
    }

    fn extract_code_blocks(&self, name: &str, language: Option<&str>) -> Result<MCPToolResult> {
        let note = self
            .notes_dir
            .find_note(name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;
        let content = note.read()?;

        let mut blocks = Vec::new();
        let mut in_code_block = false;
        let mut current_block = String::new();
        let mut current_lang = String::new();

        for line in content.lines() {
            if line.starts_with("```") {
                if in_code_block {
                    if language.is_none() || language == Some(&current_lang) {
                        blocks.push(json!({
                            "language": current_lang,
                            "code": current_block.trim()
                        }));
                    }
                    current_block.clear();
                    current_lang.clear();
                    in_code_block = false;
                } else {
                    current_lang = line.trim_start_matches("```").trim().to_string();
                    in_code_block = true;
                }
            } else if in_code_block {
                current_block.push_str(line);
                current_block.push('\n');
            }
        }

        Ok(MCPToolResult::success(json!({
            "name": name,
            "blocks": blocks,
            "count": blocks.len()
        })))
    }

    fn merge_notes(&self, note_names: &[String], output_name: &str) -> Result<MCPToolResult> {
        let mut merged_content = String::new();

        for name in note_names {
            let note = self
                .notes_dir
                .find_note(name)?
                .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;
            let content = note.read()?;

            merged_content.push_str(&format!("# {}\n\n", name));
            merged_content.push_str(&content);
            merged_content.push_str("\n\n---\n\n");
        }

        self.create_note(output_name, &merged_content, None)
    }

    fn move_note(&self, name: &str, folder: &str) -> Result<MCPToolResult> {
        let note = self
            .notes_dir
            .find_note(name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;
        let old_path = note.path();

        let folder_path = self.notes_dir.root().join(folder);
        std::fs::create_dir_all(&folder_path)?;

        let new_path = folder_path.join(name);
        std::fs::rename(&old_path, &new_path)?;

        Ok(MCPToolResult::success(json!({
            "message": format!("Nota '{}' movida a '{}'", name, folder),
            "name": name,
            "folder": folder
        })))
    }

    fn create_daily_note(&self, template: Option<&str>) -> Result<MCPToolResult> {
        use chrono::Local;

        let today = Local::now().format("%Y-%m-%d").to_string();
        let name = format!("{}.md", today);

        let content = if let Some(tmpl) = template {
            tmpl.replace("{date}", &today)
        } else {
            format!(
                "# Daily Note - {}\n\n## Tareas\n\n- [ ] \n\n## Notas\n\n",
                today
            )
        };

        self.create_note(&name, &content, None)
    }

    fn find_and_replace(
        &self,
        find: &str,
        replace: &str,
        note_names: Option<&[String]>,
    ) -> Result<MCPToolResult> {
        let notes_to_process: Vec<_> = if let Some(names) = note_names {
            names
                .iter()
                .filter_map(|name| self.notes_dir.find_note(name).ok().flatten())
                .collect()
        } else {
            self.notes_dir.list_notes()?
        };

        let mut updated = Vec::new();

        for note in notes_to_process {
            let content = note.read()?;
            if content.contains(find) {
                let new_content = content.replace(find, replace);
                note.write(&new_content)?;
                updated.push(note.name().to_string());
            }
        }

        Ok(MCPToolResult::success(json!({
            "find": find,
            "replace": replace,
            "updated_notes": updated,
            "count": updated.len()
        })))
    }

    fn get_app_info(&self) -> Result<MCPToolResult> {
        Ok(MCPToolResult::success(json!({
            "name": "NotNative",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Aplicación de notas minimalista con IA",
            "mcp_version": "1.0.0"
        })))
    }

    fn get_workspace_path(&self) -> Result<MCPToolResult> {
        Ok(MCPToolResult::success(json!({
            "path": self.notes_dir.root().display().to_string()
        })))
    }
}
