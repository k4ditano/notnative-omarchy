use anyhow::{Result, anyhow};
use chrono::{
    DateTime, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike, Utc,
};
use regex::Regex;
use std::sync::LazyLock;

use super::models::{Priority, RepeatPattern};
use crate::i18n::Language;

// ============================================================================
// REGEX ESTÁTICOS - Compilados una sola vez para mejor rendimiento
// ============================================================================

/// Formato V2 español: !!RECORDAR(fecha [prioridad] [repetir=patron], texto)
static SPANISH_REGEX_V2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"!!RECORDAR\(([^,]+),\s*(.*?)\)").unwrap()
});

/// Formato V2 inglés: !!REMIND(date [priority] [repeat=pattern], text)
static ENGLISH_REGEX_V2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"!!REMIND\(([^,]+),\s*(.*?)\)").unwrap()
});

/// Formato Interno (Widget): [REMINDER:params|text]
static INTERNAL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[REMINDER:(.*?)\|(.*?)\]").unwrap()
});

/// Resultado del parsing de un recordatorio
#[derive(Debug, Clone)]
pub struct ParsedReminder {
    pub title: String,
    pub due_date: DateTime<Utc>,
    pub priority: Priority,
    pub repeat_pattern: RepeatPattern,
    pub original_text: String,
}

/// Parser de recordatorios en markdown
#[derive(Debug)]
pub struct ReminderParser;

impl ReminderParser {
    pub fn new() -> Self {
        Self
    }

    /// Extrae todos los recordatorios de un texto
    pub fn extract_reminders(&self, text: &str, language: Language) -> Vec<ParsedReminder> {
        let mut reminders = Vec::new();

        // Buscar formato interno (Widget) - Prioridad alta ya que es lo que hay en el buffer en modo Normal
        for cap in INTERNAL_REGEX.captures_iter(text) {
            let params = cap.get(1).map_or("", |m| m.as_str());
            let title = cap.get(2).map_or("", |m| m.as_str()).trim();
            let original = cap.get(0).map_or("", |m| m.as_str());

            // Usar idioma actual o intentar detectar (el formato interno es agnóstico)
            if let Ok(parsed) = self.parse_params(params, title, original, language) {
                reminders.push(parsed);
            }
        }

        // Buscar en español (V2)
        for cap in SPANISH_REGEX_V2.captures_iter(text) {
            let params = cap.get(1).map_or("", |m| m.as_str());
            let title = cap.get(2).map_or("", |m| m.as_str()).trim();
            let original = cap.get(0).map_or("", |m| m.as_str());

            if let Ok(parsed) = self.parse_params(params, title, original, language) {
                reminders.push(parsed);
            }
        }

        // Buscar en inglés (V2)
        for cap in ENGLISH_REGEX_V2.captures_iter(text) {
            let params = cap.get(1).map_or("", |m| m.as_str());
            let title = cap.get(2).map_or("", |m| m.as_str()).trim();
            let original = cap.get(0).map_or("", |m| m.as_str());

            if let Ok(parsed) = self.parse_params(params, title, original, Language::English) {
                reminders.push(parsed);
            }
        }

        reminders
    }

    /// Parsea los parámetros de un recordatorio
    fn parse_params(
        &self,
        params: &str,
        title: &str,
        original: &str,
        language: Language,
    ) -> Result<ParsedReminder> {
        let parts: Vec<&str> = params.split_whitespace().collect();

        if parts.is_empty() {
            return Err(anyhow!("Parámetros vacíos"));
        }

        // Parsear fecha (primer parámetro puede ser múltiples palabras)
        let (due_date, consumed) = self.parse_date(&parts, language)?;

        // Parsear prioridad y patrón de repetición
        let mut priority = Priority::Medium;
        let mut repeat_pattern = RepeatPattern::None;

        for part in parts.iter().skip(consumed) {
            let part_lower = part.to_lowercase();

            // Detectar prioridad
            if matches!(
                part_lower.as_str(),
                "baja" | "low" | "media" | "medium" | "alta" | "high" | "urgente" | "urgent"
            ) {
                priority = Priority::from_str(part);
            }

            // Detectar patrón de repetición
            if part_lower.starts_with("repetir=") || part_lower.starts_with("repeat=") {
                if let Some(pattern_str) = part_lower.split('=').nth(1) {
                    repeat_pattern = RepeatPattern::from_str(pattern_str);
                }
            } else if matches!(
                part_lower.as_str(),
                "diario" | "daily" | "semanal" | "weekly" | "mensual" | "monthly"
            ) {
                repeat_pattern = RepeatPattern::from_str(part);
            }
        }

        Ok(ParsedReminder {
            title: title.to_string(),
            due_date,
            priority,
            repeat_pattern,
            original_text: original.to_string(),
        })
    }

    /// Parsea una fecha desde los parámetros
    /// Retorna (fecha, cantidad de tokens consumidos)
    fn parse_date(&self, parts: &[&str], language: Language) -> Result<(DateTime<Utc>, usize)> {
        if parts.is_empty() {
            return Err(anyhow!("Sin parámetros de fecha"));
        }

        let first = parts[0].to_lowercase();

        // Palabras clave relativas (intentar ambos idiomas para mayor flexibilidad)
        if first == "hoy" || first == "today" {
            return self
                .parse_time_today(&parts[1..], language)
                .map(|(dt, c)| (dt, c + 1));
        } else if first == "mañana" || first == "manana" || first == "tomorrow" {
            return self
                .parse_time_tomorrow(&parts[1..], language)
                .map(|(dt, c)| (dt, c + 1));
        }

        // Intentar parsear fecha absoluta: YYYY-MM-DD HH:MM o DD/MM/YYYY HH:MM
        if parts.len() >= 2 {
            let date_str = parts[0];
            let time_str = parts[1];

            // Formato ISO: 2025-11-20 15:00
            if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                if let Ok(time) = NaiveTime::parse_from_str(time_str, "%H:%M") {
                    let dt = NaiveDateTime::new(date, time);
                    let local_dt = Local.from_local_datetime(&dt).unwrap();
                    return Ok((local_dt.with_timezone(&Utc), 2));
                }
            }

            // Formato europeo: 20/11/2025 15:00
            if let Ok(date) = NaiveDate::parse_from_str(date_str, "%d/%m/%Y") {
                if let Ok(time) = NaiveTime::parse_from_str(time_str, "%H:%M") {
                    let dt = NaiveDateTime::new(date, time);
                    let local_dt = Local.from_local_datetime(&dt).unwrap();
                    return Ok((local_dt.with_timezone(&Utc), 2));
                }
            }
        }

        // Si solo hay fecha sin hora, usar 09:00 por defecto
        if parts.len() >= 1 {
            let date_str = parts[0];

            if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                let time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
                let dt = NaiveDateTime::new(date, time);
                let local_dt = Local.from_local_datetime(&dt).unwrap();
                return Ok((local_dt.with_timezone(&Utc), 1));
            }

            if let Ok(date) = NaiveDate::parse_from_str(date_str, "%d/%m/%Y") {
                let time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
                let dt = NaiveDateTime::new(date, time);
                let local_dt = Local.from_local_datetime(&dt).unwrap();
                return Ok((local_dt.with_timezone(&Utc), 1));
            }
        }

        Err(anyhow!("Formato de fecha no reconocido"))
    }

    /// Parsea hora para "hoy" / "today"
    fn parse_time_today(
        &self,
        parts: &[&str],
        _language: Language,
    ) -> Result<(DateTime<Utc>, usize)> {
        let now = Local::now();
        let today = now.date_naive();

        if parts.is_empty() {
            // Sin hora específica, usar hora actual + 1 hora
            let future = now + Duration::hours(1);
            return Ok((future.with_timezone(&Utc), 0));
        }

        // Parsear hora: HH:MM
        if let Ok(time) = NaiveTime::parse_from_str(parts[0], "%H:%M") {
            let dt = NaiveDateTime::new(today, time);
            let local_dt = Local.from_local_datetime(&dt).unwrap();
            return Ok((local_dt.with_timezone(&Utc), 1));
        }

        // Fallback: hora actual + 1 hora
        let future = now + Duration::hours(1);
        Ok((future.with_timezone(&Utc), 0))
    }

    /// Parsea hora para "mañana" / "tomorrow"
    fn parse_time_tomorrow(
        &self,
        parts: &[&str],
        _language: Language,
    ) -> Result<(DateTime<Utc>, usize)> {
        let now = Local::now();
        let tomorrow = (now + Duration::days(1)).date_naive();

        if parts.is_empty() {
            // Sin hora específica, usar 09:00
            let time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
            let dt = NaiveDateTime::new(tomorrow, time);
            let local_dt = Local.from_local_datetime(&dt).unwrap();
            return Ok((local_dt.with_timezone(&Utc), 0));
        }

        // Parsear hora: HH:MM
        if let Ok(time) = NaiveTime::parse_from_str(parts[0], "%H:%M") {
            let dt = NaiveDateTime::new(tomorrow, time);
            let local_dt = Local.from_local_datetime(&dt).unwrap();
            return Ok((local_dt.with_timezone(&Utc), 1));
        }

        // Fallback: 09:00
        let time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
        let dt = NaiveDateTime::new(tomorrow, time);
        let local_dt = Local.from_local_datetime(&dt).unwrap();
        Ok((local_dt.with_timezone(&Utc), 0))
    }
}

impl Default for ReminderParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_spanish_today() {
        let parser = ReminderParser::new();
        let text = "!!RECORDAR(hoy 15:00, Llamar al dentista)";

        let reminders = parser.extract_reminders(text, Language::Spanish);
        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].title, "Llamar al dentista");
        assert_eq!(reminders[0].priority, Priority::Medium);
    }

    #[test]
    fn test_parse_english_tomorrow() {
        let parser = ReminderParser::new();
        let text = "!!REMIND(tomorrow 09:00 high, Team meeting)";

        let reminders = parser.extract_reminders(text, Language::English);
        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].title, "Team meeting");
        assert_eq!(reminders[0].priority, Priority::High);
    }

    #[test]
    fn test_parse_absolute_date() {
        let parser = ReminderParser::new();
        let text = "!!RECORDAR(2025-11-20 15:00 urgente, Entrega proyecto)";

        let reminders = parser.extract_reminders(text, Language::Spanish);
        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].title, "Entrega proyecto");
        assert_eq!(reminders[0].priority, Priority::Urgent);
    }

    #[test]
    fn test_parse_with_repeat() {
        let parser = ReminderParser::new();
        let text = "!!REMIND(2025-11-25 10:00 daily, Daily standup)";

        let reminders = parser.extract_reminders(text, Language::English);
        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].repeat_pattern, RepeatPattern::Daily);
    }
}
