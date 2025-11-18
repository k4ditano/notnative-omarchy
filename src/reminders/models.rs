use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Prioridad de un recordatorio
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

impl Priority {
    pub fn from_str(s: &str) -> Self {
        let s_lower = s.to_lowercase();
        match s_lower.as_str() {
            "baja" | "low" => Self::Low,
            "media" | "medium" => Self::Medium,
            "alta" | "high" => Self::High,
            "urgente" | "urgent" => Self::Urgent,
            _ => Self::Medium,
        }
    }

    pub fn to_str(self, spanish: bool) -> &'static str {
        match (self, spanish) {
            (Self::Low, true) => "baja",
            (Self::Low, false) => "low",
            (Self::Medium, true) => "media",
            (Self::Medium, false) => "medium",
            (Self::High, true) => "alta",
            (Self::High, false) => "high",
            (Self::Urgent, true) => "urgente",
            (Self::Urgent, false) => "urgent",
        }
    }

    pub fn to_i32(self) -> i32 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
            Self::Urgent => 3,
        }
    }

    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Low,
            1 => Self::Medium,
            2 => Self::High,
            3 => Self::Urgent,
            _ => Self::Medium,
        }
    }

    /// Color GTK para la UI
    pub fn color(&self) -> &'static str {
        match self {
            Self::Low => "#4a90e2",    // Azul
            Self::Medium => "#f5a623", // Naranja
            Self::High => "#e74c3c",   // Rojo
            Self::Urgent => "#8e44ad", // Púrpura
        }
    }
}

/// Patrón de repetición
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatPattern {
    None,
    Daily,
    Weekly,
    Monthly,
}

impl RepeatPattern {
    pub fn from_str(s: &str) -> Self {
        let s_lower = s.to_lowercase();
        match s_lower.as_str() {
            "diario" | "daily" | "diariamente" => Self::Daily,
            "semanal" | "weekly" | "semanalmente" => Self::Weekly,
            "mensual" | "monthly" | "mensualmente" => Self::Monthly,
            "ninguno" | "none" | "no" => Self::None,
            _ => Self::None,
        }
    }

    pub fn to_str(self, spanish: bool) -> &'static str {
        match (self, spanish) {
            (Self::None, true) => "ninguno",
            (Self::None, false) => "none",
            (Self::Daily, true) => "diario",
            (Self::Daily, false) => "daily",
            (Self::Weekly, true) => "semanal",
            (Self::Weekly, false) => "weekly",
            (Self::Monthly, true) => "mensual",
            (Self::Monthly, false) => "monthly",
        }
    }

    pub fn to_i32(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Daily => 1,
            Self::Weekly => 2,
            Self::Monthly => 3,
        }
    }

    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Daily,
            2 => Self::Weekly,
            3 => Self::Monthly,
            _ => Self::None,
        }
    }
}

/// Estado de un recordatorio
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReminderStatus {
    Pending,
    Completed,
    Snoozed,
}

impl ReminderStatus {
    pub fn to_str(self, spanish: bool) -> &'static str {
        match (self, spanish) {
            (Self::Pending, true) => "pendiente",
            (Self::Pending, false) => "pending",
            (Self::Completed, true) => "completado",
            (Self::Completed, false) => "completed",
            (Self::Snoozed, true) => "pospuesto",
            (Self::Snoozed, false) => "snoozed",
        }
    }

    pub fn to_i32(self) -> i32 {
        match self {
            Self::Pending => 0,
            Self::Completed => 1,
            Self::Snoozed => 2,
        }
    }

    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Pending,
            1 => Self::Completed,
            2 => Self::Snoozed,
            _ => Self::Pending,
        }
    }
}

/// Recordatorio completo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub id: i64,
    pub note_id: Option<i64>,
    pub title: String,
    pub description: Option<String>,
    pub due_date: DateTime<Utc>,
    pub priority: Priority,
    pub status: ReminderStatus,
    pub snooze_until: Option<DateTime<Utc>>,
    pub repeat_pattern: RepeatPattern,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Reminder {
    /// Verifica si el recordatorio debe dispararse ahora
    pub fn should_trigger(&self) -> bool {
        if self.status == ReminderStatus::Completed {
            return false;
        }

        let now = Utc::now();

        // Si está pospuesto, verificar la fecha de snooze
        if let Some(snooze) = self.snooze_until {
            return now >= snooze;
        }

        // Verificar la fecha de vencimiento
        now >= self.due_date
    }

    /// Calcula la próxima fecha según el patrón de repetición
    pub fn next_occurrence(&self) -> Option<DateTime<Utc>> {
        use chrono::Duration;

        match self.repeat_pattern {
            RepeatPattern::None => None,
            RepeatPattern::Daily => Some(self.due_date + Duration::days(1)),
            RepeatPattern::Weekly => Some(self.due_date + Duration::weeks(1)),
            RepeatPattern::Monthly => {
                // Aproximación: 30 días
                Some(self.due_date + Duration::days(30))
            }
        }
    }

    /// Formatea la fecha para mostrar en UI
    pub fn format_due_date(&self, spanish: bool) -> String {
        use chrono::Local;

        let local_time = self.due_date.with_timezone(&Local);
        let now = Local::now();

        // Si es hoy
        if local_time.date_naive() == now.date_naive() {
            if spanish {
                format!("Hoy a las {}", local_time.format("%H:%M"))
            } else {
                format!("Today at {}", local_time.format("%H:%M"))
            }
        }
        // Si es mañana
        else if local_time.date_naive() == (now + Duration::days(1)).date_naive() {
            if spanish {
                format!("Mañana a las {}", local_time.format("%H:%M"))
            } else {
                format!("Tomorrow at {}", local_time.format("%H:%M"))
            }
        }
        // Otra fecha
        else if spanish {
            local_time.format("%d/%m/%Y %H:%M").to_string()
        } else {
            local_time.format("%Y-%m-%d %H:%M").to_string()
        }
    }

    /// Verifica si está vencido
    pub fn is_overdue(&self) -> bool {
        if self.status == ReminderStatus::Completed {
            return false;
        }

        let now = Utc::now();

        if let Some(snooze) = self.snooze_until {
            return now > snooze;
        }

        now > self.due_date
    }
}
