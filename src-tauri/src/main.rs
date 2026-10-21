// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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

            // M3: Recovery check on startup
            let recovery_store = store.clone();
            let recovery_sm = state_machine.clone();
            tauri::async_runtime::block_on(async move {
                let sm = recovery_sm.lock().await.clone();
                let recovery = Recovery::new(recovery_store.clone(), sm);
                
                match recovery.check_on_startup(chrono::Local::now()) {
                    Ok(Some(action)) => {
                        info!("Recovery needed: {}", action.message);
                        
                        // Apply recovery
                        if let Err(e) = recovery.apply_recovery(action) {
                            error!("Failed to apply recovery: {}", e);
                        } else {
                            info!("Recovery applied successfully");
                            
                            // Rebuild segments
                            let deriver = SegmentDeriver::new(recovery_store.clone());
                            if let Err(e) = deriver.rebuild_segments() {
                                error!("Failed to rebuild segments after recovery: {}", e);
                            }
                        }
                    }
                    Ok(None) => {
                        info!("No recovery needed");
                    }
                    Err(e) => {
                        error!("Recovery check failed: {}", e);
                    }
                }
            });

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

            // Check for updates on startup
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = updater::trigger_update_check(&app_handle).await {
                    error!("Startup update check failed: {}", e);
                }
            });

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
fn setup_linux_sleep_detection(app_handle: AppHandle) {
    use futures::StreamExt;
    use zbus::proxy;
    
    #[proxy(
        interface = "org.freedesktop.login1.Manager",
        default_service = "org.freedesktop.login1",
        default_path = "/org/freedesktop/login1"
    )]
    trait Login1Manager {
        #[zbus(signal)]
        fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
    }

    tauri::async_runtime::spawn(async move {
        info!("Setting up Linux sleep detection via D-Bus");
        
        match zbus::Connection::system().await {
            Ok(connection) => {
                info!("Connected to D-Bus system bus");
                
                match Login1ManagerProxy::new(&connection).await {
                    Ok(proxy) => {
                        info!("Connected to logind Manager");
                        
                        let mut stream = match proxy.receive_prepare_for_sleep().await {
                            Ok(s) => s,
                            Err(e) => {
                                error!("Failed to subscribe to PrepareForSleep: {}", e);
                                return;
                            }
                        };
                        
                        info!("Subscribed to PrepareForSleep signal");
                        
                        loop {
                            match stream.next().await {
                                Some(signal) => {
                                    let args = match signal.args() {
                                        Ok(a) => a,
                                        Err(e) => {
                                            error!("Failed to parse signal args: {}", e);
                                            continue;
                                        }
                                    };
                                    
                                    if args.start {
                                        info!("System going to sleep");
                                        handle_sleep(&app_handle).await;
                                    } else {
                                        info!("System waking up from sleep");
                                        handle_wake(&app_handle).await;
                                    }
                                }
                                None => {
                                    error!("D-Bus signal stream ended");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to create logind proxy: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to connect to D-Bus: {}", e);
            }
        }
    });
}

#[cfg(target_os = "linux")]
async fn handle_sleep(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    let sm = state.state_machine.lock().await.clone();
    let recovery = Recovery::new(state.store.clone(), sm);
    
    if let Ok(events) = recovery.handle_sleep(chrono::Local::now()) {
        for event in events {
            if let Err(e) = state.store.insert_event(&event) {
                error!("Failed to insert sleep event: {}", e);
            }
        }
        
        // Rebuild segments
        let deriver = SegmentDeriver::new(state.store.clone());
        if let Err(e) = deriver.rebuild_segments() {
            error!("Failed to rebuild segments after sleep: {}", e);
        }
    }
}

#[cfg(target_os = "linux")]
async fn handle_wake(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    let sm = state.state_machine.lock().await.clone();
    let recovery = Recovery::new(state.store.clone(), sm);
    
    if let Ok(events) = recovery.handle_wake(chrono::Local::now()) {
        for event in events {
            if let Err(e) = state.store.insert_event(&event) {
                error!("Failed to insert wake event: {}", e);
            }
        }
        
        // Rebuild segments
        let deriver = SegmentDeriver::new(state.store.clone());
        if let Err(e) = deriver.rebuild_segments() {
            error!("Failed to rebuild segments after wake: {}", e);
        }
    }
}

// M3: Sleep/wake detection for Windows
#[cfg(target_os = "windows")]
fn setup_windows_sleep_detection(app_handle: AppHandle) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Power::{
        PBT_APMSUSPEND, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
        TranslateMessage, CS_HREDRAW, CS_VREDRAW, MSG, WNDCLASSW, WM_POWERBROADCAST,
        WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
    };

    info!("Setting up Windows sleep detection");

    std::thread::spawn(move || {
        unsafe {
            let class_name = windows::core::w!("PowerEventWindow");
            
            let wc = WNDCLASSW {
                lpfnWndProc: Some(power_window_proc),
                lpszClassName: class_name,
                style: CS_HREDRAW | CS_VREDRAW,
                ..Default::default()
            };

            if RegisterClassW(&wc) == 0 {
                error!("Failed to register window class for power events");
                return;
            }

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                windows::core::w!("PowerEvents"),
                WS_OVERLAPPEDWINDOW,
                0, 0, 0, 0,
                HWND(0),
                None,
                None,
                None,
            );

            if hwnd.0 == 0 {
                error!("Failed to create window for power events");
                return;
            }

            // Store app handle in window user data
            // Note: This is simplified; production code should use SetWindowLongPtrW
            
            info!("Windows power event listener started");

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND(0), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    });
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn power_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::System::Power::{PBT_APMSUSPEND, PBT_APMRESUMEAUTOMATIC};
    use windows::Win32::UI::WindowsAndMessaging::{DefWindowProcW, WM_POWERBROADCAST};

    if msg == WM_POWERBROADCAST {
        match wparam.0 as u32 {
            PBT_APMSUSPEND => {
                info!("Windows: System going to sleep");
                // TODO: Get app_handle from window user data and call handle_sleep
            }
            PBT_APMRESUMEAUTOMATIC => {
                info!("Windows: System waking up");
                // TODO: Get app_handle from window user data and call handle_wake
            }
            _ => {}
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}
