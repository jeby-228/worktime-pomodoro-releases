# 自動更新系統設置指南

本應用程式使用 `tauri-plugin-updater` 實作簽名更新系統。以下是生產環境的完整設置流程。

## 生成簽名金鑰對

```bash
# 生成新的簽名金鑰對（私鑰和公鑰）
npm run tauri signer generate -- -w ~/.tauri/worktime-pomodoro.key

# 輸出示例：
# Private key: ~/.tauri/worktime-pomodoro.key
# Public key: dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEJBREVDQUZFRQ==
```

**重要**：
- 私鑰 (`~/.tauri/worktime-pomodoro.key`) 必須妥善保管，**絕不提交到版本控制**
- 公鑰將配置到 `src-tauri/tauri.conf.json`，用於驗證更新

## 配置公鑰

在 `src-tauri/tauri.conf.json` 中設置公鑰：

```json
{
  "plugins": {
    "updater": {
      "pubkey": "YOUR_PUBLIC_KEY_HERE",
      "endpoints": [
        "https://github.com/yourname/worktime-pomodoro/releases/latest/download/latest.json"
      ]
    }
  }
}
```

## 建置簽名更新套件

```bash
# 設置私鑰環境變數
export TAURI_SIGNING_PRIVATE_KEY=$(cat ~/.tauri/worktime-pomodoro.key)

# 或在 Windows：
# $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content ~/.tauri/worktime-pomodoro.key -Raw

# 建置應用程式（會自動簽名）
npm run tauri:build
```

建置完成後，在 `src-tauri/target/release/bundle/` 目錄下會生成：
- 安裝套件（.AppImage, .deb, .dmg, .msi 等）
- 簽名檔案（.sig）
- `latest.json` 更新清單

## latest.json 結構

```json
{
  "version": "0.1.0",
  "notes": "修正了資料庫遷移問題，提升穩定性",
  "pub_date": "2026-10-21T16:00:00Z",
  "platforms": {
    "linux-x86_64": {
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQ==",
      "url": "https://github.com/yourname/worktime-pomodoro/releases/download/v0.1.0/worktime-pomodoro_0.1.0_amd64.AppImage.tar.gz"
    },
    "windows-x86_64": {
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZQ==",
      "url": "https://github.com/yourname/worktime-pomodoro/releases/download/v0.1.0/worktime-pomodoro_0.1.0_x64_en-US.msi.zip"
    }
  }
}
```

## 發佈流程

1. **更新版本號**：在 `src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 中更新版本
2. **建置簽名套件**：執行上述建置命令
3. **上傳至伺服器**：
   - 將 `.AppImage` / `.msi` 等安裝檔上傳
   - 將對應的 `.sig` 簽名檔上傳
   - 將 `latest.json` 上傳到更新端點根目錄
4. **測試更新**：在舊版本應用中觸發「檢查更新」確認

## 應用內更新配置

用戶可在應用程式設定頁面自定義更新端點 URL：

```
設定 → 更新設定 → 更新伺服器網址
預設：https://github.com/yourname/worktime-pomodoro/releases/latest/download
```

## 更新流程說明

當用戶觸發更新時（手動或啟動時自動檢查）：

1. **檢查更新**：向 `{update_url}/latest.json` 發送請求
2. **驗證簽名**：使用公鑰驗證更新套件簽名
3. **暫停任務**：自動暫停當前工作任務
4. **WAL 檢查點**：執行 `PRAGMA wal_checkpoint(TRUNCATE)` 確保資料庫一致性
5. **下載安裝**：下載更新套件並安裝
6. **重啟應用**：自動重啟應用程式

## 私有倉庫注意事項

由於本專案為私有倉庫，需要：

1. **自託管更新伺服器**：
   - 可使用 GitHub Releases（需配置存取權限）
   - 或架設專用靜態檔案伺服器（推薦）

2. **驗證存取**：
   - 如使用 GitHub Releases，考慮使用 GitHub token
   - 或將更新檔案放在內網伺服器

3. **範例：使用內網伺服器**：
   ```json
   {
     "endpoints": [
       "https://updates.company.internal/worktime-pomodoro/latest.json"
     ]
   }
   ```

## 安全性

- ✅ 所有更新套件使用 Ed25519 簽名驗證
- ✅ 私鑰不包含在應用程式或版本控制中
- ✅ 更新前自動備份資料庫（WAL checkpoint）
- ✅ 更新失敗時可回滾（保留舊版本）

## 故障排除

### 更新檢查失敗
- 檢查網路連線
- 確認更新端點 URL 正確
- 查看應用程式日誌 (`~/.local/share/worktime-pomodoro/logs/`)

### 簽名驗證失敗
- 確認公鑰配置正確
- 確認使用對應私鑰簽名的更新套件
- 重新生成簽名檔案

### 更新下載失敗
- 檢查伺服器可訪問性
- 確認檔案權限正確
- 確認 latest.json 中的 URL 可訪問
