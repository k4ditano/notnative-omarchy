use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

use crate::ai::agent::{Agent, ExecutorType};
use crate::ai::executors::react::ReActStep;
use crate::ai_chat::{ChatMessage, MessageRole};
use crate::ai_client::AIClient;
use crate::mcp::MCPToolExecutor;

/// Clasificación de la intención del usuario
#[derive(Debug, Clone)]
pub struct IntentClassification {
    pub agent_type: String,
    pub confidence: f32,
}

/// Router que clasifica la intención del usuario y delega al agente apropiado
#[derive(Clone)]
pub struct RouterAgent {
    llm: Arc<dyn AIClient>,
    agents: HashMap<String, Agent>,
}

impl std::fmt::Debug for RouterAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouterAgent")
            .field("llm", &"Arc<dyn AIClient>")
            .field("agents", &self.agents)
            .finish()
    }
}

impl RouterAgent {
    /// Crea un nuevo router con los agentes predefinidos
    pub fn new(llm: Arc<dyn AIClient>) -> Self {
        let mut agents = HashMap::new();
        agents.insert("create".to_string(), Agent::create_agent());
        agents.insert("search".to_string(), Agent::search_agent());
        agents.insert("analyze".to_string(), Agent::analyze_agent());
        agents.insert("execute".to_string(), Agent::multi_step_agent());
        agents.insert("chat".to_string(), Agent::chat_agent());

        Self { llm, agents }
    }

    /// Obtiene una referencia al cliente LLM
    pub fn get_llm(&self) -> Arc<dyn AIClient> {
        self.llm.clone()
    }

    /// Clasifica la intención y ejecuta con el agente apropiado
    pub async fn route_and_execute<F>(
        &self,
        messages: &[ChatMessage],
        context: &str,
        mcp_executor: &MCPToolExecutor,
        step_callback: F,
    ) -> Result<String>
    where
        F: FnMut(&ReActStep) + Send + 'static,
    {
        // Extraer el último mensaje del usuario como la tarea actual
        let task = messages.last().map(|m| m.content.as_str()).unwrap_or("");

        // 1. Clasificar la intención
        let classification = self.classify_intent(task).await?;

        println!(
            "🎯 Intent classified as: {} (confidence: {:.2})",
            classification.agent_type, classification.confidence
        );

        // 2. Obtener agente apropiado
        let agent = self.agents.get(&classification.agent_type).ok_or_else(|| {
            anyhow::anyhow!("Agente no encontrado: {}", classification.agent_type)
        })?;

        println!("🤖 Using agent: {}", agent.name);

        // 3. Ejecutar con el agente seleccionado (pasando historial completo y callback)
        agent
            .run(
                messages,
                context,
                self.llm.clone(),
                mcp_executor,
                step_callback,
            )
            .await
    }

    /// Clasifica la intención del usuario usando el LLM
    async fn classify_intent(&self, task: &str) -> Result<IntentClassification> {
        let classification_prompt = format!(
            r#"Clasifica esta tarea del usuario en UNA de estas categorías:

1. CREATE - Crear, modificar, actualizar o editar notas
2. SEARCH - Buscar, encontrar, listar o explorar notas
3. ANALYZE - Analizar, revisar, obtener estadísticas o examinar contenido
4. EXECUTE - Tareas complejas, preguntas sobre capacidades/herramientas, múltiples pasos
5. CHAT - Conversación casual, saludos simples

Tarea del usuario: "{}"

Responde SOLO con UNA palabra: CREATE, SEARCH, ANALYZE, EXECUTE o CHAT

Ejemplos:
- "Crea una nota sobre Rust" → CREATE
- "Busca notas sobre Python" → SEARCH
- "¿Cuántas palabras tiene esta nota?" → ANALYZE
- "¿Qué herramientas tienes?" → EXECUTE
- "Busca notas sobre X y crea un resumen" → EXECUTE
- "Hola" → CHAT
- "¿Cómo estás?" → CHAT
"#,
            task
        );

        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: classification_prompt,
            timestamp: chrono::Utc::now(),
            context_notes: Vec::new(),
        }];

        let response = self.llm.send_message(&messages, "").await?;

        // Parsear la respuesta
        let response_upper = response.to_uppercase().trim().to_string();
        let agent_type = if response_upper.contains("CREATE") {
            "create"
        } else if response_upper.contains("SEARCH") {
            "search"
        } else if response_upper.contains("ANALYZE") {
            "analyze"
        } else if response_upper.contains("EXECUTE") {
            "execute"
        } else if response_upper.contains("CHAT") {
            "chat"
        } else {
            // Por defecto, usar el agente más potente
            println!("⚠️ No se pudo clasificar, usando MultiStepAgent por defecto");
            "execute"
        };

        Ok(IntentClassification {
            agent_type: agent_type.to_string(),
            confidence: 1.0, // TODO: Implementar cálculo real de confianza
        })
    }
}
