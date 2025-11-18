use super::models::Reminder;
use crate::i18n::{I18n, Language};
use relm4::ComponentSender;
use relm4::gtk::glib;
use std::sync::{Arc, Mutex};

/// Sistema de notificaciones para recordatorios
#[derive(Debug)]
pub struct ReminderNotifier {
    app_sender: Arc<Mutex<Option<ComponentSender<crate::app::MainApp>>>>,
    i18n: Arc<Mutex<I18n>>,
}

impl ReminderNotifier {
    pub fn new(i18n: Arc<Mutex<I18n>>) -> Self {
        Self {
            app_sender: Arc::new(Mutex::new(None)),
            i18n,
        }
    }

    /// Configura el sender de la app para toast interno
    pub fn set_app_sender(&self, sender: ComponentSender<crate::app::MainApp>) {
        let mut app_sender = self.app_sender.lock().unwrap();
        *app_sender = Some(sender);
    }

    /// Envía una notificación para un recordatorio
    pub fn notify(&self, reminder: &Reminder) {
        // 1. Notificación de escritorio (libnotify)
        self.send_desktop_notification(reminder);

        // 2. Toast interno en la app
        self.send_internal_notification(reminder);

        // 3. Reproducir sonido (opcional - TODO)
        // self.play_notification_sound();
    }

    /// Envía notificación de escritorio usando notify-rust
    fn send_desktop_notification(&self, reminder: &Reminder) {
        let i18n = self.i18n.lock().unwrap();
        let is_spanish = i18n.current_language() == Language::Spanish;

        let title = if is_spanish {
            "🔔 Recordatorio"
        } else {
            "🔔 Reminder"
        };

        let body = format!(
            "{}\n{}",
            reminder.title,
            reminder.description.as_deref().unwrap_or("")
        );

        // Intentar enviar notificación de escritorio
        #[cfg(feature = "notify")]
        {
            use notify_rust::{Notification, Timeout};

            if let Err(e) = Notification::new()
                .summary(title)
                .body(&body)
                .icon("appointment-soon")
                .timeout(Timeout::Milliseconds(8000))
                .show()
            {
                eprintln!("⚠️ Error enviando notificación desktop: {}", e);
            } else {
                println!("✅ Notificación desktop enviada: {}", reminder.title);
            }
        }

        #[cfg(not(feature = "notify"))]
        {
            println!("ℹ️ Notificaciones desktop deshabilitadas (recompila con --features notify)");
            println!("🔔 {}: {}", title, body);
        }
    }

    /// Envía notificación interna usando el toast de la app
    fn send_internal_notification(&self, reminder: &Reminder) {
        let i18n = self.i18n.lock().unwrap();
        let is_spanish = i18n.current_language() == Language::Spanish;

        let message = if is_spanish {
            format!("🔔 Recordatorio: {}", reminder.title)
        } else {
            format!("🔔 Reminder: {}", reminder.title)
        };

        // Enviar a través del toast de la app
        if let Some(sender) = self.app_sender.lock().unwrap().as_ref() {
            use crate::app::AppMsg;
            sender.input(AppMsg::ShowNotification(message.clone()));
        }

        println!("{}", message);
    }
}
