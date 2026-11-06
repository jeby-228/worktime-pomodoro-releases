use anyhow::Result;
use chrono::{DateTime, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use uuid::Uuid;

use crate::migrations::MigrationManager;
use crate::models::*;
use crate::report::build_category_report;

fn lock_connection<'a>(mutex: &'a Mutex<Connection>) -> MutexGuard<'a, Connection> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

const SLOT_SELECT: &str =
    "SELECT s.id, s.uuid, s.task_name, s.slot_type, s.start_ts, s.end_ts, s.note, s.parent_id,
            s.created_at, s.updated_at, s.category_id, c.name, c.report_code
     FROM time_slots s
     LEFT JOIN categories c ON c.id = s.category_id";

const CATEGORY_COLUMNS: &str = "c.id, c.name, c.report_code, c.sort_order, c.hidden";

fn parse_ts(value: &str) -> rusqlite::Result<DateTime<Local>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Local))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(e),
            )
        })
}

fn slot_from_row(row: &Row) -> rusqlite::Result<TimeSlot> {
    let uuid_str: String = row.get(1)?;
    let uuid = Uuid::parse_str(&uuid_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            1,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        )
    })?;

    let slot_type_str: String = row.get(3)?;
    let slot_type = slot_type_str.parse::<SlotType>().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        )
    })?;

    let start_ts = parse_ts(&row.get::<_, String>(4)?)?;
    let end_ts = row
        .get::<_, Option<String>>(5)?
        .map(|s| parse_ts(&s))
        .transpose()?;
    let created_at = parse_ts(&row.get::<_, String>(8)?)?;
    let updated_at = parse_ts(&row.get::<_, String>(9)?)?;

    Ok(TimeSlot {
        id: Some(row.get(0)?),
        uuid,
        task_name: row.get(2)?,
        slot_type,
        start_ts,
        end_ts,
        note: row.get(6)?,
        parent_id: row.get(7)?,
        created_at,
        updated_at,
        category_id: row.get(10)?,
        category_name: row.get(11)?,
        category_code: row.get(12)?,
    })
}

fn category_from_row(row: &Row) -> rusqlite::Result<Category> {
    Ok(Category {
        id: row.get(0)?,
        name: row.get(1)?,
        report_code: row.get(2)?,
        sort_order: row.get(3)?,
        hidden: row.get(4)?,
    })
}

fn clean_category_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("請輸入類別名稱");
    }
    Ok(name)
}

fn clean_report_code(code: Option<&str>) -> Option<&str> {
    code.map(str::trim).filter(|text| !text.is_empty())
}

pub struct SlotDetails<'a> {
    pub task_name: &'a str,
    pub slot_type: SlotType,
    pub start_ts: DateTime<Local>,
    pub end_ts: Option<DateTime<Local>>,
    pub note: Option<&'a str>,
    pub category_id: Option<i64>,
}

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
        let conn = lock_connection(&self.conn);
        conn.execute(
            "INSERT INTO time_slots (uuid, task_name, slot_type, start_ts, end_ts, note, parent_id, created_at, updated_at, category_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                slot.category_id.filter(|_| slot.slot_type.has_category()),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn create_slots(&self, slots: &[TimeSlot]) -> Result<Vec<i64>> {
        let mut conn = lock_connection(&self.conn);
        let tx = conn.transaction()?;
        
        let mut ids = Vec::new();
        for slot in slots {
            tx.execute(
                "INSERT INTO time_slots (uuid, task_name, slot_type, start_ts, end_ts, note, parent_id, created_at, updated_at, category_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                    slot.category_id.filter(|_| slot.slot_type.has_category()),
                ],
            )?;
            ids.push(tx.last_insert_rowid());
        }
        
        tx.commit()?;
        Ok(ids)
    }

    pub fn update_slot(
        &self,
        id: i64,
        end_ts: Option<DateTime<Local>>,
        note: Option<String>,
    ) -> Result<()> {
        let conn = lock_connection(&self.conn);
        if let Some(note) = note {
            conn.execute(
                "UPDATE time_slots SET end_ts = ?1, note = ?2, updated_at = ?3 WHERE id = ?4",
                params![
                    end_ts.map(|t| t.to_rfc3339()),
                    note,
                    Local::now().to_rfc3339(),
                    id,
                ],
            )?;
        } else {
            self.close_slot_conn(&conn, id, end_ts)?;
        }
        Ok(())
    }

    /// Close a slot without erasing its note.
    pub fn close_slot(&self, id: i64, end_ts: DateTime<Local>) -> Result<()> {
        let conn = lock_connection(&self.conn);
        self.close_slot_conn(&conn, id, Some(end_ts))
    }

    fn close_slot_conn(
        &self,
        conn: &Connection,
        id: i64,
        end_ts: Option<DateTime<Local>>,
    ) -> Result<()> {
        conn.execute(
            "UPDATE time_slots SET end_ts = ?1, updated_at = ?2 WHERE id = ?3",
            params![
                end_ts.map(|t| t.to_rfc3339()),
                Local::now().to_rfc3339(),
                id,
            ],
        )?;
        Ok(())
    }

    pub fn update_slot_details(&self, id: i64, details: SlotDetails) -> Result<()> {
        let SlotDetails {
            task_name,
            slot_type,
            start_ts,
            end_ts,
            note,
            category_id,
        } = details;
        let conn = lock_connection(&self.conn);
        let category_id = category_id.filter(|_| slot_type.has_category());
        let changed = conn.execute(
            "UPDATE time_slots
             SET task_name = ?1, slot_type = ?2, start_ts = ?3, end_ts = ?4, note = ?5, updated_at = ?6,
                 category_id = ?8
             WHERE id = ?7",
            params![
                task_name,
                slot_type.as_str(),
                start_ts.to_rfc3339(),
                end_ts.map(|t| t.to_rfc3339()),
                note,
                Local::now().to_rfc3339(),
                id,
                category_id,
            ],
        )?;
        if changed == 0 {
            anyhow::bail!("找不到這個時段");
        }
        Ok(())
    }

    pub fn get_slot(&self, id: i64) -> Result<Option<TimeSlot>> {
        let conn = lock_connection(&self.conn);
        let slot = conn
            .query_row(
                &format!("{SLOT_SELECT} WHERE s.id = ?1"),
                params![id],
                slot_from_row,
            )
            .optional()?;
        Ok(slot)
    }

    pub fn get_current_slot(&self) -> Result<Option<TimeSlot>> {
        let conn = lock_connection(&self.conn);
        let slot = conn
            .query_row(
                &format!("{SLOT_SELECT} WHERE s.end_ts IS NULL ORDER BY s.start_ts DESC LIMIT 1"),
                [],
                slot_from_row,
            )
            .optional()?;
        Ok(slot)
    }

    pub fn delete_slot(&self, id: i64) -> Result<()> {
        let conn = lock_connection(&self.conn);
        conn.execute("DELETE FROM time_slots WHERE id = ?", params![id])?;
        Ok(())
    }
    pub fn get_slots_for_date(&self, date: NaiveDate) -> Result<Vec<TimeSlot>> {
        self.get_slots_for_range(date, date)
    }

    pub fn get_slots_for_range(&self, start: NaiveDate, end: NaiveDate) -> Result<Vec<TimeSlot>> {
        let conn = lock_connection(&self.conn);
        let start_dt = start
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .single()
            .or_else(|| start.and_hms_opt(0, 0, 0).and_then(|t| t.and_local_timezone(Local).earliest()))
            .ok_or_else(|| anyhow::anyhow!("Invalid local timezone for start date"))?;
        let end_dt = end
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_local_timezone(Local)
            .single()
            .or_else(|| end.and_hms_opt(23, 59, 59).and_then(|t| t.and_local_timezone(Local).earliest()))
            .ok_or_else(|| anyhow::anyhow!("Invalid local timezone for end date"))?;

        let mut stmt = conn.prepare(&format!(
            "{SLOT_SELECT}
             WHERE s.start_ts >= ?1 AND s.start_ts <= ?2
             ORDER BY s.start_ts ASC"
        ))?;

        let slots = stmt
            .query_map(
                params![start_dt.to_rfc3339(), end_dt.to_rfc3339()],
                slot_from_row,
            )?
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
        let conn = lock_connection(&self.conn);
        let start_dt = start
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .single()
            .or_else(|| start.and_hms_opt(0, 0, 0).and_then(|t| t.and_local_timezone(Local).earliest()))
            .ok_or_else(|| anyhow::anyhow!("Invalid local timezone for start date"))?;
        let end_dt = end
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_local_timezone(Local)
            .single()
            .or_else(|| end.and_hms_opt(23, 59, 59).and_then(|t| t.and_local_timezone(Local).earliest()))
            .ok_or_else(|| anyhow::anyhow!("Invalid local timezone for end date"))?;

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
                let slot_type_str: String = row.get(1)?;
                let slot_type = slot_type_str.parse::<SlotType>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                    )
                })?;

                Ok(TaskSummary {
                    task_name: row.get(0)?,
                    slot_type,
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
                let gap_minutes = next_start.signed_duration_since(current_end).num_minutes();

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
        let conn = lock_connection(&self.conn);
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
        let conn = lock_connection(&self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, task_name, last_used, use_count, slot_type
             FROM recent_tasks
             ORDER BY last_used DESC
             LIMIT ?1",
        )?;

        let tasks = stmt
            .query_map(params![limit], |row| {
                let last_used = parse_ts(&row.get::<_, String>(2)?)?;
                let slot_type_str: String = row.get(4)?;
                let slot_type = slot_type_str.parse::<SlotType>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                    )
                })?;

                Ok(RecentTask {
                    id: Some(row.get(0)?),
                    task_name: row.get(1)?,
                    last_used,
                    use_count: row.get(3)?,
                    slot_type,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tasks)
    }

    pub fn add_or_update_recent_person(&self, name: &str) -> Result<()> {
        let conn = lock_connection(&self.conn);
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
        let conn = lock_connection(&self.conn);
        let mut stmt = conn.prepare(
            "SELECT id, name, last_used, use_count
             FROM recent_people
             ORDER BY last_used DESC
             LIMIT ?1",
        )?;

        let people = stmt
            .query_map(params![limit], |row| {
                let last_used = parse_ts(&row.get::<_, String>(2)?)?;
                Ok(RecentPerson {
                    id: Some(row.get(0)?),
                    name: row.get(1)?,
                    last_used,
                    use_count: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(people)
    }

    // ==================== Categories ====================

    /// All categories in display order, hidden ones included.
    pub fn get_categories(&self) -> Result<Vec<Category>> {
        let conn = lock_connection(&self.conn);
        let mut stmt = conn.prepare(&format!(
            "SELECT {CATEGORY_COLUMNS} FROM categories c ORDER BY c.sort_order ASC, c.id ASC"
        ))?;
        let categories = stmt
            .query_map([], category_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(categories)
    }

    fn ensure_category_name_free(conn: &Connection, name: &str, except_id: i64) -> Result<()> {
        let taken: Option<i64> = conn
            .query_row(
                "SELECT id FROM categories WHERE name = ?1 AND id <> ?2",
                params![name, except_id],
                |row| row.get(0),
            )
            .optional()?;
        if taken.is_some() {
            anyhow::bail!("已經有同名的類別");
        }
        Ok(())
    }

    pub fn create_category(&self, name: &str, report_code: Option<&str>) -> Result<i64> {
        let name = clean_category_name(name)?;
        let conn = lock_connection(&self.conn);
        Self::ensure_category_name_free(&conn, name, 0)?;
        conn.execute(
            "INSERT INTO categories (name, report_code, sort_order)
             VALUES (?1, ?2, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM categories))",
            params![name, clean_report_code(report_code)],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_category(
        &self,
        id: i64,
        name: &str,
        report_code: Option<&str>,
        hidden: bool,
    ) -> Result<()> {
        let name = clean_category_name(name)?;
        let conn = lock_connection(&self.conn);
        Self::ensure_category_name_free(&conn, name, id)?;
        let changed = conn.execute(
            "UPDATE categories
             SET name = ?1, report_code = ?2, hidden = ?3, updated_at = ?4
             WHERE id = ?5",
            params![
                name,
                clean_report_code(report_code),
                hidden,
                Local::now().to_rfc3339(),
                id,
            ],
        )?;
        if changed == 0 {
            anyhow::bail!("找不到這個類別");
        }
        Ok(())
    }

    /// `ids` is the full new order, first to last.
    pub fn reorder_categories(&self, ids: &[i64]) -> Result<()> {
        let mut conn = lock_connection(&self.conn);
        let tx = conn.transaction()?;
        for (index, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE categories SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
                params![index as i64 + 1, Local::now().to_rfc3339(), id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Most recently used visible categories, padded from the display order so
    /// a new install still gets `limit` choices.
    pub fn get_recent_categories(&self, limit: usize) -> Result<Vec<Category>> {
        let conn = lock_connection(&self.conn);
        let mut stmt = conn.prepare(&format!(
            "SELECT {CATEGORY_COLUMNS}
             FROM categories c
             LEFT JOIN time_slots s ON s.category_id = c.id
             WHERE c.hidden = 0
             GROUP BY c.id
             ORDER BY MAX(s.start_ts) IS NULL, MAX(s.start_ts) DESC, c.sort_order ASC, c.id ASC
             LIMIT ?1"
        ))?;
        let categories = stmt
            .query_map(params![limit], category_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(categories)
    }

    /// The category to pre-select for a new slot: the last one used with this
    /// status, otherwise the last one used at all. Hidden categories are skipped.
    pub fn get_default_category(&self, slot_type: &SlotType) -> Result<Option<i64>> {
        if !slot_type.has_category() || !self.get_settings()?.remember_last_category {
            return Ok(None);
        }
        let conn = lock_connection(&self.conn);
        let id = conn
            .query_row(
                "SELECT s.category_id
                 FROM time_slots s
                 JOIN categories c ON c.id = s.category_id
                 WHERE c.hidden = 0
                 ORDER BY s.slot_type = ?1 DESC, s.start_ts DESC
                 LIMIT 1",
                params![slot_type.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(id)
    }

    pub fn get_category_report(&self, start: NaiveDate, end: NaiveDate) -> Result<CategoryReport> {
        if end < start {
            anyhow::bail!("結束日期不能早於開始日期");
        }
        let slots = self.get_slots_for_range(start, end)?;
        let categories = self.get_categories()?;
        Ok(build_category_report(
            &slots,
            &categories,
            start,
            end,
            Local::now(),
        ))
    }

    // ==================== Settings ====================

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = lock_connection(&self.conn);
        let value = conn
            .query_row(
                "SELECT value FROM app_state WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = lock_connection(&self.conn);
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
        if let Some(v) = self.get_setting("auto_update")? {
            settings.auto_update = v == "true";
        }
        if let Some(v) = self.get_setting("remember_last_category")? {
            settings.remember_last_category = v == "true";
        }
        if let Some(v) = self.get_setting("mini_quick_categories")? {
            settings.mini_quick_categories = v == "true";
        }

        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        let mut conn = lock_connection(&self.conn);
        let tx = conn.transaction()?;

        let set_in_tx = |tx: &rusqlite::Transaction, key: &str, value: &str| -> Result<()> {
            tx.execute(
                "INSERT OR REPLACE INTO app_state (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
            Ok(())
        };

        set_in_tx(&tx, "lunch_start", &settings.lunch_start)?;
        set_in_tx(&tx, "lunch_end", &settings.lunch_end)?;
        set_in_tx(&tx, "work_end", &settings.work_end)?;
        set_in_tx(
            &tx,
            "eod_check_enabled",
            if settings.eod_check_enabled {
                "true"
            } else {
                "false"
            },
        )?;
        set_in_tx(&tx, "pomodoro_minutes", &settings.pomodoro_minutes.to_string())?;
        set_in_tx(&tx, "theme", &settings.theme)?;
        set_in_tx(&tx, "mini_mode", &settings.mini_mode)?;
        set_in_tx(&tx, "pet_id", &settings.pet_id)?;
        set_in_tx(
            &tx,
            "auto_update",
            if settings.auto_update {
                "true"
            } else {
                "false"
            },
        )?;
        set_in_tx(
            &tx,
            "remember_last_category",
            if settings.remember_last_category {
                "true"
            } else {
                "false"
            },
        )?;
        set_in_tx(
            &tx,
            "mini_quick_categories",
            if settings.mini_quick_categories {
                "true"
            } else {
                "false"
            },
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn reparent_children(&self, from_id: i64, to_id: i64) -> Result<()> {
        let conn = lock_connection(&self.conn);
        conn.execute(
            "UPDATE time_slots SET parent_id = ?1, updated_at = ?2 WHERE parent_id = ?3",
            params![to_id, Local::now().to_rfc3339(), from_id],
        )?;
        Ok(())
    }

    /// Atomically close an interrupt slot and optionally update its parent slot.
    /// If parent_id is Some and parent_update is Some, updates the parent.
    pub fn end_interrupt_and_update_parent(
        &self,
        interrupt_id: i64,
        end_ts: DateTime<Local>,
        parent_id: Option<i64>,
        parent_end_ts: Option<DateTime<Local>>,
        parent_note: Option<String>,
    ) -> Result<()> {
        let mut conn = lock_connection(&self.conn);
        let tx = conn.transaction()?;

        tx.execute(
            "UPDATE time_slots SET end_ts = ?1, updated_at = ?2 WHERE id = ?3",
            params![end_ts.to_rfc3339(), Local::now().to_rfc3339(), interrupt_id],
        )?;

        if let Some(parent_id) = parent_id {
            if let Some(note) = parent_note {
                tx.execute(
                    "UPDATE time_slots SET end_ts = ?1, note = ?2, updated_at = ?3 WHERE id = ?4",
                    params![
                        parent_end_ts.map(|t| t.to_rfc3339()),
                        note,
                        Local::now().to_rfc3339(),
                        parent_id,
                    ],
                )?;
            } else if parent_end_ts.is_some() {
                tx.execute(
                    "UPDATE time_slots SET end_ts = ?1, updated_at = ?2 WHERE id = ?3",
                    params![
                        parent_end_ts.map(|t| t.to_rfc3339()),
                        Local::now().to_rfc3339(),
                        parent_id,
                    ],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Cut one finished slot into two at `at`. The original keeps its name and ends at `at`.
    pub fn split_slot(
        &self,
        id: i64,
        at: DateTime<Local>,
        second_name: Option<&str>,
    ) -> Result<i64> {
        let mut conn = lock_connection(&self.conn);
        let tx = conn.transaction()?;

        let slot = tx
            .query_row(
                &format!("{SLOT_SELECT} WHERE s.id = ?1"),
                params![id],
                slot_from_row,
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("找不到這個時段"))?;

        let end = slot
            .end_ts
            .ok_or_else(|| anyhow::anyhow!("進行中的紀錄要先結束才能分割"))?;
        if at <= slot.start_ts || at >= end {
            anyhow::bail!("分割時間要落在這筆紀錄中間");
        }

        let name = second_name
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .unwrap_or(slot.task_name.as_str());

        let category_id = slot.category_id.filter(|_| slot.slot_type.has_category());
        tx.execute(
            "UPDATE time_slots
             SET task_name = ?1, slot_type = ?2, start_ts = ?3, end_ts = ?4, note = ?5, updated_at = ?6,
                 category_id = ?8
             WHERE id = ?7",
            params![
                &slot.task_name,
                slot.slot_type.as_str(),
                slot.start_ts.to_rfc3339(),
                at.to_rfc3339(),
                slot.note,
                Local::now().to_rfc3339(),
                id,
                category_id,
            ],
        )?;

        let mut next = TimeSlot::new(name.to_string(), slot.slot_type, at);
        next.end_ts = Some(end);
        next.parent_id = slot.parent_id;
        next.category_id = slot.category_id;

        tx.execute(
            "INSERT INTO time_slots (uuid, task_name, slot_type, start_ts, end_ts, note, parent_id, created_at, updated_at, category_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                next.uuid.to_string(),
                next.task_name,
                next.slot_type.as_str(),
                next.start_ts.to_rfc3339(),
                next.end_ts.map(|t| t.to_rfc3339()),
                next.note,
                next.parent_id,
                next.created_at.to_rfc3339(),
                next.updated_at.to_rfc3339(),
                next.category_id.filter(|_| next.slot_type.has_category()),
            ],
        )?;

        let second_id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(second_id)
    }

    /// Join finished slots that touch or overlap. The earliest one is kept.
    pub fn merge_slots(&self, ids: &[i64], task_name: Option<&str>) -> Result<i64> {
        let mut unique = Vec::new();
        for id in ids {
            if !unique.contains(id) {
                unique.push(*id);
            }
        }
        if unique.len() < 2 {
            anyhow::bail!("請選至少兩筆紀錄");
        }

        let mut conn = lock_connection(&self.conn);
        let tx = conn.transaction()?;

        let mut slots = Vec::new();
        for id in &unique {
            let slot = tx
                .query_row(
                    &format!("{SLOT_SELECT} WHERE s.id = ?1"),
                    params![id],
                    slot_from_row,
                )
                .optional()?
                .ok_or_else(|| anyhow::anyhow!("找不到這個時段"))?;
            slots.push(slot);
        }

        slots.sort_by_key(|slot| slot.start_ts);

        for slot in &slots {
            if slot.end_ts.is_none() {
                anyhow::bail!("進行中的紀錄要先結束才能合併");
            }
        }

        for pair in slots.windows(2) {
            let prev_end = pair[0].end_ts.unwrap();
            if pair[1].start_ts > prev_end {
                anyhow::bail!("這幾筆中間有空檔，不能合併");
            }
        }

        let kept = &slots[0];
        let kept_id = kept.id.ok_or_else(|| anyhow::anyhow!("找不到這個時段"))?;
        let start = slots.iter().map(|slot| slot.start_ts).min().unwrap();
        let end = slots.iter().filter_map(|slot| slot.end_ts).max().unwrap();
        let name = task_name
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .unwrap_or(kept.task_name.as_str());

        let mut notes = Vec::new();
        for slot in &slots {
            if let Some(note) = slot
                .note
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            {
                if !notes.iter().any(|existing: &String| existing == note) {
                    notes.push(note.to_string());
                }
            }
        }
        let note = if notes.is_empty() {
            None
        } else {
            Some(notes.join("\n"))
        };

        let category_id = kept.category_id.filter(|_| kept.slot_type.has_category());
        tx.execute(
            "UPDATE time_slots
             SET task_name = ?1, slot_type = ?2, start_ts = ?3, end_ts = ?4, note = ?5, updated_at = ?6,
                 category_id = ?8
             WHERE id = ?7",
            params![
                name,
                kept.slot_type.as_str(),
                start.to_rfc3339(),
                end.to_rfc3339(),
                note,
                Local::now().to_rfc3339(),
                kept_id,
                category_id,
            ],
        )?;

        for slot in slots.iter().skip(1) {
            let id = slot.id.ok_or_else(|| anyhow::anyhow!("找不到這個時段"))?;
            tx.execute(
                "UPDATE time_slots SET parent_id = ?1, updated_at = ?2 WHERE parent_id = ?3",
                params![kept_id, Local::now().to_rfc3339(), id],
            )?;
            tx.execute("DELETE FROM time_slots WHERE id = ?", params![id])?;
        }

        tx.commit()?;
        Ok(kept_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_slot_name() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let slot = TimeSlot::new("舊名稱".to_string(), SlotType::Work, Local::now());
        let id = store.create_slot(&slot).unwrap();

        store
            .update_slot_details(
                id,
                SlotDetails {
                    task_name: "新名稱",
                    slot_type: SlotType::Meeting,
                    start_ts: slot.start_ts,
                    end_ts: None,
                    note: Some("備註"),
                    category_id: None,
                },
            )
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
        // A fixed morning time keeps both slots on one date whenever this runs.
        let date = Local::now().date_naive();
        let now = date
            .and_hms_opt(9, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .unwrap();

        let mut slot1 = TimeSlot::new("Work 1".to_string(), SlotType::Work, now);
        slot1.end_ts = Some(now + chrono::Duration::minutes(30));
        store.create_slot(&slot1).unwrap();

        let mut slot2 = TimeSlot::new(
            "Lunch".to_string(),
            SlotType::Lunch,
            now + chrono::Duration::minutes(30),
        );
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
        store
            .add_or_update_recent_task("Task A", SlotType::Work)
            .unwrap();
        store
            .add_or_update_recent_task("Task B", SlotType::Work)
            .unwrap();
        store
            .add_or_update_recent_task("Task A", SlotType::Work)
            .unwrap();

        let recent = store.get_recent_tasks(10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].task_name, "Task A");
        assert_eq!(recent[0].use_count, 2);
    }

    fn near(a: DateTime<Local>, b: DateTime<Local>) -> bool {
        a.signed_duration_since(b).num_seconds().abs() <= 1
    }

    #[test]
    fn test_split_keeps_first_half_and_names_the_second() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let start = Local::now();
        let end = start + chrono::Duration::minutes(60);
        let at = start + chrono::Duration::minutes(20);
        let mut slot = TimeSlot::new("甲".to_string(), SlotType::Work, start);
        slot.end_ts = Some(end);
        slot.note = Some("備註".to_string());
        let id = store.create_slot(&slot).unwrap();

        let second_id = store.split_slot(id, at, Some("乙")).unwrap();
        let first = store.get_slot(id).unwrap().unwrap();
        let second = store.get_slot(second_id).unwrap().unwrap();
        assert!(near(first.end_ts.unwrap(), at));
        assert!(near(second.start_ts, at));
        assert!(near(second.end_ts.unwrap(), end));
        assert_eq!(first.task_name, "甲");
        assert_eq!(second.task_name, "乙");
        assert_eq!(first.note.as_deref(), Some("備註"));
        assert!(store.split_slot(id, start, None).is_err());
    }

    #[test]
    fn test_merge_joins_touching_slots_and_rejects_a_gap() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let start = Local::now();
        let mut first = TimeSlot::new("甲".to_string(), SlotType::Work, start);
        first.end_ts = Some(start + chrono::Duration::minutes(20));
        first.note = Some("一".to_string());
        let first_id = store.create_slot(&first).unwrap();
        let mut second = TimeSlot::new(
            "乙".to_string(),
            SlotType::Meeting,
            start + chrono::Duration::minutes(20),
        );
        second.end_ts = Some(start + chrono::Duration::minutes(50));
        second.note = Some("二".to_string());
        let second_id = store.create_slot(&second).unwrap();

        let kept = store.merge_slots(&[second_id, first_id], None).unwrap();
        assert_eq!(kept, first_id);
        let merged = store.get_slot(first_id).unwrap().unwrap();
        assert_eq!(merged.task_name, "甲");
        assert_eq!(merged.slot_type, SlotType::Work);
        assert!(near(merged.start_ts, start));
        assert!(near(
            merged.end_ts.unwrap(),
            start + chrono::Duration::minutes(50)
        ));
        assert!(store.get_slot(second_id).unwrap().is_none());
        assert_eq!(merged.note.as_deref(), Some("一\n二"));

        let later = start + chrono::Duration::minutes(80);
        let mut third = TimeSlot::new("丙".to_string(), SlotType::Work, later);
        third.end_ts = Some(later + chrono::Duration::minutes(10));
        let third_id = store.create_slot(&third).unwrap();
        assert!(store.merge_slots(&[first_id, third_id], None).is_err());
    }

    #[test]
    fn test_merge_reparents_children() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let start = Local::now();
        let mut parent = TimeSlot::new("甲".to_string(), SlotType::Work, start);
        parent.end_ts = Some(start + chrono::Duration::minutes(30));
        let parent_id = store.create_slot(&parent).unwrap();
        let mut child = TimeSlot::new(
            "插隊".to_string(),
            SlotType::Interrupt,
            start + chrono::Duration::minutes(5),
        );
        child.end_ts = Some(start + chrono::Duration::minutes(10));
        child.parent_id = Some(parent_id);
        let child_id = store.create_slot(&child).unwrap();

        let mut next = TimeSlot::new(
            "乙".to_string(),
            SlotType::Work,
            start + chrono::Duration::minutes(30),
        );
        next.end_ts = Some(start + chrono::Duration::minutes(40));
        let next_id = store.create_slot(&next).unwrap();
        store.merge_slots(&[parent_id, next_id], None).unwrap();
        let interrupt = store.get_slot(child_id).unwrap().unwrap();
        assert_eq!(interrupt.parent_id, Some(parent_id));
    }

    fn category_id(store: &StoreV2, name: &str) -> i64 {
        store
            .get_categories()
            .unwrap()
            .into_iter()
            .find(|category| category.name == name)
            .unwrap()
            .id
    }

    fn slot_in(
        store: &StoreV2,
        slot_type: SlotType,
        category: Option<i64>,
        start: DateTime<Local>,
    ) -> i64 {
        let mut slot = TimeSlot::new("任務".to_string(), slot_type, start);
        slot.end_ts = Some(start + chrono::Duration::minutes(10));
        slot.category_id = category;
        store.create_slot(&slot).unwrap()
    }

    #[test]
    fn test_slot_reads_back_its_category() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let id = category_id(&store, "實作");
        store
            .update_category(id, "實作", Some(" A01 "), false)
            .unwrap();

        let slot_id = slot_in(&store, SlotType::Work, Some(id), Local::now());
        let slot = store.get_slot(slot_id).unwrap().unwrap();
        assert_eq!(slot.category_id, Some(id));
        assert_eq!(slot.category_name.as_deref(), Some("實作"));
        assert_eq!(slot.category_code.as_deref(), Some("A01"));
    }

    #[test]
    fn test_lunch_never_keeps_a_category() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let id = category_id(&store, "實作");
        let start = Local::now();

        let lunch = slot_in(&store, SlotType::Lunch, Some(id), start);
        assert_eq!(store.get_slot(lunch).unwrap().unwrap().category_id, None);

        let work = slot_in(&store, SlotType::Work, Some(id), start);
        store
            .update_slot_details(
                work,
                SlotDetails {
                    task_name: "任務",
                    slot_type: SlotType::Lunch,
                    start_ts: start,
                    end_ts: None,
                    note: None,
                    category_id: Some(id),
                },
            )
            .unwrap();
        assert_eq!(store.get_slot(work).unwrap().unwrap().category_id, None);
    }

    #[test]
    fn test_hiding_a_category_keeps_its_history() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let id = category_id(&store, "實作");
        let slot_id = slot_in(&store, SlotType::Work, Some(id), Local::now());

        store.update_category(id, "實作", None, true).unwrap();

        let slot = store.get_slot(slot_id).unwrap().unwrap();
        assert_eq!(slot.category_name.as_deref(), Some("實作"));
        let recent = store.get_recent_categories(10).unwrap();
        assert!(recent.iter().all(|category| category.id != id));
        assert_eq!(store.get_default_category(&SlotType::Work).unwrap(), None);
    }

    #[test]
    fn test_category_names_must_be_unique_and_not_blank() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let id = category_id(&store, "實作");

        assert!(store.create_category("  ", None).is_err());
        assert!(store.create_category("實作", None).is_err());
        assert!(store.update_category(id, "問題處理", None, false).is_err());
        store.update_category(id, "實作", None, false).unwrap();
    }

    #[test]
    fn test_new_category_goes_last_and_reorder_sticks() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let added = store.create_category("測試", Some("T9")).unwrap();
        assert_eq!(store.get_categories().unwrap().last().unwrap().id, added);

        let mut ids: Vec<i64> = store
            .get_categories()
            .unwrap()
            .iter()
            .map(|category| category.id)
            .collect();
        ids.reverse();
        store.reorder_categories(&ids).unwrap();

        let reordered: Vec<i64> = store
            .get_categories()
            .unwrap()
            .iter()
            .map(|category| category.id)
            .collect();
        assert_eq!(reordered, ids);
    }

    #[test]
    fn test_recent_categories_lead_and_are_padded_to_the_limit() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let docs = category_id(&store, "文件維護");
        let data = category_id(&store, "資料調整");
        let start = Local::now();
        slot_in(&store, SlotType::Work, Some(docs), start);
        slot_in(
            &store,
            SlotType::Meeting,
            Some(data),
            start + chrono::Duration::minutes(10),
        );

        let names: Vec<String> = store
            .get_recent_categories(3)
            .unwrap()
            .into_iter()
            .map(|category| category.name)
            .collect();
        assert_eq!(names, ["資料調整", "文件維護", "實作"]);
    }

    #[test]
    fn test_default_category_prefers_the_same_status() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let build = category_id(&store, "實作");
        let fix = category_id(&store, "問題處理");
        let start = Local::now();
        slot_in(&store, SlotType::Work, Some(build), start);
        slot_in(
            &store,
            SlotType::Interrupt,
            Some(fix),
            start + chrono::Duration::minutes(10),
        );

        assert_eq!(
            store.get_default_category(&SlotType::Work).unwrap(),
            Some(build)
        );
        assert_eq!(
            store.get_default_category(&SlotType::Interrupt).unwrap(),
            Some(fix)
        );
        assert_eq!(
            store.get_default_category(&SlotType::Meeting).unwrap(),
            Some(fix)
        );
        assert_eq!(store.get_default_category(&SlotType::Lunch).unwrap(), None);
    }

    #[test]
    fn test_default_category_respects_the_setting() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        slot_in(
            &store,
            SlotType::Work,
            Some(category_id(&store, "實作")),
            Local::now(),
        );

        let mut settings = store.get_settings().unwrap();
        assert!(settings.remember_last_category);
        assert!(settings.mini_quick_categories);
        settings.remember_last_category = false;
        settings.mini_quick_categories = false;
        store.save_settings(&settings).unwrap();

        let saved = store.get_settings().unwrap();
        assert!(!saved.remember_last_category);
        assert!(!saved.mini_quick_categories);
        assert_eq!(store.get_default_category(&SlotType::Work).unwrap(), None);
    }

    #[test]
    fn test_split_and_merge_keep_the_category() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let id = category_id(&store, "規格討論");
        let start = Local::now();
        let mut slot = TimeSlot::new("甲".to_string(), SlotType::Work, start);
        slot.end_ts = Some(start + chrono::Duration::minutes(60));
        slot.category_id = Some(id);
        let first = store.create_slot(&slot).unwrap();

        let second = store
            .split_slot(first, start + chrono::Duration::minutes(20), None)
            .unwrap();
        assert_eq!(
            store.get_slot(first).unwrap().unwrap().category_id,
            Some(id)
        );
        assert_eq!(
            store.get_slot(second).unwrap().unwrap().category_id,
            Some(id)
        );

        let kept = store.merge_slots(&[first, second], None).unwrap();
        assert_eq!(store.get_slot(kept).unwrap().unwrap().category_id, Some(id));
    }

    #[test]
    fn test_category_report_reads_from_the_database() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let id = category_id(&store, "實作");
        let start = Local::now();
        slot_in(&store, SlotType::Work, Some(id), start);
        slot_in(&store, SlotType::Lunch, None, start);
        let date = start.date_naive();

        let report = store.get_category_report(date, date).unwrap();
        assert_eq!(report.total_seconds, 600);
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].name.as_deref(), Some("實作"));
        assert!(store
            .get_category_report(date, date - chrono::Duration::days(1))
            .is_err());
    }

    #[test]
    fn test_corrupted_timestamp_returns_error_without_panic() {
        use tempfile::NamedTempFile;
        
        let tmp = NamedTempFile::new().unwrap();
        let store = StoreV2::new(tmp.path()).unwrap();
        
        let conn = lock_connection(&store.conn);
        conn.execute("PRAGMA foreign_keys = OFF", []).unwrap();
        conn.execute(
            "INSERT INTO time_slots (uuid, task_name, slot_type, start_ts, end_ts, created_at, updated_at)
             VALUES ('550e8400-e29b-41d4-a716-446655440000', 'Test', 'work', 'not-a-timestamp', NULL,
                     '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        drop(conn);

        let result = store.get_current_slot();
        assert!(result.is_err(), "Parsing corrupted timestamp should return error");
    }

    #[test]
    fn test_invalid_uuid_returns_error_without_panic() {
        use tempfile::NamedTempFile;
        
        let tmp = NamedTempFile::new().unwrap();
        let store = StoreV2::new(tmp.path()).unwrap();
        
        let conn = lock_connection(&store.conn);
        conn.execute("PRAGMA foreign_keys = OFF", []).unwrap();
        conn.execute(
            "INSERT INTO time_slots (uuid, task_name, slot_type, start_ts, end_ts, created_at, updated_at)
             VALUES ('not-a-uuid', 'Test', 'work', '2024-01-01T00:00:00Z', NULL,
                     '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        drop(conn);

        let result = store.get_current_slot();
        assert!(result.is_err(), "Parsing invalid UUID should return error");
    }

    #[test]
    fn test_invalid_slot_type_returns_error_without_panic() {
        use tempfile::NamedTempFile;
        
        let tmp = NamedTempFile::new().unwrap();
        let store = StoreV2::new(tmp.path()).unwrap();
        
        let conn = lock_connection(&store.conn);
        conn.execute("DROP TABLE time_slots", []).unwrap();
        conn.execute(
            "CREATE TABLE time_slots (
                id INTEGER PRIMARY KEY,
                uuid TEXT NOT NULL UNIQUE,
                task_name TEXT NOT NULL,
                slot_type TEXT NOT NULL,
                start_ts TEXT NOT NULL,
                end_ts TEXT,
                note TEXT,
                parent_id INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                category_id INTEGER,
                FOREIGN KEY (parent_id) REFERENCES time_slots(id),
                FOREIGN KEY (category_id) REFERENCES categories(id)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO time_slots (uuid, task_name, slot_type, start_ts, end_ts, created_at, updated_at)
             VALUES ('550e8400-e29b-41d4-a716-446655440000', 'Test', 'invalid-type', 
                     '2024-01-01T00:00:00Z', NULL, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);

        let result = store.get_current_slot();
        assert!(result.is_err(), "Parsing invalid slot_type should return error");
    }

    #[test]
    fn test_store_recovers_after_database_error() {
        use tempfile::NamedTempFile;
        
        let tmp = NamedTempFile::new().unwrap();
        let store = StoreV2::new(tmp.path()).unwrap();
        
        let conn = lock_connection(&store.conn);
        conn.execute("PRAGMA foreign_keys = OFF", []).unwrap();
        conn.execute(
            "INSERT INTO time_slots (uuid, task_name, slot_type, start_ts, end_ts, created_at, updated_at)
             VALUES ('not-a-uuid', 'Bad', 'work', '2024-01-01T00:00:00Z', NULL,
                     '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();
        drop(conn);

        let result = store.get_current_slot();
        assert!(result.is_err(), "Should fail on corrupted data");

        let slot = TimeSlot::new("Good Task".to_string(), SlotType::Work, Local::now());
        let id = store.create_slot(&slot).unwrap();
        let retrieved = store.get_slot(id).unwrap().unwrap();
        assert_eq!(retrieved.task_name, "Good Task");
    }

    #[test]
    fn test_create_slots_batch_all_or_nothing() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let start = Local::now();
        
        let mut slots = vec![
            TimeSlot::new("Task 1".to_string(), SlotType::Work, start),
            TimeSlot::new("Task 2".to_string(), SlotType::Work, start + chrono::Duration::minutes(10)),
        ];
        slots[0].end_ts = Some(start + chrono::Duration::minutes(10));
        slots[1].end_ts = Some(start + chrono::Duration::minutes(20));

        let ids = store.create_slots(&slots).unwrap();
        assert_eq!(ids.len(), 2);
        
        let retrieved1 = store.get_slot(ids[0]).unwrap().unwrap();
        let retrieved2 = store.get_slot(ids[1]).unwrap().unwrap();
        assert_eq!(retrieved1.task_name, "Task 1");
        assert_eq!(retrieved2.task_name, "Task 2");
    }

    #[test]
    fn test_create_slots_third_invalid_rolls_back_all() {
        use tempfile::NamedTempFile;
        
        let tmp = NamedTempFile::new().unwrap();
        let store = StoreV2::new(tmp.path()).unwrap();
        let start = Local::now();

        let conn = lock_connection(&store.conn);
        conn.execute("DROP TABLE time_slots", []).unwrap();
        conn.execute(
            "CREATE TABLE time_slots (
                id INTEGER PRIMARY KEY,
                uuid TEXT NOT NULL UNIQUE,
                task_name TEXT NOT NULL,
                slot_type TEXT NOT NULL,
                start_ts TEXT NOT NULL,
                end_ts TEXT,
                note TEXT,
                parent_id INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                category_id INTEGER,
                FOREIGN KEY (parent_id) REFERENCES time_slots(id),
                FOREIGN KEY (category_id) REFERENCES categories(id),
                CHECK (length(task_name) > 0)
            )",
            [],
        )
        .unwrap();
        drop(conn);

        let mut slots = vec![
            TimeSlot::new("Task 1".to_string(), SlotType::Work, start),
            TimeSlot::new("Task 2".to_string(), SlotType::Work, start + chrono::Duration::minutes(10)),
            TimeSlot::new("".to_string(), SlotType::Work, start + chrono::Duration::minutes(20)),
        ];
        slots[0].end_ts = Some(start + chrono::Duration::minutes(10));
        slots[1].end_ts = Some(start + chrono::Duration::minutes(20));
        slots[2].end_ts = Some(start + chrono::Duration::minutes(30));

        let result = store.create_slots(&slots);
        assert!(result.is_err());

        let all_slots = store.get_slots_for_date(start.date_naive()).unwrap();
        assert_eq!(all_slots.len(), 0, "All slots should be rolled back");
    }

    #[test]
    fn test_split_slot_lengths_sum_to_original() {
        let store = StoreV2::new(Path::new(":memory:")).unwrap();
        let start = Local::now();
        let end = start + chrono::Duration::minutes(60);
        let at = start + chrono::Duration::minutes(25);

        let mut slot = TimeSlot::new("Original".to_string(), SlotType::Work, start);
        slot.end_ts = Some(end);
        let id = store.create_slot(&slot).unwrap();

        let second_id = store.split_slot(id, at, Some("Second")).unwrap();

        let first = store.get_slot(id).unwrap().unwrap();
        let second = store.get_slot(second_id).unwrap().unwrap();

        let first_duration = first.end_ts.unwrap().signed_duration_since(first.start_ts);
        let second_duration = second.end_ts.unwrap().signed_duration_since(second.start_ts);
        let total_duration = first_duration + second_duration;
        let original_duration = end.signed_duration_since(start);

        assert_eq!(total_duration, original_duration);
    }

    #[test]
    fn test_merge_with_nonexistent_id_returns_err_with_db_unchanged() {
        use tempfile::NamedTempFile;
        
        let tmp = NamedTempFile::new().unwrap();
        let store = StoreV2::new(tmp.path()).unwrap();
        let start = Local::now();

        let mut slot = TimeSlot::new("Task".to_string(), SlotType::Work, start);
        slot.end_ts = Some(start + chrono::Duration::minutes(10));
        let id = store.create_slot(&slot).unwrap();

        let result = store.merge_slots(&[id, 9999], None);
        assert!(result.is_err());

        let retrieved = store.get_slot(id).unwrap().unwrap();
        assert_eq!(retrieved.task_name, "Task");
        assert_eq!(retrieved.end_ts, Some(start + chrono::Duration::minutes(10)));
    }

    #[test]
    fn test_save_settings_is_atomic() {
        use tempfile::NamedTempFile;
        
        let tmp = NamedTempFile::new().unwrap();
        let store = StoreV2::new(tmp.path()).unwrap();

        let mut settings = store.get_settings().unwrap();
        settings.lunch_start = "12:30".to_string();
        settings.pomodoro_minutes = 30;
        settings.theme = "dark".to_string();

        store.save_settings(&settings).unwrap();

        let saved = store.get_settings().unwrap();
        assert_eq!(saved.lunch_start, "12:30");
        assert_eq!(saved.pomodoro_minutes, 30);
        assert_eq!(saved.theme, "dark");
    }

    #[test]
    fn test_end_interrupt_and_update_parent_is_atomic() {
        use tempfile::NamedTempFile;
        
        let tmp = NamedTempFile::new().unwrap();
        let store = StoreV2::new(tmp.path()).unwrap();
        let start = Local::now();

        let mut parent = TimeSlot::new("Work".to_string(), SlotType::Work, start);
        parent.end_ts = None;
        let parent_id = store.create_slot(&parent).unwrap();

        let mut interrupt = TimeSlot::new("插隊".to_string(), SlotType::Interrupt, start + chrono::Duration::minutes(5));
        interrupt.parent_id = Some(parent_id);
        let interrupt_id = store.create_slot(&interrupt).unwrap();

        let end_time = start + chrono::Duration::minutes(10);
        store
            .end_interrupt_and_update_parent(
                interrupt_id,
                end_time,
                Some(parent_id),
                Some(end_time),
                Some("完成".to_string()),
            )
            .unwrap();

        let ended_interrupt = store.get_slot(interrupt_id).unwrap().unwrap();
        let ended_parent = store.get_slot(parent_id).unwrap().unwrap();

        assert_eq!(ended_interrupt.end_ts, Some(end_time));
        assert_eq!(ended_parent.end_ts, Some(end_time));
        assert_eq!(ended_parent.note.as_deref(), Some("完成"));
    }
}
