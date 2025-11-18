use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

use super::database::NotesDatabase;
use super::embedding_client::EmbeddingClient;
use crate::ai_chat::{ChatMessage, MessageRole};
use crate::ai_client::AIClient;

/// Resultado de búsqueda semántica
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Ruta de la nota
    pub note_path: PathBuf,
    /// Índice del chunk dentro de la nota
    pub chunk_index: usize,
    /// Texto del chunk
    pub chunk_text: String,
    /// Score de similitud (0.0 - 1.0, donde 1.0 es idéntico)
    pub similarity: f32,
    /// Snippet del texto (puede ser truncado para display)
    pub snippet: String,
}

impl SearchResult {
    /// Crea un snippet del texto limitado a max_chars
    pub fn create_snippet(text: &str, max_chars: usize) -> String {
        if text.len() <= max_chars {
            return text.to_string();
        }

        // Encontrar un límite válido de caracteres UTF-8
        let mut end = max_chars.min(text.len());
        while !text.is_char_boundary(end) && end > 0 {
            end -= 1;
        }

        if end == 0 {
            return String::new();
        }

        let truncated = &text[..end];

        // Intentar romper en espacio o puntuación
        if let Some(last_space) =
            truncated.rfind(|c: char| c.is_whitespace() || c == '.' || c == ',')
        {
            format!("{}...", &truncated[..last_space])
        } else {
            format!("{}...", truncated)
        }
    }
}

/// Opciones de búsqueda semántica
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Número máximo de resultados a retornar
    pub limit: usize,
    /// Similitud mínima requerida (0.0 - 1.0)
    pub min_similarity: f32,
    /// Filtrar por carpeta específica (opcional)
    pub folder_filter: Option<String>,
    /// Longitud máxima del snippet en caracteres
    pub snippet_length: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            min_similarity: 0.2, // Threshold más permisivo para capturar más resultados relevantes
            folder_filter: None,
            snippet_length: 200,
        }
    }
}

/// Motor de búsqueda semántica
pub struct SemanticSearch {
    client: EmbeddingClient,
    db: NotesDatabase,
    ai_client: Option<Arc<dyn AIClient>>,
}

impl SemanticSearch {
    /// Crea un nuevo motor de búsqueda
    pub fn new(client: EmbeddingClient, db: NotesDatabase) -> Self {
        Self {
            client,
            db,
            ai_client: None,
        }
    }

    /// Crea motor con capacidad de query expansion
    pub fn with_ai(
        client: EmbeddingClient,
        db: NotesDatabase,
        ai_client: Arc<dyn AIClient>,
    ) -> Self {
        Self {
            client,
            db,
            ai_client: Some(ai_client),
        }
    }

    /// Detecta si la query es una pregunta que necesita expansión
    fn is_question(query: &str) -> bool {
        let query_lower = query.to_lowercase();

        // Detectar palabras interrogativas en español
        query_lower.contains('¿') ||
        query_lower.contains('?') ||
        query_lower.starts_with("cuándo") ||
        query_lower.starts_with("cuando") ||
        query_lower.starts_with("cómo") ||
        query_lower.starts_with("como") ||
        query_lower.starts_with("dónde") ||
        query_lower.starts_with("donde") ||
        query_lower.starts_with("qué") ||
        query_lower.starts_with("que") ||
        query_lower.starts_with("quién") ||
        query_lower.starts_with("quien") ||
        query_lower.starts_with("cuál") ||
        query_lower.starts_with("cual") ||
        query_lower.starts_with("por qué") ||
        query_lower.starts_with("tengo") ||
        query_lower.starts_with("hay") ||
        // Palabras clave que indican contexto temporal/evento
        (query_lower.contains("próximo") || query_lower.contains("proximo")) ||
        query_lower.contains("siguiente") ||
        query_lower.contains("fecha") ||
        query_lower.contains("viaje")
    }

    /// Expande una query usando LLM para obtener mejores términos de búsqueda
    async fn expand_query(&self, query: &str) -> Result<String> {
        // Si no hay AI client, retornar query original
        let ai_client = match &self.ai_client {
            Some(client) => client,
            None => return Ok(query.to_string()),
        };

        eprintln!("🔄 Expandiendo query: '{}'", query);

        // Primero verificar caché de expansiones
        if let Ok(Some(cached)) = self.db.get_cached_query_expansion(query) {
            eprintln!("✅ Expansión en caché: '{}'", cached);
            return Ok(cached);
        }

        // Prompt optimizado para expansión de queries
        let expansion_prompt = format!(
            r#"Convierte esta pregunta en palabras clave para búsqueda semántica.

Reglas:
- Extrae solo conceptos clave, sinónimos y términos relacionados
- NO respondas la pregunta, solo lista palabras
- Incluye variaciones del concepto (ej: "vista" → "vista oftalmólogo ojos revisión examen visual")
- Máximo 15 palabras
- Solo palabras, separadas por espacios, sin puntuación

Pregunta: "{}"

Palabras clave:"#,
            query
        );

        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: expansion_prompt,
            timestamp: chrono::Utc::now(),
            context_notes: Vec::new(),
        }];

        match ai_client.send_message(&messages, "").await {
            Ok(expanded) => {
                let cleaned = expanded
                    .trim()
                    .lines()
                    .next()
                    .unwrap_or(&expanded)
                    .trim()
                    .to_string();

                eprintln!("✨ Query expandida: '{}' → '{}'", query, cleaned);

                // Guardar en caché
                if let Err(e) = self.db.cache_query_expansion(query, &cleaned) {
                    eprintln!("⚠️ Error guardando expansión en caché: {}", e);
                }

                Ok(cleaned)
            }
            Err(e) => {
                eprintln!("⚠️ Error expandiendo query, usando original: {}", e);
                Ok(query.to_string())
            }
        }
    }

    /// Realiza búsqueda semántica
    pub async fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<SearchResult>> {
        // 1. Expandir query si es pregunta y hay AI disponible
        let search_query = if Self::is_question(query) && self.ai_client.is_some() {
            self.expand_query(query).await?
        } else {
            query.to_string()
        };

        eprintln!("🔍 Buscando con query: '{}'", search_query);

        // 2. Intentar obtener embedding desde caché
        let query_embedding = match self.db.get_cached_query_embedding(&search_query) {
            Ok(Some(cached_embedding)) => {
                eprintln!("✅ Cache hit para query: '{}'", search_query);
                cached_embedding
            }
            _ => {
                eprintln!("❌ Cache miss para query: '{}'", search_query);

                // Generar embedding usando API
                let embedding = self
                    .client
                    .embed_text(&search_query)
                    .await
                    .context("Error generando embedding de búsqueda")?;

                // Guardar en caché para futuras búsquedas
                if let Err(e) = self.db.cache_query_embedding(&search_query, &embedding) {
                    eprintln!("⚠️ Error guardando en caché: {}", e);
                }

                embedding
            }
        };

        // 3. Obtener todos los embeddings de la base de datos
        let all_embeddings = self
            .db
            .get_all_embeddings()
            .context("Error obteniendo embeddings de BD")?;

        if all_embeddings.is_empty() {
            return Ok(Vec::new());
        }

        // 4. Calcular similitud con cada chunk
        let mut results: Vec<SearchResult> = all_embeddings
            .into_iter()
            .map(|embedding_data| {
                let similarity = cosine_similarity(&query_embedding, &embedding_data.embedding);

                eprintln!(
                    "🔍 Similitud: {:.3} - {} (chunk {})",
                    similarity, embedding_data.note_path, embedding_data.chunk_index
                );

                (embedding_data, similarity)
            })
            // 5. Filtrar por similitud mínima
            .filter(|(embedding_data, similarity)| {
                let passed = *similarity >= options.min_similarity;
                if !passed {
                    eprintln!(
                        "❌ Filtrado por baja similitud ({:.3} < {:.3}): {}",
                        similarity, options.min_similarity, embedding_data.note_path
                    );
                }
                passed
            })
            // 6. Filtrar por carpeta si se especificó
            .filter(|(embedding_data, _)| {
                if let Some(ref folder) = options.folder_filter {
                    embedding_data.note_path.starts_with(folder)
                } else {
                    true
                }
            })
            .map(|(embedding_data, similarity)| {
                let snippet = SearchResult::create_snippet(
                    &embedding_data.chunk_text,
                    options.snippet_length,
                );

                SearchResult {
                    note_path: PathBuf::from(embedding_data.note_path),
                    chunk_index: embedding_data.chunk_index,
                    chunk_text: embedding_data.chunk_text,
                    similarity,
                    snippet,
                }
            })
            .collect();

        // 7. Ordenar por similitud (mayor a menor)
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 8. Limitar número de resultados
        results.truncate(options.limit);

        Ok(results)
    }

    /// Busca notas similares a una nota dada (by path)
    pub async fn find_similar_notes(
        &self,
        note_path: &str,
        options: SearchOptions,
    ) -> Result<Vec<SearchResult>> {
        // Obtener embeddings de la nota
        let note_embeddings = self
            .db
            .get_embeddings_by_note(note_path)
            .context("Error obteniendo embeddings de la nota")?;

        if note_embeddings.is_empty() {
            anyhow::bail!("La nota no tiene embeddings indexados");
        }

        // Usar el primer chunk como representación de la nota
        let query_embedding = &note_embeddings[0].embedding;

        // Obtener todos los embeddings
        let all_embeddings = self.db.get_all_embeddings()?;

        // Calcular similitudes
        let mut results: Vec<SearchResult> = all_embeddings
            .into_iter()
            // Excluir la nota misma
            .filter(|embedding_data| embedding_data.note_path != note_path)
            .map(|embedding_data| {
                let similarity = cosine_similarity(query_embedding, &embedding_data.embedding);

                (embedding_data, similarity)
            })
            .filter(|(_, similarity)| *similarity >= options.min_similarity)
            .filter(|(embedding_data, _)| {
                if let Some(ref folder) = options.folder_filter {
                    embedding_data.note_path.starts_with(folder)
                } else {
                    true
                }
            })
            .map(|(embedding_data, similarity)| {
                let snippet = SearchResult::create_snippet(
                    &embedding_data.chunk_text,
                    options.snippet_length,
                );

                SearchResult {
                    note_path: PathBuf::from(embedding_data.note_path),
                    chunk_index: embedding_data.chunk_index,
                    chunk_text: embedding_data.chunk_text,
                    similarity,
                    snippet,
                }
            })
            .collect();

        // Ordenar por similitud
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results.truncate(options.limit);

        Ok(results)
    }

    /// Agrupa resultados por nota (combina chunks de la misma nota)
    pub fn group_by_note(results: Vec<SearchResult>) -> Vec<(PathBuf, Vec<SearchResult>)> {
        let mut grouped: std::collections::HashMap<PathBuf, Vec<SearchResult>> =
            std::collections::HashMap::new();

        for result in results {
            grouped
                .entry(result.note_path.clone())
                .or_insert_with(Vec::new)
                .push(result);
        }

        // Convertir a vector y ordenar por la mejor similitud de cada nota
        let mut grouped_vec: Vec<(PathBuf, Vec<SearchResult>)> = grouped.into_iter().collect();

        grouped_vec.sort_by(|a, b| {
            let max_a = a.1.iter().map(|r| r.similarity).fold(0.0f32, f32::max);
            let max_b = b.1.iter().map(|r| r.similarity).fold(0.0f32, f32::max);

            max_b
                .partial_cmp(&max_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        grouped_vec
    }

    /// Obtiene estadísticas del índice
    pub fn get_index_stats(&self) -> Result<(usize, usize, usize)> {
        Ok(self.db.get_embedding_stats()?)
    }
}

/// Calcula similitud coseno entre dos vectores
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();

    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if magnitude_a == 0.0 || magnitude_b == 0.0 {
        return 0.0;
    }

    dot_product / (magnitude_a * magnitude_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{EmbeddingConfig, TextChunker};
    use std::env;

    fn create_test_db() -> NotesDatabase {
        let temp_dir = env::temp_dir();
        let db_path = temp_dir.join(format!("test_search_{}.db", chrono::Utc::now().timestamp()));
        NotesDatabase::new(&db_path).unwrap()
    }

    #[test]
    fn test_cosine_similarity() {
        // Vectores idénticos
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&v1, &v2);
        assert!((sim - 1.0).abs() < 0.001);

        // Vectores ortogonales
        let v3 = vec![1.0, 0.0, 0.0];
        let v4 = vec![0.0, 1.0, 0.0];
        let sim2 = cosine_similarity(&v3, &v4);
        assert!(sim2.abs() < 0.001);

        // Vectores opuestos
        let v5 = vec![1.0, 0.0, 0.0];
        let v6 = vec![-1.0, 0.0, 0.0];
        let sim3 = cosine_similarity(&v5, &v6);
        assert!((sim3 + 1.0).abs() < 0.001);

        // Vectores diferentes longitudes
        let v7 = vec![1.0, 0.0];
        let v8 = vec![1.0, 0.0, 0.0];
        let sim4 = cosine_similarity(&v7, &v8);
        assert_eq!(sim4, 0.0);
    }

    #[test]
    fn test_create_snippet() {
        let text =
            "Este es un texto largo que necesita ser truncado para crear un snippet apropiado.";

        // Snippet más corto que el texto
        let snippet = SearchResult::create_snippet(text, 30);
        assert!(snippet.len() <= 33); // 30 + "..."
        assert!(snippet.ends_with("..."));

        // Snippet más largo que el texto
        let snippet2 = SearchResult::create_snippet(text, 200);
        assert_eq!(snippet2, text);
    }

    #[tokio::test]
    async fn test_search_empty_index() {
        let mut config = EmbeddingConfig::default();
        config.enabled = true;
        config.api_key = Some("test-key".to_string());

        let client = EmbeddingClient::new(config).unwrap();
        let db = create_test_db();

        let search = SemanticSearch::new(client, db);

        // Buscar en índice vacío debe retornar lista vacía
        let results = search.search("test query", SearchOptions::default()).await;
        assert!(results.is_err() || results.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_search_creation() {
        let mut config = EmbeddingConfig::default();
        config.enabled = true;
        config.api_key = Some("test-key".to_string());

        let client = EmbeddingClient::new(config).unwrap();
        let db = create_test_db();

        let _search = SemanticSearch::new(client, db);
    }

    #[test]
    fn test_group_by_note() {
        let results = vec![
            SearchResult {
                note_path: PathBuf::from("nota1.md"),
                chunk_index: 0,
                chunk_text: "chunk 0".to_string(),
                similarity: 0.9,
                snippet: "chunk 0".to_string(),
            },
            SearchResult {
                note_path: PathBuf::from("nota1.md"),
                chunk_index: 1,
                chunk_text: "chunk 1".to_string(),
                similarity: 0.8,
                snippet: "chunk 1".to_string(),
            },
            SearchResult {
                note_path: PathBuf::from("nota2.md"),
                chunk_index: 0,
                chunk_text: "chunk 0".to_string(),
                similarity: 0.95,
                snippet: "chunk 0".to_string(),
            },
        ];

        let grouped = SemanticSearch::group_by_note(results);

        // Deberían ser 2 notas
        assert_eq!(grouped.len(), 2);

        // La primera debería ser nota2.md (similarity 0.95)
        assert_eq!(grouped[0].0, PathBuf::from("nota2.md"));
        assert_eq!(grouped[0].1.len(), 1);

        // La segunda debería ser nota1.md (max similarity 0.9)
        assert_eq!(grouped[1].0, PathBuf::from("nota1.md"));
        assert_eq!(grouped[1].1.len(), 2);
    }

    #[test]
    fn test_search_options_default() {
        let options = SearchOptions::default();
        assert_eq!(options.limit, 10);
        assert_eq!(options.min_similarity, 0.2); // Threshold permisivo por defecto
        assert_eq!(options.snippet_length, 200);
        assert!(options.folder_filter.is_none());
    }

    #[tokio::test]
    async fn test_get_index_stats() {
        let mut config = EmbeddingConfig::default();
        config.enabled = true;
        config.api_key = Some("test-key".to_string());

        let client = EmbeddingClient::new(config).unwrap();
        let db = create_test_db();

        let search = SemanticSearch::new(client, db);

        let stats = search.get_index_stats().unwrap();
        assert_eq!(stats.0, 0); // notes
        assert_eq!(stats.1, 0); // chunks
        assert_eq!(stats.2, 0); // tokens
    }
}
