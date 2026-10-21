use anyhow::Result;
use chrono::{DateTime, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::migrations::MigrationManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Option<i64>,
    pub uuid: Uuid,
    pub ts: DateTime<Local>,
    pub mono_ms: i64,
    pub boot_id: String,
    pub kind: EventKind,
    pub task_key: Option<String>,
    pub ref_event_id: Option<i64>,
    pub note: Option<String>,
    pub source: EventSource,
    pub synced: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Start,
    End,
    Ping,
    Pause,
    Resume,
    UnknownBegin,
    UnknownEnd,
    Resolve,
}

impl EventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
            Self::Ping => "ping",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::UnknownBegin => "unknown_begin",
            Self::UnknownEnd => "unknown_end",
            Self::Resolve => "resolve",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "start" => Ok(Self::Start),
            "end" => Ok(Self::End),
            "ping" => Ok(Self::Ping),
            "pause" => Ok(Self::Pause),
            "resume" => Ok(Self::Resume),
            "unknown_begin" => Ok(Self::UnknownBegin),
            "unknown_end" => Ok(Self::UnknownEnd),
            "resolve" => Ok(Self::Resolve),
            _ => anyhow::bail!("Unknown event kind: {}", s),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    User,
    Ping,
    Auto,
    Recovery,
    Backfill,
}

impl EventSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Ping => "ping",
            Self::Auto => "auto",
            Self::Recovery => "recovery",
            Self::Backfill => "backfill",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "user" => Ok(Self::User),
            "ping" => Ok(Self::Ping),
            "auto" => Ok(Self::Auto),
            "recovery" => Ok(Self::Recovery),
            "backfill" => Ok(Self::Backfill),
            _ => anyhow::bail!("Unknown event source: {}", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCache {
    pub task_key: String,
    pub plan_name: Option<String>,
    pub stage_name: Option<String>,
    pub process_name: Option<String>,
    pub task_name: Option<String>,
    pub task_id: Option<i64>,
    pub is_adhoc: bool,
    pub is_closed: bool,
    pub last_used_at: Option<DateTime<Local>>,
    pub fetched_at: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: Option<i64>,
    pub task_key: Option<String>,
    pub start_ts: DateTime<Local>,
    pub end_ts: Option<DateTime<Local>>,
    pub minutes: Option<i64>,
    pub status: SegmentStatus,
    pub synced: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentStatus {
    Confirmed,
    Unknown,
    Recovered,
}

impl SegmentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Unknown => "unknown",
            Self::Recovered => "recovered",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "confirmed" => Ok(Self::Confirmed),
            "unknown" => Ok(Self::Unknown),
            "recovered" => Ok(Self::Recovered),
            _ => anyhow::bail!("Unknown segment status: {}", s),
        }
    }
}

pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Clone for Store {
    fn clone(&self) -> Self {
        Self {
            conn: Arc::clone(&self.conn),
        }
    }
}

impl Store {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let mut conn = Connection::open(path)?;
        
        // Run migrations first
        let migrator = MigrationManager::new()?;
        migrator.migrate(&mut conn, Some(path))?;
        
        // Enable WAL mode
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        
        // Run migrations
        let migrator = MigrationManager::new()?;
        migrator.migrate(&mut conn, None)?;
        
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        // Migrations are now run in new() before this
        Ok(())
    }

    pub fn insert_event(&self, event: &Event) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO events (uuid, ts, mono_ms, boot_id, kind, task_key, ref_event_id, note, source, synced)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                event.uuid.to_string(),
                event.ts.to_rfc3339(),
                event.mono_ms,
                event.boot_id,
                event.kind.as_str(),
                event.task_key,
                event.ref_event_id,
                event.note,
                event.source.as_str(),
                event.synced as i32,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn get_events_since(&self, since: DateTime<Local>) -> Result<Vec<Event>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, uuid, ts, mono_ms, boot_id, kind, task_key, ref_event_id, note, source, synced
             FROM events
             WHERE ts >= ?1
             ORDER BY ts ASC"
        )?;

        let events = stmt
            .query_map([since.to_rfc3339()], |row| {
                Ok(Event {
                    id: Some(row.get(0)?),
                    uuid: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                    ts: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                        .unwrap()
                        .with_timezone(&Local),
                    mono_ms: row.get(3)?,
                    boot_id: row.get(4)?,
                    kind: EventKind::from_str(&row.get::<_, String>(5)?).unwrap(),
                    task_key: row.get(6)?,
                    ref_event_id: row.get(7)?,
                    note: row.get(8)?,
                    source: EventSource::from_str(&row.get::<_, String>(9)?).unwrap(),
                    synced: row.get::<_, i32>(10)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    pub fn get_all_events(&self) -> Result<Vec<Event>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, uuid, ts, mono_ms, boot_id, kind, task_key, ref_event_id, note, source, synced
             FROM events
             ORDER BY ts ASC"
        )?;

        let events = stmt
            .query_map([], |row| {
                Ok(Event {
                    id: Some(row.get(0)?),
                    uuid: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                    ts: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                        .unwrap()
                        .with_timezone(&Local),
                    mono_ms: row.get(3)?,
                    boot_id: row.get(4)?,
                    kind: EventKind::from_str(&row.get::<_, String>(5)?).unwrap(),
                    task_key: row.get(6)?,
                    ref_event_id: row.get(7)?,
                    note: row.get(8)?,
                    source: EventSource::from_str(&row.get::<_, String>(9)?).unwrap(),
                    synced: row.get::<_, i32>(10)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    pub fn get_last_event(&self) -> Result<Option<Event>> {
        let conn = self.conn.lock().unwrap();
        
        let event = conn
            .query_row(
                "SELECT id, uuid, ts, mono_ms, boot_id, kind, task_key, ref_event_id, note, source, synced
                 FROM events
                 ORDER BY ts DESC
                 LIMIT 1",
                [],
                |row| {
                    Ok(Event {
                        id: Some(row.get(0)?),
                        uuid: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                        ts: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                            .unwrap()
                            .with_timezone(&Local),
                        mono_ms: row.get(3)?,
                        boot_id: row.get(4)?,
                        kind: EventKind::from_str(&row.get::<_, String>(5)?).unwrap(),
                        task_key: row.get(6)?,
                        ref_event_id: row.get(7)?,
                        note: row.get(8)?,
                        source: EventSource::from_str(&row.get::<_, String>(9)?).unwrap(),
                        synced: row.get::<_, i32>(10)? != 0,
                    })
                },
            )
            .optional()?;

        Ok(event)
    }

    pub fn insert_or_update_task(&self, task: &TaskCache) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO tasks_cache (task_key, plan_name, stage_name, process_name, task_name, task_id, is_adhoc, is_closed, last_used_at, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(task_key) DO UPDATE SET
                plan_name = ?2,
                stage_name = ?3,
                process_name = ?4,
                task_name = ?5,
                task_id = ?6,
                is_adhoc = ?7,
                is_closed = ?8,
                last_used_at = ?9,
                fetched_at = ?10",
            params![
                task.task_key,
                task.plan_name,
                task.stage_name,
                task.process_name,
                task.task_name,
                task.task_id,
                task.is_adhoc as i32,
                task.is_closed as i32,
                task.last_used_at.map(|dt| dt.to_rfc3339()),
                task.fetched_at.map(|dt| dt.to_rfc3339()),
            ],
        )?;

        Ok(())
    }

    pub fn get_task(&self, task_key: &str) -> Result<Option<TaskCache>> {
        let conn = self.conn.lock().unwrap();
        
        let task = conn
            .query_row(
                "SELECT task_key, plan_name, stage_name, process_name, task_name, task_id, is_adhoc, is_closed, last_used_at, fetched_at
                 FROM tasks_cache
                 WHERE task_key = ?1",
                [task_key],
                |row| {
                    Ok(TaskCache {
                        task_key: row.get(0)?,
                        plan_name: row.get(1)?,
                        stage_name: row.get(2)?,
                        process_name: row.get(3)?,
                        task_name: row.get(4)?,
                        task_id: row.get(5)?,
                        is_adhoc: row.get::<_, i32>(6)? != 0,
                        is_closed: row.get::<_, i32>(7)? != 0,
                        last_used_at: row
                            .get::<_, Option<String>>(8)?
                            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&Local)),
                        fetched_at: row
                            .get::<_, Option<String>>(9)?
                            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&Local)),
                    })
                },
            )
            .optional()?;

        Ok(task)
    }

    pub fn get_all_tasks(&self) -> Result<Vec<TaskCache>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT task_key, plan_name, stage_name, process_name, task_name, task_id, is_adhoc, is_closed, last_used_at, fetched_at
             FROM tasks_cache
             WHERE is_closed = 0
             ORDER BY last_used_at DESC"
        )?;

        let tasks = stmt
            .query_map([], |row| {
                Ok(TaskCache {
                    task_key: row.get(0)?,
                    plan_name: row.get(1)?,
                    stage_name: row.get(2)?,
                    process_name: row.get(3)?,
                    task_name: row.get(4)?,
                    task_id: row.get(5)?,
                    is_adhoc: row.get::<_, i32>(6)? != 0,
                    is_closed: row.get::<_, i32>(7)? != 0,
                    last_used_at: row
                        .get::<_, Option<String>>(8)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Local)),
                    fetched_at: row
                        .get::<_, Option<String>>(9)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Local)),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tasks)
    }

    pub fn clear_segments(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM segments", [])?;
        Ok(())
    }

    pub fn insert_segment(&self, segment: &Segment) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO segments (task_key, start_ts, end_ts, minutes, status, synced)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                segment.task_key,
                segment.start_ts.to_rfc3339(),
                segment.end_ts.map(|dt| dt.to_rfc3339()),
                segment.minutes,
                segment.status.as_str(),
                segment.synced as i32,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn get_segments_for_date(&self, date: NaiveDate) -> Result<Vec<Segment>> {
        let conn = self.conn.lock().unwrap();
        
        let start_of_day = date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .unwrap();
        let end_of_day = date
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_local_timezone(Local)
            .unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, task_key, start_ts, end_ts, minutes, status, synced
             FROM segments
             WHERE start_ts >= ?1 AND start_ts <= ?2
             ORDER BY start_ts ASC"
        )?;

        let segments = stmt
            .query_map(
                params![start_of_day.to_rfc3339(), end_of_day.to_rfc3339()],
                |row| {
                    Ok(Segment {
                        id: Some(row.get(0)?),
                        task_key: row.get(1)?,
                        start_ts: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                            .unwrap()
                            .with_timezone(&Local),
                        end_ts: row
                            .get::<_, Option<String>>(3)?
                            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&Local)),
                        minutes: row.get(4)?,
                        status: SegmentStatus::from_str(&row.get::<_, String>(5)?).unwrap(),
                        synced: row.get::<_, i32>(6)? != 0,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(segments)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()?;

        Ok(value)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;

        Ok(())
    }

    /// Checkpoint WAL to main database file
    pub fn checkpoint_wal(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.pragma_update(None, "wal_checkpoint", "TRUNCATE")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_store() {
        let store = Store::new_in_memory().unwrap();
        assert!(store.get_all_events().unwrap().is_empty());
    }

    #[test]
    fn test_insert_and_retrieve_event() {
        let store = Store::new_in_memory().unwrap();
        let event = Event {
            id: None,
            uuid: Uuid::new_v4(),
            ts: Local::now(),
            mono_ms: 1000,
            boot_id: "test-boot".to_string(),
            kind: EventKind::Start,
            task_key: Some("Plan|Stage|Process|Task".to_string()),
            ref_event_id: None,
            note: None,
            source: EventSource::User,
            synced: false,
        };

        let id = store.insert_event(&event).unwrap();
        assert!(id > 0);

        let events = store.get_all_events().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, EventKind::Start);
    }

    #[test]
    fn test_task_cache() {
        let store = Store::new_in_memory().unwrap();
        let task = TaskCache {
            task_key: "Plan|Stage|Process|Task".to_string(),
            plan_name: Some("Plan".to_string()),
            stage_name: Some("Stage".to_string()),
            process_name: Some("Process".to_string()),
            task_name: Some("Task".to_string()),
            task_id: Some(123),
            is_adhoc: false,
            is_closed: false,
            last_used_at: Some(Local::now()),
            fetched_at: Some(Local::now()),
        };

        store.insert_or_update_task(&task).unwrap();

        let retrieved = store.get_task("Plan|Stage|Process|Task").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().task_id, Some(123));
    }
}
