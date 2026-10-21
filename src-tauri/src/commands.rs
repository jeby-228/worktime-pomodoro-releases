use crate::AppState;
use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::State;
use worktime_pomodoro_lib::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskInfo {
    pub task_key: String,
    pub note: Option<String>,
}

#[tauri::command]
pub async fn start_task(state: State<'_, AppState>, task: TaskInfo) -> Result<(), String> {
    let sm = state.state_machine.lock().await;
    let now = Local::now();

    let events = sm.start_task(task.task_key.clone(), now, 0, EventSource::User, task.note);

    for event in events {
        state
            .store
            .insert_event(&event)
            .map_err(|e| e.to_string())?;
    }

    // Update task cache
    if let Ok(existing) = state.store.get_task(&task.task_key) {
        if existing.is_none() {
            // Create adhoc task
            let parts: Vec<&str> = task.task_key.split('|').collect();
            let task_cache = TaskCache {
                task_key: task.task_key.clone(),
                plan_name: parts.first().map(|s| s.to_string()),
                stage_name: parts.get(1).map(|s| s.to_string()),
                process_name: parts.get(2).map(|s| s.to_string()),
                task_name: parts.get(3).map(|s| s.to_string()),
                task_id: None,
                is_adhoc: true,
                is_closed: false,
                last_used_at: Some(now),
                fetched_at: None,
            };
            state
                .store
                .insert_or_update_task(&task_cache)
                .map_err(|e| e.to_string())?;
        }
    }

    // Rebuild segments
    let deriver = SegmentDeriver::new(state.store.clone());
    deriver.rebuild_segments().map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn end_task(state: State<'_, AppState>) -> Result<(), String> {
    let sm = state.state_machine.lock().await;
    let events = sm.end_task(Local::now(), 0, EventSource::User);

    for event in events {
        state
            .store
            .insert_event(&event)
            .map_err(|e| e.to_string())?;
    }

    let deriver = SegmentDeriver::new(state.store.clone());
    deriver.rebuild_segments().map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn pause_task(state: State<'_, AppState>) -> Result<(), String> {
    let sm = state.state_machine.lock().await;
    let events = sm.pause(Local::now(), 0, EventSource::User);

    for event in events {
        state
            .store
            .insert_event(&event)
            .map_err(|e| e.to_string())?;
    }

    let deriver = SegmentDeriver::new(state.store.clone());
    deriver.rebuild_segments().map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn continue_task(state: State<'_, crate::AppState>) -> Result<(), String> {
    let sm = state.state_machine.lock().await;
    let current_state = sm.get_state();

    if let worktime_pomodoro_lib::AppState::Working { task_key: _ } = current_state {
        // Just ping
        let ping_event = sm.ping(chrono::Local::now(), 0);
        state
            .store
            .insert_event(&ping_event)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_current_state(state: State<'_, crate::AppState>) -> Result<String, String> {
    let sm = state.state_machine.lock().await;
    let current_state = sm.get_state();
    Ok(serde_json::to_string(&current_state).unwrap())
}

#[derive(Debug, Serialize)]
pub struct DailyStatsResult {
    pub date: String,
    pub task_minutes: Vec<(String, i64)>,
    pub total_minutes: i64,
    pub unknown_minutes: i64,
    pub unsynced_count: usize,
}

#[tauri::command]
pub async fn get_daily_stats(
    state: State<'_, AppState>,
    date: Option<String>,
) -> Result<DailyStatsResult, String> {
    let date = if let Some(d) = date {
        chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").map_err(|e| e.to_string())?
    } else {
        Local::now().date_naive()
    };

    let deriver = SegmentDeriver::new(state.store.clone());
    let summary = deriver.get_daily_summary(date).map_err(|e| e.to_string())?;

    // Get unsynced count
    let events = state.store.get_all_events().map_err(|e| e.to_string())?;
    let unsynced_count = events.iter().filter(|e| !e.synced).count();

    let task_minutes: Vec<(String, i64)> = summary.task_minutes.into_iter().collect();

    Ok(DailyStatsResult {
        date: date.to_string(),
        task_minutes,
        total_minutes: summary.total_minutes,
        unknown_minutes: summary.unknown_minutes,
        unsynced_count,
    })
}

#[tauri::command]
pub async fn export_csv(
    state: State<'_, AppState>,
    date: Option<String>,
) -> Result<String, String> {
    let date = if let Some(d) = date {
        chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").map_err(|e| e.to_string())?
    } else {
        Local::now().date_naive()
    };

    let segments = state
        .store
        .get_segments_for_date(date)
        .map_err(|e| e.to_string())?;

    let mut csv = String::from("日期,任務,開始時間,結束時間,分鐘數,狀態,備註\n");

    for segment in segments {
        let task_key = segment.task_key.unwrap_or_else(|| "待確認".to_string());
        let start = segment.start_ts.format("%H:%M").to_string();
        let end = segment
            .end_ts
            .map(|dt| dt.format("%H:%M").to_string())
            .unwrap_or_else(|| "進行中".to_string());
        let minutes = segment.minutes.map(|m| m.to_string()).unwrap_or_default();
        let status = match segment.status {
            SegmentStatus::Confirmed => "已確認",
            SegmentStatus::Unknown => "待確認",
            SegmentStatus::Recovered => "已恢復",
        };

        csv.push_str(&format!(
            "{},{},{},{},{},{},\n",
            date, task_key, start, end, minutes, status
        ));
    }

    Ok(csv)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsData {
    pub username: String,
    pub interval_minutes: i32,
    pub timeout_minutes: i32,
    pub work_start: String,
    pub work_end: String,
    pub shortcut: String,
    pub autostart: bool,
    pub sync_url: String,
    pub update_url: String,
    pub update_check_on_startup: bool,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            username: "使用者".to_string(),
            interval_minutes: 25,
            timeout_minutes: 5,
            work_start: "08:00".to_string(),
            work_end: "17:30".to_string(),
            shortcut: "Ctrl+Alt+T".to_string(),
            autostart: true,
            sync_url: "https://mes.company.com/api/worktime".to_string(),
            update_url: "https://github.com/yourname/worktime-pomodoro/releases/latest/download"
                .to_string(),
            update_check_on_startup: true,
        }
    }
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsData, String> {
    let username = state
        .store
        .get_setting("username")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "使用者".to_string());
    let interval = state
        .store
        .get_setting("interval_minutes")
        .map_err(|e| e.to_string())?
        .and_then(|s| s.parse().ok())
        .unwrap_or(25);
    let timeout = state
        .store
        .get_setting("timeout_minutes")
        .map_err(|e| e.to_string())?
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    Ok(SettingsData {
        username,
        interval_minutes: interval,
        timeout_minutes: timeout,
        ..Default::default()
    })
}

#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: SettingsData,
) -> Result<(), String> {
    state
        .store
        .set_setting("username", &settings.username)
        .map_err(|e| e.to_string())?;
    state
        .store
        .set_setting("interval_minutes", &settings.interval_minutes.to_string())
        .map_err(|e| e.to_string())?;
    state
        .store
        .set_setting("timeout_minutes", &settings.timeout_minutes.to_string())
        .map_err(|e| e.to_string())?;
    state
        .store
        .set_setting("work_start", &settings.work_start)
        .map_err(|e| e.to_string())?;
    state
        .store
        .set_setting("work_end", &settings.work_end)
        .map_err(|e| e.to_string())?;
    state
        .store
        .set_setting("sync_url", &settings.sync_url)
        .map_err(|e| e.to_string())?;
    state
        .store
        .set_setting("update_url", &settings.update_url)
        .map_err(|e| e.to_string())?;
    state
        .store
        .set_setting(
            "update_check_on_startup",
            &settings.update_check_on_startup.to_string(),
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct UnknownPeriod {
    pub id: i64,
    pub start_ts: String,
    pub end_ts: Option<String>,
    pub minutes: Option<i64>,
}

#[tauri::command]
pub async fn get_unknown_periods(state: State<'_, AppState>) -> Result<Vec<UnknownPeriod>, String> {
    let date = Local::now().date_naive();
    let segments = state
        .store
        .get_segments_for_date(date)
        .map_err(|e| e.to_string())?;

    let unknown: Vec<UnknownPeriod> = segments
        .into_iter()
        .filter(|s| s.status == SegmentStatus::Unknown)
        .map(|s| UnknownPeriod {
            id: s.id.unwrap_or(0),
            start_ts: s.start_ts.format("%H:%M").to_string(),
            end_ts: s.end_ts.map(|dt| dt.format("%H:%M").to_string()),
            minutes: s.minutes,
        })
        .collect();

    Ok(unknown)
}

#[tauri::command]
pub async fn resolve_unknown_period(
    _state: State<'_, AppState>,
    _period_id: i64,
    _task_key: String,
) -> Result<(), String> {
    // In a real implementation, this would update the segment or create resolve events
    // For now, this is a stub
    Ok(())
}
