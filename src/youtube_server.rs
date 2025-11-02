use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread;

/// Servidor HTTP ligero para servir páginas de embed de YouTube
#[derive(Debug)]
pub struct YouTubeEmbedServer {
    port: u16,
    videos: Arc<Mutex<HashMap<String, String>>>,
}

impl YouTubeEmbedServer {
    /// Crea un nuevo servidor en el puerto especificado
    pub fn new(port: u16) -> Self {
        Self {
            port,
            videos: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Registra un video y retorna la URL local para cargarlo
    pub fn register_video(&self, video_id: String) -> String {
        let html = Self::generate_embed_html(&video_id);
        self.videos.lock().unwrap().insert(video_id.clone(), html);
        format!("http://localhost:{}/video/{}", self.port, video_id)
    }

    /// Genera el HTML de embed para un video
    fn generate_embed_html(video_id: &str) -> String {
        format!(r#"
<!DOCTYPE html>
<html>
<head>
    <meta name="referrer" content="no-referrer-when-downgrade">
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta http-equiv="X-UA-Compatible" content="IE=edge">
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        html, body {{
            width: 100%;
            height: 100%;
            overflow: hidden;
            background: #000;
        }}
        iframe {{
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            border: none;
        }}
    </style>
</head>
<body>
    <iframe 
        src="https://www.youtube-nocookie.com/embed/{}?autoplay=0&enablejsapi=1&rel=0&modestbranding=1&playsinline=1&controls=1&fs=1&cc_load_policy=0&iv_load_policy=3&autohide=1" 
        frameborder="0" 
        referrerpolicy="no-referrer-when-downgrade"
        sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-top-navigation-by-user-activation"
        allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share; fullscreen" 
        allowfullscreen
        loading="eager">
    </iframe>
</body>
</html>
        "#, video_id)
    }

    /// Inicia el servidor en un thread separado
    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let videos = Arc::clone(&self.videos);
        let port = self.port;

        thread::spawn(move || {
            let server = tiny_http::Server::http(format!("127.0.0.1:{}", port))
                .expect("Failed to start YouTube embed server");

            println!("YouTube embed server running on http://localhost:{}", port);

            for request in server.incoming_requests() {
                let url = request.url();
                println!("DEBUG SERVER: Request recibido: {}", url);
                
                // Extraer video_id de la URL /video/{video_id}
                if let Some(video_id) = url.strip_prefix("/video/") {
                    let videos_lock = videos.lock().unwrap();
                    
                    if let Some(html) = videos_lock.get(video_id) {
                        println!("DEBUG SERVER: Sirviendo video: {}", video_id);
                        let response = tiny_http::Response::from_string(html.clone())
                            .with_header(
                                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap()
                            )
                            .with_header(
                                tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap()
                            )
                            .with_header(
                                tiny_http::Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..]).unwrap()
                            );
                        let _ = request.respond(response);
                    } else {
                        println!("DEBUG SERVER: Video no encontrado: {}", video_id);
                        let response = tiny_http::Response::from_string("Video not found")
                            .with_status_code(404);
                        let _ = request.respond(response);
                    }
                } else {
                    println!("DEBUG SERVER: Path no reconocido: {}", url);
                    let response = tiny_http::Response::from_string("Not found")
                        .with_status_code(404);
                    let _ = request.respond(response);
                }
            }
        });

        Ok(())
    }
}
