use anyhow::{Context, Result};

/// Obtiene la transcripción de un video de YouTube de forma asíncrona
pub async fn get_transcript_async(video_id: &str) -> Result<String> {
    use yt_transcript_rs::api::YouTubeTranscriptApi;

    // Crear el cliente API
    let api = YouTubeTranscriptApi::new(None, None, None)
        .context("No se pudo crear el cliente de transcripción")?;

    // Idiomas preferidos (español e inglés)
    let languages = &["es", "en"];

    // Obtener la transcripción
    let transcript = api
        .fetch_transcript(video_id, languages, false)
        .await
        .context("No se pudo obtener la transcripción")?;

    // Formatear la transcripción agrupando fragmentos en párrafos más largos
    let mut formatted_text = String::new();
    let mut current_paragraph = String::new();
    let mut paragraph_start_time: Option<f64> = None;
    let paragraph_duration = 30.0; // Agrupar cada 30 segundos

    for snippet in transcript.snippets {
        // Si es el primer snippet o ha pasado suficiente tiempo, crear nuevo párrafo
        let should_break = if let Some(start) = paragraph_start_time {
            snippet.start - start >= paragraph_duration
        } else {
            true
        };

        if should_break && !current_paragraph.is_empty() {
            // Agregar párrafo con timestamp en negrita
            if let Some(start) = paragraph_start_time {
                formatted_text.push_str(&format!(
                    "**[{:02}:{:02}]** {}\n\n",
                    (start as u64) / 60,
                    (start as u64) % 60,
                    current_paragraph.trim()
                ));
            }
            current_paragraph.clear();
            paragraph_start_time = Some(snippet.start);
        } else if paragraph_start_time.is_none() {
            paragraph_start_time = Some(snippet.start);
        }

        // Agregar texto al párrafo actual
        if !current_paragraph.is_empty() {
            current_paragraph.push(' ');
        }
        current_paragraph.push_str(snippet.text.trim());
    }

    // Agregar el último párrafo
    if !current_paragraph.is_empty() {
        if let Some(start) = paragraph_start_time {
            formatted_text.push_str(&format!(
                "**[{:02}:{:02}]** {}\n\n",
                (start as u64) / 60,
                (start as u64) % 60,
                current_paragraph.trim()
            ));
        }
    }

    if formatted_text.is_empty() {
        anyhow::bail!("No se encontró transcripción para este video");
    }

    Ok(formatted_text)
}

/// Versión síncrona que bloquea el hilo actual
pub fn get_transcript(video_id: &str) -> Result<String> {
    // Crear un runtime de tokio para ejecutar código async
    let runtime = tokio::runtime::Runtime::new().context("No se pudo crear el runtime de tokio")?;

    runtime.block_on(get_transcript_async(video_id))
}
