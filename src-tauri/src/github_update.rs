//! App updates through tauri-plugin-updater.
//!
//! `latest.json` on the newest GitHub release lists one artifact per platform
//! (and per Linux package type) together with its minisign signature. The
//! plugin refuses any download whose signature does not verify against the
//! public key in `tauri.conf.json`, then installs it in place: the AppImage,
//! `.app` bundle or deb/rpm package is replaced where it already lives, so
//! shortcuts and the autostart entry keep pointing at the current version.
//! On Windows the plugin runs the NSIS installer and exits the app itself.

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Debug, Clone, Serialize)]
pub struct GithubUpdate {
    pub available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub notes: String,
}

async fn find_update(app: &AppHandle) -> Result<Option<Update>, String> {
    app.updater()
        .map_err(|err| format!("更新器無法啟動：{err}"))?
        .check()
        .await
        .map_err(|err| format!("無法檢查更新：{err}"))
}

#[tauri::command]
pub async fn check_github_release(app: AppHandle) -> Result<GithubUpdate, String> {
    let current_version = app.package_info().version.to_string();
    Ok(match find_update(&app).await? {
        Some(update) => GithubUpdate {
            available: true,
            current_version,
            latest_version: update.version,
            notes: update.body.unwrap_or_default(),
        },
        None => GithubUpdate {
            available: false,
            latest_version: current_version.clone(),
            current_version,
            notes: String::new(),
        },
    })
}

/// Downloads, verifies and installs the update, then restarts into it.
/// Only returns if something went wrong.
#[tauri::command]
pub async fn install_github_release(app: AppHandle) -> Result<(), String> {
    let update = find_update(&app)
        .await?
        .ok_or_else(|| "目前已是最新版本".to_string())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|err| format!("更新安裝失敗：{err}"))?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use base64::Engine;

    fn updater_config() -> serde_json::Value {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        conf["plugins"]["updater"].clone()
    }

    #[test]
    fn updater_pubkey_is_a_valid_minisign_key() {
        let encoded = updater_config()["pubkey"].as_str().unwrap().to_string();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .unwrap();
        let text = String::from_utf8(decoded).unwrap();
        assert!(minisign_verify::PublicKey::decode(&text).is_ok());
    }

    #[test]
    fn updater_only_talks_to_the_official_release_over_https() {
        let endpoints = updater_config()["endpoints"].as_array().unwrap().clone();
        assert!(!endpoints.is_empty());
        for endpoint in endpoints {
            assert!(endpoint
                .as_str()
                .unwrap()
                .starts_with("https://github.com/jeby-228/worktime-pomodoro-releases/releases/"));
        }
    }
}
