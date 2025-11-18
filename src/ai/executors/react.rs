use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::ai_chat::{ChatMessage, MessageRole};
use crate::ai_client::AIClient;
use crate::mcp::{MCPToolCall, MCPToolExecutor, MCPToolRegistry};

/// Representa un paso en el loop ReAct (Reasoning + Acting)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReActStep {
    Thought(String),     // El agente razona qué hacer
    Action(MCPToolCall), // Ejecuta una herramienta MCP
    Observation(String), // Resultado de la acción ejecutada
    Answer(String),      // Respuesta final al usuario
}

/// Ejecutor que implementa el patrón ReAct (Reason + Act)
/// Permite que el LLM piense, ejecute herramientas, observe resultados y repita hasta dar una respuesta
pub struct ReActExecutor {
    max_iterations: usize,
    llm: Arc<dyn AIClient>,
    mcp_executor: MCPToolExecutor,
    mcp_registry: MCPToolRegistry,
}

impl ReActExecutor {
    /// Crea un nuevo ejecutor ReAct
    pub fn new(
        max_iterations: usize,
        llm: Arc<dyn AIClient>,
        mcp_executor: MCPToolExecutor,
    ) -> Self {
        Self {
            max_iterations,
            llm,
            mcp_executor,
            mcp_registry: MCPToolRegistry::new(),
        }
    }

    /// Ejecuta una tarea siguiendo el patrón ReAct
    /// Devuelve todos los pasos ejecutados (pensamientos, acciones, observaciones y respuesta final)
    ///
    /// `step_callback`: función opcional que se llama con cada step generado (para UI en tiempo real)
    pub async fn run<F>(
        &self,
        chat_messages: &[ChatMessage],
        context: &str,
        mut step_callback: F,
    ) -> Result<Vec<ReActStep>>
    where
        F: FnMut(&ReActStep) + Send,
    {
        let mut steps = Vec::new();
        let mut semantic_search_count = 0;
        const MAX_SEMANTIC_SEARCHES: usize = 2;

        // Rastrear tool calls ejecutados para evitar duplicados exactos
        let mut executed_tools: Vec<String> = Vec::new();

        // Rastrear herramientas de modificación por nota (append, update, etc.)
        let mut note_modifications: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        const MAX_MODIFICATIONS_PER_NOTE: usize = 1;

        // Rastrear bloqueos por límites (no cuentan como iteraciones)
        let mut limit_blocks_count = 0;
        const MAX_LIMIT_BLOCKS: usize = 10;

        // Extraer la tarea actual (último mensaje del usuario)
        let task = chat_messages
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or("");

        // Construir mensajes iniciales con historial del chat
        let mut messages = vec![ChatMessage {
            role: MessageRole::System,
            content: self.build_system_prompt(context),
            timestamp: chrono::Utc::now(),
            context_notes: Vec::new(),
        }];

        // Agregar historial anterior del chat (excepto el último mensaje que es la tarea)
        if chat_messages.len() > 1 {
            messages.extend_from_slice(&chat_messages[..chat_messages.len() - 1]);
        }

        // Agregar la tarea actual
        messages.push(ChatMessage {
            role: MessageRole::User,
            content: task.to_string(),
            timestamp: chrono::Utc::now(),
            context_notes: Vec::new(),
        });

        for iteration in 0..self.max_iterations {
            println!(
                "🔄 ReAct iteration {}/{}",
                iteration + 1,
                self.max_iterations
            );

            // Verificar si se alcanzó el límite de bloqueos
            if limit_blocks_count >= MAX_LIMIT_BLOCKS {
                println!(
                    "⛔ Límite de bloqueos alcanzado ({}/{}). Deteniendo ejecución.",
                    limit_blocks_count, MAX_LIMIT_BLOCKS
                );
                let final_message = format!(
                    "⚠️ Se alcanzó el límite de {} intentos bloqueados. La tarea no pudo completarse debido a restricciones repetidas.",
                    MAX_LIMIT_BLOCKS
                );
                let answer_step = ReActStep::Answer(final_message);
                steps.push(answer_step.clone());
                step_callback(&answer_step); // ✨ Notificar a la UI
                return Ok(steps);
            }

            // 1. El LLM piensa qué hacer (puede incluir texto + tool calls)
            let response = self
                .llm
                .send_message_with_tools(&messages, "", Some(&self.mcp_registry))
                .await?;

            // 2. Si hay texto (pensamiento/explicación), guardarlo
            if let Some(ref content) = response.content {
                if !content.trim().is_empty() {
                    // Solo si NO hay tool calls, este es un pensamiento
                    // Si hay tool calls, el texto es parte de la acción
                    if response.tool_calls.is_empty() {
                        println!("💭 Thought: {}", content);
                        let thought_step = ReActStep::Thought(content.clone());
                        steps.push(thought_step.clone());
                        step_callback(&thought_step); // ✨ Notificar a la UI

                        // Pausa para que GTK actualice la UI
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                        // Agregar como mensaje del asistente
                        messages.push(ChatMessage {
                            role: MessageRole::Assistant,
                            content: content.clone(),
                            timestamp: chrono::Utc::now(),
                            context_notes: Vec::new(),
                        });
                    }
                }
            }

            // 3. Si hay tool calls, ejecutarlos secuencialmente
            if !response.tool_calls.is_empty() {
                let mut any_tool_executed = false; // Rastrear si se ejecutó alguna herramienta

                for tool_call in response.tool_calls {
                    // Crear firma única del tool call para detectar duplicados
                    let tool_signature = format!("{:?}", tool_call);

                    // Verificar si ya ejecutamos exactamente este mismo tool call
                    if executed_tools.contains(&tool_signature) {
                        println!("⚠️ Tool call duplicado detectado: {:?}", tool_call);
                        limit_blocks_count += 1;

                        // Mensaje más específico según el tipo de herramienta
                        let error_msg = match &tool_call {
                            MCPToolCall::ReadNote { name } => {
                                format!("⚠️ Ya intentaste leer la nota '{}'. Si no se encontró, verifica el nombre exacto (mayúsculas, acentos) y usa el nombre correcto de la lista de búsqueda anterior.", name)
                            }
                            MCPToolCall::SearchNotes { query } => {
                                format!("⚠️ Ya buscaste '{}'. Usa los resultados que obtuviste.", query)
                            }
                            _ => "⚠️ Ya ejecutaste esta herramienta con los mismos parámetros. Usa los resultados que ya obtuviste.".to_string()
                        };

                        messages.push(ChatMessage {
                            role: MessageRole::User,
                            content: error_msg.clone(),
                            timestamp: chrono::Utc::now(),
                            context_notes: Vec::new(),
                        });

                        steps.push(ReActStep::Observation(format!(
                            "{{\"success\": false, \"error\": \"{}\"}}",
                            error_msg.replace("⚠️ ", "")
                        )));

                        continue; // Saltar este tool call (NO cuenta como iteración)
                    }

                    // Detectar modificaciones repetidas sobre la misma nota
                    let (is_modification, note_name) = match &tool_call {
                        MCPToolCall::AppendToNote { name, .. } => (true, Some(name.clone())),
                        MCPToolCall::UpdateNote { name, .. } => (true, Some(name.clone())),
                        MCPToolCall::CreateNote { name, .. } => (true, Some(name.clone())),
                        _ => (false, None),
                    };

                    if is_modification {
                        if let Some(note) = note_name {
                            let count = note_modifications.entry(note.clone()).or_insert(0);

                            if *count >= MAX_MODIFICATIONS_PER_NOTE {
                                println!(
                                    "⚠️ Límite de modificaciones alcanzado para nota '{}' ({}/{})",
                                    note, count, MAX_MODIFICATIONS_PER_NOTE
                                );
                                limit_blocks_count += 1;

                                messages.push(ChatMessage {
                                    role: MessageRole::User,
                                    content: format!("⚠️ LÍMITE ALCANZADO: Ya modificaste la nota '{}' {} veces. La tarea está completada. Responde al usuario confirmando qué se hizo.", 
                                        note, count),
                                    timestamp: chrono::Utc::now(),
                                    context_notes: Vec::new(),
                                });

                                steps.push(ReActStep::Observation(
                                    format!("{{\"success\": false, \"error\": \"Límite de modificaciones alcanzado para '{}' ({}/{}). Tarea completada.\"}}", 
                                        note, count, MAX_MODIFICATIONS_PER_NOTE)
                                ));

                                continue; // Saltar (NO cuenta como iteración)
                            }

                            *count += 1;
                            println!(
                                "📝 Modificación {}/{} para nota '{}'",
                                count, MAX_MODIFICATIONS_PER_NOTE, note
                            );
                        }
                    }

                    // Verificar si es semantic_search y si se ha alcanzado el límite
                    let is_semantic_search =
                        matches!(tool_call, MCPToolCall::SemanticSearch { .. });

                    if is_semantic_search {
                        if semantic_search_count >= MAX_SEMANTIC_SEARCHES {
                            println!(
                                "⚠️ Límite de búsquedas semánticas alcanzado ({}/{})",
                                semantic_search_count, MAX_SEMANTIC_SEARCHES
                            );
                            limit_blocks_count += 1;

                            // En lugar de ejecutar, agregar mensaje informativo
                            messages.push(ChatMessage {
                                role: MessageRole::User,
                                content: format!("⚠️ LÍMITE ALCANZADO: Ya ejecutaste {} búsquedas semánticas (máximo: {}). Usa la información que ya tienes para responder al usuario. NO intentes más búsquedas semánticas.", 
                                    semantic_search_count, MAX_SEMANTIC_SEARCHES),
                                timestamp: chrono::Utc::now(),
                                context_notes: Vec::new(),
                            });

                            // Registrar en steps que se intentó pero se bloqueó
                            steps.push(ReActStep::Observation(
                                format!("{{\"success\": false, \"error\": \"Límite de búsquedas semánticas alcanzado ({}/{}). Usa la información ya obtenida.\"}}", 
                                    semantic_search_count, MAX_SEMANTIC_SEARCHES)
                            ));

                            continue; // Saltar esta herramienta (NO cuenta como iteración)
                        }
                        semantic_search_count += 1;
                        println!(
                            "🔍 Búsqueda semántica {}/{}",
                            semantic_search_count, MAX_SEMANTIC_SEARCHES
                        );
                    }

                    println!("🔧 Action: {:?}", tool_call);
                    let action_step = ReActStep::Action(tool_call.clone());
                    steps.push(action_step.clone());
                    step_callback(&action_step); // ✨ Notificar a la UI

                    // Pausa para que GTK actualice la UI
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    // Ejecutar la herramienta MCP
                    let result = self.mcp_executor.execute(tool_call.clone())?;

                    // Marcar como ejecutado
                    executed_tools.push(tool_signature);
                    any_tool_executed = true; // Se ejecutó al menos una herramienta

                    let observation = serde_json::to_string_pretty(&result)?;
                    println!("👁️ Observation: {}", observation);
                    let obs_step = ReActStep::Observation(observation.clone());
                    steps.push(obs_step.clone());
                    step_callback(&obs_step); // ✨ Notificar a la UI

                    // Pausa para que GTK actualice la UI
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    // Verificar si fue exitosa
                    let was_successful = if let Ok(obs_json) =
                        serde_json::from_str::<serde_json::Value>(&observation)
                    {
                        obs_json
                            .get("success")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    } else {
                        false
                    };

                    // Agregar observación con instrucción explícita
                    if was_successful {
                        // Verificar si es un resultado de semantic_search para dar instrucciones especiales
                        let is_semantic_search =
                            matches!(tool_call, MCPToolCall::SemanticSearch { .. });

                        let instruction = if is_semantic_search {
                            "✓ Búsqueda completada. AHORA debes:\n\
                            1. Leer las 2-3 notas más relevantes usando read_note\n\
                            2. Analizar su contenido\n\
                            3. Responder la pregunta del usuario con la información encontrada\n\
                            NO te limites a listar las notas - el usuario quiere la RESPUESTA a su pregunta."
                        } else {
                            "✓ Acción completada. Si la tarea requiere más pasos, ejecuta la SIGUIENTE herramienta necesaria. Si ya terminaste, responde al usuario confirmando qué se hizo."
                        };

                        messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: format!("Resultado:\n{}\n\n{}", observation, instruction),
                            timestamp: chrono::Utc::now(),
                            context_notes: Vec::new(),
                        });
                    } else {
                        messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: format!("Resultado:\n{}", observation),
                            timestamp: chrono::Utc::now(),
                            context_notes: Vec::new(),
                        });
                    }
                }

                // Si ninguna herramienta se ejecutó (todas bloqueadas), NO avanzar iteración
                // Continuar el loop para dar otra oportunidad al LLM
                if !any_tool_executed {
                    println!(
                        "⚠️ Todos los tool calls fueron bloqueados. Bloqueos: {}/{}",
                        limit_blocks_count, MAX_LIMIT_BLOCKS
                    );
                }

                // Continuar el loop para que el LLM procese los resultados
                continue;
            }

            // 4. Si no hay tool calls y hay contenido, verificar si es respuesta final o error
            if let Some(content) = response.content {
                if !content.trim().is_empty() {
                    // Detectar si el LLM escribió XML de function_call en lugar de usar tool calls
                    // Incluye variantes: <function_call>, <xai:function_call>, etc.
                    if content.contains("<function_call")
                        || content.contains("</function_call>")
                        || content.contains("<xai:function_call")
                        || content.contains("</xai:function_call>")
                    {
                        println!(
                            "⚠️ El modelo escribió XML manualmente en lugar de usar tool calls"
                        );

                        // Agregar mensaje correctivo
                        messages.push(ChatMessage {
                            role: MessageRole::System,
                            content: "ERROR: NO escribas XML de ningún tipo (<function_call>, <xai:function_call>, etc.). El sistema NO soporta XML manual. Debes usar ÚNICAMENTE el mecanismo nativo JSON de tool calling. Si no puedes hacer tool calls, simplemente responde la pregunta del usuario con la información que YA OBTUVISTE de las herramientas anteriores. NO repitas llamadas a herramientas en formato XML.".to_string(),
                            timestamp: chrono::Utc::now(),
                            context_notes: Vec::new(),
                        });

                        continue; // Reintentar en la siguiente iteración
                    }

                    // Detectar y limpiar bloques <think>
                    let cleaned_content = if content.contains("<think>") {
                        // Extraer solo el contenido después del </think>
                        if let Some(pos) = content.find("</think>") {
                            let after_think = &content[pos + 8..]; // 8 = len("</think>")
                            after_think.trim().to_string()
                        } else {
                            // Si hay <think> pero no </think>, remover desde <think> hasta el final del párrafo
                            if let Some(pos) = content.find("<think>") {
                                content[..pos].trim().to_string()
                            } else {
                                content.clone()
                            }
                        }
                    } else {
                        content.clone()
                    };

                    // Si después de limpiar queda contenido válido, es la respuesta final
                    if !cleaned_content.is_empty() {
                        println!("✅ Answer: {}", cleaned_content);
                        let answer_step = ReActStep::Answer(cleaned_content.clone());
                        steps.push(answer_step.clone());
                        step_callback(&answer_step); // ✨ Notificar a la UI
                        return Ok(steps);
                    }
                }
            }

            // Si llegamos aquí sin respuesta ni tools, algo salió mal
            return Err(anyhow::anyhow!(
                "El modelo no devolvió ni respuesta ni tool calls en la iteración {}",
                iteration + 1
            ));
        }

        // Si alcanzamos el máximo de iteraciones, construir respuesta final desde las acciones
        let mut action_count = 0;
        let mut successful_actions = Vec::new();

        for step in &steps {
            if let ReActStep::Observation(obs) = step {
                // Parsear el JSON de observación para ver si fue exitoso
                if let Ok(obs_json) = serde_json::from_str::<serde_json::Value>(obs) {
                    if obs_json
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        action_count += 1;
                        if let Some(data) = obs_json.get("data") {
                            if let Some(msg) = data.get("message").and_then(|v| v.as_str()) {
                                successful_actions.push(msg.to_string());
                            }
                        }
                    }
                }
            }
        }

        let final_message = if action_count > 0 {
            format!(
                "✓ Completé {} acción(es) exitosamente:\n{}",
                action_count,
                successful_actions.join("\n")
            )
        } else {
            "Se alcanzó el máximo de iteraciones sin completar ninguna acción".to_string()
        };

        let answer_step = ReActStep::Answer(final_message);
        steps.push(answer_step.clone());
        step_callback(&answer_step); // ✨ Notificar a la UI
        Ok(steps)
    }

    /// Construye el system prompt optimizado para ReAct con OpenRouter
    fn build_system_prompt(&self, context: &str) -> String {
        let tools_list = self
            .mcp_registry
            .get_tools()
            .iter()
            .map(|t| {
                if let Some(name) = t.get("name").and_then(|v| v.as_str()) {
                    if let Some(desc) = t.get("description").and_then(|v| v.as_str()) {
                        return format!("- {}: {}", name, desc);
                    }
                }
                String::new()
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r##"Eres un asistente para gestionar notas en NotNative.

REGLAS CRÍTICAS:
1. Ejecuta herramientas inmediatamente cuando el usuario pide algo, SIN explicaciones previas
2. NO uses bloques <think>, <function_call>, <xai:function_call> ni ningún tipo de XML
3. Usa ÚNICAMENTE el mecanismo nativo JSON de tool calling del sistema
4. Cuando el usuario hace una PREGUNTA (ej: "¿cuándo...?", "¿qué...?", "¿tengo información sobre...?"):
   - Usa semantic_search para encontrar notas relevantes
   - Lee las 2-3 notas más relevantes con read_note
   - Analiza el contenido y RESPONDE la pregunta con la información encontrada
5. Cuando el usuario pide "busca X" o "muéstrame X":
   - Ejecuta semantic_search
   - Muestra la lista de resultados encontrados
6. NUNCA inventes información - usa SOLO lo que está en las notas
7. Si no encuentras la información, dilo claramente

FLUJO TÍPICO:
- Usuario pregunta "¿cuándo es X?" → semantic_search → read_note (top 2-3) → Responder con la info encontrada
- Usuario dice "busca X" → semantic_search → Listar resultados
- Usuario dice "crea nota X" → create_note → Confirmar

IMPORTANTE:
- NO te limites a listar notas cuando el usuario hace una pregunta - RESPONDE la pregunta
- Después de read_note, analiza el contenido y extrae la información solicitada
- Si una herramienta falla, ajusta e intenta de nuevo (no repitas el mismo error)
- NO ejecutes la misma herramienta con los mismos parámetros más de una vez
- Responde de forma DIRECTA y CONCISA, sin razonamientos internos visibles

{}

Herramientas disponibles:
{}
"##,
            if context.is_empty() {
                "Sin notas en el contexto actual.".to_string()
            } else {
                format!("Contexto (notas adjuntas):\n{}", context)
            },
            tools_list
        )
    }
}
