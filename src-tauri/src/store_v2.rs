use anyhow::Result;
use chrono::{DateTime, Local, NaiveDate, NaiveTime};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use uuid::Uuid;

use crate::migrations::MigrationManager;
use crate::models::*;
use crate::report::build_category_report;

#[derive(Debug)]
struct ConversionError(String);

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ConversionError {}

const SLOT_SELECT: &str =
    "SELECT s.id, s.uuid, s.task_name, s.slot_type, s.start_ts, s.end_ts, s.note, s.parent_id,
            s.created_at, s.updated_at, s.category_id, c.name, c.report_code
     FROM time_slots s
     LEFT JOIN categories c ON c.id = s.category_id";

const CATEGORY_COLUMNS: &str = "c.id, c.name, c.report_code, c.sort_order, c.hidden";

fn conversion_error(idx: usize, msg: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        idx,
        rusqlite::types::Type::Text,
        Box::new(ConversionError(msg)),
    )
}

fn parse_ts(idx: usize, value: &str) -> rusqlite::Result<DateTime<Local>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Local))
        .map_err(|e| conversion_error(idx, format!("Failed to parse timestamp {value:?}: {e}")))
}

/// Read an RFC3339 column without panicking on bad data.
fn ts_at(row: &Row, idx: usize) -> rusqlite::Result<DateTime<Local>> {
    parse_ts(idx, &row.get::<_, String>(idx)?)
}

fn opt_ts_at(row: &Row, idx: usize) -> rusqlite::Result<Option<DateTime<Local>>> {
    row.get::<_, Option<String>>(idx)?
        .map(|value| parse_ts(idx, &value))
        .transpose()
}

fn slot_type_at(row: &Row, idx: usize) -> rusqlite::Result<SlotType> {
    let value: String = row.get(idx)?;
    value.parse().map_err(|e: anyhow::Error| {
        conversion_error(idx, format!("Failed to parse slot_type: {e}"))
    })
}

fn slot_from_row(row: &Row) -> rusqlite::Result<TimeSlot> {
    let uuid_str: String = row.get(1)?;
    let uuid = Uuid::parse_str(&uuid_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;

    Ok(TimeSlot {
        id: Some(row.get(0)?),
        uuid,
        task_name: row.get(2)?,
        slot_type: slot_type_at(row, 3)?,
        start_ts: ts_at(row, 4)?,
        end_ts: opt_ts_at(row, 5)?,
        note: row.get(6)?,
        parent_id: row.get(7)?,
        created_at: ts_at(row, 8)?,
        updated_at: ts_at(row, 9)?,
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

/// The first instant of `date` in local time. If midnight falls in a DST gap,
/// use the first valid instant after it.
fn local_day_start(date: NaiveDate) -> Result<DateTime<Local>> {
    let midnight = date.and_time(NaiveTime::MIN);
    midnight
        .and_local_timezone(Local)
        .earliest()
        .or_else(|| {
            (midnight + chrono::Duration::hours(1))
                .and_local_timezone(Local)
                .earliest()
        })
        .ok_or_else(|| anyhow::anyhow!("無法轉換日期到本地時區：{date}"))
}

/// Half-open bounds `[start 00:00, end + 1 day 00:00)` for `start_ts` comparisons.
///
/// TODO: start_ts is compared as an RFC3339 string, which only orders correctly
/// while every row carries the same UTC offset. Rows written under a different
/// time zone (travel, DST change) can fall on the wrong side of the bounds.
fn day_range(start: NaiveDate, end: NaiveDate) -> Result<(String, String)> {
    let after_end = end
        .succ_opt()
        .ok_or_else(|| anyhow::anyhow!("日期超出範圍：{end}"))?;
    Ok((
        local_day_start(start)?.to_rfc3339(),
        local_day_start(after_end)?.to_rfc3339(),
    ))
}

// The helpers below take `&Connection` so they work both on the plain
// connection and inside a `Transaction` (which derefs to `Connection`).

fn load_slot(conn: &Connection, id: i64) -> Result<Option<TimeSlot>> {
    let slot = conn
        .query_row(
            &format!("{SLOT_SELECT} WHERE s.id = ?1"),
            params![id],
            slot_from_row,
        )
        .optional()?;
    Ok(slot)
}

fn insert_slot(conn: &Connection, slot: &TimeSlot) -> Result<i64> {
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

/// Set `end_ts`, and the note only when one is given. Returns the number of rows changed.
fn end_slot(
    conn: &Connection,
    id: i64,
    end_ts: Option<DateTime<Local>>,
    note: Option<&str>,
) -> Result<usize> {
    let end_ts = end_ts.map(|t| t.to_rfc3339());
    let now = Local::now().to_rfc3339();
    let changed = match note {
        Some(note) => conn.execute(
            "UPDATE time_slots SET end_ts = ?1, note = ?2, updated_at = ?3 WHERE id = ?4",
            params![end_ts, note, now, id],
        )?,
        None => conn.execute(
            "UPDATE time_slots SET end_ts = ?1, updated_at = ?2 WHERE id = ?3",
            params![end_ts, now, id],
        )?,
    };
    Ok(changed)
}

fn write_slot_details(conn: &Connection, id: i64, details: &SlotDetails) -> Result<()> {
    let category_id = details
        .category_id
        .filter(|_| details.slot_type.has_category());
    let changed = conn.execute(
        "UPDATE time_slots
         SET task_name = ?1, slot_type = ?2, start_ts = ?3, end_ts = ?4, note = ?5, updated_at = ?6,
             category_id = ?8
         WHERE id = ?7",
        params![
            details.task_name,
            details.slot_type.as_str(),
            details.start_ts.to_rfc3339(),
            details.end_ts.map(|t| t.to_rfc3339()),
            details.note,
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

fn touch_recent_task(conn: &Connection, task_name: &str, slot_type: &SlotType) -> Result<()> {
    conn.execute(
        "INSERT INTO recent_tasks (task_name, last_used, use_count, slot_type)
         VALUES (?1, ?2, 1, ?3)
         ON CONFLICT(task_name) DO UPDATE SET
         last_used = ?2, use_count = use_count + 1",
        params![task_name, Local::now().to_rfc3339(), slot_type.as_str()],
    )?;
    Ok(())
}

fn reparent(conn: &Connection, from_id: i64, to_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE time_slots SET parent_id = ?1, updated_at = ?2 WHERE parent_id = ?3",
        params![to_id, Local::now().to_rfc3339(), from_id],
    )?;
    Ok(())
}

pub struct SlotDetails<'a> {
    pub task_name: &'a str,
    pub slot_type: SlotType,
    pub start_ts: DateTime<Local>,
    pub end_ts: Option<DateTime<Local>>,
    pub note: Option<&'a str>,
    pub category_id: Option<i64>,
}

/// What the stretch cut out by `carve_slot` becomes.
pub struct CarvedPart<'a> {
    pub task_name: &'a str,
    pub slot_type: SlotType,
    pub category_id: Option<i64>,
}

pub struct StoreV2 {
    conn: Arc<Mutex<Connection>>,
}

impl StoreV2 {
    /// Lock the connection, recovering if a previous holder panicked.
    /// SQLite rolls back any open transaction, so the data itself is still consistent.
    fn lock_conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("Recovered from poisoned connection mutex");
            poisoned.into_inner()
        })
    }

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
        let conn = self.lock_conn();
        insert_slot(&conn, slot)
    }

    /// Insert every slot and remember its task name, or write nothing at all.
    pub fn create_slots(&self, slots: &[TimeSlot]) -> Result<Vec<i64>> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction()?;
        let mut ids = Vec::with_capacity(slots.len());
        for slot in slots {
            ids.push(insert_slot(&tx, slot)?);
            touch_recent_task(&tx, &slot.task_name, &slot.slot_type)?;
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
        let conn = self.lock_conn();
        end_slot(&conn, id, end_ts, note.as_deref())?;
        Ok(())
    }

    /// Close a slot without erasing its note.
    pub fn close_slot(&self, id: i64, end_ts: DateTime<Local>) -> Result<()> {
        let conn = self.lock_conn();
        end_slot(&conn, id, Some(end_ts), None)?;
        Ok(())
    }

    pub fn update_slot_details(&self, id: i64, details: SlotDetails) -> Result<()> {
        let conn = self.lock_conn();
        write_slot_details(&conn, id, &details)
    }

    pub fn get_slot(&self, id: i64) -> Result<Option<TimeSlot>> {
        let conn = self.lock_conn();
        load_slot(&conn, id)
    }

    pub fn get_current_slot(&self) -> Result<Option<TimeSlot>> {
        let conn = self.lock_conn();
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
        let conn = self.lock_conn();
        conn.execute("DELETE FROM time_slots WHERE id = ?", params![id])?;
        Ok(())
    }
    pub fn get_slots_for_date(&self, date: NaiveDate) -> Result<Vec<TimeSlot>> {
        self.get_slots_for_range(date, date)
    }

    pub fn get_slots_for_range(&self, start: NaiveDate, end: NaiveDate) -> Result<Vec<TimeSlot>> {
        let conn = self.lock_conn();
        let (from, until) = day_range(start, end)?;

        let mut stmt = conn.prepare(&format!(
            "{SLOT_SELECT}
             WHERE s.start_ts >= ?1 AND s.start_ts < ?2
             ORDER BY s.start_ts ASC"
        ))?;

        let slots = stmt
            .query_map(params![from, until], slot_from_row)?
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
        let conn = self.lock_conn();
        let (from, until) = day_range(start, end)?;

        let mut stmt = conn.prepare(
            "SELECT task_name, slot_type, 
                    SUM((julianday(COALESCE(end_ts, datetime('now','localtime'))) - julianday(start_ts)) * 24 * 60) as total_minutes,
                    COUNT(*) as slot_count
             FROM time_slots 
             WHERE start_ts >= ?1 AND start_ts < ?2 AND end_ts IS NOT NULL
             GROUP BY task_name, slot_type
             ORDER BY total_minutes DESC",
        )?;

        let summaries = stmt
            .query_map(params![from, until], |row| {
                let minutes: f64 = row.get(2)?;
                Ok(TaskSummary {
                    task_name: row.get(0)?,
                    slot_type: slot_type_at(row, 1)?,
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
        let conn = self.lock_conn();
        touch_recent_task(&conn, task_name, &slot_type)
    }

    pub fn get_recent_tasks(&self, limit: usize) -> Result<Vec<RecentTask>> {
        let conn = self.lock_conn();
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
                    last_used: ts_at(row, 2)?,
                    use_count: row.get(3)?,
                    slot_type: slot_type_at(row, 4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tasks)
    }

    pub fn add_or_update_recent_person(&self, name: &str) -> Result<()> {
        let conn = self.lock_conn();
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
        let conn = self.lock_conn();
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
                    last_used: ts_at(row, 2)?,
                    use_count: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(people)
    }

    // ==================== Categories ====================

    /// All categories in display order, hidden ones included.
    pub fn get_categories(&self) -> Result<Vec<Category>> {
        let conn = self.lock_conn();
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
        let conn = self.lock_conn();
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
        let conn = self.lock_conn();
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
        let mut conn = self.lock_conn();
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
        let conn = self.lock_conn();
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
        let conn = self.lock_conn();
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
        let conn = self.lock_conn();
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
        let conn = self.lock_conn();
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
        if let Some(v) = self.get_setting("theme")? {
            settings.theme = v;
        }
        if let Some(v) = self.get_setting("auto_update")? {
            settings.auto_update = v == "true";
        }
        if let Some(v) = self.get_setting("remember_last_category")? {
            settings.remember_last_category = v == "true";
        }

        Ok(settings)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        let mut conn = self.lock_conn();
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
        set_in_tx(&tx, "theme", &settings.theme)?;
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

        tx.commit()?;
        Ok(())
    }

    pub fn reparent_children(&self, from_id: i64, to_id: i64) -> Result<()> {
        let conn = self.lock_conn();
        reparent(&conn, from_id, to_id)
    }

    /// Close the interrupt slot and end its parent work slot in one transaction.
    /// Fails without writing anything if either slot is missing.
    pub fn end_interrupt_atomic(
        &self,
        interrupt_id: i64,
        parent_id: i64,
        end_ts: DateTime<Local>,
        note: Option<String>,
    ) -> Result<()> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction()?;
        if end_slot(&tx, interrupt_id, Some(end_ts), None)? == 0 {
            anyhow::bail!("找不到插隊紀錄");
        }
        if end_slot(&tx, parent_id, Some(end_ts), note.as_deref())? == 0 {
            anyhow::bail!("找不到被插隊的工作紀錄");
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
        let mut conn = self.lock_conn();
        let tx = conn.transaction()?;

        let slot = load_slot(&tx, id)?.ok_or_else(|| anyhow::anyhow!("找不到這個時段"))?;

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

        write_slot_details(
            &tx,
            id,
            &SlotDetails {
                task_name: &slot.task_name,
                slot_type: slot.slot_type.clone(),
                start_ts: slot.start_ts,
                end_ts: Some(at),
                note: slot.note.as_deref(),
                category_id: slot.category_id,
            },
        )?;

        let mut next = TimeSlot::new(name.to_string(), slot.slot_type.clone(), at);
        next.end_ts = Some(end);
        next.note = slot.note.clone();
        next.parent_id = slot.parent_id;
        next.category_id = slot.category_id;
        let next_id = insert_slot(&tx, &next)?;

        tx.commit()?;
        Ok(next_id)
    }

    /// Give the stretch `from..to` of one finished slot to another task.
    /// The rest keeps the original task, so the slot ends up in two or three
    /// pieces. The original row stays the earliest piece. Returns the id of the
    /// carved-out piece.
    pub fn carve_slot(
        &self,
        id: i64,
        from: DateTime<Local>,
        to: DateTime<Local>,
        carved: CarvedPart<'_>,
    ) -> Result<i64> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction()?;

        let slot = load_slot(&tx, id)?.ok_or_else(|| anyhow::anyhow!("找不到這個時段"))?;
        let end = slot
            .end_ts
            .ok_or_else(|| anyhow::anyhow!("進行中的紀錄要先結束才能分割"))?;

        if from < slot.start_ts || to > end || from >= to {
            anyhow::bail!("拆出的時段要落在這筆紀錄裡");
        }
        if from == slot.start_ts && to == end {
            anyhow::bail!("拆出的時段不能是整筆紀錄，請直接編輯");
        }

        let name = Some(carved.task_name.trim())
            .filter(|text| !text.is_empty())
            .unwrap_or(slot.task_name.as_str());
        let carved_details = SlotDetails {
            task_name: name,
            slot_type: carved.slot_type.clone(),
            start_ts: from,
            end_ts: Some(to),
            note: None,
            category_id: carved.category_id,
        };
        let original = |start_ts, end_ts| SlotDetails {
            task_name: &slot.task_name,
            slot_type: slot.slot_type.clone(),
            start_ts,
            end_ts: Some(end_ts),
            note: slot.note.as_deref(),
            category_id: slot.category_id,
        };
        let insert = |details: &SlotDetails| {
            let mut piece = TimeSlot::new(
                details.task_name.to_string(),
                details.slot_type.clone(),
                details.start_ts,
            );
            piece.end_ts = details.end_ts;
            piece.note = details.note.map(str::to_string);
            piece.parent_id = slot.parent_id;
            piece.category_id = details.category_id;
            insert_slot(&tx, &piece)
        };

        let carved_id = if from > slot.start_ts {
            write_slot_details(&tx, id, &original(slot.start_ts, from))?;
            insert(&carved_details)?
        } else {
            write_slot_details(&tx, id, &carved_details)?;
            id
        };
        if to < end {
            insert(&original(to, end))?;
        }
        touch_recent_task(&tx, name, &carved.slot_type)?;

        tx.commit()?;
        Ok(carved_id)
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

        let mut conn = self.lock_conn();
        let tx = conn.transaction()?;

        let mut slots = Vec::new();
        for id in &unique {
            let slot = load_slot(&tx, *id)?.ok_or_else(|| anyhow::anyhow!("找不到這個時段"))?;
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

        write_slot_details(
            &tx,
            kept_id,
            &SlotDetails {
                task_name: name,
                slot_type: kept.slot_type.clone(),
                start_ts: start,
                end_ts: Some(end),
                note: note.as_deref(),
                category_id: kept.category_id,
            },
        )?;

        for slot in slots.iter().skip(1) {
            let id = slot.id.ok_or_else(|| anyhow::anyhow!("找不到這個時段"))?;
            reparent(&tx, id, kept_id)?;
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
        settings.remember_last_category = false;
        store.save_settings(&settings).unwrap();

        let saved = store.get_settings().unwrap();
        assert!(!saved.remember_last_category);
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

    // P1 atomicity tests

    fn temp_store() -> (tempfile::NamedTempFile, StoreV2) {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let store = StoreV2::new(temp.path()).unwrap();
        (temp, store)
    }

    /// Snapshot of every row, used to prove a failed operation left the DB untouched.
    type SlotRow = (
        i64,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<i64>,
    );

    fn dump_slots(store: &StoreV2) -> Vec<SlotRow> {
        let conn = store.lock_conn();
        let mut stmt = conn
            .prepare("SELECT id, task_name, start_ts, end_ts, note, parent_id FROM time_slots ORDER BY id")
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        rows
    }

    /// Make the given statement kind fail, so a multi-step operation dies half way.
    fn fail_on(store: &StoreV2, event: &str) {
        let conn = store.lock_conn();
        conn.execute_batch(&format!(
            "CREATE TRIGGER test_fail BEFORE {event} ON time_slots BEGIN SELECT RAISE(ABORT, 'boom'); END;"
        ))
        .unwrap();
    }

    fn finished(name: &str, start: DateTime<Local>, minutes: i64) -> TimeSlot {
        let mut slot = TimeSlot::new(name.to_string(), SlotType::Work, start);
        slot.end_ts = Some(start + chrono::Duration::minutes(minutes));
        slot
    }

    #[test]
    fn test_split_durations_add_up_to_the_original() {
        let (_temp, store) = temp_store();
        let start = Local::now() - chrono::Duration::hours(2);
        let id = store.create_slot(&finished("原始", start, 60)).unwrap();
        let original = store.get_slot(id).unwrap().unwrap();

        let at = start + chrono::Duration::seconds(1234);
        let second_id = store.split_slot(id, at, None).unwrap();
        let first = store.get_slot(id).unwrap().unwrap();
        let second = store.get_slot(second_id).unwrap().unwrap();

        let span = |s: &TimeSlot| s.end_ts.unwrap() - s.start_ts;
        assert_eq!(span(&first) + span(&second), span(&original));
        assert_eq!(first.end_ts, Some(second.start_ts));
    }

    #[test]
    fn test_split_rolls_back_when_the_insert_fails() {
        let (_temp, store) = temp_store();
        let start = Local::now() - chrono::Duration::hours(2);
        let id = store.create_slot(&finished("原始", start, 60)).unwrap();
        let before = dump_slots(&store);

        fail_on(&store, "INSERT");
        let at = start + chrono::Duration::minutes(20);
        assert!(store.split_slot(id, at, Some("新")).is_err());

        assert_eq!(dump_slots(&store), before);
    }

    fn meeting(name: &str) -> CarvedPart<'_> {
        CarvedPart {
            task_name: name,
            slot_type: SlotType::Meeting,
            category_id: None,
        }
    }

    #[test]
    fn test_carve_from_the_middle_leaves_three_pieces() {
        let (_temp, store) = temp_store();
        let start = Local::now() - chrono::Duration::hours(3);
        let mut slot = finished("寫程式", start, 120);
        slot.note = Some("原本的備註".into());
        let id = store.create_slot(&slot).unwrap();
        let end = start + chrono::Duration::minutes(120);

        let from = start + chrono::Duration::minutes(30);
        let to = start + chrono::Duration::minutes(75);
        let carved_id = store.carve_slot(id, from, to, meeting("站會")).unwrap();

        let head = store.get_slot(id).unwrap().unwrap();
        let carved = store.get_slot(carved_id).unwrap().unwrap();
        assert_eq!(
            (head.task_name.as_str(), head.start_ts, head.end_ts),
            ("寫程式", start, Some(from))
        );
        assert_eq!(
            (carved.task_name.as_str(), carved.start_ts, carved.end_ts),
            ("站會", from, Some(to))
        );
        assert_eq!(carved.slot_type, SlotType::Meeting);
        assert_eq!(carved.note, None);

        let slots = store.get_slots_for_date(start.date_naive()).unwrap();
        let tail = slots.iter().find(|s| s.start_ts == to).unwrap();
        assert_eq!(
            (tail.task_name.as_str(), tail.end_ts),
            ("寫程式", Some(end))
        );
        assert_eq!(tail.slot_type, SlotType::Work);
        assert_eq!(tail.note.as_deref(), Some("原本的備註"));
        assert_eq!(dump_slots(&store).len(), 3);

        let recent = store.get_recent_tasks(10).unwrap();
        assert!(recent.iter().any(|task| task.task_name == "站會"));
    }

    #[test]
    fn test_carve_at_either_end_leaves_two_pieces() {
        let (_temp, store) = temp_store();
        let start = Local::now() - chrono::Duration::hours(3);
        let end = start + chrono::Duration::minutes(60);
        let cut = start + chrono::Duration::minutes(20);

        let first = store.create_slot(&finished("甲", start, 60)).unwrap();
        let carved_id = store
            .carve_slot(first, start, cut, meeting("會議"))
            .unwrap();
        assert_eq!(
            carved_id, first,
            "the original row stays the earliest piece"
        );
        let carved = store.get_slot(first).unwrap().unwrap();
        assert_eq!(
            (carved.task_name.as_str(), carved.end_ts),
            ("會議", Some(cut))
        );

        let second = store
            .create_slot(&finished("乙", end + chrono::Duration::minutes(10), 60))
            .unwrap();
        let second_start = end + chrono::Duration::minutes(10);
        let second_end = second_start + chrono::Duration::minutes(60);
        let tail_from = second_start + chrono::Duration::minutes(45);
        let tail_id = store
            .carve_slot(second, tail_from, second_end, meeting(""))
            .unwrap();
        let head = store.get_slot(second).unwrap().unwrap();
        let tail = store.get_slot(tail_id).unwrap().unwrap();
        assert_eq!(head.end_ts, Some(tail_from));
        assert_eq!(
            (tail.task_name.as_str(), tail.end_ts),
            ("乙", Some(second_end)),
            "a blank name keeps the original"
        );

        assert_eq!(dump_slots(&store).len(), 4);
    }

    #[test]
    fn test_carve_rejects_ranges_outside_or_covering_the_slot() {
        let (_temp, store) = temp_store();
        let start = Local::now() - chrono::Duration::hours(3);
        let end = start + chrono::Duration::minutes(60);
        let id = store.create_slot(&finished("原始", start, 60)).unwrap();
        let before = dump_slots(&store);
        let minute = chrono::Duration::minutes(1);

        for (from, to) in [
            (start - minute, start + minute * 10),
            (start + minute * 50, end + minute),
            (start + minute * 30, start + minute * 30),
            (start + minute * 40, start + minute * 20),
            (start, end),
        ] {
            assert!(store.carve_slot(id, from, to, meeting("x")).is_err());
        }
        assert_eq!(dump_slots(&store), before);
    }

    #[test]
    fn test_carve_rolls_back_when_the_insert_fails() {
        let (_temp, store) = temp_store();
        let start = Local::now() - chrono::Duration::hours(3);
        let id = store.create_slot(&finished("原始", start, 60)).unwrap();
        let before = dump_slots(&store);

        fail_on(&store, "INSERT");
        let from = start + chrono::Duration::minutes(10);
        let to = start + chrono::Duration::minutes(20);
        assert!(store.carve_slot(id, from, to, meeting("新")).is_err());

        assert_eq!(dump_slots(&store), before);
    }

    #[test]
    fn test_merge_with_missing_id_leaves_db_unchanged() {
        let (_temp, store) = temp_store();
        let start = Local::now() - chrono::Duration::hours(2);
        let a = store.create_slot(&finished("甲", start, 20)).unwrap();
        let b = store
            .create_slot(&finished("乙", start + chrono::Duration::minutes(20), 20))
            .unwrap();
        let before = dump_slots(&store);

        assert!(store.merge_slots(&[a, b, 99999], Some("合併")).is_err());

        assert_eq!(dump_slots(&store), before);
    }

    #[test]
    fn test_merge_rolls_back_when_the_delete_fails() {
        let (_temp, store) = temp_store();
        let start = Local::now() - chrono::Duration::hours(2);
        let a = store.create_slot(&finished("甲", start, 20)).unwrap();
        let b = store
            .create_slot(&finished("乙", start + chrono::Duration::minutes(20), 20))
            .unwrap();
        let mut child = finished("插隊", start + chrono::Duration::minutes(25), 5);
        child.slot_type = SlotType::Interrupt;
        child.parent_id = Some(b);
        store.create_slot(&child).unwrap();
        let before = dump_slots(&store);

        // The kept slot is updated and the child reparented before the delete fails.
        fail_on(&store, "DELETE");
        assert!(store.merge_slots(&[a, b], Some("合併")).is_err());

        assert_eq!(dump_slots(&store), before);
    }

    #[test]
    fn test_create_slots_writes_nothing_when_the_third_is_invalid() {
        let (_temp, store) = temp_store();
        let start = Local::now() - chrono::Duration::hours(2);
        let first = finished("A", start, 10);
        let second = finished("B", start + chrono::Duration::minutes(10), 10);
        // Same uuid as the first one breaks the UNIQUE constraint.
        let mut third = finished("C", start + chrono::Duration::minutes(20), 10);
        third.uuid = first.uuid;

        assert!(store.create_slots(&[first, second, third]).is_err());

        assert!(dump_slots(&store).is_empty());
    }

    #[test]
    fn test_create_slots_rolls_back_when_recent_task_write_fails() {
        let (_temp, store) = temp_store();
        store
            .lock_conn()
            .execute_batch(
                "CREATE TRIGGER test_fail BEFORE INSERT ON recent_tasks
                 BEGIN SELECT RAISE(ABORT, 'boom'); END;",
            )
            .unwrap();
        let start = Local::now() - chrono::Duration::hours(2);

        assert!(store.create_slots(&[finished("A", start, 10)]).is_err());

        assert!(dump_slots(&store).is_empty());
    }

    #[test]
    fn test_create_slots_remembers_task_names() {
        let (_temp, store) = temp_store();
        let start = Local::now() - chrono::Duration::hours(2);
        store
            .create_slots(&[
                finished("補登", start, 10),
                finished("補登", start + chrono::Duration::minutes(10), 10),
            ])
            .unwrap();

        let recent = store.get_recent_tasks(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].task_name, "補登");
        assert_eq!(recent[0].use_count, 2);
    }

    #[test]
    fn test_create_slots_returns_ids_in_order() {
        let (_temp, store) = temp_store();
        let start = Local::now() - chrono::Duration::hours(2);
        let ids = store
            .create_slots(&[
                finished("A", start, 10),
                finished("B", start + chrono::Duration::minutes(10), 10),
            ])
            .unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(store.get_slot(ids[0]).unwrap().unwrap().task_name, "A");
        assert_eq!(store.get_slot(ids[1]).unwrap().unwrap().task_name, "B");
    }

    #[test]
    fn test_save_settings_rolls_back_when_one_write_fails() {
        let (_temp, store) = temp_store();
        let before = store.get_settings().unwrap();

        {
            let conn = store.lock_conn();
            conn.execute_batch(
                "CREATE TRIGGER test_fail BEFORE INSERT ON app_state WHEN NEW.key = 'remember_last_category'
                 BEGIN SELECT RAISE(ABORT, 'boom'); END;",
            )
            .unwrap();
        }

        let mut settings = before.clone();
        settings.lunch_start = "01:23".to_string();
        settings.theme = "something-else".to_string();
        assert!(store.save_settings(&settings).is_err());

        let after = store.get_settings().unwrap();
        assert_eq!(after.lunch_start, before.lunch_start);
        assert_eq!(after.theme, before.theme);
    }

    #[test]
    fn test_save_settings_round_trip() {
        let (_temp, store) = temp_store();
        let mut settings = store.get_settings().unwrap();
        settings.work_end = "18:30".to_string();
        settings.theme = "dark".to_string();
        settings.auto_update = false;
        store.save_settings(&settings).unwrap();

        let loaded = store.get_settings().unwrap();
        assert_eq!(loaded.work_end, "18:30");
        assert_eq!(loaded.theme, "dark");
        assert!(!loaded.auto_update);
    }

    #[test]
    fn test_end_interrupt_atomic_updates_both_slots() {
        let (_temp, store) = temp_store();
        let start = Local::now();
        let work_id = store
            .create_slot(&TimeSlot::new("工作".to_string(), SlotType::Work, start))
            .unwrap();
        let mut interrupt = TimeSlot::new(
            "插隊".to_string(),
            SlotType::Interrupt,
            start + chrono::Duration::minutes(5),
        );
        interrupt.parent_id = Some(work_id);
        let interrupt_id = store.create_slot(&interrupt).unwrap();

        let end = start + chrono::Duration::minutes(10);
        store
            .end_interrupt_atomic(interrupt_id, work_id, end, Some("完成".to_string()))
            .unwrap();

        let work_slot = store.get_slot(work_id).unwrap().unwrap();
        let interrupt_slot = store.get_slot(interrupt_id).unwrap().unwrap();
        assert_eq!(work_slot.note.as_deref(), Some("完成"));
        assert!(near(work_slot.end_ts.unwrap(), end));
        assert!(near(interrupt_slot.end_ts.unwrap(), end));
    }

    #[test]
    fn test_end_interrupt_atomic_with_missing_parent_leaves_db_unchanged() {
        let (_temp, store) = temp_store();
        let interrupt = TimeSlot::new("插隊".to_string(), SlotType::Interrupt, Local::now());
        let interrupt_id = store.create_slot(&interrupt).unwrap();
        let before = dump_slots(&store);

        assert!(store
            .end_interrupt_atomic(interrupt_id, 99999, Local::now(), None)
            .is_err());

        assert_eq!(dump_slots(&store), before);
    }

    // P2 error handling tests

    /// Insert a raw row today, bypassing the CHECK on slot_type so bad data can get in.
    fn insert_raw_slot(store: &StoreV2, uuid: &str, slot_type: &str, created_at: &str) {
        let conn = store.lock_conn();
        let start = Local::now().to_rfc3339();
        conn.execute_batch("PRAGMA ignore_check_constraints = ON;")
            .unwrap();
        conn.execute(
            "INSERT INTO time_slots (uuid, task_name, slot_type, start_ts, end_ts, created_at, updated_at)
             VALUES (?1, 'Bad', ?2, ?3, ?3, ?4, ?3)",
            params![uuid, slot_type, start, created_at],
        )
        .unwrap();
        conn.execute_batch("PRAGMA ignore_check_constraints = OFF;")
            .unwrap();
    }

    fn assert_store_still_works(store: &StoreV2) {
        let id = store
            .create_slot(&TimeSlot::new(
                "Good".to_string(),
                SlotType::Work,
                Local::now() - chrono::Duration::days(3),
            ))
            .unwrap();
        assert_eq!(store.get_slot(id).unwrap().unwrap().task_name, "Good");
        store.get_settings().unwrap();
    }

    #[test]
    fn test_garbage_timestamp_returns_error_not_panic() {
        let (_temp, store) = temp_store();
        // start_ts is valid so the row is inside today's range; created_at is garbage.
        insert_raw_slot(&store, &Uuid::new_v4().to_string(), "work", "garbage");

        assert!(store.get_slots_for_date(Local::now().date_naive()).is_err());
        assert!(store.get_day_stats(Local::now().date_naive()).is_err());
        assert_store_still_works(&store);
    }

    #[test]
    fn test_invalid_uuid_returns_error() {
        let (_temp, store) = temp_store();
        insert_raw_slot(&store, "not-a-uuid", "work", &Local::now().to_rfc3339());

        assert!(store.get_slots_for_date(Local::now().date_naive()).is_err());
        assert_store_still_works(&store);
    }

    #[test]
    fn test_invalid_slot_type_returns_error() {
        let (_temp, store) = temp_store();
        insert_raw_slot(
            &store,
            &Uuid::new_v4().to_string(),
            "nap",
            &Local::now().to_rfc3339(),
        );
        let today = Local::now().date_naive();

        assert!(store.get_slots_for_date(today).is_err());
        assert!(store.get_task_summaries(today, today).is_err());
        assert_store_still_works(&store);
    }

    #[test]
    fn test_garbage_recent_task_returns_error() {
        let (_temp, store) = temp_store();
        store
            .lock_conn()
            .execute(
                "INSERT INTO recent_tasks (task_name, last_used, use_count, slot_type)
                 VALUES ('Test', 'garbage-ts', 1, 'work')",
                [],
            )
            .unwrap();

        assert!(store.get_recent_tasks(10).is_err());
        assert_store_still_works(&store);
    }

    #[test]
    fn test_mutex_poison_recovery() {
        let (_temp, store) = temp_store();
        let store = std::sync::Arc::new(store);
        let store_clone = store.clone();

        let handle = std::thread::spawn(move || {
            let _lock = store_clone.lock_conn();
            panic!("Intentional panic to poison mutex");
        });
        assert!(handle.join().is_err());
        assert!(store.conn.is_poisoned());

        assert_store_still_works(&store);
    }

    // P4 date range boundary tests

    fn local_at(date: NaiveDate, h: u32, m: u32, s: u32, ms: u32) -> DateTime<Local> {
        date.and_hms_milli_opt(h, m, s, ms)
            .unwrap()
            .and_local_timezone(Local)
            .earliest()
            .unwrap()
    }

    #[test]
    fn test_last_half_second_of_the_day_is_in_that_day() {
        let (_temp, store) = temp_store();
        let day = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let next = day.succ_opt().unwrap();
        let slot = finished("最後一秒", local_at(day, 23, 59, 59, 500), 10);
        assert!(slot.start_ts.to_rfc3339().contains("23:59:59.5"));
        store.create_slot(&slot).unwrap();

        assert_eq!(store.get_slots_for_date(day).unwrap().len(), 1);
        assert!(store.get_slots_for_date(next).unwrap().is_empty());
        assert_eq!(store.get_task_summaries(day, day).unwrap().len(), 1);
        assert!(store.get_task_summaries(next, next).unwrap().is_empty());
    }

    #[test]
    fn test_next_midnight_belongs_to_the_next_day_only() {
        let (_temp, store) = temp_store();
        let day = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let next = day.succ_opt().unwrap();
        store
            .create_slot(&finished("午夜", local_at(next, 0, 0, 0, 0), 10))
            .unwrap();

        assert!(store.get_slots_for_date(day).unwrap().is_empty());
        assert_eq!(store.get_slots_for_date(next).unwrap().len(), 1);
        assert_eq!(store.get_slots_for_range(day, next).unwrap().len(), 1);
    }
}
