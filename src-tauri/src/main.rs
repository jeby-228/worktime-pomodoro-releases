// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Manager, WindowEvent};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod commands_v2;
use commands_v2::*;

use worktime_pomodoro_lib::{StateManager, StoreV2};

fn setup_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    info!("Logging initialized");
}

fn get_db_path(app: &tauri::AppHandle) -> PathBuf {
    let app_dir = app
        .path()
        .app_data_dir()
        .expect("Failed to resolve app data directory");
    std::fs::create_dir_all(&app_dir).expect("Failed to create app data directory");
    app_dir.join("worktime_v2.db")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    setup_logging();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let windows = app.webview_windows();
            for (_, window) in windows {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            info!("Setting up application...");

            let db_path = get_db_path(app.handle());
            info!("Database path: {}", db_path.display());

            let store = match StoreV2::new(&db_path) {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    error!("Failed to initialize database: {}", e);
                    std::process::exit(1);
                }
            };

            let state_mgr = Arc::new(StateManager::new(store.clone()));

            app.manage(store as AppStore);
            app.manage(state_mgr as AppStateMgr);

            // Create main window
            let _main_window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("timer.html".into()),
            )
            .title("工時記錄")
            .inner_size(420.0, 640.0)
            .min_inner_size(380.0, 560.0)
            .center()
            .build()?;

            info!("Application setup complete");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Hide instead of close for main window
                if window.label() == "main" {
                    window.hide().unwrap();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            get_current_slot,
            get_elapsed_seconds,
            get_remaining_seconds,
            start_task,
            end_task,
            pause_task,
            resume_task,
            start_interrupt,
            end_interrupt_and_resume,
            get_slots_for_date,
            get_day_stats,
            get_slots_for_range,
            get_task_summaries,
            find_gaps,
            get_recent_tasks,
            get_recent_people,
            get_settings,
            save_settings,
            create_slot_manual,
            update_slot_manual,
            prepare_export_data,
            export_to_xlsx,
            export_to_csv,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
