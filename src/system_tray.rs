// System tray para GTK4/Wayland
//
// Nota: El system tray tradicional (libayatana-appindicator) requiere GTK3.
// Para GTK4/Wayland, usamos una combinación de:
// 1. D-Bus para recibir comandos externos (mostrar/ocultar ventana)
// 2. Archivo de control para comunicación entre instancias
//
// Desde la terminal puedes controlar la app con:
//   echo "show" > /tmp/notnative.control
//   echo "hide" > /tmp/notnative.control
//   echo "quit" > /tmp/notnative.control

use crate::app::AppMsg;
use relm4::ComponentSender;
use relm4::gtk::{self, glib};
use std::fs;
use std::path::Path;

const CONTROL_FILE: &str = "/tmp/notnative.control";

pub fn create_system_tray(sender: ComponentSender<crate::app::MainApp>) {
    // Limpiar archivo de control si existe
    let _ = fs::remove_file(CONTROL_FILE);

    // Monitorear archivo de control cada 500ms
    glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        if Path::new(CONTROL_FILE).exists() {
            if let Ok(command) = fs::read_to_string(CONTROL_FILE) {
                let command = command.trim();
                match command {
                    "show" => {
                        sender.input(AppMsg::ShowWindow);
                        println!("📱 Comando recibido: Mostrar ventana");
                    }
                    "hide" => {
                        sender.input(AppMsg::MinimizeToTray);
                        println!("📱 Comando recibido: Ocultar ventana");
                    }
                    "toggle" => {
                        // Por ahora solo mostramos
                        sender.input(AppMsg::ShowWindow);
                        println!("📱 Comando recibido: Toggle ventana");
                    }
                    "quit" => {
                        sender.input(AppMsg::QuitApp);
                        println!("📱 Comando recibido: Salir");
                    }
                    _ => {
                        eprintln!("⚠️  Comando desconocido: {}", command);
                    }
                }
                // Limpiar el archivo después de leer
                let _ = fs::remove_file(CONTROL_FILE);
            }
        }
        glib::ControlFlow::Continue
    });

    println!("✅ Sistema de control inicializado");
    println!("💡 Controla la app con: echo 'show|hide|quit' > /tmp/notnative.control");
}
