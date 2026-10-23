# 應用程式更新系統實作指南

本文檔描述如何整合 Tauri 的自動更新功能，包含簽章驗證、安全安裝和資料保護。

## 概述

使用 `tauri-plugin-updater` 實現自簽章、可設定endpoint 的更新系統：

1. 使用者可設定更新 URL（支援私有 repo 或內網伺服器）
2. 更新前自動暫停工作並確保資料完整性
3. 安裝後自動執行恢復與遷移流程
4. 使用 minisign 簽章確保更新安全性

## 依賴設定

### Cargo.toml

```toml
[dependencies]
tauri-plugin-updater = "2"
tauri-plugin-process = "2"

# ... 其他依賴
```

### package.json

```json
{
  "devDependencies": {
    "@tauri-apps/plugin-updater": "^2",
    "@tauri-apps/plugin-process": "^2"
  }
}
```

## 簽章金鑰管理

### 1. 產生金鑰對（一次性操作）

```bash
# 安裝 Tauri CLI（如果還沒有）
npm install -g @tauri-apps/cli

# 產生金鑰
npx @tauri-apps/cli signer generate -w ~/.tauri/worktime-pomodoro.key

# 輸出示例：
# Private key: ~/.tauri/worktime-pomodoro.key
# Public key: dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEE1RjdCMzQ...
```

### 2. 保管私鑰

⚠️ **絕對不要**將私鑰提交到版本控制！

**建議做法**：

- 存放在安全位置（密碼管理器、硬體金鑰）
- CI/CD 使用環境變數 `TAURI_SIGNING_PRIVATE_KEY`
- 本機開發時從 `~/.tauri/` 讀取

### 3. 設定 .gitignore

```gitignore
# Tauri 私鑰
*.key
.tauri/
src-tauri/target/

# 不要提交簽章檔（發布時才產生）
*.sig
```

## Tauri 設定

### src-tauri/tauri.conf.json

```json
{
  "productName": "worktime-pomodoro",
  "version": "0.1.0",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:4321",
    "frontendDist": "../dist"
  },
  "bundle": {
    "active": true,
    "createUpdaterArtifacts": true,
    "targets": ["msi", "nsis"],
    "windows": {
      "certificateThumbprint": null,
      "digestAlgorithm": "sha256",
      "timestampUrl": ""
    }
  },
  "plugins": {
    "updater": {
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6..."
    }
  }
}
```

**注意**: `pubkey` 填入公鑰字串（產生金鑰時的輸出）。

## Rust 實作

### src-tauri/src/updater.rs

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    pub endpoint_url: String,
    pub check_on_startup: bool,
    pub auto_install: bool,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            endpoint_url: "https://github.com/yourname/worktime-pomodoro/releases/latest/download".to_string(),
            check_on_startup: true,
            auto_install: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub download_size: Option<u64>,
}

pub struct UpdateManager {
    app: AppHandle,
}

impl UpdateManager {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    /// 檢查更新
    pub async fn check_for_updates(&self, endpoint: &str) -> Result<UpdateInfo> {
        info!("Checking for updates from: {}", endpoint);

        let updater = self.app.updater_builder()
            .endpoints(vec![endpoint.to_string()])
            .build()?;

        match updater.check().await {
            Ok(Some(update)) => {
                info!("Update available: v{}", update.version);

                Ok(UpdateInfo {
                    available: true,
                    version: Some(update.version.clone()),
                    notes: update.body.clone(),
                    download_size: update.download_size,
                })
            }
            Ok(None) => {
                info!("No update available");
                Ok(UpdateInfo {
                    available: false,
                    version: None,
                    notes: None,
                    download_size: None,
                })
            }
            Err(e) => {
                error!("Failed to check for updates: {}", e);
                Err(e.into())
            }
        }
    }

    /// 下載並安裝更新
    pub async fn install_update(&self, endpoint: &str) -> Result<()> {
        info!("Starting update installation from: {}", endpoint);

        // 1. 暫停當前工作（如果有）
        self.pause_current_work().await?;

        // 2. 刷新 SQLite WAL
        self.flush_database().await?;

        // 3. 下載並驗證更新
        let updater = self.app.updater_builder()
            .endpoints(vec![endpoint.to_string()])
            .build()?;

        if let Some(update) = updater.check().await? {
            info!("Downloading update v{}", update.version);

            // 下載（自動驗證簽章）
            let mut downloaded = 0u64;
            update.download_and_install(
                |chunk_length, content_length| {
                    downloaded += chunk_length;
                    if let Some(total) = content_length {
                        let progress = (downloaded as f64 / total as f64 * 100.0) as u32;
                        info!("Download progress: {}%", progress);
                        // 可透過 tauri::emit 發送進度事件給前端
                    }
                },
                || {
                    info!("Download complete, installing...");
                }
            ).await?;

            info!("Update installed successfully");

            // 4. 重啟應用程式
            self.restart_app().await?;
        } else {
            warn!("No update available to install");
        }

        Ok(())
    }

    /// 暫停當前工作
    async fn pause_current_work(&self) -> Result<()> {
        // 取得狀態機
        let state_machine = self.app.state::<crate::state::StateMachine>();
        let store = self.app.state::<crate::store::Store>();

        // 如果正在工作中，記錄 end 事件
        let current_state = state_machine.get_state();
        if matches!(current_state, crate::state::AppState::Working { .. }) {
            let now = chrono::Local::now();
            let mono_ms = 0; // 從啟動時間計算

            let events = state_machine.end_task(
                now,
                mono_ms,
                crate::store::EventSource::Auto,
            );

            for event in events {
                store.insert_event(&event)?;
            }

            info!("Paused current work before update");
        }

        Ok(())
    }

    /// 刷新 SQLite WAL 到主檔案
    async fn flush_database(&self) -> Result<()> {
        let store = self.app.state::<crate::store::Store>();

        // 執行 WAL checkpoint
        store.checkpoint_wal()?;

        info!("Database flushed before update");
        Ok(())
    }

    /// 重啟應用程式
    async fn restart_app(&self) -> Result<()> {
        use tauri_plugin_process::ProcessExt;

        info!("Restarting application...");
        self.app.restart();

        Ok(())
    }
}

/// Tauri command: 檢查更新
#[tauri::command]
pub async fn check_updates(
    app: AppHandle,
    endpoint: String,
) -> Result<UpdateInfo, String> {
    let manager = UpdateManager::new(app);
    manager.check_for_updates(&endpoint)
        .await
        .map_err(|e| e.to_string())
}

/// Tauri command: 安裝更新
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    endpoint: String,
) -> Result<(), String> {
    let manager = UpdateManager::new(app);
    manager.install_update(&endpoint)
        .await
        .map_err(|e| e.to_string())
}
```

### src-tauri/src/store.rs 新增方法

```rust
impl Store {
    /// Checkpoint WAL to main database file
    pub fn checkpoint_wal(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.pragma_update(None, "wal_checkpoint", "TRUNCATE")?;

        Ok(())
    }
}
```

### src-tauri/src/main.rs 整合

```rust
use tauri::Manager;

mod updater;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            updater::check_updates,
            updater::install_update,
            // ... 其他 commands
        ])
        .setup(|app| {
            // 啟動時檢查更新（如果設定啟用）
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let store = app_handle.state::<crate::store::Store>();

                let check_on_startup = store
                    .get_setting("update_check_on_startup")
                    .unwrap_or_else(|_| Some("true".to_string()))
                    .map(|v| v == "true")
                    .unwrap_or(true);

                if check_on_startup {
                    let endpoint = store
                        .get_setting("update_endpoint")
                        .unwrap_or_default()
                        .unwrap_or_else(|| updater::UpdateConfig::default().endpoint_url);

                    let manager = updater::UpdateManager::new(app_handle.clone());
                    if let Ok(info) = manager.check_for_updates(&endpoint).await {
                        if info.available {
                            // 發送事件給前端顯示通知
                            app_handle.emit("update-available", &info).ok();
                        }
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## 前端整合（TypeScript）

### src/lib/updater.ts

```typescript
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask, message } from "@tauri-apps/plugin-dialog";

export interface UpdateInfo {
  available: boolean;
  version?: string;
  notes?: string;
  download_size?: number;
}

export class UpdateManager {
  private endpoint: string;

  constructor(endpoint?: string) {
    this.endpoint = endpoint || this.getDefaultEndpoint();
  }

  private getDefaultEndpoint(): string {
    // 從設定讀取，或使用預設值
    return (
      localStorage.getItem("update_endpoint") ||
      "https://github.com/yourname/worktime-pomodoro/releases/latest/download"
    );
  }

  async checkForUpdates(): Promise<UpdateInfo> {
    return await invoke<UpdateInfo>("check_updates", {
      endpoint: this.endpoint,
    });
  }

  async installUpdate(): Promise<void> {
    // 詢問使用者確認
    const confirmed = await ask("發現新版本，是否立即安裝？\n\n應用程式將會重新啟動。", {
      title: "更新可用",
      kind: "info",
    });

    if (confirmed) {
      try {
        await invoke("install_update", {
          endpoint: this.endpoint,
        });
      } catch (error) {
        await message(`更新安裝失敗: ${error}`, { title: "錯誤", kind: "error" });
      }
    }
  }

  listenForUpdates(callback: (info: UpdateInfo) => void): () => void {
    const unlisten = listen<UpdateInfo>("update-available", (event) => {
      callback(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }
}

// 全域實例
export const updater = new UpdateManager();
```

### src/components/UpdateNotification.astro

```astro
---
// 更新通知元件（用於主頁面）
---

<div id="update-notification" class="hidden fixed top-4 right-4 bg-blue-500 text-white p-4 rounded-lg shadow-lg z-50">
  <div class="flex items-center gap-3">
    <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
    </svg>
    <div>
      <div class="font-bold">有新版本可用</div>
      <div id="update-version" class="text-sm"></div>
    </div>
    <button id="install-update-btn" class="ml-4 px-4 py-2 bg-white text-blue-500 rounded hover:bg-gray-100">
      立即更新
    </button>
    <button id="dismiss-update-btn" class="px-2 py-2 hover:bg-blue-600 rounded">
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
      </svg>
    </button>
  </div>
</div>

<script>
  import { updater } from '../lib/updater';

  const notification = document.getElementById('update-notification')!;
  const versionText = document.getElementById('update-version')!;
  const installBtn = document.getElementById('install-update-btn')!;
  const dismissBtn = document.getElementById('dismiss-update-btn')!;

  // 監聽更新事件
  updater.listenForUpdates((info) => {
    if (info.available && info.version) {
      versionText.textContent = `版本 ${info.version}`;
      notification.classList.remove('hidden');
    }
  });

  // 安裝按鈕
  installBtn.addEventListener('click', async () => {
    await updater.installUpdate();
  });

  // 關閉按鈕
  dismissBtn.addEventListener('click', () => {
    notification.classList.add('hidden');
  });
</script>
```

## 建置與發布流程

### 1. 本機建置（開發測試）

```bash
# 設定環境變數（使用私鑰）
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/worktime-pomodoro.key)"

# 建置
npm run tauri:build

# 產出檔案（Windows 示例）：
# src-tauri/target/release/bundle/msi/worktime-pomodoro_0.1.0_x64_en-US.msi
# src-tauri/target/release/bundle/msi/worktime-pomodoro_0.1.0_x64_en-US.msi.zip
# src-tauri/target/release/bundle/msi/worktime-pomodoro_0.1.0_x64_en-US.msi.zip.sig
```

### 2. CI/CD 建置（GitHub Actions 示例）

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

jobs:
  build:
    strategy:
      matrix:
        platform: [windows-latest, ubuntu-22.04]

    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 20

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install dependencies (Ubuntu)
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev \
            build-essential curl wget file libxdo-dev \
            libssl-dev libayatana-appindicator3-dev librsvg2-dev

      - name: Install Node dependencies
        run: npm ci

      - name: Build Tauri app
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_PRIVATE_KEY }}
        run: npm run tauri:build

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.platform }}-artifacts
          path: |
            src-tauri/target/release/bundle/**/*.msi.zip
            src-tauri/target/release/bundle/**/*.msi.zip.sig
            src-tauri/target/release/bundle/**/*.AppImage.tar.gz
            src-tauri/target/release/bundle/**/*.AppImage.tar.gz.sig

  release:
    needs: build
    runs-on: ubuntu-latest

    steps:
      - name: Download artifacts
        uses: actions/download-artifact@v4

      - name: Create latest.json
        run: |
          # 產生 latest.json（需解析簽章檔）
          # 參考下節「latest.json 產生腳本」

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            **/*.msi.zip
            **/*.msi.zip.sig
            **/*.AppImage.tar.gz
            **/*.AppImage.tar.gz.sig
            latest.json
          generate_release_notes: true
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### 3. latest.json 產生腳本

```python
#!/usr/bin/env python3
"""Generate latest.json for Tauri updater"""

import json
import os
import sys
from pathlib import Path

def read_signature(sig_path: Path) -> str:
    """讀取 .sig 檔案內容"""
    return sig_path.read_text().strip()

def generate_latest_json(version: str, notes: str, artifacts_dir: Path):
    """產生 latest.json"""

    platforms = {}

    # Windows
    windows_zip = list(artifacts_dir.glob("**/windows-latest-artifacts/*.msi.zip"))
    if windows_zip:
        sig_file = windows_zip[0].with_suffix('.zip.sig')
        platforms["windows-x86_64"] = {
            "signature": read_signature(sig_file),
            "url": f"https://github.com/yourname/worktime-pomodoro/releases/download/{version}/{windows_zip[0].name}"
        }

    # Linux
    linux_appimage = list(artifacts_dir.glob("**/ubuntu-22.04-artifacts/*.AppImage.tar.gz"))
    if linux_appimage:
        sig_file = linux_appimage[0].with_suffix('.tar.gz.sig')
        platforms["linux-x86_64"] = {
            "signature": read_signature(sig_file),
            "url": f"https://github.com/yourname/worktime-pomodoro/releases/download/{version}/{linux_appimage[0].name}"
        }

    latest_json = {
        "version": version.lstrip('v'),
        "notes": notes,
        "pub_date": "2026-10-21T10:00:00Z",  # 使用實際發布時間
        "platforms": platforms
    }

    output_path = artifacts_dir / "latest.json"
    output_path.write_text(json.dumps(latest_json, indent=2))
    print(f"Generated {output_path}")

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: generate_latest_json.py <version> <notes>")
        sys.exit(1)

    generate_latest_json(
        version=sys.argv[1],
        notes=sys.argv[2],
        artifacts_dir=Path(".")
    )
```

### 4. 手動發布流程

```bash
# 1. 建置所有平台
# （在 Windows 機器）
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/worktime-pomodoro.key)"
npm run tauri:build

# （在 Linux 機器）
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/worktime-pomodoro.key)"
npm run tauri:build

# 2. 產生 latest.json
python3 scripts/generate_latest_json.py v0.2.0 "修正睡眠恢復問題"

# 3. 上傳到發布伺服器
# GitHub Release:
gh release create v0.2.0 \
  src-tauri/target/release/bundle/**/*.msi.zip \
  src-tauri/target/release/bundle/**/*.msi.zip.sig \
  src-tauri/target/release/bundle/**/*.AppImage.tar.gz \
  src-tauri/target/release/bundle/**/*.AppImage.tar.gz.sig \
  latest.json

# 或上傳到內部 HTTP 伺服器
scp latest.json *.zip *.sig user@internal-server:/var/www/updates/
```

## 安全性考量

### 1. 簽章驗證

- Tauri updater 會自動驗證 `.sig` 檔案
- 使用 minisign（與 signify 相容）
- 簽章不符會拒絕安裝

### 2. HTTPS 傳輸

- 更新 URL 必須使用 HTTPS
- 避免中間人攻擊

### 3. 私鑰保護

- 絕不提交到版本控制
- CI/CD 使用加密的 secrets
- 定期輪換金鑰（需重新發布公鑰）

### 4. 端點驗證

- 允許使用者設定更新 URL（支援內網）
- 但應驗證 URL 格式與協議

## 測試更新流程

### 1. 本機測試

```bash
# 建置舊版本
git checkout v0.1.0
npm run tauri:build
# 安裝並執行

# 建置新版本
git checkout v0.2.0
npm run tauri:build

# 設定本機 HTTP 伺服器
cd src-tauri/target/release/bundle
python3 -m http.server 8000

# 產生 latest.json 指向 localhost:8000
# 在應用程式設定中改 endpoint 為 http://localhost:8000

# 點選「檢查更新」測試
```

### 2. 模擬斷網更新

```bash
# 下載更新期間斷網，應顯示錯誤並可重試
# 驗證未完成安裝時資料庫不受影響
```

### 3. 驗證資料完整性

```bash
# 更新前：插入測試事件
# 更新後：確認事件仍存在
# 確認 Working 任務已正確暫停
```

## 疑難排解

### Q: 簽章驗證失敗

**A**: 確認 `tauri.conf.json` 的 `pubkey` 與私鑰匹配：

```bash
# 從私鑰提取公鑰
npx @tauri-apps/cli signer sign --private-key ~/.tauri/worktime-pomodoro.key --password ''
```

### Q: Windows Defender 阻擋更新

**A**:

1. 使用 EV 程式碼簽章憑證（需購買）
2. 或引導使用者新增例外

### Q: 更新後資料遺失

**A**:

1. 檢查 WAL checkpoint 是否成功
2. 檢查備份檔 `worktime.db.backup_*`
3. 從備份還原

### Q: 私有 GitHub repo 無法下載

**A**:

1. 使用 GitHub Personal Access Token
2. 或改用內部 HTTP 伺服器託管更新

## 進階：多通道更新

支援穩定版 / Beta 版：

```rust
pub enum UpdateChannel {
    Stable,
    Beta,
}

impl UpdateChannel {
    pub fn endpoint(&self) -> &str {
        match self {
            Self::Stable => "https://updates.example.com/stable/latest.json",
            Self::Beta => "https://updates.example.com/beta/latest.json",
        }
    }
}
```

使用者可在設定中選擇更新通道。

---

**文件版本**: v1.0  
**最後更新**: 2026-10-21  
**狀態**: 實作藍圖（待 Tauri 編譯環境可用時實作）
