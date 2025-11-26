// System tray para GTK4/Wayland usando StatusNotifierItem (ksni)
//
// Este módulo implementa un icono real en la bandeja del sistema que funciona en:
// - Wayland (con paneles compatibles con SNI: waybar, swaybar, etc.)
// - X11 (con cualquier panel tradicional)
//
// El icono aparece cuando la ventana está oculta y permite:
// - Click izquierdo: Mostrar/ocultar ventana
// - Click derecho: Menú con opciones (Mostrar, Ocultar, Salir)

use crate::app::AppMsg;
use crate::i18n::I18n;
use relm4::ComponentSender;
use relm4::gtk::glib;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const CONTROL_FILE: &str = "/tmp/notnative.control";

// Estructura para el StatusNotifierItem
struct NotNativeTray {
    sender: ComponentSender<crate::app::MainApp>,
    is_visible: Arc<AtomicBool>,
    i18n: Arc<std::sync::Mutex<I18n>>,
}

impl ksni::Tray for NotNativeTray {
    fn id(&self) -> String {
        "notnative".to_string()
    }

    fn title(&self) -> String {
        "NotNative".to_string()
    }

    fn icon_name(&self) -> String {
        // Intentar usar el icono instalado del sistema
        "notnative".to_string()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        // Fallback: Crear un icono simple si no encuentra el del sistema
        // Un icono de 48x48 píxeles en ARGB
        vec![]
    }

    fn status(&self) -> ksni::Status {
        if self.is_visible.load(Ordering::Relaxed) {
            ksni::Status::Active
        } else {
            ksni::Status::Active // Siempre visible cuando la ventana está oculta
        }
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        // Obtener traducciones
        let i18n = self.i18n.lock().unwrap();
        let show_label = i18n.t("tray_show_window");
        let hide_label = i18n.t("tray_hide_window");
        let quit_label = i18n.t("tray_quit");
        drop(i18n); // Liberar el lock antes de crear el menú

        vec![
            StandardItem {
                label: show_label,
                icon_name: "window-restore".to_string(),
                activate: Box::new(|this: &mut Self| {
                    this.is_visible.store(true, Ordering::Relaxed);
                    this.sender.input(AppMsg::ShowWindow);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: hide_label,
                icon_name: "window-minimize".to_string(),
                activate: Box::new(|this: &mut Self| {
                    this.is_visible.store(false, Ordering::Relaxed);
                    this.sender.input(AppMsg::MinimizeToTray);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: quit_label,
                icon_name: "application-exit".to_string(),
                activate: Box::new(|this: &mut Self| {
                    this.sender.input(AppMsg::QuitApp);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        // Click izquierdo: Toggle ventana
        let current = self.is_visible.load(Ordering::Relaxed);
        println!(
            "🖱️  Click en icono de bandeja (ventana actualmente {})",
            if current { "visible" } else { "oculta" }
        );

        if current {
            println!("   → Ocultando ventana");
            self.is_visible.store(false, Ordering::Relaxed);
            self.sender.input(AppMsg::MinimizeToTray);
        } else {
            println!("   → Mostrando ventana");
            self.is_visible.store(true, Ordering::Relaxed);
            self.sender.input(AppMsg::ShowWindow);
        }
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        // Click del medio: Siempre mostrar
        println!("🖱️  Click medio en icono de bandeja → Mostrando ventana");
        self.is_visible.store(true, Ordering::Relaxed);
        self.sender.input(AppMsg::ShowWindow);
    }
}

pub fn create_system_tray(
    sender: ComponentSender<crate::app::MainApp>,
    i18n: std::rc::Rc<std::cell::RefCell<I18n>>,
    window_visible: Arc<AtomicBool>,
) {
    // Limpiar archivo de control si existe
    let _ = std::fs::remove_file(CONTROL_FILE);

    // Usar el estado compartido de visibilidad pasado desde MainApp
    let is_visible = window_visible;
    let is_visible_clone = Arc::clone(&is_visible);

    // Convertir Rc<RefCell<I18n>> a Arc<Mutex<I18n>> para el thread
    let i18n_arc = {
        let i18n_borrowed = i18n.borrow();
        Arc::new(std::sync::Mutex::new(i18n_borrowed.clone()))
    };
    let i18n_clone = Arc::clone(&i18n_arc);

    // Intentar crear el icono de bandeja en un thread separado
    let sender_clone = sender.clone();
    std::thread::spawn(move || {
        println!("🔧 Intentando crear icono de bandeja del sistema...");

        let tray = NotNativeTray {
            sender: sender_clone,
            is_visible: is_visible_clone,
            i18n: i18n_clone,
        };

        println!("🔧 TrayService creando...");
        let service = ksni::TrayService::new(tray);

        println!("✅ Icono de bandeja del sistema inicializado (StatusNotifierItem)");
        println!("💡 El icono debería aparecer en tu panel/barra de sistema");
        println!("   Compatible con: waybar, swaybar, KDE Plasma, AGS (con widget systray)");
        println!();
        println!("⚠️  Si NO aparece el icono:");
        println!("   - Verifica que tu barra soporte StatusNotifierItem (SNI)");
        println!("   - En AGS: Asegúrate de tener el widget 'systemtray' configurado");
        println!("   - Usa el script de control: notnative-control.sh show");
        println!(
            "   - O crea un atajo: bind = SUPER, N, exec, echo 'toggle' > /tmp/notnative.control"
        );

        // spawn() no retorna nada, simplemente bloquea el thread
        // Mantener el servicio vivo
        service.spawn();
    });

    // Sistema de fallback: Monitorear archivo de control cada 500ms
    // (útil si el icono SNI no funciona en el panel del usuario)
    let monitor_counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let monitor_counter_clone = monitor_counter.clone();

    glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        if std::path::Path::new(CONTROL_FILE).exists() {
            if let Ok(command) = std::fs::read_to_string(CONTROL_FILE) {
                let command = command.trim();
                println!("📱 Comando recibido del archivo de control: '{}'", command);

                match command {
                    "show" => {
                        is_visible.store(true, Ordering::Relaxed);
                        sender.input(AppMsg::ShowWindow);
                        println!("   ➜ Ejecutando: Mostrar ventana");
                    }
                    "hide" => {
                        is_visible.store(false, Ordering::Relaxed);
                        sender.input(AppMsg::MinimizeToTray);
                        println!("   ➜ Ejecutando: Ocultar ventana");
                    }
                    "toggle" => {
                        let current = is_visible.load(Ordering::Relaxed);
                        is_visible.store(!current, Ordering::Relaxed);
                        if current {
                            sender.input(AppMsg::MinimizeToTray);
                            println!("   ➜ Ejecutando: Ocultar ventana (toggle)");
                        } else {
                            sender.input(AppMsg::ShowWindow);
                            println!("   ➜ Ejecutando: Mostrar ventana (toggle)");
                        }
                    }
                    "quicknote" => {
                        sender.input(AppMsg::ToggleQuickNote);
                        println!("   ➜ Ejecutando: Toggle Quick Note");
                    }
                    "quicknote-new" => {
                        sender.input(AppMsg::NewQuickNote);
                        println!("   ➜ Ejecutando: Nueva Quick Note");
                    }
                    "quit" => {
                        sender.input(AppMsg::QuitApp);
                        println!("   ➜ Ejecutando: Salir");
                    }
                    _ => {
                        eprintln!("⚠️  Comando desconocido: '{}'", command);
                    }
                }
                // Limpiar el archivo después de leer
                let _ = std::fs::remove_file(CONTROL_FILE);
            }
        } else {
            // Solo mostrar cada 120 iteraciones (cada minuto) para no spamear
            let count = monitor_counter_clone.fetch_add(1, Ordering::Relaxed);
            if count == 0 {
                println!(
                    "🔄 Sistema de control por archivo activo (monitoreando /tmp/notnative.control)"
                );
            }
        }
        glib::ControlFlow::Continue
    });

    println!("✅ Sistema de control inicializado");
    println!(
        "💡 Controla la app con: echo 'show|hide|toggle|quicknote|quicknote-new|quit' > /tmp/notnative.control"
    );
    println!("💡 O usa el icono de la bandeja del sistema si está disponible");
    println!("📝 Quick Notes: echo 'quicknote' > /tmp/notnative.control");
}
