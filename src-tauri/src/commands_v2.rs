use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tauri::State;

// When commands_v2 is a submodule of main.rs (the binary crate),
// we can't import worktime_pomodoro_lib directly here.
// Instead, main.rs will re-export what we need.

// These will be imported via `use crate::*` from main.rs
use crate::{
    build_export_rows, export_columns, AppSettings, AppStateMgr, AppStore, CarvedPart, Category,
    CategoryReport, ExportCell, ExportRowV2, ExportTypeLabels, Gap, RecentPerson, RecentTask,
    SlotDetails, SlotType, StateData, TimeSlot,
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

// ==================== Task Control Commands ====================

#[derive(Debug, Deserialize)]
pub struct StartTaskRequest {
    pub task_name: String,
    pub slot_type: String,
    #[serde(default)]
    pub category_id: Option<i64>,
}

#[tauri::command]
pub fn start_task(request: StartTaskRequest, state_mgr: State<AppStateMgr>) -> Result<i64, String> {
    let slot_type = request
        .slot_type
        .parse::<SlotType>()
        .map_err(|e| e.to_string())?;
    state_mgr
        .start_task(request.task_name, slot_type, request.category_id)
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
    #[serde(default)]
    pub category_id: Option<i64>,
}

#[tauri::command]
pub fn start_interrupt(
    request: StartInterruptRequest,
    state_mgr: State<AppStateMgr>,
) -> Result<i64, String> {
    state_mgr
        .start_interrupt(request.who, request.reason, request.category_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn end_interrupt_and_resume(state_mgr: State<AppStateMgr>) -> Result<Option<i64>, String> {
    state_mgr
        .end_interrupt_and_resume()
        .map_err(|e| e.to_string())
}

/// The task an interrupt will return to.
#[tauri::command]
pub fn get_resume_slot(
    state_mgr: State<AppStateMgr>,
    store: State<AppStore>,
) -> Result<Option<TimeSlot>, String> {
    match state_mgr.get_state().paused_slot_id {
        Some(id) => store.get_slot(id).map_err(|e| e.to_string()),
        None => Ok(None),
    }
}

// ==================== Category Commands ====================

#[tauri::command]
pub fn get_categories(store: State<AppStore>) -> Result<Vec<Category>, String> {
    store.get_categories().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_default_category(
    slot_type: String,
    store: State<AppStore>,
) -> Result<Option<i64>, String> {
    let slot_type = slot_type.parse::<SlotType>().map_err(|e| e.to_string())?;
    store
        .get_default_category(&slot_type)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct CreateCategoryRequest {
    pub name: String,
    pub report_code: Option<String>,
}

#[tauri::command]
pub fn create_category(
    request: CreateCategoryRequest,
    store: State<AppStore>,
) -> Result<i64, String> {
    store
        .create_category(&request.name, request.report_code.as_deref())
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryRequest {
    pub id: i64,
    pub name: String,
    pub report_code: Option<String>,
    pub hidden: bool,
}

#[tauri::command]
pub fn update_category(
    request: UpdateCategoryRequest,
    store: State<AppStore>,
) -> Result<(), String> {
    store
        .update_category(
            request.id,
            &request.name,
            request.report_code.as_deref(),
            request.hidden,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reorder_categories(ids: Vec<i64>, store: State<AppStore>) -> Result<(), String> {
    store.reorder_categories(&ids).map_err(|e| e.to_string())
}

// ==================== Query Commands ====================

#[tauri::command]
pub fn get_slots_for_date(date: String, store: State<AppStore>) -> Result<Vec<TimeSlot>, String> {
    let parsed_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    store
        .get_slots_for_date(parsed_date)
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct DateRangeRequest {
    #[serde(alias = "startDate")]
    pub start_date: String,
    #[serde(alias = "endDate")]
    pub end_date: String,
}

#[tauri::command]
pub fn get_slots_for_range(
    request: DateRangeRequest,
    store: State<AppStore>,
) -> Result<Vec<TimeSlot>, String> {
    let start =
        NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    let end =
        NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    store
        .get_slots_for_range(start, end)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_category_report(
    request: DateRangeRequest,
    store: State<AppStore>,
) -> Result<CategoryReport, String> {
    let start =
        NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    let end =
        NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    store
        .get_category_report(start, end)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn find_gaps(date: String, store: State<AppStore>) -> Result<Vec<Gap>, String> {
    let parsed_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    store.find_gaps(parsed_date).map_err(|e| e.to_string())
}

// ==================== Recent Items Commands ====================

#[tauri::command]
pub fn get_recent_tasks(
    limit: Option<usize>,
    store: State<AppStore>,
) -> Result<Vec<RecentTask>, String> {
    store
        .get_recent_tasks(limit.unwrap_or(10))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_recent_people(
    limit: Option<usize>,
    store: State<AppStore>,
) -> Result<Vec<RecentPerson>, String> {
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
    #[serde(default)]
    pub category_id: Option<i64>,
}

#[tauri::command]
pub fn create_slot_manual(
    request: CreateSlotRequest,
    store: State<AppStore>,
) -> Result<i64, String> {
    use chrono::DateTime;

    let slot_type = request
        .slot_type
        .parse::<SlotType>()
        .map_err(|e| e.to_string())?;
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

    if end_ts.is_some_and(|end| end <= start_ts) {
        return Err("結束時間要比開始時間晚".to_string());
    }
    let task_name = request.task_name.trim();
    if task_name.is_empty() {
        return Err("請輸入任務名稱".to_string());
    }

    let mut slot = TimeSlot::new(task_name.to_string(), slot_type, start_ts);
    slot.end_ts = end_ts;
    slot.note = request.note;
    slot.category_id = request.category_id;

    let ids = store.create_slots(&[slot]).map_err(|e| e.to_string())?;
    Ok(ids[0])
}

#[derive(Debug, Deserialize)]
pub struct UpdateSlotRequest {
    pub id: i64,
    pub task_name: String,
    pub slot_type: String,
    pub start_ts: String,
    pub end_ts: Option<String>,
    pub note: Option<String>,
    #[serde(default)]
    pub category_id: Option<i64>,
}

#[tauri::command]
pub fn update_slot_manual(
    request: UpdateSlotRequest,
    store: State<AppStore>,
) -> Result<(), String> {
    use chrono::DateTime;

    let task_name = request.task_name.trim();
    if task_name.is_empty() {
        return Err("請輸入任務名稱".to_string());
    }

    let slot_type = request
        .slot_type
        .parse::<SlotType>()
        .map_err(|e| e.to_string())?;
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
    let note = request
        .note
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());

    store
        .update_slot_details(
            request.id,
            SlotDetails {
                task_name,
                slot_type: slot_type.clone(),
                start_ts,
                end_ts,
                note,
                category_id: request.category_id,
            },
        )
        .map_err(|e| e.to_string())?;
    store
        .add_or_update_recent_task(task_name, slot_type)
        .map_err(|e| e.to_string())?;
    Ok(())
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

#[derive(Debug, Deserialize)]
pub struct FillGapEntry {
    pub task_name: String,
    pub start_time: String,
    pub end_time: String,
    #[serde(default)]
    pub category_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct FillGapsRequest {
    pub entries: Vec<FillGapEntry>,
}

#[tauri::command]
pub fn fill_gaps(request: FillGapsRequest, store: State<AppStore>) -> Result<Vec<i64>, String> {
    use chrono::DateTime;

    if request.entries.is_empty() {
        return Err("請至少填一筆工作".to_string());
    }

    let mut slots = Vec::new();
    for entry in &request.entries {
        let task_name = entry.task_name.trim();
        if task_name.is_empty() {
            return Err("請輸入任務名稱".to_string());
        }
        let start_ts = DateTime::parse_from_rfc3339(&entry.start_time)
            .map_err(|e| e.to_string())?
            .with_timezone(&chrono::Local);
        let end_ts = DateTime::parse_from_rfc3339(&entry.end_time)
            .map_err(|e| e.to_string())?
            .with_timezone(&chrono::Local);
        if end_ts <= start_ts {
            return Err("結束時間要比開始時間晚".to_string());
        }
        let mut slot = TimeSlot::new(task_name.to_string(), SlotType::Work, start_ts);
        slot.end_ts = Some(end_ts);
        slot.category_id = entry.category_id;
        slots.push(slot);
    }

    store.create_slots(&slots).map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct CarveSlotRequest {
    pub id: i64,
    pub from: String,
    pub to: String,
    pub task_name: String,
    pub slot_type: String,
    #[serde(default)]
    pub category_id: Option<i64>,
}

#[tauri::command]
pub fn carve_slot(request: CarveSlotRequest, store: State<AppStore>) -> Result<i64, String> {
    use chrono::DateTime;

    let parse = |value: &str| {
        DateTime::parse_from_rfc3339(value)
            .map(|ts| ts.with_timezone(&chrono::Local))
            .map_err(|e| e.to_string())
    };
    let slot_type = request
        .slot_type
        .parse::<SlotType>()
        .map_err(|e| e.to_string())?;
    store
        .carve_slot(
            request.id,
            parse(&request.from)?,
            parse(&request.to)?,
            CarvedPart {
                task_name: &request.task_name,
                slot_type,
                category_id: request.category_id,
            },
        )
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
pub struct MergeSlotsRequest {
    pub ids: Vec<i64>,
    pub task_name: Option<String>,
}

#[tauri::command]
pub fn merge_slots(request: MergeSlotsRequest, store: State<AppStore>) -> Result<i64, String> {
    store
        .merge_slots(&request.ids, request.task_name.as_deref())
        .map_err(|e| e.to_string())
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

    let id = store.create_slot(&slot).map_err(|e| e.to_string())?;

    if let Some(person) = request
        .person
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        store
            .add_or_update_recent_person(person)
            .map_err(|e| e.to_string())?;
    }

    Ok(id)
}

// ==================== Export Commands ====================

#[derive(Debug, Serialize)]
pub struct ExportResult {
    pub path: String,
    pub count: i64,
}

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub start_date: String,
    pub end_date: String,
    pub include_interrupts: bool,
    #[serde(default)]
    pub type_labels: ExportTypeLabels,
}

fn prepare_export_data(
    request: ExportRequest,
    store: State<AppStore>,
) -> Result<Vec<ExportRowV2>, String> {
    let start =
        NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    let end =
        NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d").map_err(|e| e.to_string())?;

    let slots = store
        .get_slots_for_range(start, end)
        .map_err(|e| e.to_string())?;

    Ok(build_export_rows(
        slots,
        request.include_interrupts,
        &request.type_labels,
    ))
}

use rust_xlsxwriter::*;
use std::path::PathBuf;

#[tauri::command]
pub fn export_to_xlsx(
    request: ExportRequest,
    output_path: String,
    store: State<AppStore>,
) -> Result<ExportResult, String> {
    let rows = prepare_export_data(request, store)?;
    let count = rows.len() as i64;
    let path = PathBuf::from(output_path);

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();

    let header_format = Format::new().set_bold();

    let columns = export_columns(&rows);
    for (col, column) in columns.iter().enumerate() {
        let col = col as u16;
        worksheet
            .write_with_format(0, col, column.header, &header_format)
            .map_err(|e| e.to_string())?;
        worksheet
            .set_column_width(col, column.width)
            .map_err(|e| e.to_string())?;

        for (i, row) in rows.iter().enumerate() {
            let row_num = (i + 1) as u32;
            match (column.value)(row) {
                ExportCell::Text(text) => worksheet.write(row_num, col, text),
                ExportCell::Number(number) => worksheet.write(row_num, col, number),
            }
            .map_err(|e| e.to_string())?;
        }
    }

    workbook.save(&path).map_err(|e| e.to_string())?;

    Ok(ExportResult {
        path: path.to_string_lossy().to_string(),
        count,
    })
}

#[tauri::command]
pub fn export_to_csv(
    request: ExportRequest,
    output_path: String,
    store: State<AppStore>,
) -> Result<ExportResult, String> {
    let rows = prepare_export_data(request, store)?;
    let count = rows.len() as i64;
    let path = PathBuf::from(output_path);

    let mut wtr = csv::Writer::from_path(&path).map_err(|e| e.to_string())?;

    let columns = export_columns(&rows);
    wtr.write_record(columns.iter().map(|column| column.header))
        .map_err(|e| e.to_string())?;

    for row in &rows {
        wtr.write_record(columns.iter().map(|column| (column.value)(row).to_text()))
            .map_err(|e| e.to_string())?;
    }

    wtr.flush().map_err(|e| e.to_string())?;

    Ok(ExportResult {
        path: path.to_string_lossy().to_string(),
        count,
    })
}

// ==================== Report Image Commands ====================

/// Writes a PNG chosen through the save dialog. The app has no fs plugin.
#[tauri::command]
pub fn save_report_image(output_path: String, bytes: Vec<u8>) -> Result<String, String> {
    std::fs::write(&output_path, bytes).map_err(|e| e.to_string())?;
    Ok(output_path)
}
