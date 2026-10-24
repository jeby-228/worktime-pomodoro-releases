// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

// Re-export types from lib for commands_v2 to use
use worktime_pomodoro_lib::*;

type AppStore = Arc<StoreV2>;
type AppStateMgr = Arc<StateManager>;

mod commands_v2;
mod github_update;
use commands_v2::*;
use github_update::{check_github_release, install_github_release};

mod tray;

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
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            info!("App setup started");

            let db_path = get_db_path(app.handle());
            info!("Database path: {:?}", db_path);

            let store = match StoreV2::new(&db_path) {
                Ok(s) => {
                    info!("StoreV2 initialized successfully");
                    Arc::new(s)
                }
                Err(e) => {
                    error!("Failed to initialize StoreV2: {}", e);
                    return Err(e.into());
                }
            };

            let state_mgr = Arc::new(StateManager::new(Arc::clone(&store)));
            info!("StateManager initialized");

            app.manage(store.clone() as AppStore);
            app.manage(state_mgr as AppStateMgr);

            let labels: Vec<String> = app.webview_windows().keys().cloned().collect();
            info!("windows at setup: {:?}", labels);
            if let Ok(settings) = store.get_settings() {
                match sync_companion_windows(app.handle(), &settings.mini_mode) {
                    Ok(()) => info!("companion windows synced for {}", settings.mini_mode),
                    Err(err) => error!("failed to sync companion windows: {err}"),
                }
            }

            // Create tray icon
            tray::create_tray(app.handle())?;

            // Note: The main window is already created from tauri.conf.json
            // No need to create it again here to avoid "webview with label 'main' already exists" error

            info!("App setup completed");
            Ok(())
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
            delete_slot,
            fill_gap,
            handle_idle_return,
            prepare_export_data,
            export_to_xlsx,
            export_to_csv,
            show_minibar,
            hide_minibar,
            set_minibar_height,
            focus_main_window,
            open_quick_input,
            show_pet,
            hide_pet,
            preview_companion_mode,
            check_github_release,
            install_github_release,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}
