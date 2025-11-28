use crate::ai::memory::NoteMemory;
use crate::ai::rig_adapter::{RigClient, RigClientBackend};
use crate::ai::tools::{
    CreateNote, IndexAllNotes, ListNotes, ReadNote, SearchNotes, SemanticSearch,
};
use crate::ai::tools_analysis::{
    AnalyzeNoteStructure, ExtractCodeBlocks, FuzzySearch, GenerateToc, GetWordCount,
};
use crate::ai::tools_extended::{
    AppendToNote, DeleteNote, GetAllTags, GetNotesWithTag, GetRecentNotes, UpdateNote,
};
use crate::ai::tools_folders::{
    BatchCreateFolders, BatchMoveNotes, BatchRenameNotes, CreateFolder, DeleteFolder, ListFolders,
    MoveNote, RenameNote,
};
use crate::ai::tools_reminders::{CreateReminder, DeleteReminder, ModifyReminder};
use crate::ai::tools_tags::{AddTag, DuplicateNote, MergeNotes, RemoveTag};
use crate::ai::tools_utility::{
    CreateDailyNote, FindAndReplace, GetAppInfo, GetSystemDateTime, GetWorkspacePath,
};
use crate::ai::tools_web::{FetchUrl, WebSearch};
use crate::ai_chat::ChatMessage;
use crate::ai_client::AIClient;
use crate::mcp::MCPToolExecutor;
use anyhow::Result;
use rig::client::CompletionClient;
use rig::client::EmbeddingsClient;
use rig::completion::Prompt;
use rig::providers::openai::EmbeddingModel as OpenAIEmbeddingModel;
use rig::tool::Tool; // Import Tool trait to call .call()
use std::sync::Arc;

pub struct RigExecutor;

impl RigExecutor {
    pub async fn run(
        llm: Arc<dyn AIClient>,
        messages: &[ChatMessage],
        context: &str,
        mcp_executor: &MCPToolExecutor,
    ) -> Result<String> {
        println!("🔧 [RigExecutor::run] Iniciando ejecución");

        // Intentar hacer downcast a RigClient
        let client = llm.as_any().downcast_ref::<RigClient>().ok_or_else(|| {
            anyhow::anyhow!(
                "El ejecutor RIG requiere un RigClient. \
                     Asegúrate de que la feature 'rig-agent' está habilitada \
                     y que estás usando OpenAI como proveedor."
            )
        })?;

        println!("✅ [RigExecutor::run] Cliente RIG obtenido correctamente");

        // Initialize paths
        let db_path = mcp_executor.get_db_path();
        let notes_path = mcp_executor.get_notes_dir().root().to_path_buf();

        // --- PRE-FETCH CONTEXT ---
        // Para evitar que el modelo falle al llamar herramientas iniciales,
        // pre-cargamos la lista de notas y carpetas en el contexto.
        let list_notes_tool = ListNotes::new(db_path.clone());
        let list_folders_tool = ListFolders::new(db_path.clone(), notes_path.clone());

        let notes_context = match list_notes_tool
            .call(crate::ai::tools::ListNotesArgs { folder: None })
            .await
        {
            Ok(list) => format!("Current Notes List:\n{}\n", list),
            Err(_) => "Could not retrieve notes list.\n".to_string(),
        };

        let folders_context = match list_folders_tool
            .call(crate::ai::tools_folders::ListFoldersArgs {})
            .await
        {
            Ok(list) => format!("Current Folders List:\n{}\n", list),
            Err(_) => "Could not retrieve folders list.\n".to_string(),
        };

        let preloaded_context = format!("{}\n{}", notes_context, folders_context);
        println!(
            "📝 [RigExecutor] Contexto pre-cargado: {} caracteres",
            preloaded_context.len()
        );

        // Build the prompt from messages
        let mut prompt = if !context.is_empty() {
            format!("System: {}\n\n", context)
        } else {
            String::new()
        };

        // Add preloaded context to system prompt
        prompt.push_str(&format!("System: {}\n\n", preloaded_context));

        for m in messages {
            match m.role {
                crate::ai_chat::MessageRole::User => {
                    prompt.push_str(&format!("User: {}\n", m.content))
                }
                crate::ai_chat::MessageRole::Assistant => {
                    prompt.push_str(&format!("Assistant: {}\n", m.content))
                }
                crate::ai_chat::MessageRole::System => {
                    prompt.push_str(&format!("System: {}\n", m.content))
                }
            }
        }

        println!(
            "📝 [RigExecutor::run] Prompt construido: {} caracteres",
            prompt.len()
        );

        // Run agent based on backend
        let response = match &client.backend {
            RigClientBackend::OpenAI(oa_client) => {
                // Usar NoteMemory compartido del MCP executor en lugar de crear uno nuevo
                let memory = mcp_executor.get_note_memory().borrow().clone();

                // Initialize tools
                let create_note =
                    CreateNote::new(db_path.clone(), notes_path.clone(), memory.clone());
                let read_note = ReadNote::new(db_path.clone());
                let search_notes = SearchNotes::new(db_path.clone());
                let list_notes = ListNotes::new(db_path.clone());
                let update_note = UpdateNote::new(db_path.clone());
                let append_to_note = AppendToNote::new(db_path.clone());
                let delete_note = DeleteNote::new(db_path.clone());
                let get_notes_with_tag = GetNotesWithTag::new(db_path.clone());
                let get_all_tags = GetAllTags::new(db_path.clone());
                let get_recent_notes = GetRecentNotes::new(db_path.clone());
                let get_word_count = GetWordCount::new(db_path.clone());
                let generate_toc = GenerateToc::new(db_path.clone());
                let extract_code_blocks = ExtractCodeBlocks::new(db_path.clone());
                let analyze_note_structure = AnalyzeNoteStructure::new(db_path.clone());
                let fuzzy_search = FuzzySearch::new(db_path.clone());
                let list_folders = ListFolders::new(db_path.clone(), notes_path.clone());
                let create_folder = CreateFolder::new(notes_path.clone());
                let batch_create_folders = BatchCreateFolders::new(notes_path.clone());
                let delete_folder = DeleteFolder::new(db_path.clone(), notes_path.clone());
                let move_note = MoveNote::new(db_path.clone(), notes_path.clone());
                let batch_move_notes = BatchMoveNotes::new(db_path.clone(), notes_path.clone());
                let rename_note = RenameNote::new(db_path.clone());
                let batch_rename_notes = BatchRenameNotes::new(db_path.clone());
                let add_tag = AddTag::new(db_path.clone());
                let remove_tag = RemoveTag::new(db_path.clone());
                let duplicate_note = DuplicateNote::new(db_path.clone());
                let merge_notes = MergeNotes::new(db_path.clone());
                let find_and_replace = FindAndReplace::new(db_path.clone());
                let create_daily_note = CreateDailyNote::new(db_path.clone(), notes_path.clone());
                let create_reminder = CreateReminder::new(db_path.clone());
                let delete_reminder = DeleteReminder::new(db_path.clone());
                let modify_reminder = ModifyReminder::new(db_path.clone());
                let get_system_date_time = GetSystemDateTime::new();
                let get_app_info = GetAppInfo::new(notes_path.clone());
                let get_workspace_path = GetWorkspacePath::new(notes_path.clone());
                let web_search = WebSearch::new();
                let fetch_url = FetchUrl::new();

                // Build the agent
                let mut agent_builder = oa_client.agent(&client.model)
                    .temperature(client.temperature as f64)
                    .preamble("You are a helpful assistant for managing markdown notes. You have access to comprehensive tools for creating, reading, updating, deleting, searching, analyzing, and organizing notes.

CRITICAL RULE - ALWAYS PROVIDE A FINAL RESPONSE:
After executing ANY tool(s), you MUST ALWAYS provide a final text response to the user summarizing what you did.
NEVER finish a conversation without a text message. Even if tools executed successfully, you MUST write a summary.
If you created notes, moved files, or performed any action, confirm it with a clear message.

SHOWING NOTE CONTENT:
When the user asks to see content from a note (tables, lists, data, etc.), you MUST include the COMPLETE content in your response.
DO NOT say 'here is the table' without actually showing the table. Copy the exact content from the note into your response.
If the content is a Markdown table, include the FULL table with all rows. Never truncate or summarize tables unless explicitly asked.

IMPORTANT: When answering questions based on search results, ALWAYS cite the notes you used.
Use the format `[Note Name](Note Name)` or `[[Note Name]]` to refer to notes, so the user can click on them.
If you find relevant information in the search snippets, summarize it and link to the source note.
You can manage tags, folders, perform text operations, and provide workspace information.
When organizing notes, follow this STRICT protocol:
1. PLAN: Review the 'Current Notes List' and 'Current Folders List'. Decide on a folder structure.
2. CREATE FOLDERS: Use `batch_create_folders` to create ALL necessary folders in a single step.
3. MOVE NOTES: Use `batch_move_notes` to move notes into their respective folders. Do NOT use `move_note` one by one.
4. DO NOT RENAME: Do not rename notes unless explicitly asked.
5. SUMMARY: Provide a final summary of your actions. ALWAYS include a link to any note you created or modified using the format `[Note Name](Note Name)`.

IMPORTANT: DO NOT create new notes unless the user explicitly asks you to (e.g., \"create a note\", \"save this\").
If the user asks for a summary, a search, or an explanation, JUST provide the answer in the chat. DO NOT create a note with the result.

LANGUAGE INSTRUCTION: You must answer in the same language as the user's request. If the user speaks Spanish, you MUST answer in Spanish.")
                    .tool(create_note)
                    .tool(read_note)
                    .tool(update_note)
                    .tool(append_to_note)
                    .tool(delete_note)
                    .tool(rename_note)
                    .tool(batch_rename_notes)
                    .tool(duplicate_note)
                    .tool(merge_notes)
                    .tool(search_notes)
                    .tool(fuzzy_search)
                    .tool(list_notes)
                    .tool(get_recent_notes)
                    .tool(get_notes_with_tag)
                    .tool(get_all_tags)
                    .tool(add_tag)
                    .tool(remove_tag)
                    .tool(get_word_count)
                    .tool(generate_toc)
                    .tool(extract_code_blocks)
                    .tool(analyze_note_structure)
                    .tool(list_folders)
                    .tool(create_folder)
                    .tool(batch_create_folders)
                    .tool(delete_folder)
                    .tool(move_note)
                    .tool(batch_move_notes)
                    .tool(find_and_replace)
                    .tool(create_daily_note)
                    .tool(create_reminder)
                    .tool(delete_reminder)
                    .tool(modify_reminder)
                    .tool(get_system_date_time)
                    .tool(get_app_info)
                    .tool(get_workspace_path)
                    .tool(web_search)
                    .tool(fetch_url);

                if let Some(mem) = memory {
                    let semantic_search = SemanticSearch {
                        memory: mem.clone(),
                    };
                    let index_all = IndexAllNotes::new(db_path.clone(), mem.clone());

                    agent_builder = agent_builder.tool(semantic_search).tool(index_all);
                }

                let agent = agent_builder.build();
                println!("🤖 [RigExecutor] Agente OpenAI construido, llamando a prompt()...");

                // Log prompt preview
                let preview = if prompt.len() > 500 {
                    format!("{}...{}", &prompt[..200], &prompt[prompt.len() - 200..])
                } else {
                    prompt.clone()
                };
                println!("📝 [RigExecutor] Prompt preview:\n{}", preview);

                let result = agent.prompt(&prompt).multi_turn(30).await?;

                // Si el resultado viene vacío, intentar obtener un resumen
                if result.is_empty() || result.trim().is_empty() {
                    println!("⚠️ [RigExecutor] Respuesta vacía de OpenAI. Solicitando resumen...");

                    let summary_prompt = format!(
                        "{}\n\nIMPORTANTE: Las herramientas ya se ejecutaron. Proporciona un RESUMEN BREVE de lo que hiciste.",
                        prompt
                    );

                    let simple_agent = oa_client
                        .agent(&client.model)
                        .temperature(0.3)
                        .preamble(
                            "Resume las acciones completadas. Responde en el idioma del usuario.",
                        )
                        .build();

                    match simple_agent.prompt(&summary_prompt).await {
                        Ok(summary) if !summary.is_empty() => {
                            println!("✅ [RigExecutor] Resumen obtenido: {} chars", summary.len());
                            summary
                        }
                        _ => {
                            "✅ Las operaciones se completaron. Verifica los cambios en tu workspace.".to_string()
                        }
                    }
                } else {
                    println!(
                        "✅ [RigExecutor] Respuesta recibida de OpenAI: {} caracteres",
                        result.len()
                    );
                    result
                }
            }
            RigClientBackend::OpenRouter(or_client) => {
                // Usar NoteMemory compartido del MCP executor
                let memory = mcp_executor.get_note_memory().borrow().clone();

                // Use the same embedding model type as OpenAI since NoteMemory is generic over it
                type EmbeddingModel = rig::providers::openai::EmbeddingModel;

                let create_note: CreateNote<EmbeddingModel> =
                    CreateNote::new(db_path.clone(), notes_path.clone(), memory.clone());
                let read_note = ReadNote::new(db_path.clone());
                let search_notes = SearchNotes::new(db_path.clone());
                let list_notes = ListNotes::new(db_path.clone());
                let update_note = UpdateNote::new(db_path.clone());
                let append_to_note = AppendToNote::new(db_path.clone());
                let delete_note = DeleteNote::new(db_path.clone());
                let get_notes_with_tag = GetNotesWithTag::new(db_path.clone());
                let get_all_tags = GetAllTags::new(db_path.clone());
                let get_recent_notes = GetRecentNotes::new(db_path.clone());
                let get_word_count = GetWordCount::new(db_path.clone());
                let generate_toc = GenerateToc::new(db_path.clone());
                let extract_code_blocks = ExtractCodeBlocks::new(db_path.clone());
                let analyze_note_structure = AnalyzeNoteStructure::new(db_path.clone());
                let fuzzy_search = FuzzySearch::new(db_path.clone());
                let list_folders = ListFolders::new(db_path.clone(), notes_path.clone());
                let create_folder = CreateFolder::new(notes_path.clone());
                let batch_create_folders = BatchCreateFolders::new(notes_path.clone());
                let delete_folder = DeleteFolder::new(db_path.clone(), notes_path.clone());
                let move_note = MoveNote::new(db_path.clone(), notes_path.clone());
                let batch_move_notes = BatchMoveNotes::new(db_path.clone(), notes_path.clone());
                let rename_note = RenameNote::new(db_path.clone());
                let batch_rename_notes = BatchRenameNotes::new(db_path.clone());
                let add_tag = AddTag::new(db_path.clone());
                let remove_tag = RemoveTag::new(db_path.clone());
                let duplicate_note = DuplicateNote::new(db_path.clone());
                let merge_notes = MergeNotes::new(db_path.clone());
                let find_and_replace = FindAndReplace::new(db_path.clone());
                let create_daily_note = CreateDailyNote::new(db_path.clone(), notes_path.clone());
                let create_reminder = CreateReminder::new(db_path.clone());
                let delete_reminder = DeleteReminder::new(db_path.clone());
                let modify_reminder = ModifyReminder::new(db_path.clone());
                let get_system_date_time = GetSystemDateTime::new();
                let get_app_info = GetAppInfo::new(notes_path.clone());
                let get_workspace_path = GetWorkspacePath::new(notes_path.clone());
                let web_search = WebSearch::new();
                let fetch_url = FetchUrl::new();

                let mut agent_builder = or_client.agent(&client.model)
                    .temperature(client.temperature as f64)
                    .preamble("You are a helpful assistant for managing markdown notes. You have access to comprehensive tools for creating, reading, updating, deleting, searching, analyzing, and organizing notes.

CRITICAL RULE - ALWAYS PROVIDE A FINAL RESPONSE:
After executing ANY tool(s), you MUST ALWAYS provide a final text response to the user summarizing what you did.
NEVER finish a conversation without a text message. Even if tools executed successfully, you MUST write a summary.
If you created notes, moved files, or performed any action, confirm it with a clear message.

SHOWING NOTE CONTENT:
When the user asks to see content from a note (tables, lists, data, etc.), you MUST include the COMPLETE content in your response.
DO NOT say 'here is the table' without actually showing the table. Copy the exact content from the note into your response.
If the content is a Markdown table, include the FULL table with all rows. Never truncate or summarize tables unless explicitly asked.

IMPORTANT: When answering questions based on search results, ALWAYS cite the notes you used.
Use the format `[Note Name](Note Name)` or `[[Note Name]]` to refer to notes, so the user can click on them.
If you find relevant information in the search snippets, summarize it and link to the source note.
You can manage tags, folders, perform text operations, and provide workspace information.
When organizing notes, follow this STRICT protocol:
1. PLAN: Review the 'Current Notes List' and 'Current Folders List'. Decide on a folder structure.
2. CREATE FOLDERS: Use `batch_create_folders` to create ALL necessary folders in a single step.
3. MOVE NOTES: Use `batch_move_notes` to move notes into their respective folders. Do NOT use `move_note` one by one.
4. DO NOT RENAME: Do not rename notes unless explicitly asked.
5. SUMMARY: Provide a final summary of your actions. ALWAYS include a link to any note you created or modified using the format `[Note Name](Note Name)`.

IMPORTANT: DO NOT create new notes unless the user explicitly asks you to (e.g., \"create a note\", \"save this\").
If the user asks for a summary, a search, or an explanation, JUST provide the answer in the chat. DO NOT create a note with the result.

LANGUAGE INSTRUCTION: You must answer in the same language as the user's request. If the user speaks Spanish, you MUST answer in Spanish.")
                    .tool(create_note)
                    .tool(read_note)
                    .tool(update_note)
                    .tool(append_to_note)
                    .tool(delete_note)
                    .tool(rename_note)
                    .tool(batch_rename_notes)
                    .tool(duplicate_note)
                    .tool(merge_notes)
                    .tool(search_notes)
                    .tool(fuzzy_search)
                    .tool(list_notes)
                    .tool(get_recent_notes)
                    .tool(get_notes_with_tag)
                    .tool(get_all_tags)
                    .tool(add_tag)
                    .tool(remove_tag)
                    .tool(get_word_count)
                    .tool(generate_toc)
                    .tool(extract_code_blocks)
                    .tool(analyze_note_structure)
                    .tool(list_folders)
                    .tool(create_folder)
                    .tool(batch_create_folders)
                    .tool(delete_folder)
                    .tool(move_note)
                    .tool(batch_move_notes)
                    .tool(find_and_replace)
                    .tool(create_daily_note)
                    .tool(create_reminder)
                    .tool(delete_reminder)
                    .tool(modify_reminder)
                    .tool(get_system_date_time)
                    .tool(get_app_info)
                    .tool(get_workspace_path)
                    .tool(web_search)
                    .tool(fetch_url);

                if let Some(mem) = memory {
                    let semantic_search = SemanticSearch {
                        memory: mem.clone(),
                    };
                    let index_all = IndexAllNotes::new(db_path.clone(), mem.clone());

                    agent_builder = agent_builder.tool(semantic_search).tool(index_all);
                }

                let agent = agent_builder.build();
                println!(
                    "🤖 [RigExecutor] Agente OpenRouter construido (Model: {}), llamando a prompt()...",
                    client.model
                );

                // Log prompt preview
                let preview = if prompt.len() > 500 {
                    format!("{}...{}", &prompt[..200], &prompt[prompt.len() - 200..])
                } else {
                    prompt.clone()
                };
                println!("📝 [RigExecutor] Prompt preview:\n{}", preview);

                let result = agent.prompt(&prompt).multi_turn(30).await?;

                println!("🔍 [RigExecutor] Raw result length: {}", result.len());

                // Si el resultado viene vacío, puede que el modelo ejecutó herramientas pero no dio respuesta final
                // Intentamos hacer una llamada adicional pidiendo un resumen
                if result.is_empty() || result.trim().is_empty() {
                    println!("⚠️ [RigExecutor] Respuesta vacía. Solicitando resumen al modelo...");

                    // Intentar obtener un resumen simple sin herramientas
                    let summary_prompt = format!(
                        "{}\n\nIMPORTANTE: Las herramientas ya se ejecutaron. Ahora proporciona un RESUMEN BREVE de lo que hiciste. NO uses más herramientas, solo responde con texto.",
                        prompt
                    );

                    // Crear un agente simple sin herramientas para el resumen
                    let simple_agent = or_client.agent(&client.model)
                        .temperature(0.3)
                        .preamble("Eres un asistente que resume acciones completadas. Responde en el mismo idioma que el usuario.")
                        .build();

                    match simple_agent.prompt(&summary_prompt).await {
                        Ok(summary) if !summary.is_empty() => {
                            println!(
                                "✅ [RigExecutor] Resumen obtenido: {} caracteres",
                                summary.len()
                            );
                            summary
                        }
                        _ => {
                            println!("⚠️ [RigExecutor] No se pudo obtener resumen");
                            "✅ Las operaciones se completaron. Verifica los cambios en tu workspace.".to_string()
                        }
                    }
                } else {
                    println!(
                        "✅ [RigExecutor] Respuesta recibida de OpenRouter: {} caracteres",
                        result.len()
                    );
                    result
                }
            }
        };

        println!("✅ [RigExecutor::run] Ejecución completada exitosamente");
        Ok(response)
    }
}
