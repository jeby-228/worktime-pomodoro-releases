# V2 Simple Pomodoro - 開發進度

## ✅ 已完成

### Phase 1: 後端編譯與測試
- [x] 修復 MigrationManager API 調用
- [x] 修正模組導入結構 (main.rs / commands_v2.rs / lib.rs)
- [x] 更新命令處理器列表
- [x] StateManager 正確初始化
- [x] 所有 40 個單元測試通過
- [x] 提交 Cargo.lock

### 資料庫與後端核心
- [x] SQLite schema (time_slots, recent_people, recent_tasks, app_state)
- [x] StoreV2 CRUD operations
- [x] StateManager 狀態機 (Idle/Working/Interrupt/Paused)
- [x] 插隊邏輯與 parent_id 關聯
- [x] 日期範圍查詢、任務彙總、Gap 檢測
- [x] Excel/CSV 匯出 (rust_xlsxwriter)
- [x] 所有後端 commands 實作完成

### i18n 系統
- [x] 5 語言 (zh-TW, zh-CN, en, vi, th)
- [x] 動態載入、熱切換
- [x] 所有 locale keys 一致性

### 桌面佈局架構
- [x] 視窗配置更新為 960x640 (最小 800x560, 可調整大小)
- [x] Sidebar.svelte 左側可收合導航 (16px 圖標 / 224px 展開)
- [x] DesktopLayout.svelte 主佈局容器
- [x] 響應式設計基礎

### 計時·今天雙面板儀表板
- [x] TimerCard.svelte 大數字計時器卡片
- [x] TodayTimeline.svelte 今天時間軸 + 統計摘要
- [x] index.astro 雙面板佈局 (384px timer + flex-1 timeline)
- [x] 狀態指示、控制按鈕、Gap 警告

## 🚧 進行中 / 待實作

### Phase 2: 主要 UI 頁面
- [ ] 紀錄頁面 (records.astro)
  - [ ] 全寬數據表格 + 工具列
  - [ ] 依任務彙總側邊欄
  - [ ] 日期篩選、搜尋、類型篩選
  - [ ] 匯出對話框（欄位選擇、格式選擇）
- [ ] 設定頁面 (settings.astro)
  - [ ] 兩欄佈局 (section list + form)
  - [ ] 一般：語言（跟隨系統）、主題（淺/深/系統）、開機自啟動
  - [ ] 計時：番茄鐘時長、閒置偵測閾值
  - [ ] 時程：午休時間、下班時間、EOD 檢查
  - [ ] 快捷鍵設定
  - [ ] 迷你模式：none/mini/pet/both
  - [ ] 寵物匯入、預設匯出
  - [ ] 關於 / 更新

### Phase 3: 互動對話框與抽屜
- [ ] 快速輸入 palette (E-quick-input，~560x360 cmdk style)
  - [ ] 搜尋/輸入任務名稱
  - [ ] 最近任務 chips
  - [ ] 快速區塊（開始工作、插隊、會議...）
  - [ ] Ctrl+Alt+T 全域快捷鍵
- [ ] 插隊 UI
  - [ ] 誰？（最近聯絡人 chips）
  - [ ] 原因？（選填）
  - [ ] 暫停當前任務並開始插隊 slot
- [ ] Slot 編輯抽屜 (D-edit-sheet，右側滑出)
  - [ ] 類型選擇 (work/interrupt/meeting/lunch)
  - [ ] 任務名稱、單號
  - [ ] 時間範圍
  - [ ] 備註
  - [ ] 刪除按鈕、儲存 Ctrl+Enter
- [ ] 空閒恢復 prompt
  - [ ] "離開 N 分鐘，要算成："
  - [ ] 選項：繼續原任務/午休/會議/不算
- [ ] EOD review 對話框
  - [ ] 今天有 N 個未記錄時段
  - [ ] Gap 列表 + 一鍵補登

### Phase 4: 系統整合
- [ ] 系統托盤
  - [ ] 圖標顯示狀態 + 分鐘數
  - [ ] 選單：顯示/隱藏、快速輸入、插隊、退出
- [ ] 全域快捷鍵
  - [ ] Ctrl+Alt+T 快速輸入
  - [ ] Ctrl+Alt+I 插隊
  - [ ] 可自訂
- [ ] 自訂通知 toasts
  - [ ] 任務開始/結束/暫停/繼續
  - [ ] 插隊開始/返回
  - [ ] 番茄鐘完成
  - [ ] 空閒偵測、EOD 檢查

### Phase 5: 迷你 Bar
- [ ] 240x48 always-on-top window
- [ ] 可拖曳、懸停淡入/淡出
- [ ] 顯示任務名稱 + 倒數
- [ ] 暫停/插隊按鈕

### Phase 6: 桌面寵物系統
- [ ] Codex .codex-pet.zip 加載器
  - [ ] pet.json metadata parser
  - [ ] 1536x1872 spritesheet (8x9 cells of 192x208)
- [ ] 狀態動畫映射
  - [ ] working, interrupt, idle, lunch, unrecorded, done
- [ ] 預設「番茄君」spritesheet
  - [ ] 從 design-system/pet/*.svg 建構
- [ ] 匯入自訂寵物

### Phase 7: 測試、打包、文件
- [ ] 單元測試 (Svelte components)
- [ ] E2E 測試 (關鍵流程)
- [ ] npm run tauri:build 生成 .deb
- [ ] 光/暗主題截圖所有畫面
- [ ] 更新 README 文件

## 架構決策

### 視窗大小
- 主視窗：960x640 (最小 800x560)，可調整大小、記憶位置
- 迷你 Bar：240x48 always-on-top
- 桌面寵物：192x208（單幀）
- 快速輸入：~560x360 居中對話框

### 佈局模式
- **計時·今天** (index.astro)：雙面板（timer card 384px + today timeline flex-1）
- **紀錄** (records.astro)：全寬表格 + 右側摘要面板（可收合）
- **設定** (settings.astro)：兩欄（左側 section list ~200px + 右側表單）
- **窄屏 (<800px)**：側邊欄自動收合為圖標模式

### 顏色與主題
- D-mono zinc tokens (pure monochrome)
- 唯一例外：
  - destructive red
  - warning amber (#915108 / #FBBF24) 用於 gaps / unrecorded text

### 技術堆疊
- **後端**: Tauri 2, Rust 1.99, SQLite (rusqlite + rusqlite_migration), rust_xlsxwriter
- **前端**: Astro 4.16, Svelte 5 (Runes), Tailwind CSS 3.4, lucide-svelte, clsx, tailwind-merge
- **i18n**: 自建 Svelte store 系統，動態 JSON

## 下一步建議

1. 完成紀錄頁面（表格 + 篩選 + 匯出對話框）
2. 完成設定頁面（兩欄表單 + 所有配置項）
3. 實作快速輸入 cmdk palette
4. 實作 Slot 編輯抽屜
5. 實作插隊、空閒恢復、EOD review 對話框
6. 整合後端 Tauri commands (invoke)
7. 實作系統托盤 + 全域快捷鍵
8. 實作迷你 Bar 視窗
9. 實作桌面寵物系統
10. 測試、打包 .deb、截圖

## 估算完成度

- 資料庫 & 後端核心: **100%** ✅
- i18n 系統: **100%** ✅
- 跨平台配置: **100%** ✅ (Windows 離線 WebView2, Linux WebKitGTK 4.1, Wayland 降級)
- 桌面佈局架構: **90%** (待整合主題切換)
- 計時·今天頁面: **80%** (UI 完成 D-mono Item 風格，待連接後端)
- 紀錄頁面: **60%** (列表視圖完成，待表格視圖、篩選、匯出對話框)
- 設定頁面: **0%**
- 對話框 & 抽屜: **0%**
- 系統整合: **0%**
- 迷你模式: **0%**
- 測試 & 打包: **0%**

**總體完成度**: ~42%

---

*Last updated: 2026-10-22*
