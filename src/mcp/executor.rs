use anyhow::Result;
use serde_json::json;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::core::{NotesConfig, NotesDatabase, NotesDirectory};
use crate::i18n::I18n;
use crate::mcp::tools::{MCPToolCall, MCPToolResult};

/// Ejecutor de herramientas MCP
#[derive(Debug, Clone)]
pub struct MCPToolExecutor {
    notes_dir: NotesDirectory,
    notes_db: Rc<RefCell<NotesDatabase>>,
    notes_config: Rc<RefCell<NotesConfig>>,
    i18n: Rc<RefCell<I18n>>,
}

impl MCPToolExecutor {
    pub fn new(
        notes_dir: NotesDirectory,
        notes_db: Rc<RefCell<NotesDatabase>>,
        notes_config: Rc<RefCell<NotesConfig>>,
        i18n: Rc<RefCell<I18n>>,
    ) -> Self {
        Self {
            notes_dir,
            notes_db,
            notes_config,
            i18n,
        }
    }

    /// Ejecuta una llamada de herramienta y devuelve el resultado
    pub fn execute(&self, tool: MCPToolCall) -> Result<MCPToolResult> {
        match tool {
            // === Gestión de notas ===
            MCPToolCall::CreateNote {
                name,
                content,
                folder,
            } => self.create_note(&name, &content, folder.as_deref()),
            MCPToolCall::ReadNote { name } => self.read_note(&name),
            MCPToolCall::UpdateNote { name, content } => self.update_note(&name, &content),
            MCPToolCall::AppendToNote { name, content } => self.append_to_note(&name, &content),
            MCPToolCall::DeleteNote { name } => self.delete_note(&name),
            MCPToolCall::ListNotes { folder } => self.list_notes(folder.as_deref()),
            MCPToolCall::RenameNote { old_name, new_name } => {
                self.rename_note(&old_name, &new_name)
            }
            MCPToolCall::DuplicateNote { name, new_name } => self.duplicate_note(&name, &new_name),

            // === Búsqueda ===
            MCPToolCall::SearchNotes { query } => self.search_notes(&query),
            MCPToolCall::GetNotesWithTag { tag } => self.get_notes_with_tag(&tag),
            MCPToolCall::FuzzySearch { query, limit } => self.fuzzy_search(&query, limit),
            MCPToolCall::GetRecentNotes { limit } => self.get_recent_notes(limit),

            // === Análisis ===
            MCPToolCall::AnalyzeNoteStructure { name } => self.analyze_note_structure(&name),
            MCPToolCall::GetWordCount { name } => self.get_word_count(&name),
            MCPToolCall::SuggestRelatedNotes { name, limit } => {
                self.suggest_related_notes(&name, limit)
            }
            MCPToolCall::GetAllTags { .. } => self.get_all_tags(),

            // === Transformaciones ===
            MCPToolCall::GenerateTableOfContents { name, max_level } => {
                self.generate_table_of_contents(&name, max_level)
            }
            MCPToolCall::ExtractCodeBlocks { name, language } => {
                self.extract_code_blocks(&name, language.as_deref())
            }
            MCPToolCall::MergeNotes {
                note_names,
                output_name,
            } => self.merge_notes(&note_names, &output_name),

            // === Organización ===
            MCPToolCall::CreateFolder { name, parent } => {
                self.create_folder(&name, parent.as_deref())
            }
            MCPToolCall::DeleteFolder { name, recursive } => {
                self.delete_folder(&name, recursive.unwrap_or(false))
            }
            MCPToolCall::RenameFolder { old_name, new_name } => {
                self.rename_folder(&old_name, &new_name)
            }
            MCPToolCall::MoveFolder { name, new_parent } => {
                self.move_folder(&name, new_parent.as_deref())
            }
            MCPToolCall::ListFolders { .. } => self.list_folders(),
            MCPToolCall::FindEmptyItems { item_type } => {
                self.find_empty_items(item_type.as_deref())
            }
            MCPToolCall::GetSystemDateTime { .. } => self.get_system_datetime(),
            MCPToolCall::MoveNote { name, folder } => self.move_note(&name, &folder),
            MCPToolCall::AddTag { note, tag } => self.add_tag(&note, &tag),
            MCPToolCall::RemoveTag { note, tag } => self.remove_tag(&note, &tag),
            MCPToolCall::CreateTag { tag } => self.create_tag(&tag),
            MCPToolCall::AddMultipleTags { note, tags } => self.add_multiple_tags(&note, &tags),
            MCPToolCall::AnalyzeAndTagNote { name, max_tags } => {
                self.analyze_and_tag_note(&name, max_tags.unwrap_or(5))
            }

            // === Automatización ===
            MCPToolCall::CreateDailyNote { template } => {
                self.create_daily_note(template.as_deref())
            }
            MCPToolCall::FindAndReplace {
                find,
                replace,
                note_names,
            } => self.find_and_replace(&find, &replace, note_names.as_deref()),

            // === Sistema ===
            MCPToolCall::GetAppInfo => self.get_app_info(),
            MCPToolCall::GetWorkspacePath => self.get_workspace_path(),

            // === Semantic Search ===
            MCPToolCall::SemanticSearch {
                query,
                limit,
                min_similarity,
                folder,
            } => self.semantic_search(query, limit, min_similarity, folder),
            MCPToolCall::FindSimilarNotes {
                note_path,
                limit,
                min_similarity,
            } => self.find_similar_notes(note_path, limit, min_similarity),
            MCPToolCall::GetEmbeddingStats => self.get_embedding_stats(),
            MCPToolCall::IndexNote { note_path } => self.index_note(note_path),
            MCPToolCall::ReindexAllNotes => self.reindex_all_notes(),

            // === Recordatorios ===
            MCPToolCall::CreateReminder {
                title,
                due_date,
                description,
                priority,
                repeat,
                note_name,
            } => self.create_reminder(
                &title,
                &due_date,
                description,
                priority,
                repeat.as_deref(),
                note_name.as_deref(),
            ),

            MCPToolCall::ListReminders { status, limit } => {
                self.list_reminders(status.as_deref(), limit)
            }

            MCPToolCall::CompleteReminder { id } => self.complete_reminder(id),

            MCPToolCall::SnoozeReminder { id, minutes } => self.snooze_reminder(id, minutes as i64),

            MCPToolCall::DeleteReminder { id } => self.delete_reminder(id),

            // === UI - DESHABILITADAS (pendiente de implementar) ===
            // MCPToolCall::OpenNote { .. }
            // | MCPToolCall::ShowNotification { .. }
            // | MCPToolCall::HighlightNote { .. }
            // | MCPToolCall::ToggleSidebar
            // | MCPToolCall::SwitchMode { .. }
            // | MCPToolCall::RefreshSidebar
            // | MCPToolCall::FocusSearch => {
            //     Ok(MCPToolResult::error(
            //         "Herramienta de UI requiere canal de comunicación con la app (pendiente de implementar)".to_string()
            //     ))
            // }

            // === No implementadas aún ===
            _ => Ok(MCPToolResult::error(
                "Herramienta no implementada todavía".to_string(),
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

        // Si el nombre ya contiene la ruta de carpeta (ej: "test/nota"), extraer solo el nombre
        let (actual_folder, base_name) = if clean_name.contains('/') {
            let parts: Vec<&str> = clean_name.rsplitn(2, '/').collect();
            if parts.len() == 2 {
                (Some(parts[1]), parts[0])
            } else {
                (None, clean_name)
            }
        } else {
            (None, clean_name)
        };

        // Determinar la carpeta final: priorizar la extraída del nombre
        let final_folder = actual_folder.or(folder);

        // Si se especifica una carpeta, crear el archivo directamente en esa carpeta
        let file_path = if let Some(folder_name) = final_folder {
            // Asegurar que la carpeta existe
            let folder_path = self.notes_dir.root().join(folder_name);
            std::fs::create_dir_all(&folder_path)?;

            // Crear ruta completa al archivo usando solo el nombre base
            folder_path.join(format!("{}.md", base_name))
        } else {
            self.notes_dir.root().join(format!("{}.md", base_name))
        };

        // Escribir el contenido directamente
        std::fs::write(&file_path, content)?;

        // Indexar en BD (el nombre debe ser solo el base_name, sin carpeta)
        if let Err(e) = self.notes_db.borrow().index_note(
            base_name,
            file_path.to_str().unwrap_or(""),
            content,
            final_folder,
        ) {
            eprintln!("Error indexando nota: {}", e);
        }

        Ok(MCPToolResult::success(json!({
            "note_name": base_name,
            "message": self.i18n.borrow().t("mcp_note_created").replace("{}", base_name),
            "path": file_path.to_str().unwrap_or("")
        })))
    }

    fn read_note(&self, name: &str) -> Result<MCPToolResult> {
        match self.notes_dir.find_note(name) {
            Ok(Some(note)) => match note.read() {
                Ok(content) => Ok(MCPToolResult::success(json!({
                    "note_name": name,
                    "content": content,
                    "message": self.i18n.borrow().t("mcp_note_read").replace("{}", name)
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
        // Primero intentar encontrar la nota normalmente
        let note_result = self.notes_dir.find_note(name);

        // Si no se encuentra y el nombre no tiene ruta, buscar en carpetas conocidas
        let note_to_update = if note_result.as_ref().ok().and_then(|n| n.as_ref()).is_none()
            && !name.contains('/')
        {
            // Intentar buscar en carpetas comunes
            let folders = vec!["Docs VS", "Desarrollo", "Internet"];
            let mut found = None;

            for folder in folders {
                let folder_path = self
                    .notes_dir
                    .root()
                    .join(folder)
                    .join(format!("{}.md", name));
                if folder_path.exists() {
                    found = Some(folder_path);
                    break;
                }
            }

            if let Some(path) = found {
                // Actualizar directamente el archivo
                std::fs::write(&path, content)?;

                return Ok(MCPToolResult::success(json!({
                    "note_name": name,
                    "message": self.i18n.borrow().t("mcp_note_updated").replace("{}", name),
                    "size": content.len(),
                    "path": path.to_str().unwrap_or("")
                })));
            } else {
                return Ok(MCPToolResult::error(format!(
                    "Nota '{}' no encontrada",
                    name
                )));
            }
        } else {
            note_result
        };

        match note_to_update {
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
                            "message": self.i18n.borrow().t("mcp_note_updated").replace("{}", name),
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
                                    "message": self.i18n.borrow().t("mcp_content_appended").replace("{}", name),
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
                            "message": self.i18n.borrow().t("mcp_note_deleted").replace("{}", name)
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
            "message": self.i18n.borrow().t("mcp_notes_found").replace("{}", &note_names.len().to_string())
        })))
    }

    fn search_notes(&self, query: &str) -> Result<MCPToolResult> {
        // Estrategia de búsqueda mejorada:
        // 1. Búsqueda fuzzy en nombres de archivos
        // 2. Si hay pocos resultados, también buscar en contenido (FTS)
        // 3. Combinar y rankear resultados

        let query_lower = query.to_lowercase();
        let mut combined_results: std::collections::HashMap<String, f32> =
            std::collections::HashMap::new();

        // 1. Búsqueda fuzzy en nombres (peso alto)
        let notes = self.notes_dir.list_notes()?;
        for note in &notes {
            let name = note.name();
            let name_lower = name.to_lowercase();

            // Puntuación por coincidencia exacta
            if name_lower == query_lower {
                combined_results.insert(name.to_string(), 100.0);
                continue;
            }

            // Puntuación por contener el query
            if name_lower.contains(&query_lower) {
                let score = 50.0 + (query_lower.len() as f32 / name_lower.len() as f32) * 30.0;
                combined_results.insert(name.to_string(), score);
                continue;
            }

            // Puntuación fuzzy: contar caracteres que coinciden en orden
            let mut query_chars = query_lower.chars();
            let mut current_char = query_chars.next();
            let mut matches = 0;

            for name_char in name_lower.chars() {
                if let Some(qc) = current_char {
                    if name_char == qc {
                        matches += 1;
                        current_char = query_chars.next();
                    }
                }
            }

            if matches > 0 {
                let score = (matches as f32 / query_lower.len() as f32) * 40.0;
                combined_results.insert(name.to_string(), score);
            }
        }

        // 2. Si hay menos de 5 resultados, buscar también en contenido
        if combined_results.len() < 5 {
            match self.notes_db.borrow().search_notes(query) {
                Ok(fts_results) => {
                    for result in fts_results {
                        // Agregar con peso menor si no está ya
                        combined_results
                            .entry(result.note_name.clone())
                            .or_insert_with(|| {
                                // Convertir relevancia de FTS a puntuación (normalizar)
                                20.0 + (result.relevance.abs() * 10.0).min(30.0)
                            });
                    }
                }
                Err(e) => {
                    eprintln!("Error en búsqueda FTS: {}", e);
                }
            }
        }

        // 3. Ordenar por puntuación
        let mut results: Vec<_> = combined_results.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Limitar a top 20
        let note_names: Vec<String> = results
            .into_iter()
            .take(20)
            .map(|(name, _score)| name)
            .collect();

        Ok(MCPToolResult::success(json!({
            "results": note_names,
            "count": note_names.len(),
            "query": query,
            "message": self.i18n.borrow().t("mcp_search_results")
                .replace("{}", &note_names.len().to_string())
                .replace("{}", query)
        })))
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
                    "message": self.i18n.borrow().t("mcp_notes_with_tag").replace("{}", &results.len().to_string()).replace("{}", tag)
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
                    "message": self.i18n.borrow().t("mcp_tags_found").replace("{}", &tag_names.len().to_string())
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
                "message": self.i18n.borrow().t("mcp_folder_created").replace("{}", name)
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
            "message": self.i18n.borrow().t("mcp_folders_found").replace("{}", &folders.len().to_string())
        })))
    }

    fn find_empty_items(&self, item_type: Option<&str>) -> Result<MCPToolResult> {
        let search_type = item_type.unwrap_or("all");
        let base_path = self.notes_dir.root();

        let mut empty_notes = Vec::new();
        let mut empty_folders = Vec::new();

        // Función recursiva para explorar directorios
        fn scan_directory(
            path: &std::path::Path,
            base: &std::path::Path,
            search_type: &str,
            empty_notes: &mut Vec<String>,
            empty_folders: &mut Vec<String>,
        ) -> Result<()> {
            if let Ok(entries) = std::fs::read_dir(path) {
                let mut folder_has_content = false;

                for entry in entries.flatten() {
                    let entry_path = entry.path();

                    if entry_path.is_dir() {
                        // Recursivamente escanear subcarpeta
                        scan_directory(&entry_path, base, search_type, empty_notes, empty_folders)?;
                        folder_has_content = true;
                    } else if entry_path.is_file() {
                        folder_has_content = true;

                        // Verificar si es una nota vacía
                        if (search_type == "notes" || search_type == "all")
                            && entry_path.extension().and_then(|s| s.to_str()) == Some("md")
                        {
                            if let Ok(content) = std::fs::read_to_string(&entry_path) {
                                let trimmed = content.trim();
                                if trimmed.is_empty()
                                    || trimmed == "# "
                                    || trimmed.starts_with("# \n")
                                {
                                    if let Ok(relative) = entry_path.strip_prefix(base) {
                                        if let Some(name) = relative.to_str() {
                                            empty_notes
                                                .push(name.trim_end_matches(".md").to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Si la carpeta no tiene contenido y no es la raíz
                if !folder_has_content
                    && path != base
                    && (search_type == "folders" || search_type == "all")
                {
                    if let Ok(relative) = path.strip_prefix(base) {
                        if let Some(name) = relative.to_str() {
                            empty_folders.push(name.to_string());
                        }
                    }
                }
            }
            Ok(())
        }

        scan_directory(
            base_path,
            base_path,
            search_type,
            &mut empty_notes,
            &mut empty_folders,
        )?;

        let total = empty_notes.len() + empty_folders.len();
        let message = match search_type {
            "notes" => format!("✓ {} empty notes found", empty_notes.len()),
            "folders" => format!("✓ {} empty folders found", empty_folders.len()),
            _ => format!(
                "✓ {} empty items found ({} notes, {} folders)",
                total,
                empty_notes.len(),
                empty_folders.len()
            ),
        };

        Ok(MCPToolResult::success(json!({
            "empty_notes": empty_notes,
            "empty_folders": empty_folders,
            "total": total,
            "message": message
        })))
    }

    fn get_system_datetime(&self) -> Result<MCPToolResult> {
        use chrono::{Datelike, Local, Timelike};

        let now = Local::now();

        // Obtener nombre del día de la semana en español
        let weekday = match now.weekday() {
            chrono::Weekday::Mon => "Lunes",
            chrono::Weekday::Tue => "Martes",
            chrono::Weekday::Wed => "Miércoles",
            chrono::Weekday::Thu => "Jueves",
            chrono::Weekday::Fri => "Viernes",
            chrono::Weekday::Sat => "Sábado",
            chrono::Weekday::Sun => "Domingo",
        };

        // Obtener nombre del mes en español
        let month = match now.month() {
            1 => "Enero",
            2 => "Febrero",
            3 => "Marzo",
            4 => "Abril",
            5 => "Mayo",
            6 => "Junio",
            7 => "Julio",
            8 => "Agosto",
            9 => "Septiembre",
            10 => "Octubre",
            11 => "Noviembre",
            12 => "Diciembre",
            _ => "Desconocido",
        };

        Ok(MCPToolResult::success(json!({
            "date": now.format("%Y-%m-%d").to_string(),
            "time": now.format("%H:%M:%S").to_string(),
            "datetime": now.format("%Y-%m-%d %H:%M:%S").to_string(),
            "datetime_readable": format!("{}, {} de {} de {} a las {}:{:02}",
                weekday, now.day(), month, now.year(), now.hour(), now.minute()),
            "weekday": weekday,
            "day": now.day(),
            "month": now.month(),
            "month_name": month,
            "year": now.year(),
            "hour": now.hour(),
            "minute": now.minute(),
            "second": now.second(),
            "timezone": now.format("%Z").to_string(),
            "timezone_offset": now.format("%:z").to_string(),
            "timestamp": now.timestamp(),
            "message": format!("✓ Fecha y hora del sistema: {}, {} de {} de {} a las {}:{:02}",
                weekday, now.day(), month, now.year(), now.hour(), now.minute())
        })))
    }

    // === Nuevas funciones ===

    fn rename_note(&self, old_name: &str, new_name: &str) -> Result<MCPToolResult> {
        let note = self
            .notes_dir
            .find_note(old_name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;
        let old_path = note.path();

        // Obtener el directorio donde está la nota
        let parent_dir = old_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("No se pudo obtener el directorio padre"))?;

        // Generar nombre único si ya existe
        let unique_new_name = self.generate_unique_filename(parent_dir, new_name);

        let new_path = parent_dir.join(&unique_new_name);

        std::fs::rename(&old_path, &new_path)?;

        // Actualizar en la base de datos
        let content = std::fs::read_to_string(&new_path)?;
        let folder = old_path
            .parent()
            .and_then(|p| p.strip_prefix(self.notes_dir.root()).ok())
            .filter(|p| !p.as_os_str().is_empty())
            .and_then(|p| p.to_str());

        // Extraer nombre sin extensión para la BD
        let db_name = unique_new_name.trim_end_matches(".md");

        if let Err(e) = self.notes_db.borrow().index_note(
            db_name,
            new_path.to_str().unwrap_or(""),
            &content,
            folder,
        ) {
            eprintln!("⚠️ Error actualizando BD después de renombrar nota: {}", e);
        }

        let result_message = if unique_new_name != new_name {
            format!(
                "Nota renombrada de '{}' a '{}' (el nombre '{}' ya existía)",
                old_name,
                db_name,
                new_name.trim_end_matches(".md")
            )
        } else {
            format!("Nota renombrada de '{}' a '{}'", old_name, db_name)
        };

        Ok(MCPToolResult::success(json!({
            "message": result_message,
            "old_name": old_name,
            "new_name": db_name
        })))
    }

    /// Genera un nombre de archivo único verificando si ya existe
    /// y añadiendo (1), (2), etc. si es necesario
    fn generate_unique_filename(&self, dir: &std::path::Path, base_name: &str) -> String {
        // Asegurar que tiene extensión .md
        let with_ext = if base_name.ends_with(".md") {
            base_name.to_string()
        } else {
            format!("{}.md", base_name)
        };

        let base_path = dir.join(&with_ext);
        if !base_path.exists() {
            return with_ext;
        }

        // Extraer nombre sin extensión
        let name_without_ext = base_name.trim_end_matches(".md");

        // Si existe, buscar el primer número disponible
        for i in 1..1000 {
            let new_name = format!("{} ({}).md", name_without_ext, i);
            let new_path = dir.join(&new_name);
            if !new_path.exists() {
                return new_name;
            }
        }

        // Si llegamos aquí (muy improbable), usar timestamp
        format!(
            "{} ({}).md",
            name_without_ext,
            chrono::Local::now().timestamp()
        )
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

        let new_path = folder_path.join(format!("{}.md", name.trim_end_matches(".md")));
        std::fs::rename(&old_path, &new_path)?;

        // Actualizar en la base de datos
        let content = std::fs::read_to_string(&new_path)?;
        let new_name = format!("{}/{}", folder, name.trim_end_matches(".md"));

        if let Err(e) = self.notes_db.borrow().index_note(
            &new_name,
            new_path.to_str().unwrap_or(""),
            &content,
            Some(folder),
        ) {
            eprintln!("⚠️ Error actualizando BD después de mover nota: {}", e);
        }

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Nota '{}' movida a carpeta '{}'", name, folder),
            "name": name,
            "folder": folder,
            "new_path": new_path.display().to_string()
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

    // ==================== Gestión de Carpetas ====================

    fn delete_folder(&self, name: &str, recursive: bool) -> Result<MCPToolResult> {
        let folder_path = self.notes_dir.root().join(name);

        if !folder_path.exists() {
            return Ok(MCPToolResult::error(format!(
                "La carpeta '{}' no existe",
                name
            )));
        }

        if !folder_path.is_dir() {
            return Ok(MCPToolResult::error(format!(
                "'{}' no es una carpeta",
                name
            )));
        }

        // Verificar si está vacía si recursive = false
        if !recursive {
            let entries = std::fs::read_dir(&folder_path)?;
            if entries.count() > 0 {
                return Ok(MCPToolResult::error(format!(
                    "La carpeta '{}' no está vacía. Usa recursive=true para eliminar con contenido",
                    name
                )));
            }
        }

        // Eliminar todas las notas de esta carpeta de la BD primero
        if recursive {
            if let Err(e) = self.notes_db.borrow().delete_notes_in_folder(name) {
                eprintln!("⚠️ Error eliminando notas de BD: {}", e);
            }
        }

        // Eliminar carpeta del filesystem
        if recursive {
            std::fs::remove_dir_all(&folder_path)?;
        } else {
            std::fs::remove_dir(&folder_path)?;
        }

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Carpeta '{}' eliminada", name),
            "folder_name": name,
            "recursive": recursive
        })))
    }

    fn rename_folder(&self, old_name: &str, new_name: &str) -> Result<MCPToolResult> {
        let old_path = self.notes_dir.root().join(old_name);
        let new_path = self.notes_dir.root().join(new_name);

        if !old_path.exists() {
            return Ok(MCPToolResult::error(format!(
                "La carpeta '{}' no existe",
                old_name
            )));
        }

        if !old_path.is_dir() {
            return Ok(MCPToolResult::error(format!(
                "'{}' no es una carpeta",
                old_name
            )));
        }

        if new_path.exists() {
            return Ok(MCPToolResult::error(format!(
                "Ya existe una carpeta llamada '{}'",
                new_name
            )));
        }

        // Renombrar en el filesystem
        std::fs::rename(&old_path, &new_path)?;

        // Actualizar todas las notas de esta carpeta en la BD
        if let Err(e) = self
            .notes_db
            .borrow()
            .update_notes_folder(old_name, new_name)
        {
            eprintln!("⚠️ Error actualizando carpeta en BD: {}", e);
        }

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Carpeta renombrada: '{}' → '{}'", old_name, new_name),
            "old_name": old_name,
            "new_name": new_name
        })))
    }

    fn move_folder(&self, name: &str, new_parent: Option<&str>) -> Result<MCPToolResult> {
        let old_path = self.notes_dir.root().join(name);

        if !old_path.exists() {
            return Ok(MCPToolResult::error(format!(
                "La carpeta '{}' no existe",
                name
            )));
        }

        if !old_path.is_dir() {
            return Ok(MCPToolResult::error(format!(
                "'{}' no es una carpeta",
                name
            )));
        }

        // Calcular nuevo path
        let folder_name = old_path.file_name().unwrap().to_string_lossy();
        let new_path = if let Some(parent) = new_parent {
            let parent_path = self.notes_dir.root().join(parent);
            if !parent_path.exists() {
                return Ok(MCPToolResult::error(format!(
                    "La carpeta padre '{}' no existe",
                    parent
                )));
            }
            parent_path.join(folder_name.as_ref())
        } else {
            self.notes_dir.root().join(folder_name.as_ref())
        };

        if new_path.exists() {
            return Ok(MCPToolResult::error(format!(
                "Ya existe una carpeta en el destino: {}",
                new_path.display()
            )));
        }

        // Calcular el nuevo path relativo para la BD
        let new_folder_path = if let Some(parent) = new_parent {
            format!("{}/{}", parent, folder_name)
        } else {
            folder_name.to_string()
        };

        // Mover en el filesystem
        std::fs::rename(&old_path, &new_path)?;

        // Actualizar todas las notas de esta carpeta en la BD
        if let Err(e) = self
            .notes_db
            .borrow()
            .update_notes_folder(name, &new_folder_path)
        {
            eprintln!("⚠️ Error actualizando carpeta en BD: {}", e);
        }

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Carpeta '{}' movida a '{}'", name, new_parent.unwrap_or("raíz")),
            "folder_name": name,
            "new_parent": new_parent,
            "new_path": new_path.display().to_string()
        })))
    }

    // ==================== Gestión de Tags ====================

    fn add_tag(&self, note_name: &str, tag: &str) -> Result<MCPToolResult> {
        let note = self
            .notes_dir
            .find_note(note_name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;

        let mut content = note.read()?;

        // Buscar sección de frontmatter
        if content.starts_with("---\n") {
            // Ya tiene frontmatter, buscar tags:
            if let Some(end_idx) = content[4..].find("---\n") {
                let frontmatter = &content[4..end_idx + 4];

                if frontmatter.contains("tags:") {
                    // Agregar a la lista existente
                    let lines: Vec<&str> = frontmatter.lines().collect();
                    let mut new_frontmatter = String::new();
                    let mut in_tags = false;

                    for line in lines {
                        if line.starts_with("tags:") {
                            new_frontmatter.push_str(line);
                            new_frontmatter.push('\n');
                            in_tags = true;
                        } else if in_tags
                            && (line.starts_with("  - ") || line.starts_with("    - "))
                        {
                            new_frontmatter.push_str(line);
                            new_frontmatter.push('\n');
                        } else {
                            if in_tags {
                                // Agregar nuevo tag
                                new_frontmatter.push_str(&format!("  - {}\n", tag));
                                in_tags = false;
                            }
                            new_frontmatter.push_str(line);
                            new_frontmatter.push('\n');
                        }
                    }

                    if in_tags {
                        new_frontmatter.push_str(&format!("  - {}\n", tag));
                    }

                    content = format!("---\n{}---\n{}", new_frontmatter, &content[end_idx + 8..]);
                } else {
                    // Agregar campo tags al frontmatter
                    let rest = &content[end_idx + 8..];
                    content = format!("---\n{}tags:\n  - {}\n---\n{}", frontmatter, tag, rest);
                }
            }
        } else {
            // No tiene frontmatter, crearlo
            content = format!("---\ntags:\n  - {}\n---\n\n{}", tag, content);
        }

        note.write(&content)?;

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Tag '{}' añadido a '{}'", tag, note_name),
            "note_name": note_name,
            "tag": tag
        })))
    }

    fn remove_tag(&self, note_name: &str, tag: &str) -> Result<MCPToolResult> {
        let note = self
            .notes_dir
            .find_note(note_name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;

        let content = note.read()?;

        // Simple approach: eliminar línea que contiene el tag
        let new_content = content
            .lines()
            .filter(|line| !line.trim().eq(&format!("- {}", tag)))
            .collect::<Vec<_>>()
            .join("\n");

        note.write(&new_content)?;

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Tag '{}' eliminado de '{}'", tag, note_name),
            "note_name": note_name,
            "tag": tag
        })))
    }

    fn create_tag(&self, tag: &str) -> Result<MCPToolResult> {
        // Los tags en markdown no se "crean" por sí solos, solo existen cuando se usan
        // Esta función es principalmente informativa
        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Tag '{}' listo para usar", tag),
            "tag": tag,
            "info": "Los tags se crean automáticamente al añadirlos a una nota"
        })))
    }

    fn add_multiple_tags(&self, note_name: &str, tags: &[String]) -> Result<MCPToolResult> {
        let mut added = Vec::new();
        let mut failed = Vec::new();

        for tag in tags {
            match self.add_tag(note_name, tag) {
                Ok(_) => added.push(tag.clone()),
                Err(e) => failed.push(format!("{}: {}", tag, e)),
            }
        }

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ {} tags añadidos a '{}'", added.len(), note_name),
            "note_name": note_name,
            "added_tags": added,
            "failed_tags": failed,
            "total": tags.len()
        })))
    }

    fn analyze_and_tag_note(&self, name: &str, max_tags: i32) -> Result<MCPToolResult> {
        let note = self
            .notes_dir
            .find_note(name)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada"))?;

        let content = note.read()?;

        // Análisis simple: extraer palabras clave del contenido
        let mut word_freq: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        // Palabras a ignorar (stop words en español)
        let stop_words = vec![
            "el", "la", "de", "que", "y", "a", "en", "un", "ser", "se", "no", "haber", "por",
            "con", "su", "para", "como", "estar", "tener", "le", "lo", "todo", "pero", "más",
            "hacer", "o", "poder", "decir", "este", "ir", "otro", "ese", "si", "me", "ya", "ver",
            "porque", "dar", "cuando", "él", "muy", "sin", "vez", "mucho", "saber", "qué", "sobre",
            "mi", "alguno", "mismo", "yo", "también", "hasta", "año", "dos", "querer", "entre",
            "así", "primero", "desde", "grande", "eso", "ni", "nos", "llegar", "pasar", "tiempo",
            "ella", "los", "las", "del", "al", "una", "unos", "unas", "sus",
        ];

        for word in content.split_whitespace() {
            let clean = word
                .to_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string();

            if clean.len() > 3 && !stop_words.contains(&clean.as_str()) {
                *word_freq.entry(clean).or_insert(0) += 1;
            }
        }

        // Ordenar por frecuencia
        let mut freq_vec: Vec<_> = word_freq.into_iter().collect();
        freq_vec.sort_by(|a, b| b.1.cmp(&a.1));

        // Tomar los top max_tags
        let suggested_tags: Vec<String> = freq_vec
            .into_iter()
            .take(max_tags as usize)
            .map(|(word, _)| word)
            .collect();

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Analizando '{}' - {} tags sugeridos", name, suggested_tags.len()),
            "note_name": name,
            "suggested_tags": suggested_tags,
            "max_tags": max_tags,
            "info": "Estos son tags sugeridos basados en frecuencia de palabras. Usa 'add_multiple_tags' para aplicarlos."
        })))
    }

    // === Semantic Search Methods ===

    fn semantic_search(
        &self,
        query: String,
        limit: Option<usize>,
        min_similarity: Option<f32>,
        folder: Option<String>,
    ) -> Result<MCPToolResult> {
        use crate::core::{EmbeddingClient, SearchOptions, SemanticSearch};

        // Obtener configuración de embeddings del NotesConfig real
        let mut embedding_config = self.notes_config.borrow().get_embedding_config().clone();

        // Si embeddings no tiene API key, usar la de AI config
        if embedding_config.api_key.is_none() {
            embedding_config.api_key = self.notes_config.borrow().get_ai_config().api_key.clone();
        }

        // Verificar si embeddings está habilitado y configurado
        if !embedding_config.enabled {
            return Ok(MCPToolResult::error(
                "Embeddings no habilitado. Actívalo en Configuración > Embeddings.".to_string(),
            ));
        }

        if !embedding_config.is_valid() {
            return Ok(MCPToolResult::error(
                "Embeddings no configurado correctamente. Verifica la API key en Configuración."
                    .to_string(),
            ));
        }

        let client = EmbeddingClient::new(embedding_config.clone())?;

        // Crear una copia temporal de la base de datos para uso en búsqueda
        let db_path = self.notes_dir.db_path();
        let temp_db = crate::core::NotesDatabase::new(&db_path)?;
        let search_engine = SemanticSearch::new(client, temp_db);

        let threshold_usado = min_similarity.unwrap_or(0.2);
        println!("🎯 Threshold de similitud: {}", threshold_usado);

        let options = SearchOptions {
            limit: limit.unwrap_or(10),
            min_similarity: threshold_usado,
            folder_filter: folder,
            ..Default::default()
        };

        // Ejecutar búsqueda semántica (async -> sync boundary)
        let results = tokio::runtime::Runtime::new()?
            .block_on(async { search_engine.search(&query, options).await })?;

        if results.is_empty() {
            return Ok(MCPToolResult::success(json!({
                "message": format!("No se encontraron notas similares a '{}'", query),
                "query": query,
                "results": [],
                "total": 0
            })));
        }

        let results_json: Vec<_> = results
            .iter()
            .map(|r| {
                // Extraer solo el nombre del archivo sin la ruta completa
                let note_name = r
                    .note_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("desconocido");

                json!({
                    "note_name": note_name,
                    "note_path": r.note_path.display().to_string(),
                    "chunk_index": r.chunk_index,
                    "similarity": format!("{:.2}%", r.similarity * 100.0),
                    "snippet": &r.snippet
                })
            })
            .collect();

        // Crear una lista legible de los nombres de notas para la respuesta
        let note_names_list: Vec<String> = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let note_name = r
                    .note_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("desconocido");
                format!(
                    "{}. \"{}\" - Similitud: {:.0}%",
                    i + 1,
                    note_name.trim_end_matches(".md"),
                    r.similarity * 100.0
                )
            })
            .collect();

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ {} resultados para '{}'. RESPONDE AL USUARIO CON ESTA LISTA EXACTA:\n\n{}",
                results.len(),
                query,
                note_names_list.join("\n")),
            "query": query,
            "results": results_json,
            "note_names_list": note_names_list,
            "total": results.len(),
            "instruction": "Muestra al usuario la lista de resultados EXACTAMENTE como aparece en 'note_names_list'. NO inventes notas."
        })))
    }

    fn find_similar_notes(
        &self,
        note_path: String,
        limit: Option<usize>,
        min_similarity: Option<f32>,
    ) -> Result<MCPToolResult> {
        use crate::core::{EmbeddingClient, SearchOptions, SemanticSearch};

        // Obtener configuración de embeddings del NotesConfig real
        let mut embedding_config = self.notes_config.borrow().get_embedding_config().clone();

        // Si embeddings no tiene API key, usar la de AI config
        if embedding_config.api_key.is_none() {
            embedding_config.api_key = self.notes_config.borrow().get_ai_config().api_key.clone();
        }

        if !embedding_config.enabled || !embedding_config.is_valid() {
            return Ok(MCPToolResult::error(
                "Embeddings no configurado. Habilita embeddings en la configuración.".to_string(),
            ));
        }

        let client = EmbeddingClient::new(embedding_config.clone())?;
        let db_path = self.notes_dir.db_path();
        let temp_db = crate::core::NotesDatabase::new(&db_path)?;
        let search_engine = SemanticSearch::new(client, temp_db);

        let options = SearchOptions {
            limit: limit.unwrap_or(10),
            min_similarity: min_similarity.unwrap_or(0.2),
            folder_filter: None,
            ..Default::default()
        };

        // Ejecutar búsqueda de notas similares
        let results = tokio::runtime::Runtime::new()?
            .block_on(async { search_engine.find_similar_notes(&note_path, options).await })?;

        if results.is_empty() {
            return Ok(MCPToolResult::success(json!({
                "message": format!("No se encontraron notas similares a '{}'", note_path),
                "note_path": note_path,
                "results": [],
                "total": 0
            })));
        }

        // Agrupar por nota
        let grouped = SemanticSearch::group_by_note(results);

        let results_json: Vec<_> = grouped
            .iter()
            .map(|(path, chunks)| {
                let avg_similarity =
                    chunks.iter().map(|c| c.similarity).sum::<f32>() / chunks.len() as f32;
                let best_snippet = chunks
                    .iter()
                    .max_by(|a, b| a.similarity.partial_cmp(&b.similarity).unwrap())
                    .map(|c| &c.snippet);

                json!({
                    "note_path": path.display().to_string(),
                    "similarity": chunks[0].similarity,
                    "chunk_count": chunks.len(),
                    "avg_similarity": avg_similarity,
                    "best_snippet": best_snippet
                })
            })
            .collect();

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ {} notas similares a '{}'", grouped.len(), note_path),
            "note_path": note_path,
            "results": results_json,
            "total": grouped.len()
        })))
    }

    fn get_embedding_stats(&self) -> Result<MCPToolResult> {
        // Obtener configuración de embeddings del NotesConfig real
        let embedding_config = self.notes_config.borrow().get_embedding_config().clone();
        let db_ref = self.notes_db.borrow();
        let (notes_count, chunks_count, tokens_count) = db_ref.get_embedding_stats()?;

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Estadísticas del índice de embeddings"),
            "stats": {
                "total_notes": notes_count,
                "total_chunks": chunks_count,
                "total_tokens": tokens_count,
                "model": embedding_config.model,
                "dimension": embedding_config.dimension,
                "max_chunk_tokens": embedding_config.max_chunk_tokens,
                "overlap_tokens": embedding_config.overlap_tokens
            }
        })))
    }

    fn index_note(&self, note_path: String) -> Result<MCPToolResult> {
        use crate::core::{EmbeddingClient, EmbeddingIndexer, TextChunker};

        // Obtener configuración de embeddings del NotesConfig real
        let mut embedding_config = self.notes_config.borrow().get_embedding_config().clone();

        // Si embeddings no tiene API key, usar la de AI config
        if embedding_config.api_key.is_none() {
            embedding_config.api_key = self.notes_config.borrow().get_ai_config().api_key.clone();
        }

        if !embedding_config.enabled || !embedding_config.is_valid() {
            return Ok(MCPToolResult::error(
                "Embeddings no configurado. Habilita embeddings en la configuración y añade una API key.".to_string()
            ));
        }

        // Leer contenido de la nota
        let note = self
            .notes_dir
            .find_note(&note_path)?
            .ok_or_else(|| anyhow::anyhow!("Nota no encontrada: {}", note_path))?;

        let content = note.read()?;
        let client = EmbeddingClient::new(embedding_config.clone())?;
        let db_path = self.notes_dir.db_path();
        let temp_db = crate::core::NotesDatabase::new(&db_path)?;

        let chunker = TextChunker::new();
        let indexer = EmbeddingIndexer::new(client, temp_db, chunker);

        // Indexar nota (async)
        let note_path_buf = std::path::PathBuf::from(&note_path);
        let result = tokio::runtime::Runtime::new()?
            .block_on(async { indexer.index_note(&note_path_buf, &content).await })?;

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Nota '{}' indexada: {} chunks", note_path, result),
            "note_path": note_path,
            "total_chunks": result
        })))
    }

    fn reindex_all_notes(&self) -> Result<MCPToolResult> {
        use crate::core::{EmbeddingClient, EmbeddingIndexer, TextChunker};
        use std::path::PathBuf;

        let mut config = self.notes_config.borrow().get_embedding_config().clone();

        // Si embeddings no tiene API key, usar la de AI config
        if config.api_key.is_none() {
            config.api_key = self.notes_config.borrow().get_ai_config().api_key.clone();
        }

        if !config.is_valid() {
            return Ok(MCPToolResult::error(
                "Embeddings no configurado. Habilita embeddings en la configuración y añade una API key.".to_string()
            ));
        }

        // Obtener todas las notas
        let all_notes = self.notes_dir.list_notes()?;
        let notes_with_content: Vec<(PathBuf, String)> = all_notes
            .iter()
            .filter_map(|note| {
                let content = note.read().ok()?;
                Some((PathBuf::from(&note.name), content))
            })
            .collect();

        let total_notes = notes_with_content.len();
        let client = EmbeddingClient::new(config.clone())?;
        let db_path = self.notes_dir.db_path();
        let temp_db = crate::core::NotesDatabase::new(&db_path)?;

        let chunker = TextChunker::new();
        let indexer = EmbeddingIndexer::new(client, temp_db, chunker);

        // Re-indexar todas las notas
        let result = tokio::runtime::Runtime::new()?
            .block_on(async { indexer.index_notes(notes_with_content, None).await })?;

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Re-indexación completa: {} notas, {} chunks", total_notes, result.total_chunks),
            "total_notes": result.total_notes,
            "total_chunks": result.total_chunks,
            "indexed_notes": result.indexed_notes,
            "errors": result.errors
        })))
    }

    // ==================== Recordatorios ====================

    fn create_reminder(
        &self,
        title: &str,
        due_date: &str,
        description: Option<String>,
        priority: Option<String>,
        repeat_pattern: Option<&str>,
        note_name: Option<&str>,
    ) -> Result<MCPToolResult> {
        use crate::reminders::{Priority, Reminder, ReminderStatus, RepeatPattern};
        use chrono::{DateTime, Utc};

        // Parsear fecha como UTC
        let due_date_parsed = DateTime::parse_from_rfc3339(due_date)
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|_| {
                // Intentar formato más simple: "YYYY-MM-DD HH:MM"
                chrono::NaiveDateTime::parse_from_str(due_date, "%Y-%m-%d %H:%M")
                    .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
            })
            .map_err(|e| {
                anyhow::anyhow!(
                    "Formato de fecha inválido: {}. Use 'YYYY-MM-DD HH:MM' o RFC3339",
                    e
                )
            })?;

        // Parsear prioridad
        let priority_enum = match priority.as_deref() {
            Some("urgent") | Some("urgente") => Priority::Urgent,
            Some("high") | Some("alta") => Priority::High,
            Some("medium") | Some("media") => Priority::Medium,
            Some("low") | Some("baja") => Priority::Low,
            None => Priority::Medium,
            _ => Priority::Medium,
        };

        // Parsear patrón de repetición
        let repeat_enum = match repeat_pattern {
            Some("daily") | Some("diario") => RepeatPattern::Daily,
            Some("weekly") | Some("semanal") => RepeatPattern::Weekly,
            Some("monthly") | Some("mensual") => RepeatPattern::Monthly,
            None => RepeatPattern::None,
            _ => RepeatPattern::None,
        };

        // Buscar note_id si se proporciona note_name
        let note_id = if let Some(name) = note_name {
            // Buscar la nota en la base de datos usando get_note
            match self.notes_db.borrow().get_note(name) {
                Ok(Some(metadata)) => {
                    println!(
                        "✓ Vinculando recordatorio a nota '{}' (ID: {})",
                        name, metadata.id
                    );
                    Some(metadata.id)
                }
                Ok(None) => {
                    println!(
                        "⚠️ Nota '{}' no encontrada, creando recordatorio sin vínculo",
                        name
                    );
                    None
                }
                Err(e) => {
                    println!("⚠️ Error buscando nota '{}': {}", name, e);
                    None
                }
            }
        } else {
            None
        };

        // Crear recordatorio en la base de datos
        let db_path = self.notes_dir.root().parent().unwrap().join("reminders.db");
        let conn = rusqlite::Connection::open(&db_path)?;
        let reminders_db = crate::reminders::ReminderDatabase::new(conn);
        reminders_db.ensure_schema()?;

        let id = reminders_db.create_reminder(
            note_id,
            title,
            description.as_deref(),
            due_date_parsed,
            priority_enum,
            repeat_enum,
        )?;

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Recordatorio '{}' creado (ID: {})", title, id),
            "reminder_id": id,
            "title": title,
            "due_date": due_date_parsed.to_rfc3339(),
            "priority": format!("{:?}", priority_enum),
            "repeat": format!("{:?}", repeat_enum),
            "note_name": note_name,
            "note_id": note_id
        })))
    }

    fn list_reminders(&self, status: Option<&str>, limit: Option<i32>) -> Result<MCPToolResult> {
        use crate::reminders::ReminderStatus;

        let db_path = self.notes_dir.root().parent().unwrap().join("reminders.db");
        let conn = rusqlite::Connection::open(&db_path)?;
        let reminders_db = crate::reminders::ReminderDatabase::new(conn);

        // Convertir status a enum si se proporciona
        let status_filter = status.and_then(|s| {
            if s == "all" {
                None
            } else {
                Some(match s {
                    "pending" | "pendiente" => ReminderStatus::Pending,
                    "completed" | "completado" => ReminderStatus::Completed,
                    "snoozed" | "pospuesto" => ReminderStatus::Snoozed,
                    _ => ReminderStatus::Pending,
                })
            }
        });

        let all_reminders = reminders_db.list_reminders(status_filter)?;

        // Aplicar límite si se especifica
        let mut filtered = all_reminders;
        if let Some(lim) = limit {
            filtered.truncate(lim as usize);
        }

        let is_spanish = self.i18n.borrow().current_language() == crate::i18n::Language::Spanish;

        let reminders_json: Vec<_> = filtered
            .iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "title": r.title,
                    "description": r.description,
                    "due_date": r.format_due_date(is_spanish),
                    "priority": format!("{:?}", r.priority),
                    "status": format!("{:?}", r.status),
                    "repeat": format!("{:?}", r.repeat_pattern),
                    "note_id": r.note_id
                })
            })
            .collect();

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ {} recordatorios encontrados", filtered.len()),
            "reminders": reminders_json,
            "total": filtered.len()
        })))
    }

    fn complete_reminder(&self, id: i64) -> Result<MCPToolResult> {
        use crate::reminders::ReminderStatus;

        let db_path = self.notes_dir.root().parent().unwrap().join("reminders.db");
        let conn = rusqlite::Connection::open(&db_path)?;
        let reminders_db = crate::reminders::ReminderDatabase::new(conn);

        reminders_db.update_status(id, ReminderStatus::Completed)?;

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Recordatorio {} marcado como completado", id),
            "reminder_id": id,
            "status": "completed"
        })))
    }

    fn snooze_reminder(&self, id: i64, minutes: i64) -> Result<MCPToolResult> {
        use chrono::{Duration, Utc};

        let db_path = self.notes_dir.root().parent().unwrap().join("reminders.db");
        let conn = rusqlite::Connection::open(&db_path)?;
        let reminders_db = crate::reminders::ReminderDatabase::new(conn);

        // Calcular la nueva fecha de snooze
        let snooze_until = Utc::now() + Duration::minutes(minutes);
        reminders_db.snooze_reminder(id, snooze_until)?;

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Recordatorio {} pospuesto {} minutos", id, minutes),
            "reminder_id": id,
            "snoozed_minutes": minutes
        })))
    }

    fn delete_reminder(&self, id: i64) -> Result<MCPToolResult> {
        let db_path = self.notes_dir.root().parent().unwrap().join("reminders.db");
        let conn = rusqlite::Connection::open(&db_path)?;
        let reminders_db = crate::reminders::ReminderDatabase::new(conn);

        reminders_db.delete_reminder(id)?;

        Ok(MCPToolResult::success(json!({
            "message": format!("✓ Recordatorio {} eliminado", id),
            "reminder_id": id
        })))
    }
}
