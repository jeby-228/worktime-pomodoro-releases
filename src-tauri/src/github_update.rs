//! GitHub release updater, following TokenTracker's check → checksum → install path.
//!
//! The latest public release is resolved from the GitHub API, with the official
//! `/releases/latest` redirect as a fallback when the API is rate-limited. The
//! installer is never launched unless `SHA256SUMS` contains its digest and the
//! downloaded bytes match.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const API_LATEST: &str =
    "https://api.github.com/repos/jeby-228/worktime-pomodoro-releases/releases/latest";
const PAGE_LATEST: &str = "https://github.com/jeby-228/worktime-pomodoro-releases/releases/latest";
const DOWNLOAD_BASE: &str =
    "https://github.com/jeby-228/worktime-pomodoro-releases/releases/download";
const CHECKSUMS_NAME: &str = "SHA256SUMS";
const TAG_PREFIX: &str = "/jeby-228/worktime-pomodoro-releases/releases/tag/";

#[derive(Debug, Clone, Serialize)]
pub struct GithubUpdate {
    pub available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub notes: String,
    pub asset_name: String,
}

#[derive(Debug, Deserialize)]
struct ApiRelease {
    tag_name: String,
    body: Option<String>,
    assets: Vec<ApiAsset>,
}

#[derive(Debug, Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

struct ResolvedRelease {
    tag: String,
    version: String,
    notes: String,
    asset_name: String,
    download_url: String,
}

pub fn parse_stable_version(tag: &str) -> Result<(u64, u64, u64), String> {
    let version = tag.trim().strip_prefix('v').unwrap_or(tag.trim());
    if version.is_empty() || version.contains('-') || version.contains('+') {
        return Err(format!("版本標籤不是穩定的三段式版本：{tag}"));
    }
    let mut parts = version.split('.');
    let major = parse_part(parts.next(), tag)?;
    let minor = parse_part(parts.next(), tag)?;
    let patch = parse_part(parts.next(), tag)?;
    if parts.next().is_some() {
        return Err(format!("版本標籤不是穩定的三段式版本：{tag}"));
    }
    Ok((major, minor, patch))
}

fn parse_part(part: Option<&str>, tag: &str) -> Result<u64, String> {
    let part = part.ok_or_else(|| format!("版本標籤不是穩定的三段式版本：{tag}"))?;
    if part.is_empty() || !part.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(format!("版本標籤不是穩定的三段式版本：{tag}"));
    }
    part.parse()
        .map_err(|_| format!("版本標籤不是穩定的三段式版本：{tag}"))
}

pub fn is_newer(latest: &str, current: &str) -> Result<bool, String> {
    Ok(parse_stable_version(latest)? > parse_stable_version(current)?)
}

pub fn installer_name(version: &str, os: &str, arch: &str) -> Result<String, String> {
    match (os, arch) {
        ("linux", "x86_64") => Ok(format!("worktime-pomodoro_{version}_amd64.AppImage")),
        ("windows", "x86_64") => Ok(format!("worktime-pomodoro_{version}_x64-setup.exe")),
        ("macos", "aarch64") => Ok(format!("worktime-pomodoro_{version}_aarch64.dmg")),
        ("macos", "x86_64") => Ok(format!("worktime-pomodoro_{version}_x64.dmg")),
        _ => Err(format!("這個系統沒有對應的安裝檔：{os}-{arch}")),
    }
}

/// `<64 hex>  <name>` or `<64 hex> *<name>`. Missing lines mean "no checksum", not a mismatch.
pub fn find_digest(checksums_body: &str, file_name: &str) -> Option<String> {
    if checksums_body.trim().is_empty() || file_name.trim().is_empty() {
        return None;
    }
    for raw_line in checksums_body.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (digest, rest) = line.split_once(' ')?;
        if !is_sha256_hex(digest) {
            continue;
        }
        let name = rest.trim_start_matches([' ', '*']);
        if name.eq_ignore_ascii_case(file_name) {
            return Some(digest.to_ascii_lowercase());
        }
    }
    None
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("worktime-pomodoro")
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|err| err.to_string())
}

fn tag_from_release_page(url: &reqwest::Url) -> Result<String, String> {
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("最新版頁面沒有回到官方的 GitHub 版本標籤".to_string());
    }
    let tag = url
        .path()
        .strip_prefix(TAG_PREFIX)
        .filter(|tag| !tag.is_empty() && !tag.contains('/'))
        .ok_or_else(|| "最新版頁面沒有回到官方的 GitHub 版本標籤".to_string())?;
    Ok(tag.to_string())
}

async fn resolve_latest(client: &reqwest::Client) -> Result<ResolvedRelease, String> {
    let response = client
        .get(API_LATEST)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|err| format!("無法連到 GitHub：{err}"))?;

    if response.status().is_success() {
        let body = response
            .bytes()
            .await
            .map_err(|err| format!("GitHub 回傳的版本資料無法讀取：{err}"))?;
        let release: ApiRelease = serde_json::from_slice(&body)
            .map_err(|err| format!("GitHub 回傳的版本資料無法讀取：{err}"))?;
        return release_from_api(release);
    }

    let status = response.status();
    if status != reqwest::StatusCode::FORBIDDEN && status != reqwest::StatusCode::TOO_MANY_REQUESTS
    {
        return Err(format!("GitHub 版本查詢失敗（{status}）"));
    }

    let page = client
        .head(PAGE_LATEST)
        .send()
        .await
        .map_err(|err| format!("無法改走最新版頁面：{err}"))?;
    if !page.status().is_success() {
        return Err(format!("最新版頁面查詢失敗（{}）", page.status()));
    }
    let tag = tag_from_release_page(page.url())?;
    let version = parse_stable_version(&tag)
        .map(|(_, _, _)| tag.trim().trim_start_matches('v').to_string())?;
    let asset_name = installer_name(&version, std::env::consts::OS, std::env::consts::ARCH)?;
    let download_url = format!("{DOWNLOAD_BASE}/{tag}/{asset_name}");
    let checksums_url = format!("{DOWNLOAD_BASE}/{tag}/{CHECKSUMS_NAME}");
    let checksums = client
        .get(&checksums_url)
        .send()
        .await
        .map_err(|err| format!("無法下載 SHA256SUMS：{err}"))?
        .error_for_status()
        .map_err(|err| format!("這個版本沒有 SHA256SUMS：{err}"))?
        .text()
        .await
        .map_err(|err| err.to_string())?;
    if find_digest(&checksums, &asset_name).is_none() {
        return Err(format!("SHA256SUMS 沒有 {asset_name}"));
    }
    Ok(ResolvedRelease {
        tag,
        version,
        notes: String::new(),
        asset_name,
        download_url,
    })
}

fn release_from_api(release: ApiRelease) -> Result<ResolvedRelease, String> {
    let version = parse_stable_version(&release.tag_name)
        .map(|_| release.tag_name.trim().trim_start_matches('v').to_string())?;
    let asset_name = installer_name(&version, std::env::consts::OS, std::env::consts::ARCH)?;
    let asset = release
        .assets
        .into_iter()
        .find(|asset| asset.name.eq_ignore_ascii_case(&asset_name) && asset.size > 0)
        .ok_or_else(|| format!("這個版本沒有可用的安裝檔 {asset_name}"))?;
    Ok(ResolvedRelease {
        tag: release.tag_name,
        version,
        notes: release.body.unwrap_or_default(),
        asset_name: asset.name,
        download_url: asset.browser_download_url,
    })
}

#[tauri::command]
pub async fn check_github_release(app: AppHandle) -> Result<GithubUpdate, String> {
    let current_version = app.package_info().version.to_string();
    let client = http_client()?;
    let release = resolve_latest(&client).await?;
    let available = is_newer(&release.version, &current_version)?;
    Ok(GithubUpdate {
        available,
        current_version,
        latest_version: release.version,
        notes: release.notes,
        asset_name: if available {
            release.asset_name
        } else {
            String::new()
        },
    })
}

#[tauri::command]
pub async fn install_github_release(app: AppHandle) -> Result<String, String> {
    let current_version = app.package_info().version.to_string();
    let client = http_client()?;
    let release = resolve_latest(&client).await?;
    if !is_newer(&release.version, &current_version)? {
        return Err("目前已是最新版本".to_string());
    }

    let checksums_url = format!("{DOWNLOAD_BASE}/{}/{CHECKSUMS_NAME}", release.tag);
    let checksums = client
        .get(&checksums_url)
        .send()
        .await
        .map_err(|err| format!("無法下載 SHA256SUMS：{err}"))?
        .error_for_status()
        .map_err(|_| "這個版本還沒有 SHA256SUMS，已取消安裝".to_string())?
        .text()
        .await
        .map_err(|err| err.to_string())?;
    let expected = find_digest(&checksums, &release.asset_name)
        .ok_or_else(|| format!("SHA256SUMS 沒有 {}", release.asset_name))?;

    let bytes = client
        .get(&release.download_url)
        .send()
        .await
        .map_err(|err| format!("無法下載安裝檔：{err}"))?
        .error_for_status()
        .map_err(|err| format!("安裝檔下載失敗：{err}"))?
        .bytes()
        .await
        .map_err(|err| format!("安裝檔下載失敗：{err}"))?;
    let actual = sha256_hex(&bytes);
    if !expected.eq_ignore_ascii_case(&actual) {
        return Err("安裝檔的 SHA-256 不符，已取消安裝".to_string());
    }

    let dir = update_dir(&app)?;
    let path = dir.join(&release.asset_name);
    std::fs::write(&path, &bytes).map_err(|err| format!("無法儲存安裝檔：{err}"))?;
    make_executable(&path)?;
    let action = launch_installer(&path)?;
    if action == "exit" {
        app.exit(0);
    }
    Ok(action.to_string())
}

fn update_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?
        .join("updates");
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir)
}

fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(path)
            .map_err(|err| err.to_string())?
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).map_err(|err| err.to_string())?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn launch_installer(path: &Path) -> Result<&'static str, String> {
    let mut command = match std::env::consts::OS {
        "windows" => {
            let mut command = std::process::Command::new(path);
            command.arg("/S");
            command
        }
        "linux" => std::process::Command::new(path),
        "macos" => {
            let mut command = std::process::Command::new("open");
            command.arg(path);
            command
        }
        other => return Err(format!("這個系統無法安裝更新：{other}")),
    };
    command
        .spawn()
        .map_err(|err| format!("無法啟動安裝程式：{err}"))?;
    Ok(if std::env::consts::OS == "macos" {
        "opened-dmg"
    } else {
        "exit"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_versions_are_three_numbers() {
        assert_eq!(parse_stable_version("v1.2.3").unwrap(), (1, 2, 3));
        assert!(parse_stable_version("1.2").is_err());
        assert!(parse_stable_version("1.2.3.4").is_err());
        assert!(parse_stable_version("1.2.3-beta").is_err());
    }

    #[test]
    fn newer_version_compares_numerically() {
        assert!(is_newer("1.0.1", "1.0.0").unwrap());
        assert!(!is_newer("1.0.0", "1.0.0").unwrap());
        assert!(!is_newer("1.0.0", "1.1.0").unwrap());
    }

    #[test]
    fn checksum_line_matches_token_tracker_format() {
        let body = "\
# comment
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  other.txt
ABCDEF0123456789abcdef0123456789ABCDEF0123456789abcdef0123456789 *worktime-pomodoro_1.0.0_amd64.AppImage
";
        assert_eq!(
            find_digest(body, "worktime-pomodoro_1.0.0_amd64.AppImage").as_deref(),
            Some("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
        );
        assert!(find_digest(body, "missing").is_none());
    }

    #[test]
    fn installer_names_follow_the_release_assets() {
        assert_eq!(
            installer_name("1.0.0", "linux", "x86_64").unwrap(),
            "worktime-pomodoro_1.0.0_amd64.AppImage"
        );
        assert_eq!(
            installer_name("1.0.0", "windows", "x86_64").unwrap(),
            "worktime-pomodoro_1.0.0_x64-setup.exe"
        );
        assert_eq!(
            installer_name("1.0.0", "macos", "aarch64").unwrap(),
            "worktime-pomodoro_1.0.0_aarch64.dmg"
        );
    }

    #[test]
    fn release_page_must_stay_on_the_official_tag() {
        let ok = reqwest::Url::parse(
            "https://github.com/jeby-228/worktime-pomodoro-releases/releases/tag/v1.0.0",
        )
        .unwrap();
        assert_eq!(tag_from_release_page(&ok).unwrap(), "v1.0.0");
        let foreign = reqwest::Url::parse("https://example.com/releases/tag/v1.0.0").unwrap();
        assert!(tag_from_release_page(&foreign).is_err());
    }
}
