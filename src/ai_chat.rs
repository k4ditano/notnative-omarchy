use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::core::NoteFile;

/// Rol de un mensaje en el chat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Proveedor de IA
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AIProvider {
    OpenAI,
    Anthropic,
    Ollama,
    Custom,
}

/// Configuración del modelo de IA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIModelConfig {
    pub provider: AIProvider,
    pub model: String,
    pub max_tokens: usize,
    pub temperature: f32,
}

impl Default for AIModelConfig {
    fn default() -> Self {
        Self {
            provider: AIProvider::OpenAI,
            model: "gpt-4".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
        }
    }
}

/// Mensaje individual en el chat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub context_notes: Vec<String>, // Nombres de notas adjuntas al momento del mensaje
}

impl ChatMessage {
    pub fn new(role: MessageRole, content: String, context_notes: Vec<String>) -> Self {
        Self {
            role,
            content,
            timestamp: Utc::now(),
            context_notes,
        }
    }
}

/// Sesión de chat con la IA
#[derive(Debug, Clone)]
pub struct ChatSession {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub attached_notes: Vec<NoteFile>,
    pub model_config: AIModelConfig,
    pub created_at: DateTime<Utc>,
}

impl ChatSession {
    /// Crea una nueva sesión de chat
    pub fn new(config: AIModelConfig) -> Self {
        Self {
            id: None,
            name: None,
            messages: Vec::new(),
            attached_notes: Vec::new(),
            model_config: config,
            created_at: Utc::now(),
        }
    }

    /// Agrega un mensaje a la sesión
    pub fn add_message(&mut self, role: MessageRole, content: String) {
        let note_names: Vec<String> = self
            .attached_notes
            .iter()
            .map(|n| n.name().to_string())
            .collect();

        self.messages
            .push(ChatMessage::new(role, content, note_names));
    }

    /// Adjunta una nota al contexto
    pub fn attach_note(&mut self, note: NoteFile) {
        // Solo agregar si no está ya en la lista
        if !self.attached_notes.iter().any(|n| n.name() == note.name()) {
            self.attached_notes.push(note);
        }
    }

    /// Quita una nota del contexto
    pub fn detach_note(&mut self, note_name: &str) {
        self.attached_notes.retain(|n| n.name() != note_name);
    }

    /// Limpia todas las notas del contexto
    pub fn clear_context(&mut self) {
        self.attached_notes.clear();
    }

    /// Calcula el total aproximado de tokens en el contexto
    /// Estimación: 1 token ≈ 4 caracteres
    pub fn total_context_tokens(&self) -> usize {
        let notes_chars: usize = self
            .attached_notes
            .iter()
            .filter_map(|n| n.read().ok())
            .map(|content| content.len())
            .sum();

        let messages_chars: usize = self.messages.iter().map(|m| m.content.len()).sum();

        (notes_chars + messages_chars) / 4
    }

    /// Serializa todas las notas adjuntas en formato markdown
    pub fn build_context(&self) -> Result<String> {
        let mut context = String::new();

        for note in &self.attached_notes {
            let content = note.read()?;
            context.push_str(&format!(
                "# Nota: {}\n\n{}\n\n---\n\n",
                note.name(),
                content
            ));
        }

        Ok(context)
    }

    /// Verifica si el contexto excede el límite de tokens
    pub fn is_context_too_large(&self) -> bool {
        self.total_context_tokens() > self.model_config.max_tokens
    }

    /// Reduce el contexto eliminando las notas más antiguas
    pub fn trim_context(&mut self) {
        while self.is_context_too_large() && !self.attached_notes.is_empty() {
            self.attached_notes.remove(0);
        }
    }

    /// Obtiene el número de notas adjuntas
    pub fn context_count(&self) -> usize {
        self.attached_notes.len()
    }

    /// Limpia el historial de mensajes
    pub fn clear_history(&mut self) {
        self.messages.clear();
    }
}

/// Información de un modelo de OpenRouter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub pricing: OpenRouterPricing,
    #[serde(deserialize_with = "deserialize_context_length")]
    pub context_length: u32,
    #[serde(default)]
    pub architecture: Option<OpenRouterArchitecture>,
    #[serde(default)]
    pub top_provider: Option<OpenRouterProvider>,
    #[serde(default)]
    pub supported_parameters: Vec<String>,
}

// Deserializador personalizado para context_length que acepta string o número
fn deserialize_context_length<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Deserialize};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ContextLength {
        Number(u32),
        String(String),
    }

    match ContextLength::deserialize(deserializer)? {
        ContextLength::Number(n) => Ok(n),
        ContextLength::String(s) => s.parse().map_err(de::Error::custom),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterArchitecture {
    #[serde(default)]
    pub input_modalities: Vec<String>,
    #[serde(default)]
    pub output_modalities: Vec<String>,
    #[serde(default)]
    pub tokenizer: Option<String>,
    #[serde(default)]
    pub instruct_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterPricing {
    pub prompt: String,
    pub completion: String,
    #[serde(default)]
    pub request: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterProvider {
    #[serde(default, deserialize_with = "deserialize_optional_u32")]
    pub context_length: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_optional_u32")]
    pub max_completion_tokens: Option<u32>,
    #[serde(default)]
    pub is_moderated: Option<bool>,
}

// Deserializador para Option<u32> que acepta string o número
fn deserialize_optional_u32<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Deserialize};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OptionalU32 {
        Number(u32),
        String(String),
        None,
    }

    match OptionalU32::deserialize(deserializer) {
        Ok(OptionalU32::Number(n)) => Ok(Some(n)),
        Ok(OptionalU32::String(s)) => {
            if s.is_empty() {
                Ok(None)
            } else {
                s.parse().map(Some).map_err(de::Error::custom)
            }
        }
        Ok(OptionalU32::None) | Err(_) => Ok(None),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterModelsResponse {
    pub data: Vec<OpenRouterModel>,
}

/// Obtiene la lista de modelos disponibles desde OpenRouter
pub async fn fetch_openrouter_models() -> Result<Vec<OpenRouterModel>> {
    let response = reqwest::get("https://openrouter.ai/api/v1/models")
        .await?
        .json::<OpenRouterModelsResponse>()
        .await?;

    Ok(response.data)
}

/// Filtra modelos gratuitos
pub fn filter_free_models(models: &[OpenRouterModel]) -> Vec<OpenRouterModel> {
    models
        .iter()
        .filter(|m| {
            m.pricing.prompt == "0"
                || m.pricing.prompt == "0.0"
                || m.pricing.prompt.starts_with("0.00")
        })
        .cloned()
        .collect()
}

/// Agrupa modelos por proveedor
pub fn group_models_by_provider(
    models: &[OpenRouterModel],
) -> std::collections::HashMap<String, Vec<OpenRouterModel>> {
    let mut grouped = std::collections::HashMap::new();

    for model in models {
        let provider = model.id.split('/').next().unwrap_or("other").to_string();
        grouped
            .entry(provider)
            .or_insert_with(Vec::new)
            .push(model.clone());
    }

    grouped
}

/// Busca modelos por texto (ID, nombre o descripción)
pub fn search_models(models: &[OpenRouterModel], query: &str) -> Vec<OpenRouterModel> {
    if query.trim().is_empty() {
        return models.to_vec();
    }

    let query_lower = query.to_lowercase();
    models
        .iter()
        .filter(|m| {
            m.id.to_lowercase().contains(&query_lower)
                || m.name.to_lowercase().contains(&query_lower)
                || m.description
                    .as_ref()
                    .map_or(false, |d| d.to_lowercase().contains(&query_lower))
        })
        .cloned()
        .collect()
}

/// Genera descripción legible para un modelo
pub fn format_model_display(model: &OpenRouterModel) -> String {
    let price_str = if model.pricing.prompt == "0" || model.pricing.prompt.starts_with("0.00") {
        "Gratis ✨".to_string()
    } else {
        // Convertir de precio por token a precio por millón de tokens
        if let Ok(price) = model.pricing.prompt.parse::<f64>() {
            let price_per_million = price * 1_000_000.0;
            format!("${:.2}/1M", price_per_million)
        } else {
            format!("${}/token", model.pricing.prompt)
        }
    };

    let context_str = if model.context_length >= 1_000_000 {
        format!("{}M ctx", model.context_length / 1_000_000)
    } else if model.context_length >= 1_000 {
        format!("{}K ctx", model.context_length / 1_000)
    } else {
        format!("{} ctx", model.context_length)
    };

    let modalities = model
        .architecture
        .as_ref()
        .map(|arch| {
            let inputs = &arch.input_modalities;
            let mut icons = String::new();
            if inputs.contains(&"image".to_string()) {
                icons.push_str(" 🖼️");
            }
            if inputs.contains(&"audio".to_string()) {
                icons.push_str(" �");
            }
            if inputs.contains(&"video".to_string()) {
                icons.push_str(" 🎥");
            }
            icons
        })
        .unwrap_or_default();

    format!("{} • {}{}", price_str, context_str, modalities)
}

/// Obtiene descripción completa de un modelo para mostrar en tooltips
pub fn format_model_tooltip(model: &OpenRouterModel) -> String {
    let mut tooltip = format!("ID: {}\n", model.id);

    if let Some(desc) = &model.description {
        tooltip.push_str(&format!("\n{}\n", desc));
    }

    tooltip.push_str(&format!("\nContexto: {} tokens", model.context_length));
    tooltip.push_str(&format!(
        "\nPrecio (prompt): ${}/token",
        model.pricing.prompt
    ));
    tooltip.push_str(&format!(
        "\nPrecio (completion): ${}/token",
        model.pricing.completion
    ));

    if let Some(arch) = &model.architecture {
        if !arch.input_modalities.is_empty() {
            tooltip.push_str(&format!(
                "\nModalidades entrada: {}",
                arch.input_modalities.join(", ")
            ));
        }
        if !arch.output_modalities.is_empty() {
            tooltip.push_str(&format!(
                "\nModalidades salida: {}",
                arch.output_modalities.join(", ")
            ));
        }
        if let Some(tokenizer) = &arch.tokenizer {
            tooltip.push_str(&format!("\nTokenizer: {}", tokenizer));
        }
    }

    if let Some(provider) = &model.top_provider {
        if let Some(max_tokens) = provider.max_completion_tokens {
            tooltip.push_str(&format!("\nMáx tokens completion: {}", max_tokens));
        }
        if let Some(moderated) = provider.is_moderated {
            if moderated {
                tooltip.push_str("\n⚠️ Contenido moderado");
            }
        }
    }

    tooltip
}
