use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tauri::State;

// When commands_v2 is a submodule of main.rs (the binary crate),
// we can't import worktime_pomodoro_lib directly here.
// Instead, main.rs will re-export what we need.

// These will be imported via `use crate::*` from main.rs
use crate::{
    AppSettings, AppStateMgr, AppStore, DayStats, Gap, RecentPerson, RecentTask, SlotType,
    StateData, TaskSummary, TimeSlot,
};

// ==================== State & Info Commands ====================

#[tauri::command]
pub fn get_app_state(state_mgr: State<AppStateMgr>) -> Result<StateData, String> {
    Ok(state_mgr.get_state())
}

#[tauri::command]
pub fn get_current_slot(store: State<AppStore>) -> Result<Option<TimeSlot>, String> {
    store.get_current_slot().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_elapsed_seconds(state_mgr: State<AppStateMgr>) -> Option<i64> {
    state_mgr.get_elapsed_seconds()
}

#[tauri::command]
pub fn get_remaining_seconds(state_mgr: State<AppStateMgr>) -> Option<i64> {
    state_mgr.get_remaining_seconds()
}

// ==================== Task Control Commands ====================

#[derive(Debug, Deserialize)]
pub struct StartTaskRequest {
    pub task_name: String,
    pub slot_type: String,
}

#[tauri::command]
pub fn start_task(
    request: StartTaskRequest,
    state_mgr: State<AppStateMgr>,
) -> Result<i64, String> {
    let slot_type = SlotType::from_str(&request.slot_type).map_err(|e| e.to_string())?;
    state_mgr
        .start_task(request.task_name, slot_type)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn end_task(note: Option<String>, state_mgr: State<AppStateMgr>) -> Result<(), String> {
    state_mgr.end_task(note).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pause_task(state_mgr: State<AppStateMgr>) -> Result<i64, String> {
    state_mgr.pause_task().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resume_task(state_mgr: State<AppStateMgr>) -> Result<i64, String> {
    state_mgr.resume_task().map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct StartInterruptRequest {
    pub who: Option<String>,
    pub reason: Option<String>,
}

#[tauri::command]
pub fn start_interrupt(
    request: StartInterruptRequest,
    state_mgr: State<AppStateMgr>,
) -> Result<i64, String> {
    state_mgr
        .start_interrupt(request.who, request.reason)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn end_interrupt_and_resume(state_mgr: State<AppStateMgr>) -> Result<Option<i64>, String> {
    state_mgr
        .end_interrupt_and_resume()
        .map_err(|e| e.to_string())
}

// ==================== Query Commands ====================

#[tauri::command]
pub fn get_slots_for_date(date: String, store: State<AppStore>) -> Result<Vec<TimeSlot>, String> {
    let parsed_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    store
        .get_slots_for_date(parsed_date)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_day_stats(date: String, store: State<AppStore>) -> Result<DayStats, String> {
    let parsed_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    store.get_day_stats(parsed_date).map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct DateRangeRequest {
    pub start_date: String,
    pub end_date: String,
}

#[tauri::command]
pub fn get_slots_for_range(
    request: DateRangeRequest,
    store: State<AppStore>,
) -> Result<Vec<TimeSlot>, String> {
    let start = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d")
        .map_err(|e| e.to_string())?;
    let end =
        NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    store
        .get_slots_for_range(start, end)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_task_summaries(
    request: DateRangeRequest,
    store: State<AppStore>,
) -> Result<Vec<TaskSummary>, String> {
    let start = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d")
        .map_err(|e| e.to_string())?;
    let end =
        NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    store
        .get_task_summaries(start, end)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn find_gaps(date: String, store: State<AppStore>) -> Result<Vec<Gap>, String> {
    let parsed_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    store.find_gaps(parsed_date).map_err(|e| e.to_string())
}

// ==================== Recent Items Commands ====================

#[tauri::command]
pub fn get_recent_tasks(limit: Option<usize>, store: State<AppStore>) -> Result<Vec<RecentTask>, String> {
    store
        .get_recent_tasks(limit.unwrap_or(10))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_recent_people(limit: Option<usize>, store: State<AppStore>) -> Result<Vec<RecentPerson>, String> {
    store
        .get_recent_people(limit.unwrap_or(10))
        .map_err(|e| e.to_string())
}

// ==================== Settings Commands ====================

#[tauri::command]
pub fn get_settings(store: State<AppStore>) -> Result<AppSettings, String> {
    store.get_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_settings(settings: AppSettings, store: State<AppStore>) -> Result<(), String> {
    store.save_settings(&settings).map_err(|e| e.to_string())
}

// ==================== Slot Edit Commands ====================

#[derive(Debug, Deserialize)]
pub struct CreateSlotRequest {
    pub task_name: String,
    pub slot_type: String,
    pub start_ts: String,
    pub end_ts: Option<String>,
    pub note: Option<String>,
}

#[tauri::command]
pub fn create_slot_manual(
    request: CreateSlotRequest,
    store: State<AppStore>,
) -> Result<i64, String> {
    use chrono::DateTime;

    let slot_type = SlotType::from_str(&request.slot_type).map_err(|e| e.to_string())?;
    let start_ts = DateTime::parse_from_rfc3339(&request.start_ts)
        .map_err(|e| e.to_string())?
        .with_timezone(&chrono::Local);
    let end_ts = if let Some(end_str) = request.end_ts {
        Some(
            DateTime::parse_from_rfc3339(&end_str)
                .map_err(|e| e.to_string())?
                .with_timezone(&chrono::Local),
        )
    } else {
        None
    };

    let mut slot = TimeSlot::new(request.task_name, slot_type, start_ts);
    slot.end_ts = end_ts;
    slot.note = request.note;

    store.create_slot(&slot).map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct UpdateSlotRequest {
    pub id: i64,
    pub end_ts: Option<String>,
    pub note: Option<String>,
}

#[tauri::command]
pub fn update_slot_manual(
    request: UpdateSlotRequest,
    store: State<AppStore>,
) -> Result<(), String> {
    use chrono::DateTime;

    let end_ts = if let Some(end_str) = request.end_ts {
        Some(
            DateTime::parse_from_rfc3339(&end_str)
                .map_err(|e| e.to_string())?
                .with_timezone(&chrono::Local),
        )
    } else {
        None
    };

    store
        .update_slot(request.id, end_ts, request.note)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_slot(slot_id: i64, store: State<AppStore>) -> Result<(), String> {
    store.delete_slot(slot_id).map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct FillGapRequest {
    pub start_time: String,
    pub end_time: String,
    pub task_title: String,
    pub person: Option<String>,
    pub note: Option<String>,
}

#[tauri::command]
pub fn fill_gap(request: FillGapRequest, store: State<AppStore>) -> Result<i64, String> {
    use chrono::DateTime;

    let start_ts = DateTime::parse_from_rfc3339(&request.start_time)
        .map_err(|e| e.to_string())?
        .with_timezone(&chrono::Local);
    let end_ts = DateTime::parse_from_rfc3339(&request.end_time)
        .map_err(|e| e.to_string())?
        .with_timezone(&chrono::Local);

    let mut slot = TimeSlot::new(request.task_title, SlotType::Work, start_ts);
    slot.end_ts = Some(end_ts);
    slot.note = request.note;

    store.create_slot(&slot).map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct IdleReturnRequest {
    pub action: String,
    pub idle_minutes: i64,
}

#[tauri::command]
pub fn handle_idle_return(
    request: IdleReturnRequest,
    state_mgr: State<AppStateMgr>,
) -> Result<(), String> {
    match request.action.as_str() {
        "continue" => {
            // Resume task, treat idle time as part of current task
            state_mgr.resume_task().map_err(|e| e.to_string())?;
        }
        "lunch" => {
            // End current task and create a lunch slot for the idle period
            // This would require more complex logic, simplified here
            state_mgr.end_task().map_err(|e| e.to_string())?;
        }
        "meeting" => {
            // End current task and create a meeting slot for the idle period
            state_mgr.end_task().map_err(|e| e.to_string())?;
        }
        "discard" => {
            // End current task, discard the idle time
            state_mgr.end_task().map_err(|e| e.to_string())?;
        }
        _ => return Err("Invalid action".to_string()),
    }
    Ok(())
}

// ==================== Export Commands ====================

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportRowV2 {
    pub date: String,
    pub task_name: String,
    pub slot_type: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_minutes: i64,
    pub note: String,
}

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub start_date: String,
    pub end_date: String,
    pub include_interrupts: bool,
}

#[tauri::command]
pub fn prepare_export_data(
    request: ExportRequest,
    store: State<AppStore>,
) -> Result<Vec<ExportRowV2>, String> {
    let start = NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d")
        .map_err(|e| e.to_string())?;
    let end =
        NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d").map_err(|e| e.to_string())?;

    let slots = store
        .get_slots_for_range(start, end)
        .map_err(|e| e.to_string())?;

    let rows: Vec<ExportRowV2> = slots
        .into_iter()
        .filter(|slot| {
            request.include_interrupts || slot.slot_type != SlotType::Interrupt
        })
        .filter_map(|slot| {
            let end_ts = slot.end_ts?;
            let duration = slot.duration_minutes()?;

            Some(ExportRowV2 {
                date: slot.start_ts.format("%Y-%m-%d").to_string(),
                task_name: slot.task_name.clone(),
                slot_type: slot.slot_type.as_str().to_string(),
                start_time: slot.start_ts.format("%H:%M:%S").to_string(),
                end_time: end_ts.format("%H:%M:%S").to_string(),
                duration_minutes: duration,
                note: slot.note.unwrap_or_default(),
            })
        })
        .collect();

    Ok(rows)
}

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use rust_xlsxwriter::*;

#[tauri::command]
pub fn export_to_xlsx(
    request: ExportRequest,
    output_path: String,
    store: State<AppStore>,
) -> Result<String, String> {
    let rows = prepare_export_data(request, store)?;
    let path = PathBuf::from(output_path);

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let header_format = Format::new().set_bold();
    
    worksheet.write_with_format(0, 0, "日期", &header_format).map_err(|e| e.to_string())?;
    worksheet.write_with_format(0, 1, "任務", &header_format).map_err(|e| e.to_string())?;
    worksheet.write_with_format(0, 2, "類型", &header_format).map_err(|e| e.to_string())?;
    worksheet.write_with_format(0, 3, "開始時間", &header_format).map_err(|e| e.to_string())?;
    worksheet.write_with_format(0, 4, "結束時間", &header_format).map_err(|e| e.to_string())?;
    worksheet.write_with_format(0, 5, "分鐘", &header_format).map_err(|e| e.to_string())?;
    worksheet.write_with_format(0, 6, "備註", &header_format).map_err(|e| e.to_string())?;

    for (i, row) in rows.iter().enumerate() {
        let row_num = (i + 1) as u32;
        worksheet.write(row_num, 0, &row.date).map_err(|e| e.to_string())?;
        worksheet.write(row_num, 1, &row.task_name).map_err(|e| e.to_string())?;
        worksheet.write(row_num, 2, &row.slot_type).map_err(|e| e.to_string())?;
        worksheet.write(row_num, 3, &row.start_time).map_err(|e| e.to_string())?;
        worksheet.write(row_num, 4, &row.end_time).map_err(|e| e.to_string())?;
        worksheet.write(row_num, 5, row.duration_minutes).map_err(|e| e.to_string())?;
        worksheet.write(row_num, 6, &row.note).map_err(|e| e.to_string())?;
    }

    worksheet.set_column_width(0, 12).map_err(|e| e.to_string())?;
    worksheet.set_column_width(1, 30).map_err(|e| e.to_string())?;
    worksheet.set_column_width(2, 10).map_err(|e| e.to_string())?;
    worksheet.set_column_width(3, 10).map_err(|e| e.to_string())?;
    worksheet.set_column_width(4, 10).map_err(|e| e.to_string())?;
    worksheet.set_column_width(5, 8).map_err(|e| e.to_string())?;
    worksheet.set_column_width(6, 40).map_err(|e| e.to_string())?;

    workbook.save(&path).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn export_to_csv(
    request: ExportRequest,
    output_path: String,
    store: State<AppStore>,
) -> Result<String, String> {
    let rows = prepare_export_data(request, store)?;
    let path = PathBuf::from(output_path);

    let mut wtr = csv::Writer::from_path(&path).map_err(|e| e.to_string())?;
    
    wtr.write_record(&["日期", "任務", "類型", "開始時間", "結束時間", "分鐘", "備註"])
        .map_err(|e| e.to_string())?;

    for row in rows {
        wtr.write_record(&[
            row.date,
            row.task_name,
            row.slot_type,
            row.start_time,
            row.end_time,
            row.duration_minutes.to_string(),
            row.note,
        ])
        .map_err(|e| e.to_string())?;
    }

    wtr.flush().map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}
