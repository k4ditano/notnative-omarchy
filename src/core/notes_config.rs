use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Configuración del asistente AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConfig {
    /// API Key para el proveedor de AI
    #[serde(default)]
    pub api_key: Option<String>,
    /// Proveedor de AI (openai, anthropic, ollama)
    #[serde(default = "default_ai_provider")]
    pub provider: String,
    /// Modelo a utilizar (gpt-4, gpt-3.5-turbo, claude-3, etc.)
    #[serde(default = "default_ai_model")]
    pub model: String,
    /// Temperatura para la generación (0.0 - 2.0)
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Máximo de tokens en la respuesta
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Guardar historial de chat en la base de datos
    #[serde(default = "default_save_history")]
    pub save_history: bool,
    /// URL base personalizada para APIs (útil para Ollama local)
    #[serde(default)]
    pub custom_api_url: Option<String>,
}

fn default_ai_provider() -> String {
    "openrouter".to_string()
}

fn default_ai_model() -> String {
    "google/gemini-flash-1.5".to_string()
}

fn default_temperature() -> f32 {
    0.7
}

fn default_max_tokens() -> u32 {
    2000
}

fn default_save_history() -> bool {
    true
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            provider: default_ai_provider(),
            model: default_ai_model(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            save_history: default_save_history(),
            custom_api_url: None,
        }
    }
}

/// Configuración del orden y organización de notas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotesConfig {
    /// Orden personalizado de las notas (nota -> posición)
    pub order: HashMap<String, usize>,
    /// Carpetas que están expandidas
    pub expanded_folders: Vec<String>,
    /// Preferencia de idioma (código ISO 639-1: "es", "en", etc.)
    #[serde(default)]
    pub language: Option<String>,
    /// Directorio de trabajo personalizado (notas y assets)
    #[serde(default)]
    pub workspace_dir: Option<String>,
    /// Salida de audio preferida (sink de PulseAudio)
    #[serde(default)]
    pub audio_output_sink: Option<String>,
    /// Última nota abierta
    #[serde(default)]
    pub last_opened_note: Option<String>,
    /// Configuración del asistente AI
    #[serde(default)]
    pub ai_config: AIConfig,
}

impl Default for NotesConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl NotesConfig {
    /// Crea una nueva configuración vacía
    pub fn new() -> Self {
        Self {
            order: HashMap::new(),
            expanded_folders: Vec::new(),
            language: None,
            workspace_dir: None,
            audio_output_sink: None,
            last_opened_note: None,
            ai_config: AIConfig::default(),
        }
    }

    /// Carga la configuración desde un archivo
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: NotesConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Guarda la configuración a un archivo
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Obtiene la posición de una nota en el orden personalizado
    pub fn get_position(&self, note_name: &str) -> Option<usize> {
        self.order.get(note_name).copied()
    }

    /// Establece la posición de una nota
    pub fn set_position(&mut self, note_name: String, position: usize) {
        self.order.insert(note_name, position);
    }

    /// Remueve una nota del orden
    pub fn remove_note(&mut self, note_name: &str) {
        self.order.remove(note_name);
    }

    /// Mueve una nota a una nueva posición, reordenando las demás
    pub fn move_note(&mut self, note_name: &str, new_position: usize) {
        // Obtener posición actual
        let old_position = self.get_position(note_name);

        // Actualizar posiciones de todas las notas afectadas
        if let Some(old_pos) = old_position {
            if old_pos < new_position {
                // Moviendo hacia abajo: decrementar posiciones entre old y new
                for (_name, pos) in self.order.iter_mut() {
                    if *pos > old_pos && *pos <= new_position {
                        *pos -= 1;
                    }
                }
            } else if old_pos > new_position {
                // Moviendo hacia arriba: incrementar posiciones entre new y old
                for (_name, pos) in self.order.iter_mut() {
                    if *pos >= new_position && *pos < old_pos {
                        *pos += 1;
                    }
                }
            }
        } else {
            // Nueva nota: incrementar todas las posiciones >= new_position
            for (_name, pos) in self.order.iter_mut() {
                if *pos >= new_position {
                    *pos += 1;
                }
            }
        }

        // Establecer nueva posición
        self.order.insert(note_name.to_string(), new_position);
    }

    /// Verifica si una carpeta está expandida
    pub fn is_folder_expanded(&self, folder: &str) -> bool {
        self.expanded_folders.contains(&folder.to_string())
    }

    /// Alterna el estado de expansión de una carpeta
    pub fn toggle_folder(&mut self, folder: String) {
        if let Some(pos) = self.expanded_folders.iter().position(|f| f == &folder) {
            self.expanded_folders.remove(pos);
        } else {
            self.expanded_folders.push(folder);
        }
    }

    /// Obtiene la preferencia de idioma
    pub fn get_language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// Establece la preferencia de idioma
    pub fn set_language(&mut self, lang: Option<String>) {
        self.language = lang;
    }

    /// Obtiene el directorio de trabajo personalizado
    pub fn get_workspace_dir(&self) -> Option<&str> {
        self.workspace_dir.as_deref()
    }

    /// Establece el directorio de trabajo personalizado
    pub fn set_workspace_dir(&mut self, dir: Option<String>) {
        self.workspace_dir = dir;
    }

    /// Obtiene la salida de audio preferida
    pub fn get_audio_output_sink(&self) -> Option<&str> {
        self.audio_output_sink.as_deref()
    }

    /// Establece la salida de audio preferida
    pub fn set_audio_output_sink(&mut self, sink: Option<String>) {
        self.audio_output_sink = sink;
    }

    /// Obtiene la última nota abierta
    pub fn get_last_opened_note(&self) -> Option<&str> {
        self.last_opened_note.as_deref()
    }

    /// Establece la última nota abierta
    pub fn set_last_opened_note(&mut self, note: Option<String>) {
        self.last_opened_note = note;
    }

    /// Ruta por defecto del archivo de configuración
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("notnative")
            .join("config.json")
    }

    /// Obtiene la carpeta de assets para las notas
    pub fn assets_dir() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("notnative")
            .join("assets")
    }

    /// Asegura que el directorio de assets exista
    pub fn ensure_assets_dir() -> Result<PathBuf> {
        let assets_dir = Self::assets_dir();
        std::fs::create_dir_all(&assets_dir)?;
        Ok(assets_dir)
    }

    /// Obtiene la configuración de AI
    pub fn get_ai_config(&self) -> &AIConfig {
        &self.ai_config
    }

    /// Obtiene la configuración de AI mutable
    pub fn get_ai_config_mut(&mut self) -> &mut AIConfig {
        &mut self.ai_config
    }

    /// Establece la API key del asistente AI
    pub fn set_ai_api_key(&mut self, api_key: Option<String>) {
        self.ai_config.api_key = api_key;
    }

    /// Establece el proveedor de AI
    pub fn set_ai_provider(&mut self, provider: String) {
        self.ai_config.provider = provider;
    }

    /// Establece el modelo de AI
    pub fn set_ai_model(&mut self, model: String) {
        self.ai_config.model = model;
    }

    /// Establece la temperatura de AI
    pub fn set_ai_temperature(&mut self, temperature: f32) {
        self.ai_config.temperature = temperature.clamp(0.0, 2.0);
    }

    /// Establece el máximo de tokens
    pub fn set_ai_max_tokens(&mut self, max_tokens: u32) {
        self.ai_config.max_tokens = max_tokens;
    }

    /// Establece si se debe guardar el historial
    pub fn set_ai_save_history(&mut self, save_history: bool) {
        self.ai_config.save_history = save_history;
    }
}
