use anyhow::Result;
use std::sync::Arc;

use crate::ai::executors::react::{ReActExecutor, ReActStep};
use crate::ai_chat::{ChatMessage, MessageRole};
use crate::ai_client::AIClient;
use crate::mcp::{MCPToolExecutor, get_all_tool_definitions};

/// Tipo de ejecutor que usa un agente
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorType {
    ReAct, // Razonamiento + Acción (con herramientas)
    Basic, // Chat simple sin herramientas
    Rig,   // Agente RIG nativo
}

/// Agente especializado en un tipo de tarea
#[derive(Debug, Clone)]
pub struct Agent {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub allowed_tools: Vec<String>, // Nombres de herramientas MCP permitidas
    executor_type: ExecutorType,
}

impl Agent {
    /// Ejecuta una tarea con este agente
    pub async fn run<F>(
        &self,
        messages: &[ChatMessage],
        context: &str,
        llm: Arc<dyn AIClient>,
        mcp_executor: &MCPToolExecutor,
        step_callback: F,
    ) -> Result<String>
    where
        F: FnMut(&ReActStep) + Send + 'static,
    {
        // Extraer el último mensaje como la tarea actual
        let task = messages.last().map(|m| m.content.as_str()).unwrap_or("");
        match self.executor_type {
            ExecutorType::ReAct => {
                // Usar ReAct executor con herramientas (pasar historial completo)
                let executor = ReActExecutor::new(
                    10, // max iterations
                    llm,
                    mcp_executor.clone(),
                );

                let steps = executor.run(messages, context, step_callback).await?;

                // Extraer la respuesta final del último paso
                if let Some(crate::ai::executors::react::ReActStep::Answer(answer)) = steps.last() {
                    return Ok(answer.clone());
                }

                // Si no hay respuesta final, construir una desde los pasos
                let mut response = String::new();
                for step in &steps {
                    match step {
                        crate::ai::executors::react::ReActStep::Thought(text) => {
                            if !response.is_empty() {
                                response.push_str("\n\n");
                            }
                            response.push_str(text);
                        }
                        crate::ai::executors::react::ReActStep::Answer(text) => {
                            if !response.is_empty() {
                                response.push_str("\n\n");
                            }
                            response.push_str(text);
                        }
                        _ => {}
                    }
                }

                if response.is_empty() {
                    response =
                        "No pude completar la tarea. Intenta reformular tu solicitud.".to_string();
                }

                Ok(response)
            }
            ExecutorType::Basic => {
                // Chat simple sin herramientas - usar historial completo
                let mut full_messages = vec![ChatMessage::new(
                    MessageRole::System,
                    format!("{}\n\nContexto: {}", self.instructions, context),
                    Vec::new(),
                )];

                // Agregar historial completo del chat
                full_messages.extend_from_slice(messages);

                llm.send_message(&full_messages, "").await
            }
            ExecutorType::Rig => {
                use crate::ai::executors::RigExecutor;
                return RigExecutor::run(llm.clone(), messages, context, mcp_executor).await;
            }
        }
    }
}

// ==================== AGENTES PREDEFINIDOS ====================

impl Agent {
    /// Agente especializado en crear y modificar notas
    pub fn create_agent() -> Self {
        Self {
            name: "CreateAgent".to_string(),
            description: "Especializado en crear y modificar notas con contenido bien estructurado".to_string(),
            instructions: "
            SOLO DEBES CREAR 1 NOTA Y ESPERAR CONFIRMACIÓN, UNA VEZ CREADA NO LA VUELVAS A CREAR.
            Eres un experto en crear notas en formato Markdown bien estructurado.

Sigue estas reglas de trabajo:

1. Tu objetivo principal es **crear UNA nota por solicitud**.
2. Usa estructuras bien formateadas con encabezados, listas y énfasis (`**negritas**`, `_itálicas_`), manteniendo coherencia y claridad.
3. Puedes usar las herramientas: create_note, update_note, append_to_note, add_tags y add_multiple_tags.
4. **Después de ejecutar `create_note` y recibir una confirmación exitosa, no vuelvas a ejecutarla.**
   En su lugar, da una respuesta final confirmando la creación de la nota.
5. Si la nota ya existe o se ha creado, utiliza `update_note` o `append_to_note` solo si el contenido necesita cambios.
6. **Nunca ejecutes más de una herramienta por iteración.**
7. Cuando completes la acción necesaria (creación o modificación exitosa), **responde al usuario con un resumen de lo que hiciste e INCLUYE SIEMPRE un enlace a la nota modificada o creada usando el formato `[Nombre de la nota](Nombre de la nota)`** y detén el flujo.
8. No intentes crear de nuevo la misma nota si ya existe o ya fue confirmada.
9. Si ocurre un error con la herramienta, intenta una sola vez un método alternativo (por ejemplo, `update_note` si `create_note` falla).
10. Si no puedes continuar o no hay suficiente información, explica claramente qué falta en tu respuesta y termina.

Este agente debe siempre producir UNA sola nota final por tarea, y nunca entrar en un ciclo repetitivo.".to_string(),
            allowed_tools: vec![
                "create_note".to_string(),
                "update_note".to_string(),
                "append_to_note".to_string(),
                "add_tags".to_string(),
                "add_multiple_tags".to_string(),
            ],
            executor_type: ExecutorType::ReAct,
        }
    }

    /// Agente especializado en buscar notas
    pub fn search_agent() -> Self {
        Self {
            name: "SearchAgent".to_string(),
            description: "Busca y encuentra notas usando múltiples métodos".to_string(),
            instructions: "Eres un experto en encontrar información. Usas búsqueda semántica, tags, búsqueda fuzzy y full-text según sea necesario.
            SOLO DEBES BUSCAR UNA VEZ Y ESPERAR RESULTADOS. PROHIBIDO REPETIR ACCIONES YA REALIZADAS, ESPERA SIEMPRE A LA RESULTA DE LA ACCIÓN ANTERIOR.
            Proporciona resultados relevantes y concisos basados en la consulta del usuario.

            PROHIBIDO REPETIR ACCIONES YA REALIZADAS, CONFIA EN LOS RESULTADOS OBTENIDOS.
            ".to_string(),
            allowed_tools: vec![
                "search_notes".to_string(),
                "semantic_search".to_string(),
                "get_notes_with_tag".to_string(),
                "fuzzy_search".to_string(),
                "list_notes".to_string(),
                "get_recent_notes".to_string(),
            ],
            executor_type: ExecutorType::ReAct,
        }
    }

    /// Agente especializado en analizar contenido
    pub fn analyze_agent() -> Self {
        Self {
            name: "AnalyzeAgent".to_string(),
            description: "Analiza estructura, contenido y relaciones entre notas".to_string(),
            instructions: "Eres un experto en análisis de texto. Examinas estructura, estadísticas, detectas patrones y sugieres mejoras. PROHIBIDO inventar resultados. PROHIBIDO REPETIR ACCIONES YA REALIZADAS, espera siempre el resultado de la acción anterior.".to_string(),
            allowed_tools: vec![
                "analyze_note_structure".to_string(),
                "get_word_count".to_string(),
                "suggest_related_notes".to_string(),
                "find_similar_notes".to_string(),
                "get_all_tags".to_string(),
            ],
            executor_type: ExecutorType::ReAct,
        }
    }

    /// Agente para tareas complejas multi-paso
    pub fn multi_step_agent() -> Self {
        // Este agente tiene acceso a TODAS las herramientas
        let all_tools: Vec<String> = get_all_tool_definitions()
            .iter()
            .map(|t| t.name.clone())
            .collect();

        Self {
            name: "MultiStepAgent".to_string(),
            description: "Ejecuta tareas complejas que requieren múltiples pasos y herramientas".to_string(),
            instructions: "Eres un planificador experto. Descompones tareas complejas en pasos, ejecutas herramientas en secuencia y sintetizas resultados. PROHIBIDO repetir acciones completadas. Espera el resultado de cada herramienta antes de continuar. Al finalizar, responde con una única respuesta final clara en Markdown. Si has creado o modificado una nota, incluye un enlace a ella en tu respuesta final.".to_string(),
            allowed_tools: all_tools,
            executor_type: ExecutorType::ReAct,
        }
    }

    /// Agente conversacional básico (sin herramientas)
    pub fn chat_agent() -> Self {
        Self {
            name: "ChatAgent".to_string(),
            description: "Conversación general, saludos y ayuda básica".to_string(),
            instructions: "Eres un asistente amigable de NotNative. Responde de forma concisa a saludos, conversación casual y preguntas generales. Si preguntan sobre herramientas, menciona que NotNative puede crear, buscar y organizar notas, analizar contenido, gestionar tags y usar búsqueda semántica. Sé breve pero amigable.".to_string(),
            allowed_tools: Vec::new(),
            executor_type: ExecutorType::Basic,
        }
    }

    /// Agente nativo RIG con herramientas integradas
    pub fn rig_agent() -> Self {
        Self {
            name: "RigAgent".to_string(),
            description: "Agente nativo usando RIG Framework".to_string(),
            instructions: "Eres un asistente inteligente potenciado por RIG.".to_string(),
            allowed_tools: vec![], // Las herramientas se definen en el ejecutor RIG
            executor_type: ExecutorType::Rig,
        }
    }
}
