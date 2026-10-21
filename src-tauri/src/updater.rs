use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;
use tracing::info;
use worktime_pomodoro_lib::EventSource;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[tauri::command]
pub async fn check_updates(app: AppHandle, endpoint: Option<String>) -> Result<UpdateInfo, String> {
    info!("Checking for updates");

    let current_version = app.package_info().version.to_string();

    // Get endpoint from settings or use provided one
    let state = app.state::<crate::AppState>();
    let update_url = if let Some(url) = endpoint {
        url
    } else {
        state
            .store
            .get_setting("update_url")
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| {
                "https://github.com/yourname/worktime-pomodoro/releases/latest/download".to_string()
            })
    };

    info!("Checking updates from: {}", update_url);

    // Parse URL
    let endpoint_url =
        url::Url::parse(&update_url).map_err(|e| format!("Invalid update URL: {}", e))?;

    // Use tauri-plugin-updater
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint_url])
        .map_err(|e| format!("Failed to set endpoints: {}", e))?
        .build()
        .map_err(|e| format!("Failed to build updater: {}", e))?;

    match updater.check().await {
        Ok(Some(update)) => {
            info!("Update available: v{}", update.version);
            Ok(UpdateInfo {
                available: true,
                current_version,
                latest_version: Some(update.version.clone()),
                notes: update.body.clone(),
                date: update.date.map(|dt| dt.to_string()),
            })
        }
        Ok(None) => {
            info!("No update available");
            Ok(UpdateInfo {
                available: false,
                current_version,
                latest_version: None,
                notes: Some("You are running the latest version".to_string()),
                date: None,
            })
        }
        Err(e) => Err(format!("Update check failed: {}", e)),
    }
}

#[tauri::command]
pub async fn install_update(app: AppHandle, endpoint: Option<String>) -> Result<(), String> {
    info!("Installing update");

    // Get app state to pause work
    let state = app.state::<crate::AppState>();

    // 1. Pause current work
    {
        let sm = state.state_machine.lock().await;
        let events = sm.pause(chrono::Local::now(), 0, EventSource::Auto);
        for event in events {
            state
                .store
                .insert_event(&event)
                .map_err(|e| e.to_string())?;
        }
    }

    // 2. Checkpoint WAL
    info!("Checkpointing WAL before update");
    state.store.checkpoint_wal().map_err(|e| e.to_string())?;

    // 3. Get update URL
    let update_url = if let Some(url) = endpoint {
        url
    } else {
        state
            .store
            .get_setting("update_url")
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| {
                "https://github.com/yourname/worktime-pomodoro/releases/latest/download".to_string()
            })
    };

    // 4. Download and install update
    info!("Downloading update from: {}", update_url);

    // Parse URL
    let endpoint_url =
        url::Url::parse(&update_url).map_err(|e| format!("Invalid update URL: {}", e))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint_url])
        .map_err(|e| format!("Failed to set endpoints: {}", e))?
        .build()
        .map_err(|e| format!("Failed to build updater: {}", e))?;

    match updater.check().await {
        Ok(Some(update)) => {
            info!("Downloading and installing v{}", update.version);

            // Download and install
            let mut downloaded = 0;
            update
                .download_and_install(
                    |chunk_length, content_length| {
                        downloaded += chunk_length;
                        if let Some(total) = content_length {
                            let progress = (downloaded as f64 / total as f64) * 100.0;
                            info!("Download progress: {:.1}%", progress);
                        }
                    },
                    || {
                        info!("Update downloaded, installing...");
                    },
                )
                .await
                .map_err(|e| format!("Failed to download/install update: {}", e))?;

            info!("Update installed, restarting application");

            // Restart the app
            app.restart();
        }
        Ok(None) => Err("No update available".to_string()),
        Err(e) => Err(format!("Update check failed: {}", e)),
    }
}

pub async fn trigger_update_check(app: &AppHandle) -> Result<()> {
    let state = app.state::<crate::AppState>();

    // Check if update check on startup is enabled
    let check_enabled = state
        .store
        .get_setting("update_check_on_startup")?
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(true);

    if !check_enabled {
        info!("Update check on startup is disabled");
        return Ok(());
    }

    // Read update URL from settings
    let endpoint = state.store.get_setting("update_url")?.unwrap_or_else(|| {
        "https://github.com/yourname/worktime-pomodoro/releases/latest/download".to_string()
    });

    info!("Triggering update check: {}", endpoint);

    // Parse URL
    let endpoint_url = url::Url::parse(&endpoint)?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint_url])?
        .build()?;

    if let Some(update) = updater.check().await? {
        info!("Update available: v{}", update.version);

        // Emit event to frontend
        app.emit(
            "update-available",
            UpdateInfo {
                available: true,
                current_version: app.package_info().version.to_string(),
                latest_version: Some(update.version.clone()),
                notes: update.body.clone(),
                date: update.date.map(|dt| dt.to_string()),
            },
        )?;
    } else {
        info!("No update available");
    }

    Ok(())
}
