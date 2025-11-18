use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use super::models::{Priority, Reminder, ReminderStatus, RepeatPattern};

/// Operaciones de base de datos para recordatorios
#[derive(Debug)]
pub struct ReminderDatabase {
    conn: Connection,
}

impl ReminderDatabase {
    /// Crea una nueva conexión a la base de datos
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    /// Crea la tabla de recordatorios si no existe
    pub fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                note_id INTEGER,
                title TEXT NOT NULL,
                description TEXT,
                due_date INTEGER NOT NULL,
                priority INTEGER DEFAULT 1,
                status INTEGER DEFAULT 0,
                snooze_until INTEGER,
                repeat_pattern INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE SET NULL
            );
            
            CREATE INDEX IF NOT EXISTS idx_reminders_due_date ON reminders(due_date);
            CREATE INDEX IF NOT EXISTS idx_reminders_status ON reminders(status);
            CREATE INDEX IF NOT EXISTS idx_reminders_note_id ON reminders(note_id);
            "#,
        )?;

        Ok(())
    }

    /// Crea un nuevo recordatorio
    pub fn create_reminder(
        &self,
        note_id: Option<i64>,
        title: &str,
        description: Option<&str>,
        due_date: DateTime<Utc>,
        priority: Priority,
        repeat_pattern: RepeatPattern,
    ) -> Result<i64> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            r#"
            INSERT INTO reminders (note_id, title, description, due_date, priority, status, repeat_pattern, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8)
            "#,
            params![
                note_id,
                title,
                description,
                due_date.timestamp(),
                priority.to_i32(),
                repeat_pattern.to_i32(),
                now,
                now
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Obtiene un recordatorio por ID
    pub fn get_reminder(&self, id: i64) -> Result<Option<Reminder>> {
        let result = self
            .conn
            .query_row(
                r#"
                SELECT id, note_id, title, description, due_date, priority, status, 
                       snooze_until, repeat_pattern, created_at, updated_at
                FROM reminders
                WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(Reminder {
                        id: row.get(0)?,
                        note_id: row.get(1)?,
                        title: row.get(2)?,
                        description: row.get(3)?,
                        due_date: DateTime::from_timestamp(row.get(4)?, 0).unwrap(),
                        priority: Priority::from_i32(row.get(5)?),
                        status: ReminderStatus::from_i32(row.get(6)?),
                        snooze_until: row
                            .get::<_, Option<i64>>(7)?
                            .and_then(|ts| DateTime::from_timestamp(ts, 0)),
                        repeat_pattern: RepeatPattern::from_i32(row.get(8)?),
                        created_at: DateTime::from_timestamp(row.get(9)?, 0).unwrap(),
                        updated_at: DateTime::from_timestamp(row.get(10)?, 0).unwrap(),
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    /// Lista todos los recordatorios
    pub fn list_reminders(&self, status_filter: Option<ReminderStatus>) -> Result<Vec<Reminder>> {
        let query = if let Some(status) = status_filter {
            format!(
                r#"
                SELECT id, note_id, title, description, due_date, priority, status, 
                       snooze_until, repeat_pattern, created_at, updated_at
                FROM reminders
                WHERE status = {}
                ORDER BY due_date ASC
                "#,
                status.to_i32()
            )
        } else {
            r#"
            SELECT id, note_id, title, description, due_date, priority, status, 
                   snooze_until, repeat_pattern, created_at, updated_at
            FROM reminders
            ORDER BY due_date ASC
            "#
            .to_string()
        };

        let mut stmt = self.conn.prepare(&query)?;
        let reminders = stmt
            .query_map([], |row| {
                Ok(Reminder {
                    id: row.get(0)?,
                    note_id: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    due_date: DateTime::from_timestamp(row.get(4)?, 0).unwrap(),
                    priority: Priority::from_i32(row.get(5)?),
                    status: ReminderStatus::from_i32(row.get(6)?),
                    snooze_until: row
                        .get::<_, Option<i64>>(7)?
                        .and_then(|ts| DateTime::from_timestamp(ts, 0)),
                    repeat_pattern: RepeatPattern::from_i32(row.get(8)?),
                    created_at: DateTime::from_timestamp(row.get(9)?, 0).unwrap(),
                    updated_at: DateTime::from_timestamp(row.get(10)?, 0).unwrap(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(reminders)
    }

    /// Lista recordatorios de una nota específica
    pub fn list_reminders_by_note(&self, note_id: i64) -> Result<Vec<Reminder>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, note_id, title, description, due_date, priority, status, 
                   snooze_until, repeat_pattern, created_at, updated_at
            FROM reminders
            WHERE note_id = ?1
            ORDER BY due_date ASC
            "#,
        )?;

        let reminders = stmt
            .query_map([note_id], |row| {
                Ok(Reminder {
                    id: row.get(0)?,
                    note_id: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    due_date: DateTime::from_timestamp(row.get(4)?, 0).unwrap(),
                    priority: Priority::from_i32(row.get(5)?),
                    status: ReminderStatus::from_i32(row.get(6)?),
                    snooze_until: row
                        .get::<_, Option<i64>>(7)?
                        .and_then(|ts| DateTime::from_timestamp(ts, 0)),
                    repeat_pattern: RepeatPattern::from_i32(row.get(8)?),
                    created_at: DateTime::from_timestamp(row.get(9)?, 0).unwrap(),
                    updated_at: DateTime::from_timestamp(row.get(10)?, 0).unwrap(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(reminders)
    }

    /// Actualiza el estado de un recordatorio
    pub fn update_status(&self, id: i64, status: ReminderStatus) -> Result<()> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            "UPDATE reminders SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.to_i32(), now, id],
        )?;

        Ok(())
    }

    /// Pospone un recordatorio
    pub fn snooze_reminder(&self, id: i64, snooze_until: DateTime<Utc>) -> Result<()> {
        let now = Utc::now().timestamp();

        self.conn.execute(
            "UPDATE reminders SET snooze_until = ?1, status = 2, updated_at = ?2 WHERE id = ?3",
            params![snooze_until.timestamp(), now, id],
        )?;

        Ok(())
    }

    /// Actualiza un recordatorio
    pub fn update_reminder(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<Option<&str>>,
        due_date: Option<DateTime<Utc>>,
        priority: Option<Priority>,
        repeat_pattern: Option<RepeatPattern>,
    ) -> Result<()> {
        let now = Utc::now().timestamp();

        // Obtener el recordatorio actual
        let current = self
            .get_reminder(id)?
            .context("Recordatorio no encontrado")?;

        self.conn.execute(
            r#"
            UPDATE reminders 
            SET title = ?1, description = ?2, due_date = ?3, priority = ?4, 
                repeat_pattern = ?5, updated_at = ?6
            WHERE id = ?7
            "#,
            params![
                title.unwrap_or(&current.title),
                description.unwrap_or(current.description.as_deref()),
                due_date.unwrap_or(current.due_date).timestamp(),
                priority.unwrap_or(current.priority).to_i32(),
                repeat_pattern.unwrap_or(current.repeat_pattern).to_i32(),
                now,
                id
            ],
        )?;

        Ok(())
    }

    /// Elimina un recordatorio
    pub fn delete_reminder(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM reminders WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Obtiene recordatorios que deben dispararse
    pub fn get_pending_triggers(&self) -> Result<Vec<Reminder>> {
        let now = Utc::now().timestamp();

        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, note_id, title, description, due_date, priority, status, 
                   snooze_until, repeat_pattern, created_at, updated_at
            FROM reminders
            WHERE status != 1
              AND (
                  (status = 0 AND due_date <= ?1)
                  OR (status = 2 AND snooze_until <= ?1)
              )
            ORDER BY priority DESC, due_date ASC
            "#,
        )?;

        let reminders = stmt
            .query_map(params![now], |row| {
                Ok(Reminder {
                    id: row.get(0)?,
                    note_id: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    due_date: DateTime::from_timestamp(row.get(4)?, 0).unwrap(),
                    priority: Priority::from_i32(row.get(5)?),
                    status: ReminderStatus::from_i32(row.get(6)?),
                    snooze_until: row
                        .get::<_, Option<i64>>(7)?
                        .and_then(|ts| DateTime::from_timestamp(ts, 0)),
                    repeat_pattern: RepeatPattern::from_i32(row.get(8)?),
                    created_at: DateTime::from_timestamp(row.get(9)?, 0).unwrap(),
                    updated_at: DateTime::from_timestamp(row.get(10)?, 0).unwrap(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(reminders)
    }

    /// Cuenta recordatorios pendientes
    pub fn count_pending(&self) -> Result<usize> {
        let now = Utc::now().timestamp();

        let count: i64 = self.conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM reminders
            WHERE status != 1
              AND (
                  (status = 0 AND due_date <= ?1)
                  OR (status = 2 AND snooze_until <= ?1)
              )
            "#,
            params![now],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }

    /// Obtiene recordatorios por nota
    pub fn get_reminders_by_note(&self, note_id: i64) -> Result<Vec<Reminder>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, note_id, title, description, due_date, priority, status, 
                   snooze_until, repeat_pattern, created_at, updated_at
            FROM reminders
            WHERE note_id = ?1
            ORDER BY due_date ASC
            "#,
        )?;

        let reminders = stmt
            .query_map(params![note_id], |row| {
                Ok(Reminder {
                    id: row.get(0)?,
                    note_id: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    due_date: DateTime::from_timestamp(row.get(4)?, 0).unwrap(),
                    priority: Priority::from_i32(row.get(5)?),
                    status: ReminderStatus::from_i32(row.get(6)?),
                    snooze_until: row
                        .get::<_, Option<i64>>(7)?
                        .and_then(|ts| DateTime::from_timestamp(ts, 0)),
                    repeat_pattern: RepeatPattern::from_i32(row.get(8)?),
                    created_at: DateTime::from_timestamp(row.get(9)?, 0).unwrap(),
                    updated_at: DateTime::from_timestamp(row.get(10)?, 0).unwrap(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(reminders)
    }
}
