use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};

// When commands_v2 is a submodule of main.rs (the binary crate),
// we can't import worktime_pomodoro_lib directly here.
// Instead, main.rs will re-export what we need.

// These will be imported via `use crate::*` from main.rs
use crate::{
    build_export_rows, export_columns, AppSettings, AppStateMgr, AppStore, CarvedPart, Category,
    CategoryReport, DayStats, ExportCell, ExportRowV2, ExportTypeLabels, Gap, RecentPerson,
    RecentTask, SlotDetails, SlotType, StateData, TaskSummary, TimeSlot,
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

#[tauri::command]
pub fn switch_category(
    category_id: Option<i64>,
    state_mgr: State<AppStateMgr>,
) -> Result<i64, String> {
    state_mgr
        .switch_category(category_id)
        .map_err(|e| e.to_string())
}

// ==================== Category Commands ====================

#[tauri::command]
pub fn get_categories(store: State<AppStore>) -> Result<Vec<Category>, String> {
    store.get_categories().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_recent_categories(
    limit: Option<usize>,
    store: State<AppStore>,
) -> Result<Vec<Category>, String> {
    store
        .get_recent_categories(limit.unwrap_or(3))
        .map_err(|e| e.to_string())
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

#[tauri::command]
pub fn get_day_stats(date: String, store: State<AppStore>) -> Result<DayStats, String> {
    let parsed_date = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    store.get_day_stats(parsed_date).map_err(|e| e.to_string())
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
pub fn get_task_summaries(
    request: DateRangeRequest,
    store: State<AppStore>,
) -> Result<Vec<TaskSummary>, String> {
    let start =
        NaiveDate::parse_from_str(&request.start_date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    let end =
        NaiveDate::parse_from_str(&request.end_date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    store
        .get_task_summaries(start, end)
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
pub fn save_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
    store: State<AppStore>,
) -> Result<(), String> {
    store.save_settings(&settings).map_err(|e| e.to_string())?;
    sync_companion_windows(&app, &settings.mini_mode)
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
pub struct SplitSlotRequest {
    pub id: i64,
    pub at: String,
    pub second_task_name: Option<String>,
}

#[tauri::command]
pub fn split_slot(request: SplitSlotRequest, store: State<AppStore>) -> Result<i64, String> {
    use chrono::DateTime;

    let at = DateTime::parse_from_rfc3339(&request.at)
        .map_err(|e| e.to_string())?
        .with_timezone(&chrono::Local);
    store
        .split_slot(request.id, at, request.second_task_name.as_deref())
        .map_err(|e| e.to_string())
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

#[derive(Debug, Deserialize)]
pub struct IdleReturnRequest {
    pub action: String,
    pub idle_minutes: i64,
}

#[tauri::command]
pub fn handle_idle_return(
    request: IdleReturnRequest,
    state_mgr: State<AppStateMgr>,
    store: State<AppStore>,
) -> Result<(), String> {
    let now = chrono::Local::now();
    let idle_start = now - chrono::Duration::minutes(request.idle_minutes.max(0));

    match request.action.as_str() {
        "continue" => match state_mgr.resume_task() {
            Ok(_) => {}
            Err(_) if state_mgr.get_state().current_slot_id.is_some() => {}
            Err(e) => return Err(e.to_string()),
        },
        "lunch" | "meeting" => {
            state_mgr
                .end_task_at(idle_start)
                .map_err(|e| e.to_string())?;
            let (name, slot_type) = if request.action == "lunch" {
                ("午休", SlotType::Lunch)
            } else {
                ("會議", SlotType::Meeting)
            };
            let mut slot = TimeSlot::new(name.to_string(), slot_type.clone(), idle_start);
            slot.end_ts = Some(now);
            store.create_slot(&slot).map_err(|e| e.to_string())?;
            store
                .add_or_update_recent_task(name, slot_type)
                .map_err(|e| e.to_string())?;
        }
        "discard" => {
            state_mgr
                .end_task_at(idle_start)
                .map_err(|e| e.to_string())?;
        }
        _ => return Err("Invalid action".to_string()),
    }
    Ok(())
}

// ==================== Window Management Commands ====================

#[tauri::command]
pub fn show_minibar(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("minibar") {
        window.show().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn hide_minibar(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("minibar") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn fit_minibar(window: &tauri::WebviewWindow, height: f64) -> Result<(), String> {
    let next = height.clamp(48.0, 220.0);
    window.set_resizable(true).map_err(|e| e.to_string())?;
    window
        .set_min_size(Some(tauri::LogicalSize::new(240.0, 48.0)))
        .map_err(|e| e.to_string())?;
    window
        .set_max_size(Some(tauri::LogicalSize::new(240.0, 220.0)))
        .map_err(|e| e.to_string())?;
    window
        .set_size(tauri::LogicalSize::new(240.0, next))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_minibar_height(app: tauri::AppHandle, height: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("minibar") {
        fit_minibar(&window, height)?;
    }
    Ok(())
}

#[tauri::command]
pub fn focus_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.unminimize().map_err(|e| e.to_string())?;
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_quick_input(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.unminimize().map_err(|e| e.to_string())?;
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        window
            .emit("tray:quick_input", ())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn show_pet(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("pet") {
        window.show().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn hide_pet(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("pet") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn preview_companion_mode(app: tauri::AppHandle, mode: String) -> Result<(), String> {
    sync_companion_windows(&app, &mode)
}

pub fn sync_companion_windows(app: &tauri::AppHandle, mode: &str) -> Result<(), String> {
    let show_pet = mode == "pet" || mode == "both";
    let show_bar = mode == "mini" || mode == "both";
    set_companion_window(app, "pet", show_pet, true)?;
    set_companion_window(app, "minibar", show_bar, false)?;
    Ok(())
}

fn set_companion_window(
    app: &tauri::AppHandle,
    label: &str,
    visible: bool,
    bottom: bool,
) -> Result<(), String> {
    let Some(window) = app.get_webview_window(label) else {
        return Ok(());
    };
    if visible {
        if !window.is_visible().unwrap_or(false) {
            if let Err(err) = place_companion_window(&window, bottom) {
                tracing::warn!("failed to place {label}: {err}");
            }
        }
        let _ = window.set_background_color(Some(tauri::utils::config::Color(0, 0, 0, 0)));
        if label == "minibar" {
            let _ = fit_minibar(&window, 48.0);
        }
        window.show().map_err(|e| e.to_string())?;
        let _ = window.set_always_on_top(true);
        tracing::info!("showed companion window {label}");
    } else if window.is_visible().unwrap_or(false) {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn place_companion_window(window: &tauri::WebviewWindow, bottom: bool) -> Result<(), String> {
    let Some(monitor) = window.primary_monitor().map_err(|e| e.to_string())? else {
        return Ok(());
    };
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let area = monitor.position();
    let screen = monitor.size();
    let margin = 32;
    let x = area.x + screen.width as i32 - size.width as i32 - margin;
    let y = if bottom {
        area.y + screen.height as i32 - size.height as i32 - margin
    } else {
        area.y + margin
    };
    window
        .set_position(tauri::PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    Ok(())
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

#[tauri::command]
pub fn prepare_export_data(
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
