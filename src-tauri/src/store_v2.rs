use anyhow::Result;
use chrono::{DateTime, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::models::*;
use crate::migrations::MigrationManager;

pub struct StoreV2 {
    conn: Arc<Mutex<Connection>>,
}

impl StoreV2 {
    pub fn new(path: &Path) -> Result<Self> {
        let mut conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;

        let mgr = MigrationManager::new()?;
        mgr.migrate(&mut conn, Some(path))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    // ==================== Time Slots ====================

    pub fn create_slot(&self, slot: &TimeSlot) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO time_slots (uuid, task_name, slot_type, start_ts, end_ts, note, parent_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                slot.uuid.to_string(),
                slot.task_name,
                slot.slot_type.as_str(),
                slot.start_ts.to_rfc3339(),
                slot.end_ts.map(|t| t.to_rfc3339()),
                slot.note,
                slot.parent_id,
                slot.created_at.to_rfc3339(),
                slot.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_slot(&self, id: i64, end_ts: Option<DateTime<Local>>, note: Option<String>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE time_slots SET end_ts = ?1, note = ?2, updated_at = ?3 WHERE id = ?4",
            params![
                end_ts.map(|t| t.to_rfc3339()),
                note,
                Local::now().to_rfc3339(),
                id,
            ],
        )?;
        Ok(())
    }

    pub fn update_slot_details(
        &self,
        id: i64,
        task_name: &str,
        slot_type: SlotType,
        start_ts: DateTime<Local>,
        end_ts: Option<DateTime<Local>>,
        note: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute(
            "UPDATE time_slots
             SET task_name = ?1, slot_type = ?2, start_ts = ?3, end_ts = ?4, note = ?5, updated_at = ?6
             WHERE id = ?7",
            params![
                task_name,
                slot_type.as_str(),
                start_ts.to_rfc3339(),
                end_ts.map(|t| t.to_rfc3339()),
                note,
                Local::now().to_rfc3339(),
                id,
            ],
        )?;
        if changed == 0 {
            anyhow::bail!("找不到這個時段");
        }
        Ok(())
    }

    pub fn get_slot(&self, id: i64) -> Result<Option<TimeSlot>> {
        let conn = self.conn.lock().unwrap();
        let slot = conn
            .query_row(
                "SELECT id, uuid, task_name, slot_type, start_ts, end_ts, note, parent_id, created_at, updated_at
                 FROM time_slots WHERE id = ?1",
                params![id],
                |row| {
                    Ok(TimeSlot {
                        id: Some(row.get(0)?),
                        uuid: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                        task_name: row.get(2)?,
                        slot_type: SlotType::from_str(&row.get::<_, String>(3)?).unwrap(),
                        start_ts: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                            .unwrap()
                            .with_timezone(&Local),
                        end_ts: row
                            .get::<_, Option<String>>(5)?
                            .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Local)),
                        note: row.get(6)?,
                        parent_id: row.get(7)?,
                        created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                            .unwrap()
                            .with_timezone(&Local),
                        updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                            .unwrap()
                            .with_timezone(&Local),
                    })
                },
            )
            .optional()?;
        Ok(slot)
    }

    pub fn get_current_slot(&self) -> Result<Option<TimeSlot>> {
        let conn = self.conn.lock().unwrap();
        let slot = conn
            .query_row(
                "SELECT id, uuid, task_name, slot_type, start_ts, end_ts, note, parent_id, created_at, updated_at
                 FROM time_slots WHERE end_ts IS NULL ORDER BY start_ts DESC LIMIT 1",
                [],
                |row| {
                    Ok(TimeSlot {
                        id: Some(row.get(0)?),
                        uuid: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                        task_name: row.get(2)?,
                        slot_type: SlotType::from_str(&row.get::<_, String>(3)?).unwrap(),
                        start_ts: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                            .unwrap()
                            .with_timezone(&Local),
                        end_ts: row
                            .get::<_, Option<String>>(5)?
                            .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Local)),
                        note: row.get(6)?,
                        parent_id: row.get(7)?,
                        created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                            .unwrap()
                            .with_timezone(&Local),
                        updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                            .unwrap()
                            .with_timezone(&Local),
                    })
                },
            )
            .optional()?;
        Ok(slot)
    }


    pub fn delete_slot(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM time_slots WHERE id = ?", params![id])?;
        Ok(())
    }
    pub fn get_slots_for_date(&self, date: NaiveDate) -> Result<Vec<TimeSlot>> {
        let conn = self.conn.lock().unwrap();
        let start = date.and_hms_opt(0, 0, 0).unwrap();
        let end = date.and_hms_opt(23, 59, 59).unwrap();
        let start_str = start.and_local_timezone(Local).unwrap().to_rfc3339();
        let end_str = end.and_local_timezone(Local).unwrap().to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT id, uuid, task_name, slot_type, start_ts, end_ts, note, parent_id, created_at, updated_at
             FROM time_slots 
             WHERE start_ts >= ?1 AND start_ts <= ?2
             ORDER BY start_ts ASC",
        )?;

        let slots = stmt
            .query_map(params![start_str, end_str], |row| {
                Ok(TimeSlot {
                    id: Some(row.get(0)?),
                    uuid: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                    task_name: row.get(2)?,
                    slot_type: SlotType::from_str(&row.get::<_, String>(3)?).unwrap(),
                    start_ts: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .unwrap()
                        .with_timezone(&Local),
                    end_ts: row
                        .get::<_, Option<String>>(5)?
                        .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Local)),
                    note: row.get(6)?,
                    parent_id: row.get(7)?,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                        .unwrap()
                        .with_timezone(&Local),
                    updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                        .unwrap()
                        .with_timezone(&Local),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(slots)
    }

    pub fn get_slots_for_range(&self, start: NaiveDate, end: NaiveDate) -> Result<Vec<TimeSlot>> {
        let conn = self.conn.lock().unwrap();
        let start_dt = start.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).unwrap();
        let end_dt = end.and_hms_opt(23, 59, 59).unwrap().and_local_timezone(Local).unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, uuid, task_name, slot_type, start_ts, end_ts, note, parent_id, created_at, updated_at
             FROM time_slots 
             WHERE start_ts >= ?1 AND start_ts <= ?2
             ORDER BY start_ts ASC",
        )?;

        let slots = stmt
            .query_map(params![start_dt.to_rfc3339(), end_dt.to_rfc3339()], |row| {
                Ok(TimeSlot {
                    id: Some(row.get(0)?),
                    uuid: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                    task_name: row.get(2)?,
                    slot_type: SlotType::from_str(&row.get::<_, String>(3)?).unwrap(),
                    start_ts: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .unwrap()
                        .with_timezone(&Local),
                    end_ts: row
                        .get::<_, Option<String>>(5)?
                        .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Local)),
                    note: row.get(6)?,
                    parent_id: row.get(7)?,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                        .unwrap()
                        .with_timezone(&Local),
                    updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                        .unwrap()
                        .with_timezone(&Local),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(slots)
    }

    // ==================== Stats ====================

    pub fn get_day_stats(&self, date: NaiveDate) -> Result<DayStats> {
        let slots = self.get_slots_for_date(date)?;
        let mut work_minutes = 0i64;
        let mut interrupt_minutes = 0i64;
        let mut meeting_minutes = 0i64;
        let mut lunch_minutes = 0i64;

        for slot in &slots {
            if let Some(minutes) = slot.duration_minutes() {
                match slot.slot_type {
                    SlotType::Work => work_minutes += minutes,
                    SlotType::Interrupt => interrupt_minutes += minutes,
                    SlotType::Meeting => meeting_minutes += minutes,
                    SlotType::Lunch => lunch_minutes += minutes,
                }
            }
        }

        Ok(DayStats {
            date: date.to_string(),
            total_minutes: work_minutes + interrupt_minutes + meeting_minutes + lunch_minutes,
            work_minutes,
            interrupt_minutes,
            meeting_minutes,
            lunch_minutes,
            slots,
        })
    }

    pub fn get_task_summaries(&self, start: NaiveDate, end: NaiveDate) -> Result<Vec<TaskSummary>> {
        let conn = self.conn.lock().unwrap();
        let start_dt = start.and_hms_opt(0, 0, 0).unwrap().and_local_timezone(Local).unwrap();
        let end_dt = end.and_hms_opt(23, 59, 59).unwrap().and_local_timezone(Local).unwrap();

        let mut stmt = conn.prepare(
            "SELECT task_name, slot_type, 
                    SUM((julianday(COALESCE(end_ts, datetime('now','localtime'))) - julianday(start_ts)) * 24 * 60) as total_minutes,
                    COUNT(*) as slot_count
             FROM time_slots 
             WHERE start_ts >= ?1 AND start_ts <= ?2 AND end_ts IS NOT NULL
             GROUP BY task_name, slot_type
             ORDER BY total_minutes DESC",
        )?;

        let summaries = stmt
            .query_map(params![start_dt.to_rfc3339(), end_dt.to_rfc3339()], |row| {
                let minutes: f64 = row.get(2)?;
                Ok(TaskSummary {
                    task_name: row.get(0)?,
                    slot_type: SlotType::from_str(&row.get::<_, String>(1)?).unwrap(),
                    total_minutes: minutes.round() as i64,
                    total_hours: minutes / 60.0,
                    percentage: 0.0,
                    slot_count: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let grand_total: i64 = summaries.iter().map(|s| s.total_minutes).sum();
        let summaries_with_pct: Vec<TaskSummary> = summaries
            .into_iter()
            .map(|mut s| {
                s.percentage = if grand_total > 0 {
                    (s.total_minutes as f64 / grand_total as f64) * 100.0
                } else {
                    0.0
                };
                s
            })
            .collect();

        Ok(summaries_with_pct)
    }

    pub fn find_gaps(&self, date: NaiveDate) -> Result<Vec<Gap>> {
        let slots = self.get_slots_for_date(date)?;
        let mut gaps = Vec::new();

        for i in 0..slots.len().saturating_sub(1) {
            if let Some(current_end) = slots[i].end_ts {
                let next_start = slots[i + 1].start_ts;
                let gap_minutes = next_start
                    .signed_duration_since(current_end)
                    .num_minutes();

                if gap_minutes >= 5 {
                    gaps.push(Gap {
                        start_ts: current_end,
                        end_ts: next_start,
                        minutes: gap_minutes,
                    });
                }
            }
        }

        Ok(gaps)
    }

    // ==================== Recent Items ====================

    pub fn add_or_update_recent_task(&self, task_name: &str, slot_type: SlotType) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO recent_tasks (task_name, last_used, use_count, slot_type)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(task_name) DO UPDATE SET
             last_used = ?2, use_count = use_count + 1",
            params![task_name, Local::now().to_rfc3339(), slot_type.as_str()],
        )?;
        Ok(())
    }

    pub fn get_recent_tasks(&self, limit: usize) -> Result<Vec<RecentTask>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, task_name, last_used, use_count, slot_type
             FROM recent_tasks
             ORDER BY last_used DESC
             LIMIT ?1",
        )?;

        let tasks = stmt
            .query_map(params![limit], |row| {
                Ok(RecentTask {
                    id: Some(row.get(0)?),
                    task_name: row.get(1)?,
                    last_used: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                        .unwrap()
                        .with_timezone(&Local),
                    use_count: row.get(3)?,
                    slot_type: SlotType::from_str(&row.get::<_, String>(4)?).unwrap(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tasks)
    }

    pub fn add_or_update_recent_person(&self, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO recent_people (name, last_used, use_count)
             VALUES (?1, ?2, 1)
             ON CONFLICT(name) DO UPDATE SET
             last_used = ?2, use_count = use_count + 1",
            params![name, Local::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn get_recent_people(&self, limit: usize) -> Result<Vec<RecentPerson>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, last_used, use_count
             FROM recent_people
             ORDER BY last_used DESC
             LIMIT ?1",
        )?;

        let people = stmt
            .query_map(params![limit], |row| {
                Ok(RecentPerson {
                    id: Some(row.get(0)?),
                    name: row.get(1)?,
                    last_used: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                        .unwrap()
                        .with_timezone(&Local),
                    use_count: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(people)
    }

    // ==================== Settings ====================

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let value = conn
            .query_row("SELECT value FROM app_state WHERE key = ?1", params![key], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(value)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO app_state (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_settings(&self) -> Result<AppSettings> {
        let mut settings = AppSettings::default();

        if let Some(v) = self.get_setting("lunch_start")? {
            settings.lunch_start = v;
        }
        if let Some(v) = self.get_setting("lunch_end")? {
            settings.lunch_end = v;
        }
        if let Some(v) = self.get_setting("work_end")? {
            settings.work_end = v;
        }
        if let Some(v) = self.get_setting("eod_check_enabled")? {
            settings.eod_check_enabled = v == "true";
        }
        if let Some(v) = self.get_setting("pomodoro_minutes")? {
            settings.pomodoro_minutes = v.parse().unwrap_or(25);
        }
        if let Some(v) = self.get_setting("theme")? {
            settings.theme = v;
        }
        if let Some(v) = self.get_setting("mini_mode")? {
            settings.mini_mode = v;
        }
        if let Some(v) = self.get_setting("pet_id")? {
            settings.pet_id = v;
        }

        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        self.set_setting("lunch_start", &settings.lunch_start)?;
        self.set_setting("lunch_end", &settings.lunch_end)?;
        self.set_setting("work_end", &settings.work_end)?;
        self.set_setting("eod_check_enabled", if settings.eod_check_enabled { "true" } else { "false" })?;
        self.set_setting("pomodoro_minutes", &settings.pomodoro_minutes.to_string())?;
        self.set_setting("theme", &settings.theme)?;
        self.set_setting("mini_mode", &settings.mini_mode)?;
        self.set_setting("pet_id", &settings.pet_id)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn test_update_slot_name() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let slot = TimeSlot::new("舊名稱".to_string(), SlotType::Work, Local::now());
        let id = store.create_slot(&slot).unwrap();

        store
            .update_slot_details(id, "新名稱", SlotType::Meeting, slot.start_ts, None, Some("備註"))
            .unwrap();

        let retrieved = store.get_slot(id).unwrap().unwrap();
        assert_eq!(retrieved.task_name, "新名稱");
        assert_eq!(retrieved.slot_type, SlotType::Meeting);
        assert_eq!(retrieved.note.as_deref(), Some("備註"));
    }

    #[test]
    fn test_create_and_get_slot() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let slot = TimeSlot::new("Test Task".to_string(), SlotType::Work, Local::now());
        let id = store.create_slot(&slot).unwrap();
        let retrieved = store.get_slot(id).unwrap().unwrap();
        assert_eq!(retrieved.task_name, "Test Task");
        assert_eq!(retrieved.slot_type, SlotType::Work);
    }

    #[test]
    fn test_current_slot() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let slot = TimeSlot::new("Current Task".to_string(), SlotType::Work, Local::now());
        let id = store.create_slot(&slot).unwrap();
        let current = store.get_current_slot().unwrap().unwrap();
        assert_eq!(current.id.unwrap(), id);
        assert!(current.is_ongoing());
    }

    #[test]
    fn test_day_stats() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let now = Local::now();
        let date = now.date_naive();

        let mut slot1 = TimeSlot::new("Work 1".to_string(), SlotType::Work, now);
        slot1.end_ts = Some(now + chrono::Duration::minutes(30));
        store.create_slot(&slot1).unwrap();

        let mut slot2 = TimeSlot::new("Lunch".to_string(), SlotType::Lunch, now + chrono::Duration::minutes(30));
        slot2.end_ts = Some(now + chrono::Duration::minutes(90));
        store.create_slot(&slot2).unwrap();

        let stats = store.get_day_stats(date).unwrap();
        assert_eq!(stats.work_minutes, 30);
        assert_eq!(stats.lunch_minutes, 60);
        assert_eq!(stats.total_minutes, 90);
    }

    #[test]
    fn test_recent_tasks() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        store.add_or_update_recent_task("Task A", SlotType::Work).unwrap();
        store.add_or_update_recent_task("Task B", SlotType::Work).unwrap();
        store.add_or_update_recent_task("Task A", SlotType::Work).unwrap();

        let recent = store.get_recent_tasks(10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].task_name, "Task A");
        assert_eq!(recent[0].use_count, 2);
    }
}
