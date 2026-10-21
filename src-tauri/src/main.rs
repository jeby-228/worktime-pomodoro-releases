// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex;
use tracing::{error, info};
use worktime_pomodoro_lib::*;

mod commands;
mod scheduler;
mod updater;

use commands::*;
use scheduler::PomodoroScheduler;

pub struct AppState {
    pub store: Store,
    pub state_machine: Arc<Mutex<StateMachine>>,
    pub scheduler: Arc<Mutex<PomodoroScheduler>>,
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("worktime_pomodoro=debug,tauri=info")
        .init();

    info!("Starting worktime-pomodoro application");

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            info!("Another instance attempted to start");
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            info!("Setting up application");

            // Get data directory
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");
            std::fs::create_dir_all(&data_dir)?;

            let db_path = data_dir.join("worktime.db");
            info!("Database path: {:?}", db_path);

            // Initialize store
            let store = Store::new(&db_path)?;
            info!("Store initialized");

            // Initialize state machine
            let state_machine = Arc::new(Mutex::new(StateMachine::new()));

            // TODO: Recovery check on startup
            // This would require StateMachine to implement Clone
            // or refactor Recovery to accept Arc<Mutex<StateMachine>>

            // Initialize scheduler
            let scheduler = Arc::new(Mutex::new(PomodoroScheduler::new(
                app.handle().clone(),
                store.clone(),
                state_machine.clone(),
            )));

            // Start scheduler
            let scheduler_clone = scheduler.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = scheduler_clone.lock().await.start().await {
                    error!("Scheduler error: {}", e);
                }
            });

            // Manage app state
            app.manage(AppState {
                store: store.clone(),
                state_machine: state_machine.clone(),
                scheduler: scheduler.clone(),
            });

            // Setup system tray
            setup_system_tray(app)?;

            // Setup global shortcut
            setup_global_shortcut(app)?;

            // Setup sleep/wake detection (M3)
            #[cfg(target_os = "linux")]
            setup_linux_sleep_detection(app.handle().clone());

            #[cfg(target_os = "windows")]
            setup_windows_sleep_detection(app.handle().clone());

            info!("Application setup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Task commands
            start_task,
            end_task,
            pause_task,
            continue_task,
            get_current_state,
            // Stats commands
            get_daily_stats,
            export_csv,
            // Settings commands
            get_settings,
            save_settings,
            // Resolve commands
            get_unknown_periods,
            resolve_unknown_period,
            // Updater commands
            updater::check_updates,
            updater::install_update,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                // Prevent exit, just hide window
                api.prevent_exit();
            }
        });
}

fn setup_system_tray(app: &mut tauri::App) -> Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;

    let quit_item = MenuItem::with_id(app, "quit", "結束程式", true, None::<&str>)?;
    let start_item = MenuItem::with_id(app, "start_task", "開始/切換任務", true, None::<&str>)?;
    let pause_item = MenuItem::with_id(app, "pause_task", "暫停工作", true, None::<&str>)?;
    let stats_item = MenuItem::with_id(app, "show_stats", "今日統計", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "show_settings", "設定", true, None::<&str>)?;
    let check_updates_item = MenuItem::with_id(app, "check_updates", "檢查更新", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &start_item,
            &pause_item,
            &stats_item,
            &settings_item,
            &check_updates_item,
            &quit_item,
        ],
    )?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("工時番茄鐘")
        .on_menu_event(|app, event| {
            handle_tray_event(app, event.id().as_ref());
        })
        .build(app)?;

    Ok(())
}

fn handle_tray_event(app: &AppHandle, menu_id: &str) {
    match menu_id {
        "start_task" => {
            if let Err(e) = open_window(app, "ping", "/ping", 500, 700) {
                error!("Failed to open ping window: {}", e);
            }
        }
        "pause_task" => {
            let state = app.state::<AppState>();
            tauri::async_runtime::block_on(async {
                let sm = state.state_machine.lock().await;
                let events = sm.pause(chrono::Local::now(), 0, EventSource::User);
                for event in events {
                    if let Err(e) = state.store.insert_event(&event) {
                        error!("Failed to insert pause event: {}", e);
                    }
                }
            });
        }
        "show_stats" => {
            if let Err(e) = open_window(app, "stats", "/stats", 900, 700) {
                error!("Failed to open stats window: {}", e);
            }
        }
        "show_settings" => {
            if let Err(e) = open_window(app, "settings", "/settings", 700, 900) {
                error!("Failed to open settings window: {}", e);
            }
        }
        "check_updates" => {
            // Trigger update check
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = updater::trigger_update_check(&app_handle).await {
                    error!("Update check failed: {}", e);
                }
            });
        }
        "quit" => {
            // End current task before quitting
            let state = app.state::<AppState>();
            tauri::async_runtime::block_on(async {
                let sm = state.state_machine.lock().await;
                let events = sm.end_task(chrono::Local::now(), 0, EventSource::Auto);
                for event in events {
                    if let Err(e) = state.store.insert_event(&event) {
                        error!("Failed to insert end event: {}", e);
                    }
                }
                
                // Checkpoint WAL
                if let Err(e) = state.store.checkpoint_wal() {
                    error!("Failed to checkpoint WAL: {}", e);
                }
            });

            app.exit(0);
        }
        _ => {}
    }
}

fn open_window(app: &AppHandle, label: &str, url: &str, width: u32, height: u32) -> Result<()> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    if let Some(window) = app.get_webview_window(label) {
        window.set_focus()?;
        return Ok(());
    }

    WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title("工時番茄鐘")
        .inner_size(width as f64, height as f64)
        .resizable(true)
        .center()
        .build()?;

    Ok(())
}

fn setup_global_shortcut(app: &mut tauri::App) -> Result<()> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let app_handle = app.handle().clone();
    
    app.global_shortcut().on_shortcut("Ctrl+Alt+T", move |_app, _shortcut, _event| {
        if let Err(e) = open_window(&app_handle, "ping", "/ping", 500, 700) {
            error!("Failed to open ping window via shortcut: {}", e);
        }
    })?;

    // Register the shortcut
    app.global_shortcut()
        .register("Ctrl+Alt+T")?;

    Ok(())
}

// M3: Sleep/wake detection for Linux
#[cfg(target_os = "linux")]
fn setup_linux_sleep_detection(_app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Best-effort: try to connect to login1 D-Bus for sleep signals
        if let Ok(_connection) = zbus::Connection::system().await {
            info!("Connected to D-Bus for sleep detection");
            
            // In real implementation, subscribe to PrepareForSleep signal
            // For now, this is a stub
            // let proxy = login1::ManagerProxy::new(&connection).await;
            // let mut signal_stream = proxy.receive_prepare_for_sleep().await;
            
            info!("Linux sleep detection stub active");
        } else {
            info!("D-Bus not available, sleep detection disabled");
        }
    });
}

// M3: Sleep/wake detection for Windows
#[cfg(target_os = "windows")]
fn setup_windows_sleep_detection(_app_handle: AppHandle) {
    info!("Windows sleep detection would use WM_POWERBROADCAST");
    // Implementation would use windows crate to hook into power events
    // This is a stub as it requires unsafe Windows API calls
}
