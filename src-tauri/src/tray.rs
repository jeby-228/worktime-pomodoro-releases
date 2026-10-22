use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};
use tracing::info;

pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show_i = MenuItemBuilder::with_id("show", "顯示主視窗").build(app)?;
    let quick_input_i = MenuItemBuilder::with_id("quick_input", "快速輸入 (Ctrl+Alt+T)").build(app)?;
    let interrupt_i = MenuItemBuilder::with_id("interrupt", "插隊 (Ctrl+Alt+I)").build(app)?;
    let separator_i = PredefinedMenuItem::separator(app)?;
    let pause_resume_i = MenuItemBuilder::with_id("pause_resume", "暫停 / 繼續").build(app)?;
    let end_task_i = MenuItemBuilder::with_id("end_task", "結束任務").build(app)?;
    let separator2_i = PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItemBuilder::with_id("quit", "退出").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_i)
        .item(&separator_i)
        .item(&quick_input_i)
        .item(&interrupt_i)
        .item(&separator2_i)
        .item(&pause_resume_i)
        .item(&end_task_i)
        .item(&separator2_i)
        .item(&quit_i)
        .build()?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .icon(app.default_window_icon().unwrap().clone())
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quick_input" => {
                info!("Tray: Quick input clicked");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.emit("tray:quick_input", ());
                }
            }
            "interrupt" => {
                info!("Tray: Interrupt clicked");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.emit("tray:interrupt", ());
                }
            }
            "pause_resume" => {
                info!("Tray: Pause/Resume clicked");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.emit("tray:pause_resume", ());
                }
            }
            "end_task" => {
                info!("Tray: End task clicked");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.emit("tray:end_task", ());
                }
            }
            "quit" => {
                info!("Tray: Quit clicked");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
