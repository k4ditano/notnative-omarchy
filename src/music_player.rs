use libmpv2::Mpv;
use rustypipe::{client::RustyPipe, param::search_filter::SearchFilter};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Estado del reproductor de música
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Idle,    // Sin canción cargada
    Loading, // Cargando canción
    Playing, // Reproduciendo
    Paused,  // En pausa
    Error,   // Error en reproducción
}

/// Información de una canción
#[derive(Debug, Clone)]
pub struct Song {
    pub title: String,
    pub artists: Vec<String>,
    pub video_id: String,
    pub duration: Option<String>,
}

impl Song {
    pub fn new(title: String, video_id: String, artists: Vec<String>) -> Self {
        Self {
            title,
            artists,
            video_id,
            duration: None,
        }
    }

    pub fn artist_names(&self) -> String {
        self.artists.join(", ")
    }
}

/// Errores del reproductor
#[derive(Error, Debug)]
pub enum PlayerError {
    #[error("MPV error: {0}")]
    Mpv(#[from] libmpv2::Error),

    #[error("Failed to initialize player")]
    InitializationError,

    #[error("YouTube fetch error: {0}")]
    YoutubeFetch(String),

    #[error("No song loaded")]
    NoSong,

    #[error("RustyPipe error: {0}")]
    RustyPipe(String),
}

/// Cliente de YouTube para buscar y obtener URLs de audio
pub struct YouTubeClient {
    client: RustyPipe,
}

impl YouTubeClient {
    pub fn new() -> Self {
        // Crear directorio de caché para rustypipe
        let mut cache_dir = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        cache_dir.push("notnative");
        cache_dir.push("rustypipe");

        let rp = RustyPipe::builder()
            .storage_dir(cache_dir)
            .build()
            .expect("Failed to create RustyPipe client");

        Self { client: rp }
    }

    /// Busca música en YouTube
    pub async fn search(&self, query: &str) -> Result<Vec<Song>, PlayerError> {
        println!("Buscando música en YouTube: {}", query);

        // Buscar en YouTube (filtro de música se aplicará en los resultados)
        let results = self
            .client
            .query()
            .search(query)
            .await
            .map_err(|e| PlayerError::RustyPipe(format!("Search error: {:?}", e)))?;

        let mut songs = Vec::new();

        // Convertir resultados a nuestro formato Song (filtrar solo videos con música)
        for item in results.items.items.iter().take(10) {
            use rustypipe::model::YouTubeItem;

            if let YouTubeItem::Video(video) = item {
                // Extraer nombre del canal como artista
                let channel_name = video
                    .channel
                    .as_ref()
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string());
                let artists = vec![channel_name];

                songs.push(Song {
                    title: video.name.clone(),
                    video_id: video.id.clone(),
                    artists,
                    duration: video.duration.map(|d| format!("{}:{:02}", d / 60, d % 60)),
                });
            }
        }

        println!("Encontradas {} canciones", songs.len());
        Ok(songs)
    }

    /// Obtiene la URL del audio de un video
    pub async fn fetch_audio_url(&self, video_id: &str) -> Result<String, PlayerError> {
        println!("Obteniendo URL de audio para: {}", video_id);

        // Obtener información del player
        let player = self
            .client
            .query()
            .player(video_id)
            .await
            .map_err(|e| PlayerError::RustyPipe(format!("Player error: {:?}", e)))?;

        // Buscar el stream de audio de mejor calidad
        let audio_stream = player
            .audio_streams
            .iter()
            .max_by_key(|s| s.bitrate)
            .ok_or_else(|| PlayerError::YoutubeFetch("No audio stream found".to_string()))?;

        println!(
            "Stream de audio encontrado: bitrate={}, codec={:?}",
            audio_stream.bitrate, audio_stream.mime
        );

        Ok(audio_stream.url.clone())
    }
}

/// Reproductor de música usando MPV
pub struct MusicPlayer {
    mpv: Arc<Mpv>,
    state: Arc<Mutex<PlayerState>>,
    current_song: Arc<Mutex<Option<Song>>>,
    current_time: Arc<Mutex<f64>>,
    volume: Arc<Mutex<u8>>,
    youtube_client: YouTubeClient,
}

impl std::fmt::Debug for MusicPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MusicPlayer")
            .field("state", &self.state())
            .field("volume", &self.volume())
            .finish()
    }
}

impl MusicPlayer {
    /// Crea un nuevo reproductor de música
    ///
    /// # Argumentos
    /// * `audio_sink` - Opcional: nombre del sink de audio de PulseAudio (ej: "alsa_output.pci-0000_00_1f.3.analog-stereo")
    pub fn new(audio_sink: Option<&str>) -> Result<Self, PlayerError> {
        // Configurar locale para MPV (requiere LC_NUMERIC=C)
        unsafe {
            libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
        }

        println!("🎵 Inicializando MPV...");
        let mpv = Mpv::new().map_err(|e| {
            eprintln!("❌ Error creando instancia MPV: {:?}", e);
            e
        })?;

        // Configurar MPV para reproducción de audio (con manejo de errores individual)
        if let Err(e) = mpv.set_property("video", "no") {
            eprintln!("⚠️  Error configurando 'video': {:?}", e);
        }

        // IMPORTANTE: Habilitar ytdl para soporte de YouTube
        if let Err(e) = mpv.set_property("ytdl", "yes") {
            eprintln!("⚠️  Error configurando 'ytdl': {:?}", e);
            eprintln!("    ADVERTENCIA: Sin ytdl, MPV no podrá reproducir URLs de YouTube");
            eprintln!("    Instala yt-dlp: sudo apt install yt-dlp");
        } else {
            println!("✓ ytdl habilitado para soporte de YouTube");
        }

        if let Err(e) = mpv.set_property("audio-channels", "stereo") {
            eprintln!("⚠️  Error configurando 'audio-channels': {:?}", e);
        }

        if let Err(e) = mpv.set_property("volume", 80) {
            eprintln!("⚠️  Error configurando 'volume': {:?}", e);
        }

        // Asegurarse de que el audio no está silenciado
        if let Err(e) = mpv.set_property("mute", false) {
            eprintln!("⚠️  Error configurando 'mute': {:?}", e);
        }

        // Configurar salida de audio (PulseAudio)
        // Importante: MPV necesita usar "pulse" como backend primero
        if let Err(e) = mpv.set_property("ao", "pulse") {
            eprintln!("⚠️  Error configurando 'ao': {:?}", e);
        }

        // Configuraciones adicionales de PulseAudio
        // pulse-allow-suspended: permite usar PulseAudio incluso si el sink está suspendido
        if let Err(e) = mpv.set_property("pulse-allow-suspended", true) {
            eprintln!("⚠️  Error configurando 'pulse-allow-suspended': {:?}", e);
        }

        // pulse-latency-hacks: habilita hacks de latencia para PulseAudio
        if let Err(e) = mpv.set_property("pulse-latency-hacks", true) {
            eprintln!("⚠️  Error configurando 'pulse-latency-hacks': {:?}", e);
        }

        // audio-buffer: tamaño del buffer de audio (en segundos)
        // Un valor más alto puede ayudar con problemas de sincronización
        if let Err(e) = mpv.set_property("audio-buffer", 0.5) {
            eprintln!("⚠️  Error configurando 'audio-buffer': {:?}", e);
        }

        // audio-client-name: nombre del cliente de audio para PulseAudio
        if let Err(e) = mpv.set_property("audio-client-name", "NotNative Music") {
            eprintln!("⚠️  Error configurando 'audio-client-name': {:?}", e);
        }

        // gapless-audio: desactivar para evitar problemas con inicio de audio
        if let Err(e) = mpv.set_property("gapless-audio", "no") {
            eprintln!("⚠️  Error configurando 'gapless-audio': {:?}", e);
        }

        // initial-audio-sync: asegurar que el audio se sincroniza correctamente al inicio
        if let Err(e) = mpv.set_property("initial-audio-sync", true) {
            eprintln!("⚠️  Error configurando 'initial-audio-sync': {:?}", e);
        }

        // Configurar dispositivo de audio (usar el sink configurado o auto)
        if let Some(sink) = audio_sink {
            // MPV usa formato "pulse/sink_name" para PulseAudio
            let mpv_audio_device = format!("pulse/{}", sink);
            println!("🔊 Configurando salida de audio: {}", mpv_audio_device);

            if let Err(e) = mpv.set_property("audio-device", mpv_audio_device.as_str()) {
                eprintln!(
                    "⚠️  Error configurando audio-device '{}': {:?}",
                    mpv_audio_device, e
                );
                eprintln!("    Intentando con 'pulse' (default)...");
                let _ = mpv.set_property("audio-device", "pulse");
            } else {
                println!("✓ Salida de audio configurada: {}", mpv_audio_device);
            }
        } else {
            println!("🔊 Usando salida de audio por defecto de PulseAudio");
            if let Err(e) = mpv.set_property("audio-device", "pulse") {
                eprintln!("⚠️  Error configurando 'audio-device': {:?}", e);
            }
        }

        // Optimizaciones de caché y red (opcional, no falla si no funciona)
        let _ = mpv.set_property("cache", "yes");
        let _ = mpv.set_property("demuxer-max-bytes", 128 * 1024 * 1024);
        let _ = mpv.set_property("demuxer-readahead-secs", 20.0);
        let _ = mpv.set_property(
            "http-header-fields",
            "User-Agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
        );

        println!("✓ MPV inicializado correctamente");

        Ok(Self {
            mpv: Arc::new(mpv),
            state: Arc::new(Mutex::new(PlayerState::Idle)),
            current_song: Arc::new(Mutex::new(None)),
            current_time: Arc::new(Mutex::new(0.0)),
            volume: Arc::new(Mutex::new(80)),
            youtube_client: YouTubeClient::new(),
        })
    }

    /// Obtiene el estado actual del reproductor
    pub fn state(&self) -> PlayerState {
        *self.state.lock().unwrap()
    }

    /// Obtiene la canción actual
    pub fn current_song(&self) -> Option<Song> {
        self.current_song.lock().unwrap().clone()
    }

    /// Obtiene el tiempo actual de reproducción
    pub fn current_time(&self) -> f64 {
        // Intentar obtener el tiempo desde MPV
        if let Ok(time) = self.mpv.get_property::<f64>("time-pos") {
            *self.current_time.lock().unwrap() = time;
            time
        } else {
            *self.current_time.lock().unwrap()
        }
    }

    /// Obtiene el volumen actual (0-100)
    pub fn volume(&self) -> u8 {
        *self.volume.lock().unwrap()
    }

    /// Busca música en YouTube
    pub async fn search(&self, query: &str) -> Result<Vec<Song>, PlayerError> {
        self.youtube_client.search(query).await
    }

    /// Reproduce una canción
    pub async fn play(&self, song: Song) -> Result<(), PlayerError> {
        *self.state.lock().unwrap() = PlayerState::Loading;
        *self.current_song.lock().unwrap() = Some(song.clone());

        println!("⏳ Cargando: {} - {}", song.title, song.artist_names());

        // IMPORTANTE: Despertar el sink de audio si está suspendido
        // Esto es necesario porque MPV no siempre activa sinks suspendidos
        if let Ok(device) = self.mpv.get_property::<String>("audio-device") {
            if device.starts_with("pulse/") {
                let sink_name = device.strip_prefix("pulse/").unwrap_or("");
                println!("🔊 Activando sink de audio: {}", sink_name);

                // Reproducir silencio brevemente para activar el sink
                let _ = std::process::Command::new("paplay")
                    .arg("/dev/zero")
                    .arg("--device")
                    .arg(sink_name)
                    .arg("--channels=2")
                    .arg("--rate=48000")
                    .arg("--format=s16le")
                    .arg("--volume=0")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .and_then(|mut child| {
                        // Esperar 50ms y matar el proceso
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        child.kill()
                    });

                println!("✓ Sink activado");
            }
        }

        println!("🎵 Reproduciendo: {} - {}", song.title, song.artist_names());

        // CRÍTICO: Usar el video ID directamente - MPV con ytdl manejará la descarga
        // Esto es más confiable que obtener el URL manualmente
        let ytdl_url = format!("https://www.youtube.com/watch?v={}", song.video_id);
        println!("📺 URL de YouTube: {}", ytdl_url);

        // CRÍTICO: Asegurarse de que no está pausado ANTES de cargar
        let _ = self.mpv.set_property("pause", false);

        // Forzar volumen alto para asegurar que se escucha
        let _ = self.mpv.set_property("volume", 100);

        // Cargar archivo usando ytdl - MPV manejará YouTube internamente
        self.mpv.command("loadfile", &[&ytdl_url, "replace"])?;

        // CRITICO: Esperar a que MPV realmente cargue el stream
        println!("Esperando a que MPV cargue el stream...");
        let mut loaded = false;
        for wait_attempt in 1..=20 {
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            // Verificar si MPV ya no esta idle
            if let Ok(idle) = self.mpv.get_property::<bool>("idle-active") {
                if !idle {
                    println!("Stream cargado en MPV (intento {})", wait_attempt);
                    loaded = true;
                    break;
                }
            }

            // Verificar tracks
            if let Ok(track_count) = self.mpv.get_property::<i64>("track-list/count") {
                if track_count > 0 {
                    println!(
                        "Tracks detectados: {} (intento {})",
                        track_count, wait_attempt
                    );
                    loaded = true;
                    break;
                }
            }

            if wait_attempt % 5 == 0 {
                println!("Cargando... ({}/20)", wait_attempt);
            }
        }

        if !loaded {
            println!("ADVERTENCIA: MPV no cargo el stream en 4 segundos");
        }

        // Forzar reproduccion
        for _attempt in 1..=3 {
            let _ = self.mpv.set_property("pause", false);
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
        if let Ok(vol) = self.mpv.get_property::<i64>("volume") {
            println!("🔊 Volumen actual de MPV: {}%", vol);
        }

        if let Ok(muted) = self.mpv.get_property::<bool>("mute") {
            println!("🔇 MPV silenciado: {}", muted);
            if muted {
                println!("⚠️  Desactivando silencio...");
                let _ = self.mpv.set_property("mute", false);
            }
        }

        if let Ok(device) = self.mpv.get_property::<String>("audio-device") {
            println!("🔊 Dispositivo de audio activo: {}", device);
        }

        // Diagnósticos adicionales
        if let Ok(ao) = self.mpv.get_property::<String>("current-ao") {
            println!("🔊 Audio output driver actual: {}", ao);
        }

        if let Ok(idle) = self.mpv.get_property::<bool>("idle-active") {
            println!("💤 MPV idle: {}", idle);
        }

        if let Ok(core_idle) = self.mpv.get_property::<bool>("core-idle") {
            println!("💤 MPV core-idle: {}", core_idle);
        }

        if let Ok(demuxer_cache_state) = self.mpv.get_property::<String>("demuxer-cache-state") {
            println!("📦 Cache state: {}", demuxer_cache_state);
        }

        // Verificar si hay audio tracks
        if let Ok(track_list) = self.mpv.get_property::<String>("track-list") {
            println!("🎵 Track list: {}", track_list);
        }

        // Verificar si el archivo realmente se cargó
        if let Ok(path) = self.mpv.get_property::<String>("path") {
            println!(
                "📁 Path cargado: {}",
                &path[..std::cmp::min(path.len(), 100)]
            );
        }

        *self.state.lock().unwrap() = PlayerState::Playing;

        Ok(())
    }

    /// Pausa la reproducción
    pub fn pause(&self) -> Result<(), PlayerError> {
        if self.state() == PlayerState::Playing {
            self.mpv.set_property("pause", true)?;
            *self.state.lock().unwrap() = PlayerState::Paused;
            println!("⏸️ Música pausada");
            Ok(())
        } else {
            Err(PlayerError::NoSong)
        }
    }

    /// Reanuda la reproducción
    pub fn resume(&self) -> Result<(), PlayerError> {
        if self.state() == PlayerState::Paused {
            self.mpv.set_property("pause", false)?;
            *self.state.lock().unwrap() = PlayerState::Playing;
            println!("▶️ Música reanudada");
            Ok(())
        } else {
            Err(PlayerError::NoSong)
        }
    }

    /// Alterna entre reproducir y pausar
    pub fn toggle_play_pause(&self) -> Result<(), PlayerError> {
        match self.state() {
            PlayerState::Playing => self.pause(),
            PlayerState::Paused => self.resume(),
            _ => Err(PlayerError::NoSong),
        }
    }

    /// Detiene la reproducción
    pub fn stop(&self) -> Result<(), PlayerError> {
        self.mpv.command("stop", &[])?;
        *self.state.lock().unwrap() = PlayerState::Idle;
        *self.current_song.lock().unwrap() = None;
        *self.current_time.lock().unwrap() = 0.0;
        println!("⏹️ Reproducción detenida");
        Ok(())
    }

    /// Salta adelante en la reproducción (segundos)
    pub fn seek_forward(&self, seconds: f64) -> Result<(), PlayerError> {
        if self.state() == PlayerState::Playing || self.state() == PlayerState::Paused {
            self.mpv
                .command("seek", &[&seconds.to_string(), "relative"])?;
            println!("⏩ Avanzando {} segundos", seconds);
            Ok(())
        } else {
            Err(PlayerError::NoSong)
        }
    }

    /// Salta atrás en la reproducción (segundos)
    pub fn seek_backward(&self, seconds: f64) -> Result<(), PlayerError> {
        if self.state() == PlayerState::Playing || self.state() == PlayerState::Paused {
            self.mpv
                .command("seek", &[&format!("-{}", seconds), "relative"])?;
            println!("⏪ Retrocediendo {} segundos", seconds);
            Ok(())
        } else {
            Err(PlayerError::NoSong)
        }
    }

    /// Establece el volumen (0-100)
    pub fn set_volume(&self, vol: u8) -> Result<(), PlayerError> {
        let vol = vol.min(100);
        self.mpv.set_property("volume", vol as i64)?;
        *self.volume.lock().unwrap() = vol;
        println!("🔊 Volumen: {}%", vol);
        Ok(())
    }

    /// Aumenta el volumen
    pub fn volume_up(&self) -> Result<(), PlayerError> {
        let current = self.volume();
        self.set_volume(current.saturating_add(10))
    }

    /// Disminuye el volumen
    pub fn volume_down(&self) -> Result<(), PlayerError> {
        let current = self.volume();
        self.set_volume(current.saturating_sub(10))
    }
}

impl Default for MusicPlayer {
    fn default() -> Self {
        Self::new(None).expect("Failed to create music player")
    }
}
