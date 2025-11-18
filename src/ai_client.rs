use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::ai_chat::{AIModelConfig, AIProvider, ChatMessage, MessageRole};
use crate::mcp::{MCPToolCall, MCPToolRegistry, MCPToolResult};

/// Respuesta de la IA que puede incluir llamadas a funciones
#[derive(Debug, Clone)]
pub struct AIResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<MCPToolCall>,
}

impl AIResponse {
    pub fn text(content: String) -> Self {
        Self {
            content: Some(content),
            tool_calls: Vec::new(),
        }
    }

    pub fn with_tools(content: Option<String>, tool_calls: Vec<MCPToolCall>) -> Self {
        Self {
            content,
            tool_calls,
        }
    }
}

/// Trait para clientes de IA
#[async_trait]
pub trait AIClient: Send + Sync {
    /// Envía mensajes a la IA y obtiene una respuesta (puede incluir tool calls)
    async fn send_message_with_tools(
        &self,
        messages: &[ChatMessage],
        context: &str,
        tools: Option<&MCPToolRegistry>,
    ) -> Result<AIResponse>;

    /// Versión simple sin soporte de tools (retrocompatibilidad)
    async fn send_message(&self, messages: &[ChatMessage], context: &str) -> Result<String> {
        let response = self
            .send_message_with_tools(messages, context, None)
            .await?;
        Ok(response.content.unwrap_or_default())
    }

    /// Envía mensaje con streaming (chunks de texto en tiempo real)
    /// Devuelve un canal de tokio para recibir chunks
    async fn send_message_streaming(
        &self,
        messages: &[ChatMessage],
        context: &str,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<String>>;
}

/// Cliente para OpenAI
pub struct OpenAIClient {
    api_key: String,
    model: String,
    max_tokens: usize,
    temperature: f32,
}

impl OpenAIClient {
    pub fn new(api_key: String, model: String, max_tokens: usize, temperature: f32) -> Self {
        Self {
            api_key,
            model,
            max_tokens,
            temperature,
        }
    }
}

#[async_trait]
impl AIClient for OpenAIClient {
    async fn send_message_with_tools(
        &self,
        messages: &[ChatMessage],
        context: &str,
        tools: Option<&MCPToolRegistry>,
    ) -> Result<AIResponse> {
        use async_openai::{
            Client,
            config::OpenAIConfig,
            types::{
                ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
                ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
                CreateChatCompletionRequestArgs,
            },
        };

        // Configurar para OpenRouter si se detecta
        let mut config = OpenAIConfig::new().with_api_key(&self.api_key);

        // OpenRouter usa una URL base diferente
        if self.api_key.starts_with("sk-or-") {
            config = config.with_api_base("https://openrouter.ai/api/v1");
        }

        let client = Client::with_config(config);

        let mut api_messages = Vec::new();
        let mut raw_messages: Vec<Value> = Vec::new();

        // Solo añadir system prompt automático si no viene uno en los mensajes
        let has_system_prompt = messages
            .iter()
            .any(|m| matches!(m.role, MessageRole::System));

        if !has_system_prompt {
            // System prompt simplificado - inspirado en llm-chain
            let system_message = if !context.is_empty() {
                format!(
                    "Eres un asistente para gestionar notas en NotNative.\n\n\
                    Notas adjuntas:\n{}\n\n\
                    REGLA: Ejecuta herramientas directamente, sin explicaciones previas. \
                    Puedes incluir texto junto con el tool call para explicar.",
                    context
                )
            } else {
                "Eres un asistente para gestionar notas en NotNative.\n\n\
                REGLA: Ejecuta herramientas directamente, sin explicaciones previas. \
                Puedes incluir texto junto con el tool call para explicar."
                    .to_string()
            };

            let system_msg = ChatCompletionRequestSystemMessageArgs::default()
                .content(system_message.clone())
                .build()?;
            api_messages.push(ChatCompletionRequestMessage::System(system_msg));
            raw_messages.push(json!({
                "role": "system",
                "content": system_message.clone(),
            }));
        }

        // Agregar historial de mensajes
        for msg in messages {
            match msg.role {
                MessageRole::User => {
                    let user_msg = ChatCompletionRequestUserMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?;
                    api_messages.push(ChatCompletionRequestMessage::User(user_msg));
                    raw_messages.push(json!({
                        "role": "user",
                        "content": msg.content.clone(),
                    }));
                }
                MessageRole::Assistant => {
                    let assistant_msg = ChatCompletionRequestAssistantMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?;
                    api_messages.push(ChatCompletionRequestMessage::Assistant(assistant_msg));
                    raw_messages.push(json!({
                        "role": "assistant",
                        "content": msg.content.clone(),
                    }));
                }
                MessageRole::System => {
                    let system_msg = ChatCompletionRequestSystemMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?;
                    api_messages.push(ChatCompletionRequestMessage::System(system_msg));
                    raw_messages.push(json!({
                        "role": "system",
                        "content": msg.content.clone(),
                    }));
                }
            }
        }

        // Enviar mediante OpenRouter cuando la API key lo indique
        if self.api_key.starts_with("sk-or-") {
            return self.send_via_openrouter(raw_messages, tools).await;
        }

        // TODO: Implementar function calling para OpenAI nativo
        // Por ahora, solo soportamos tools en OpenRouter

        // Crear request
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(api_messages)
            .max_tokens(self.max_tokens as u16)
            .temperature(self.temperature)
            .build()?;

        // Enviar request
        let response = client.chat().create(request).await?;

        // Extraer respuesta
        let reply = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("No se recibió respuesta de la IA"))?;

        Ok(AIResponse::text(reply))
    }

    async fn send_message_streaming(
        &self,
        messages: &[ChatMessage],
        context: &str,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<String>> {
        use async_openai::{
            Client,
            config::OpenAIConfig,
            types::{
                ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
                ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
                CreateChatCompletionRequestArgs,
            },
        };
        use futures::StreamExt;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        // Configurar cliente
        let mut config = OpenAIConfig::new().with_api_key(&self.api_key);
        if self.api_key.starts_with("sk-or-") {
            config = config.with_api_base("https://openrouter.ai/api/v1");
        }
        let client = Client::with_config(config);

        // Construir mensajes
        let mut api_messages = Vec::new();

        // System prompt si hay contexto
        if !context.is_empty() {
            let system_msg = ChatCompletionRequestSystemMessageArgs::default()
                .content(format!(
                    "Eres un asistente conversacional amigable y útil.\n\nContexto:\n{}",
                    context
                ))
                .build()?;
            api_messages.push(ChatCompletionRequestMessage::System(system_msg));
        }

        // Agregar mensajes del historial
        for msg in messages {
            match msg.role {
                MessageRole::User => {
                    let user_msg = ChatCompletionRequestUserMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?;
                    api_messages.push(ChatCompletionRequestMessage::User(user_msg));
                }
                MessageRole::Assistant => {
                    let assistant_msg = ChatCompletionRequestAssistantMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?;
                    api_messages.push(ChatCompletionRequestMessage::Assistant(assistant_msg));
                }
                MessageRole::System => {
                    let system_msg = ChatCompletionRequestSystemMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?;
                    api_messages.push(ChatCompletionRequestMessage::System(system_msg));
                }
            }
        }

        // Crear request con streaming habilitado
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(api_messages)
            .max_tokens(self.max_tokens as u16)
            .temperature(self.temperature)
            .stream(true) // Habilitar streaming!
            .build()?;

        // Spawn task para manejar el stream
        tokio::spawn(async move {
            match client.chat().create_stream(request).await {
                Ok(mut stream) => {
                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(response) => {
                                // Extraer contenido del delta
                                if let Some(choice) = response.choices.first() {
                                    if let Some(content) = &choice.delta.content {
                                        // Enviar chunk por el canal
                                        if tx.send(content.clone()).is_err() {
                                            break; // Receptor cerrado
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Error en stream: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error creando stream: {}", e);
                }
            }
        });

        Ok(rx)
    }
}

impl OpenAIClient {
    async fn send_via_openrouter(
        &self,
        raw_messages: Vec<Value>,
        tools: Option<&MCPToolRegistry>,
    ) -> Result<AIResponse> {
        #[derive(Deserialize, Debug)]
        struct CompletionResponse {
            choices: Vec<Choice>,
        }

        #[derive(Deserialize, Debug)]
        struct Choice {
            message: ChoiceMessage,
        }

        #[derive(Deserialize, Debug)]
        struct ChoiceMessage {
            #[serde(default)]
            content: Option<String>,
            #[serde(default)]
            tool_calls: Option<Vec<ToolCallData>>,
        }

        #[derive(Deserialize, Debug)]
        struct ToolCallData {
            id: String,
            #[serde(rename = "type")]
            call_type: String,
            function: FunctionCall,
        }

        #[derive(Deserialize, Debug)]
        struct FunctionCall {
            name: String,
            arguments: String,
        }

        let client = reqwest::Client::new();

        let mut request_body = json!({
            "model": self.model,
            "messages": raw_messages,
            "temperature": self.temperature,
            "max_tokens": self.max_tokens,
        });

        // Agregar tools si están disponibles
        if let Some(registry) = tools {
            let openai_tools = registry.get_tools();
            if !openai_tools.is_empty() {
                request_body["tools"] = json!(openai_tools);
                request_body["tool_choice"] = json!("auto");
                // IMPORTANTE: Forzar parallel_tool_calls = false para que el modelo
                // tenga más oportunidad de generar contenido explicativo
                request_body["parallel_tool_calls"] = json!(false);
            }
        }

        // Debug: mostrar el mensaje de sistema
        if let Some(messages_array) = request_body["messages"].as_array() {
            if let Some(first_msg) = messages_array.first() {
                if let Some(content) = first_msg["content"].as_str() {
                    println!(
                        "📋 Mensaje de sistema (primeros 200 chars):\n{}",
                        content.chars().take(200).collect::<String>()
                    );
                }
            }
        }

        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://github.com/k4ditano/notnative-app")
            .header("X-Title", "NotNative")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<sin cuerpo>".to_string());
            return Err(anyhow::anyhow!("OpenRouter respondió {}: {}", status, body));
        }

        let completion: CompletionResponse = response.json().await?;
        let message = completion
            .choices
            .first()
            .map(|c| &c.message)
            .ok_or_else(|| anyhow::anyhow!("OpenRouter no devolvió mensaje"))?;

        // Parsear tool calls si existen
        let mut parsed_tool_calls = Vec::new();
        if let Some(tool_calls) = &message.tool_calls {
            for tc in tool_calls {
                // El arguments viene como JSON string, necesitamos parsearlo y agregar el campo "type" con el nombre de la función
                // El arguments viene como JSON string con los parámetros de la función
                // Necesitamos construir el objeto completo con "tool" y "args"
                match serde_json::from_str::<Value>(&tc.function.arguments) {
                    Ok(args) => {
                        // Convertir snake_case a PascalCase para el nombre del tool
                        let tool_name = tc
                            .function
                            .name
                            .split('_')
                            .map(|word| {
                                let mut chars = word.chars();
                                match chars.next() {
                                    None => String::new(),
                                    Some(first) => {
                                        first.to_uppercase().collect::<String>() + chars.as_str()
                                    }
                                }
                            })
                            .collect::<String>();

                        // Construir el objeto completo con formato: { "tool": "CreateNote", "args": {...} }
                        let tool_call_obj = json!({
                            "tool": tool_name,
                            "args": args
                        });

                        // Ahora intentar parsear como MCPToolCall
                        match serde_json::from_value::<MCPToolCall>(tool_call_obj) {
                            Ok(tool_call) => {
                                println!(
                                    "✓ Tool call parseado: {} → {:?}",
                                    tc.function.name, tool_name
                                );
                                parsed_tool_calls.push(tool_call);
                            }
                            Err(e) => {
                                eprintln!(
                                    "⚠️ No se pudo parsear tool call '{}': {} - Args: {}",
                                    tc.function.name, e, tc.function.arguments
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "⚠️ Arguments no es JSON válido: {} - {}",
                            e, tc.function.arguments
                        );
                    }
                }
            }
        }

        Ok(AIResponse {
            content: message.content.clone(),
            tool_calls: parsed_tool_calls,
        })
    }
}

/// Cliente para Anthropic (Claude) - stub para implementación futura
pub struct AnthropicClient {
    api_key: String,
    model: String,
    max_tokens: usize,
}

impl AnthropicClient {
    pub fn new(api_key: String, model: String, max_tokens: usize) -> Self {
        Self {
            api_key,
            model,
            max_tokens,
        }
    }
}

#[async_trait]
impl AIClient for AnthropicClient {
    async fn send_message_with_tools(
        &self,
        _messages: &[ChatMessage],
        _context: &str,
        _tools: Option<&MCPToolRegistry>,
    ) -> Result<AIResponse> {
        // TODO: Implementar usando anthropic-sdk
        Err(anyhow::anyhow!(
            "Anthropic client no implementado aún. Usa OpenAI/OpenRouter."
        ))
    }

    async fn send_message_streaming(
        &self,
        _messages: &[ChatMessage],
        _context: &str,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<String>> {
        Err(anyhow::anyhow!(
            "Streaming no implementado para Anthropic aún."
        ))
    }
}

/// Cliente para Ollama (modelos locales) - stub para implementación futura
pub struct OllamaClient {
    endpoint: String,
    model: String,
}

impl OllamaClient {
    pub fn new(model: String) -> Self {
        Self {
            endpoint: "http://localhost:11434".to_string(),
            model,
        }
    }

    pub fn with_endpoint(mut self, endpoint: String) -> Self {
        self.endpoint = endpoint;
        self
    }
}

#[async_trait]
impl AIClient for OllamaClient {
    async fn send_message_with_tools(
        &self,
        _messages: &[ChatMessage],
        _context: &str,
        _tools: Option<&MCPToolRegistry>,
    ) -> Result<AIResponse> {
        // TODO: Implementar usando ollama-rs
        Err(anyhow::anyhow!(
            "Ollama client no implementado aún. Usa OpenRouter por ahora."
        ))
    }

    async fn send_message_streaming(
        &self,
        _messages: &[ChatMessage],
        _context: &str,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<String>> {
        Err(anyhow::anyhow!(
            "Streaming no implementado para Ollama aún."
        ))
    }
}

/// Factory para crear clientes de IA según la configuración
pub fn create_client(config: &AIModelConfig, api_key: &str) -> Result<Box<dyn AIClient>> {
    match config.provider {
        AIProvider::OpenAI => Ok(Box::new(OpenAIClient::new(
            api_key.to_string(),
            config.model.clone(),
            config.max_tokens,
            config.temperature,
        ))),
        AIProvider::Anthropic => Ok(Box::new(AnthropicClient::new(
            api_key.to_string(),
            config.model.clone(),
            config.max_tokens,
        ))),
        AIProvider::Ollama => Ok(Box::new(OllamaClient::new(config.model.clone()))),
        AIProvider::Custom => Err(anyhow::anyhow!("Custom provider no implementado aún")),
    }
}
