use anyhow::Result;
use chrono::{Local, Timelike};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
use tracing::{error, info};
use worktime_pomodoro_lib::*;

pub struct PomodoroScheduler {
    app_handle: AppHandle,
    store: Store,
    state_machine: Arc<Mutex<StateMachine>>,
    interval_minutes: u64,
}

impl PomodoroScheduler {
    pub fn new(
        app_handle: AppHandle,
        store: Store,
        state_machine: Arc<Mutex<StateMachine>>,
    ) -> Self {
        Self {
            app_handle,
            store,
            state_machine,
            interval_minutes: 25,
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting Pomodoro scheduler with {} minute intervals", self.interval_minutes);

        let mut ticker = interval(Duration::from_secs(self.interval_minutes * 60));

        loop {
            ticker.tick().await;
            
            if let Err(e) = self.on_tick().await {
                error!("Error during scheduler tick: {}", e);
            }
        }
    }

    async fn on_tick(&self) -> Result<()> {
        let now = Local::now();
        info!("Pomodoro timer tick at {}", now.format("%H:%M:%S"));

        // Check if we're in working hours
        if !self.is_working_hours() {
            info!("Outside working hours, skipping ping");
            return Ok(());
        }

        // Record ping event
        let sm = self.state_machine.lock().await;
        let ping_event = sm.ping(now, 0);
        self.store.insert_event(&ping_event)?;

        // Show ping window
        self.show_ping_window()?;

        // Send notification
        self.send_notification()?;

        Ok(())
    }

    fn is_working_hours(&self) -> bool {
        let now = Local::now();
        let hour = now.hour();
        
        // Default working hours: 8:00 - 17:30
        // TODO: Read from settings
        hour >= 8 && hour < 18
    }

    fn show_ping_window(&self) -> Result<()> {
        use tauri::{WebviewUrl, WebviewWindowBuilder};

        if let Some(window) = self.app_handle.get_webview_window("ping") {
            window.set_focus()?;
            return Ok(());
        }

        WebviewWindowBuilder::new(
            &self.app_handle,
            "ping",
            WebviewUrl::App("/ping".into()),
        )
        .title("番茄鐘到了！")
        .inner_size(500.0, 700.0)
        .resizable(false)
        .center()
        .always_on_top(true)
        .build()?;

        Ok(())
    }

    fn send_notification(&self) -> Result<()> {
        use tauri_plugin_notification::NotificationExt;

        self.app_handle
            .notification()
            .builder()
            .title("番茄鐘到了！")
            .body("請確認目前正在進行的任務")
            .show()?;

        Ok(())
    }
}
