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
            show_main_window(app);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
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

            // Tray / shortcut failures must not block the main window.
            if let Err(e) = setup_system_tray(app) {
                error!("System tray unavailable: {e:#}");
            }
            if let Err(e) = setup_global_shortcut(app) {
                error!("Global shortcut unavailable: {e:#}");
            }

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

            let start_minimized = std::env::args().any(|a| a == "--minimized");
            if start_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            } else {
                show_main_window(app.handle());
            }

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
            export_xlsx,
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
        .run(|app_handle, event| match event {
            tauri::RunEvent::ExitRequested { api, code, .. } => {
                // Keep running in the tray unless quit was requested.
                if code.is_none() {
                    api.prevent_exit();
                }
            }
            tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } if label == "main" => {
                api.prevent_close();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            _ => {}
        });
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }

    if let Err(e) = open_window(app, "main", "/", 800, 600) {
        error!("Failed to open main window: {e:#}");
    }
}

fn setup_system_tray(app: &mut tauri::App) -> Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;

    let open_item = MenuItem::with_id(app, "open_main", "開啟主畫面", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "結束程式", true, None::<&str>)?;
    let start_item = MenuItem::with_id(app, "start_task", "開始/切換任務", true, None::<&str>)?;
    let pause_item = MenuItem::with_id(app, "pause_task", "暫停工作", true, None::<&str>)?;
    let stats_item = MenuItem::with_id(app, "show_stats", "今日統計", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "show_settings", "設定", true, None::<&str>)?;
    let check_updates_item =
        MenuItem::with_id(app, "check_updates", "檢查更新", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &open_item,
            &start_item,
            &pause_item,
            &stats_item,
            &settings_item,
            &check_updates_item,
            &quit_item,
        ],
    )?;

    let mut tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("what are you doing")
        .on_menu_event(|app, event| {
            handle_tray_event(app, event.id().as_ref());
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;

    Ok(())
}

fn handle_tray_event(app: &AppHandle, menu_id: &str) {
    match menu_id {
        "open_main" => show_main_window(app),
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
        let _ = window.show();
        let _ = window.unminimize();
        window.set_focus()?;
        return Ok(());
    }

    WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title("what are you doing")
        .inner_size(width as f64, height as f64)
        .resizable(true)
        .center()
        .build()?;

    Ok(())
}

fn setup_global_shortcut(app: &mut tauri::App) -> Result<()> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let app_handle = app.handle().clone();

    app.global_shortcut()
        .on_shortcut("Ctrl+Alt+T", move |_app, _shortcut, _event| {
            if let Err(e) = open_window(&app_handle, "ping", "/ping", 500, 700) {
                error!("Failed to open ping window via shortcut: {}", e);
            }
        })?;

    // Register the shortcut
    app.global_shortcut().register("Ctrl+Alt+T")?;

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
fn setup_windows_sleep_detection(_app_handle: AppHandle) {
    use std::ptr::null_mut;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, RegisterClassW, TranslateMessage,
        CS_HREDRAW, CS_VREDRAW, MSG, WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
    };

    info!("Setting up Windows sleep detection");

    std::thread::spawn(move || unsafe {
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

        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            windows::core::w!("PowerEvents"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            0,
            0,
            HWND(null_mut()),
            None,
            None,
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                error!("Failed to create window for power events: {e}");
                return;
            }
        };

        if hwnd.0.is_null() {
            error!("Failed to create window for power events");
            return;
        }

        info!("Windows power event listener started");

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND(null_mut()), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn power_window_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{DefWindowProcW, WM_POWERBROADCAST};

    // winuser.h: PBT_APMSUSPEND = 0x0004, PBT_APMRESUMEAUTOMATIC = 0x0012
    const PBT_APMSUSPEND: u32 = 0x0004;
    const PBT_APMRESUMEAUTOMATIC: u32 = 0x0012;

    if msg == WM_POWERBROADCAST {
        match wparam.0 as u32 {
            PBT_APMSUSPEND => {
                info!("Windows: System going to sleep");
            }
            PBT_APMRESUMEAUTOMATIC => {
                info!("Windows: System waking up");
            }
            _ => {}
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}
