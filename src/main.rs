#![allow(
    clippy::collapsible_if,
    clippy::needless_borrows_for_generic_args,
    clippy::op_ref,
    clippy::manual_strip,
    clippy::needless_option_as_deref,
    clippy::double_ended_iterator_last,
    clippy::inherent_to_string,
    clippy::derivable_impls,
    clippy::single_char_add_str,
    clippy::only_used_in_recursion,
    clippy::while_let_on_iterator,
    clippy::if_same_then_else,
    clippy::match_result_ok,
    clippy::clone_on_copy,
    clippy::len_zero,
    clippy::unnecessary_map_or,
    clippy::unwrap_or_default,
    clippy::field_reassign_with_default,
    dead_code,
    unused_variables,
    unused_imports
)]

mod ai_chat;
mod ai_client;
mod app;
mod core;
mod file_watcher;
mod i18n;
mod mcp;
mod music_player;
mod system_tray;
mod youtube_server;
mod youtube_transcript;

use relm4::{
    RelmApp,
    gtk::{self, gio, glib, prelude::*},
};

use crate::app::{APP_ID, MainApp, ThemePreference};

fn load_theme_css() -> (String, bool) {
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
    let theme_dir = format!("{}/.config/omarchy/current/theme", home_dir);

    let css_files = vec![
        format!("{}/walker.css", theme_dir),
        format!("{}/waybar.css", theme_dir),
        format!("{}/swayosd.css", theme_dir),
    ];

    // Primero, cargamos y extraemos las variables de color de Omarchy
    let mut omarchy_css = String::new();
    let mut theme_loaded = false;

    for css_file in &css_files {
        if let Ok(content) = std::fs::read_to_string(css_file) {
            omarchy_css.push_str(&content);
            omarchy_css.push('\n');
            theme_loaded = true;
        }
    }

    // Cargar el CSS de la aplicación
    // Prioridad: 1) Desarrollo local, 2) Sistema instalado
    println!("🔍 [main.rs] Intentando cargar CSS...");
    let app_css = std::fs::read_to_string("assets/style.css")
        .inspect(|_| println!("✅ [main.rs] CSS cargado desde: assets/style.css"))
        .ok()
        .or_else(|| {
            println!("🔍 [main.rs] Intentando ./notnative-app/assets/style.css");
            std::fs::read_to_string("./notnative-app/assets/style.css")
                .inspect(|_| {
                    println!("✅ [main.rs] CSS cargado desde: ./notnative-app/assets/style.css")
                })
                .ok()
        })
        .or_else(|| {
            // Rutas de desarrollo basadas en el ejecutable
            if let Ok(exe_path) = std::env::current_exe() {
                let css_path = exe_path
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .map(|p| p.join("assets/style.css"));

                if let Some(ref path) = css_path {
                    println!("🔍 [main.rs] Intentando ruta exe: {:?}", path);
                    if let Ok(content) = std::fs::read_to_string(path) {
                        println!("✅ [main.rs] CSS cargado desde ruta exe: {:?}", path);
                        return Some(content);
                    }
                }
            }
            None
        })
        .or_else(|| {
            println!("🔍 [main.rs] Intentando /usr/share/notnative-app/assets/style.css");
            std::fs::read_to_string("/usr/share/notnative-app/assets/style.css")
                .inspect(|_| {
                    println!(
                        "✅ [main.rs] CSS cargado desde: /usr/share/notnative-app/assets/style.css"
                    )
                })
                .ok()
        })
        .or_else(|| {
            println!("🔍 [main.rs] Intentando /usr/share/notnative/assets/style.css (fallback)");
            std::fs::read_to_string("/usr/share/notnative/assets/style.css")
                .inspect(|_| {
                    println!(
                        "✅ [main.rs] CSS cargado desde: /usr/share/notnative/assets/style.css"
                    )
                })
                .ok()
        });

    // Combinamos los CSS: primero las variables de Omarchy, luego el CSS de la app
    let mut combined_css = String::new();

    // Agregar las variables de Omarchy al principio
    if theme_loaded {
        combined_css.push_str("/* Variables de color de Omarchy */\n");
        combined_css.push_str(&omarchy_css);
        combined_css.push('\n');
    }

    // Agregar el CSS de la aplicación
    if let Some(app_css_content) = app_css {
        combined_css.push_str(&app_css_content);
    }

    (combined_css, theme_loaded)
}

fn main() -> anyhow::Result<()> {
    // Single instance detection
    let lock_file_path = "/tmp/notnative.lock";
    let control_file_path = "/tmp/notnative.control";

    // Verificar si ya existe una instancia
    if std::path::Path::new(lock_file_path).exists() {
        // Leer el PID del lock file
        if let Ok(pid_str) = std::fs::read_to_string(lock_file_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                // Verificar si el proceso realmente existe
                let proc_path = format!("/proc/{}", pid);
                if std::path::Path::new(&proc_path).exists() {
                    // El proceso existe, enviar comando para mostrar la ventana
                    println!("✅ NotNative ya está corriendo (PID: {})", pid);
                    println!("📱 Mostrando ventana existente...");

                    // Enviar comando "show" a través del archivo de control
                    if let Err(e) = std::fs::write(control_file_path, "show") {
                        eprintln!("⚠️ Error enviando comando show: {}", e);
                        eprintln!("💡 Puedes mostrar la ventana manualmente con:");
                        eprintln!("   echo 'show' > {}", control_file_path);
                    }

                    std::process::exit(0);
                }
            }
        }
        // Si llegamos aquí, el lock file existe pero el proceso no, lo eliminamos
        let _ = std::fs::remove_file(lock_file_path);
    }

    // Crear lock file con nuestro PID
    let pid = std::process::id();
    std::fs::write(lock_file_path, pid.to_string())?;

    // Asegurar que se elimine el lock file al salir
    let lock_cleanup = lock_file_path.to_string();
    ctrlc::set_handler(move || {
        let _ = std::fs::remove_file(&lock_cleanup);
        std::process::exit(0);
    })?;

    // Inicializar GTK primero
    gtk::init().expect("No se pudo inicializar GTK");
    glib::set_application_name("NotNative");

    // Cargar tema inicial
    let (combined_css, theme_loaded) = load_theme_css();
    let theme_provider = gtk::CssProvider::new();

    if !combined_css.is_empty() {
        theme_provider.load_from_data(&combined_css);
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("No se pudo obtener el display"),
            &theme_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        if theme_loaded {
            println!("✓ Tema Omarchy cargado desde ~/.config/omarchy/current/theme/");
        } else {
            println!("⚠ Tema Omarchy no encontrado, usando estilos por defecto");
        }

        // Debug: mostrar tamaño del CSS cargado
        println!("  CSS size: {} bytes", combined_css.len());
    } else {
        println!("⚠ No se pudo cargar ningún CSS");
    }

    // Usar GTK Application en lugar de Adwaita Application
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    let relm_app = RelmApp::from_app(app);

    relm_app.run::<MainApp>(ThemePreference::FollowSystem);

    Ok(())
}
