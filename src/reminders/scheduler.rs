use chrono::Utc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::database::ReminderDatabase;
use super::models::{Reminder, ReminderStatus};
use super::notifications::ReminderNotifier;

/// Scheduler que monitorea recordatorios pendientes
#[derive(Debug)]
pub struct ReminderScheduler {
    db: Arc<Mutex<ReminderDatabase>>,
    notifier: Arc<ReminderNotifier>,
    running: Arc<Mutex<bool>>,
}

impl ReminderScheduler {
    pub fn new(db: Arc<Mutex<ReminderDatabase>>, notifier: Arc<ReminderNotifier>) -> Self {
        Self {
            db,
            notifier,
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Inicia el scheduler en un thread separado
    pub fn start(&self) {
        let mut running = self.running.lock().unwrap();
        if *running {
            println!("⏰ Scheduler ya está corriendo");
            return;
        }

        *running = true;
        drop(running);

        let db = Arc::clone(&self.db);
        let notifier = Arc::clone(&self.notifier);
        let running_flag = Arc::clone(&self.running);

        std::thread::spawn(move || {
            println!("⏰ Scheduler de recordatorios iniciado (check cada 30s)");

            loop {
                // Verificar si debe seguir corriendo
                {
                    let running = running_flag.lock().unwrap();
                    if !*running {
                        println!("⏰ Scheduler detenido");
                        break;
                    }
                }

                // Verificar recordatorios pendientes
                if let Ok(db_lock) = db.lock() {
                    match db_lock.get_pending_triggers() {
                        Ok(reminders) => {
                            for reminder in reminders {
                                Self::process_reminder(&reminder, &db_lock, &notifier);
                            }
                        }
                        Err(e) => {
                            eprintln!("⚠️ Error al obtener recordatorios pendientes: {}", e);
                        }
                    }
                }

                // Esperar 30 segundos
                std::thread::sleep(Duration::from_secs(30));
            }
        });
    }

    /// Procesa un recordatorio que debe dispararse
    fn process_reminder(reminder: &Reminder, db: &ReminderDatabase, notifier: &ReminderNotifier) {
        println!("🔔 Disparando recordatorio: {}", reminder.title);

        // Enviar notificación
        notifier.notify(reminder);

        // Si tiene patrón de repetición, crear el siguiente
        if let Some(next_date) = reminder.next_occurrence() {
            println!("   ↻ Programando próxima ocurrencia: {}", next_date);

            if let Err(e) = db.create_reminder(
                reminder.note_id,
                &reminder.title,
                reminder.description.as_deref(),
                next_date,
                reminder.priority,
                reminder.repeat_pattern,
            ) {
                eprintln!("   ❌ Error creando repetición: {}", e);
            }

            // Marcar el actual como completado
            if let Err(e) = db.update_status(reminder.id, ReminderStatus::Completed) {
                eprintln!("   ❌ Error actualizando estado: {}", e);
            }
        } else {
            // Sin repetición, simplemente dejar pendiente (el usuario lo completará manualmente)
            println!("   ℹ️ Recordatorio sin repetición, queda pendiente");
        }
    }

    /// Detiene el scheduler
    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
        println!("⏰ Deteniendo scheduler de recordatorios...");
    }

    /// Fuerza una verificación inmediata
    pub fn check_now(&self) {
        if let Ok(db_lock) = self.db.lock() {
            match db_lock.get_pending_triggers() {
                Ok(reminders) => {
                    println!(
                        "🔍 Verificación manual: {} recordatorios pendientes",
                        reminders.len()
                    );
                    for reminder in reminders {
                        Self::process_reminder(&reminder, &db_lock, &self.notifier);
                    }
                }
                Err(e) => {
                    eprintln!("⚠️ Error en verificación manual: {}", e);
                }
            }
        }
    }
}

impl Drop for ReminderScheduler {
    fn drop(&mut self) {
        self.stop();
    }
}
