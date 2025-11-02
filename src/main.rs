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

mod app;
mod core;
mod i18n;
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

    let mut combined_css = String::new();
    let mut theme_loaded = false;

    for css_file in &css_files {
        if let Ok(content) = std::fs::read_to_string(css_file) {
            combined_css.push_str(&content);
            combined_css.push('\n');
            theme_loaded = true;
        }
    }

    // Cargar el CSS de la aplicación
    // Prioridad: 1) Sistema instalado, 2) Desarrollo local
    let app_css = std::fs::read_to_string("/usr/share/notnative/assets/style.css")
        .ok()
        .or_else(|| {
            // Rutas de desarrollo
            if let Ok(exe_path) = std::env::current_exe() {
                exe_path
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .map(|p| p.join("assets/style.css"))
                    .and_then(|path| std::fs::read_to_string(&path).ok())
            } else {
                None
            }
        })
        .or_else(|| std::fs::read_to_string("assets/style.css").ok())
        .or_else(|| std::fs::read_to_string("./notnative-app/assets/style.css").ok());

    if let Some(app_css_content) = app_css {
        combined_css.push_str(&app_css_content);
    }

    (combined_css, theme_loaded)
}

fn main() -> anyhow::Result<()> {
    // Inicializar GTK primero
    gtk::init().expect("No se pudo inicializar GTK");
    glib::set_application_name("NotNative");

    // Cargar tema inicial
    let (combined_css, theme_loaded) = load_theme_css();
    let theme_provider = gtk::CssProvider::new();

    if theme_loaded || !combined_css.is_empty() {
        theme_provider.load_from_data(&combined_css);
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("No se pudo obtener el display"),
            &theme_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        println!("✓ Tema Omarchy cargado");
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
