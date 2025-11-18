use anyhow::{Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::database::NotesDatabase;
use super::embedding_client::EmbeddingClient;
use super::embedding_config::IndexStats;
use super::text_chunker::{TextChunk, TextChunker};

/// Progreso de la indexación
#[derive(Debug, Clone)]
pub struct IndexProgress {
    pub total_notes: usize,
    pub processed_notes: usize,
    pub total_chunks: usize,
    pub processed_chunks: usize,
    pub current_note: Option<String>,
    pub errors: usize,
}

impl IndexProgress {
    pub fn percentage(&self) -> f32 {
        if self.total_notes == 0 {
            return 0.0;
        }
        (self.processed_notes as f32 / self.total_notes as f32) * 100.0
    }
}

/// Callback para reportar progreso de indexación
pub type ProgressCallback = Arc<dyn Fn(IndexProgress) + Send + Sync>;

/// Indexador de embeddings para notas
pub struct EmbeddingIndexer {
    client: Arc<EmbeddingClient>,
    db: Arc<Mutex<NotesDatabase>>,
    chunker: TextChunker,
}

impl EmbeddingIndexer {
    /// Crea un nuevo indexador
    pub fn new(client: EmbeddingClient, db: NotesDatabase, chunker: TextChunker) -> Self {
        Self {
            client: Arc::new(client),
            db: Arc::new(Mutex::new(db)),
            chunker,
        }
    }

    /// Indexa una nota individual
    pub async fn index_note(&self, note_path: &Path, content: &str) -> Result<usize> {
        // Chunkear el contenido
        let chunks = self
            .chunker
            .chunk_by_paragraphs(content)
            .context("Error chunkeando texto")?;

        if chunks.is_empty() {
            return Ok(0);
        }

        // Extraer solo el texto de los chunks
        let chunk_texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();

        // Generar embeddings en batch
        let embeddings = self
            .client
            .embed_batch(&chunk_texts)
            .await
            .with_context(|| {
                format!(
                    "Error generando embeddings para {} chunks",
                    chunk_texts.len()
                )
            })?;

        eprintln!(
            "✅ DEBUG indexer: Embeddings recibidos: {} (chunks: {})",
            embeddings.len(),
            chunks.len()
        );

        if embeddings.len() != chunks.len() {
            anyhow::bail!(
                "Mismatch entre chunks ({}) y embeddings ({})",
                chunks.len(),
                embeddings.len()
            );
        }

        eprintln!("🔍 DEBUG indexer: Guardando en base de datos...");
        // Guardar en base de datos
        let note_path_str = note_path.to_string_lossy().to_string();
        let db = self.db.lock().await;

        // Primero eliminar embeddings antiguos de esta nota
        db.delete_embeddings_by_note(&note_path_str)?;

        // Insertar nuevos embeddings
        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            db.insert_embedding(
                &note_path_str,
                chunk.index,
                &chunk.text,
                embedding,
                chunk.token_count,
            )?;
        }

        Ok(chunks.len())
    }

    /// Indexa múltiples notas con reporte de progreso
    pub async fn index_notes(
        &self,
        notes: Vec<(PathBuf, String)>, // (path, content)
        progress_callback: Option<ProgressCallback>,
    ) -> Result<IndexStats> {
        let total_notes = notes.len();
        let mut stats = IndexStats {
            total_notes: 0,
            indexed_notes: 0,
            total_chunks: 0,
            total_tokens: 0,
            skipped_notes: 0,
            errors: Vec::new(),
        };

        let mut progress = IndexProgress {
            total_notes,
            processed_notes: 0,
            total_chunks: 0,
            processed_chunks: 0,
            current_note: None,
            errors: 0,
        };

        for (note_path, content) in notes {
            progress.current_note = Some(note_path.to_string_lossy().to_string());

            if let Some(ref callback) = progress_callback {
                callback(progress.clone());
            }

            match self.index_note(&note_path, &content).await {
                Ok(chunk_count) => {
                    stats.indexed_notes += 1;
                    stats.total_chunks += chunk_count;
                    progress.processed_chunks += chunk_count;
                    progress.total_chunks += chunk_count;
                }
                Err(e) => {
                    let error_msg = format!("Error indexando {}: {}", note_path.display(), e);
                    eprintln!("❌ {}", error_msg);
                    stats.errors.push(error_msg);
                    progress.errors += 1;
                }
            }

            progress.processed_notes += 1;
            stats.total_notes = progress.processed_notes;
        }

        // Reporte final
        if let Some(ref callback) = progress_callback {
            progress.current_note = None;
            callback(progress);
        }

        // Obtener estadísticas de tokens desde la BD
        let db = self.db.lock().await;
        let (indexed_notes, total_chunks, total_tokens) = db.get_embedding_stats()?;
        stats.indexed_notes = indexed_notes;
        stats.total_chunks = total_chunks;
        stats.total_tokens = total_tokens;

        Ok(stats)
    }

    /// Verifica si una nota necesita ser re-indexada
    pub async fn needs_reindex(
        &self,
        note_path: &Path,
        note_modified: chrono::DateTime<Utc>,
    ) -> Result<bool> {
        let note_path_str = note_path.to_string_lossy().to_string();
        let db = self.db.lock().await;

        match db.get_embedding_timestamp(&note_path_str)? {
            Some(indexed_at) => {
                // Re-indexar si la nota fue modificada después de ser indexada
                Ok(note_modified > indexed_at)
            }
            None => {
                // No hay embeddings, necesita indexar
                Ok(true)
            }
        }
    }

    /// Indexa solo las notas que necesitan actualización
    pub async fn index_updated_notes(
        &self,
        notes: Vec<(PathBuf, String, chrono::DateTime<Utc>)>, // (path, content, modified)
        progress_callback: Option<ProgressCallback>,
    ) -> Result<IndexStats> {
        // Filtrar notas que necesitan re-indexación
        let mut notes_to_index = Vec::new();

        for (path, content, modified) in notes {
            if self.needs_reindex(&path, modified).await? {
                notes_to_index.push((path, content));
            }
        }

        println!(
            "📊 {} de {} notas necesitan indexación",
            notes_to_index.len(),
            notes_to_index.len()
        );

        self.index_notes(notes_to_index, progress_callback).await
    }

    /// Obtiene estadísticas actuales de indexación
    pub async fn get_stats(&self) -> Result<IndexStats> {
        let db = self.db.lock().await;
        let (indexed_notes, total_chunks, total_tokens) = db.get_embedding_stats()?;

        Ok(IndexStats {
            total_notes: indexed_notes,
            indexed_notes,
            total_chunks,
            total_tokens,
            skipped_notes: 0,
            errors: Vec::new(),
        })
    }

    /// Elimina los embeddings de una nota
    pub async fn remove_note_embeddings(&self, note_path: &Path) -> Result<()> {
        let note_path_str = note_path.to_string_lossy().to_string();
        let db = self.db.lock().await;
        db.delete_embeddings_by_note(&note_path_str)?;
        Ok(())
    }

    /// Limpia todos los embeddings
    pub async fn clear_all_embeddings(&self) -> Result<()> {
        let db = self.db.lock().await;

        // Obtener todas las notas únicas con embeddings
        let all_embeddings = db.get_all_embeddings()?;
        let unique_notes: std::collections::HashSet<String> = all_embeddings
            .into_iter()
            .map(|embedding_data| embedding_data.note_path)
            .collect();

        // Eliminar embeddings de cada nota
        for note_path in unique_notes {
            db.delete_embeddings_by_note(&note_path)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ChunkConfig, EmbeddingConfig};
    use std::env;

    fn create_test_db() -> NotesDatabase {
        let temp_dir = env::temp_dir();
        let db_path = temp_dir.join(format!("test_indexer_{}.db", Utc::now().timestamp()));
        NotesDatabase::new(&db_path).unwrap()
    }

    #[tokio::test]
    async fn test_indexer_creation() {
        let mut config = EmbeddingConfig::default();
        config.enabled = true;
        config.api_key = Some("test-key".to_string());

        let client = EmbeddingClient::new(config).unwrap();
        let db = create_test_db();
        let chunker = TextChunker::new();

        let _indexer = EmbeddingIndexer::new(client, db, chunker);
    }

    #[tokio::test]
    async fn test_needs_reindex() {
        let mut config = EmbeddingConfig::default();
        config.enabled = true;
        config.api_key = Some("test-key".to_string());

        let client = EmbeddingClient::new(config).unwrap();
        let db = create_test_db();
        let chunker = TextChunker::new();

        let indexer = EmbeddingIndexer::new(client, db, chunker);

        let path = PathBuf::from("/test/note.md");
        let now = Utc::now();

        // Nota nueva debería necesitar indexación
        let needs = indexer.needs_reindex(&path, now).await.unwrap();
        assert!(needs);
    }

    #[tokio::test]
    async fn test_get_stats() {
        let mut config = EmbeddingConfig::default();
        config.enabled = true;
        config.api_key = Some("test-key".to_string());

        let client = EmbeddingClient::new(config).unwrap();
        let db = create_test_db();
        let chunker = TextChunker::new();

        let indexer = EmbeddingIndexer::new(client, db, chunker);

        let stats = indexer.get_stats().await.unwrap();
        assert_eq!(stats.indexed_notes, 0);
        assert_eq!(stats.total_chunks, 0);
        assert_eq!(stats.total_tokens, 0);
    }

    #[tokio::test]
    async fn test_clear_all_embeddings() {
        let mut config = EmbeddingConfig::default();
        config.enabled = true;
        config.api_key = Some("test-key".to_string());

        let client = EmbeddingClient::new(config).unwrap();
        let db = create_test_db();
        let chunker = TextChunker::new();

        let indexer = EmbeddingIndexer::new(client, db, chunker);

        // Limpiar debería funcionar incluso si no hay embeddings
        let result = indexer.clear_all_embeddings().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_index_progress_percentage() {
        let progress = IndexProgress {
            total_notes: 100,
            processed_notes: 50,
            total_chunks: 200,
            processed_chunks: 100,
            current_note: None,
            errors: 0,
        };

        assert_eq!(progress.percentage(), 50.0);

        let empty_progress = IndexProgress {
            total_notes: 0,
            processed_notes: 0,
            total_chunks: 0,
            processed_chunks: 0,
            current_note: None,
            errors: 0,
        };

        assert_eq!(empty_progress.percentage(), 0.0);
    }
}
