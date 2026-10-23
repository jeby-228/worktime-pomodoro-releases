# 番茄工時 UI 設計規格（3 個方向，皆含淺色／深色）

檔案：`tokens-{A,B,C}.css`（CSS 變數，用 `[data-theme="light|dark"]` 切換；沒設 data-theme 時跟系統 `prefers-color-scheme`）、`A/B/C.png`（02 計時 + 05 今天，淺／深對照）、`demo.css` + `gen.py`（產生預覽用）。

## 三個方向

|                | 個性                                                | 主色 light / dark     | 圓角 sm/md/lg |
| -------------- | --------------------------------------------------- | --------------------- | ------------- |
| **A 番茄暖橘** | 延續 v1 草圖，溫暖、有番茄感、親切                  | `#C2410C` / `#FF8A50` | 6/10/14       |
| **B 石板冷灰** | 冷靜中性、像 VS Code／Linear 的開發工具，資訊密度高 | `#2563EB` / `#6EA8FE` | 4/6/8         |
| **C 抹茶森林** | 低刺激綠色、專注安定感，最不刺眼                    | `#2F6F4A` / `#6BCB8B` | 8/12/18       |

## 語意色（hex：light → dark）

| token         | A                 | B                 | C                 |
| ------------- | ----------------- | ----------------- | ----------------- |
| bg            | #FBF7F3 → #1A1412 | #F6F7F9 → #0F1217 | #F4F6F1 → #111713 |
| surface       | #FFFFFF → #241C18 | #FFFFFF → #171B22 | #FFFFFF → #19211C |
| surface-2     | #F4ECE5 → #2E2420 | #EEF1F4 → #1F2530 | #E9EEE4 → #212B25 |
| text          | #2B211C → #F5EDE7 | #1F2937 → #E6E9EE | #1E2A22 → #E5EDE6 |
| text-2        | #6B5A50 → #BBA99E | #525C6B → #9AA4B2 | #52625A → #9DB0A3 |
| border        | #E6D9CF → #3D312B | #D9DEE5 → #2A313C | #D6DED0 → #2C3830 |
| accent / work | #C2410C → #FF8A50 | #2563EB → #6EA8FE | #2F6F4A → #6BCB8B |
| interrupt     | #BE185D → #F472B6 | #C2410C → #FB923C | #C2410C → #FB8B5A |
| meeting       | #6D28D9 → #B39DFA | #7C3AED → #B692F6 | #3B5BDB → #91A7FF |
| lunch         | #2F7D32 → #7BCB7F | #0F766E → #2DD4BF | #7E57C2 → #B39DDB |
| gap / warning | #A16207 → #F2C14E | #A16207 → #FACC15 | #A16207 → #F2C14E |
| success       | #2F7D32 → #7BCB7F | #15803D → #4ADE80 | #2F6F4A → #6BCB8B |
| danger        | #B91C1C → #F87171 | #DC2626 → #F87171 | #C53030 → #F28B82 |

每色另有 `--c-*-soft`（color-mix 淺底，light 14%、dark 22%）作為底色。主按鈕文字 `--c-on-accent`：light 用白、dark 用 bg 深色。

**對比檢查**：所有前景語意色（文字、副文字、accent、work、interrupt、meeting、lunch、gap、success、danger）在 bg 與 surface 上，以及 on-accent 在 accent 上，全部 ≥ 4.5:1（WCAG AA 一般文字）。最低值為 light 的 gap ≈4.9、B 的 danger ≈4.8。

## 字體與尺寸

- 內文：`Noto Sans TC`。計時、時間、數字用 `JetBrains Mono`（備援 Noto Sans Mono），並加 `font-variant-numeric: tabular-nums`，避免數字跳動。
- 字級：xs 12 / sm 13 / md 15（內文）/ lg 18（任務名）/ xl 22（環內時間）/ timer 56（大計時）。字重 400/500/700，行高 1.25 / 1.5。
- 間距：4 / 8 / 12 / 16 / 24 / 32 / 48（`--sp-1..7`）。
- 陰影：shadow-1 卡片、shadow-2 浮層、shadow-3 dialog／toast；dark 模式陰影加深，並靠 border 分層。
- Focus：`--focus-ring` 3px accent 40%，所有可互動元件必須有。

## 元件規則

- **Button**：高 40，radius-md。primary 用 accent 底配 on-accent 字，每個畫面最多一個。secondary 用 surface 底加 border。danger 用透明底配 danger 字（不給實心，避免搶眼），刪除一定要再確認。插隊鍵用 interrupt 虛線框加 soft 底，並標示快捷鍵 kbd。
- **Input**：高 40，surface 底、border；focus 時 border 換 accent 並加 focus-ring；錯誤時 border 用 danger，下方附 12px 說明。
- **Chip**：pill，surface-2 底、text-2 字；選中時用 accent-soft 底、accent 字。
- **List row**：最小高 44，hover 用 surface-2，時間欄用 mono 加 text-2。
- **Timeline slot**：左側 4px 色條代表類型（work、meeting、lunch 用 soft 底）。interrupt 縮排 24px 並用 soft 底。gap 用 1.5px 虛線 gap 色，透明底，點擊即補登。進行中的 slot 色條加粗，並標示「進行中」。
- **Dialog**：radius-lg，shadow-3，遮罩為黑 40%（dark 60%）；按鈕依序為 danger（左）、secondary、primary（右）。
- **Toast**：反色（text 底配 bg 字），底部置中，3 秒後消失，可附「復原」連結。
- **Tray 顏色**：work 色＝計時中，interrupt 色＝插隊中，gap 色＝未記錄或閒置，lunch 色＝午休，灰＝停止；圖示上顯示剩餘分鐘數。顏色一律用該方向的 dark 值，確保在深色工作列上清楚。

## 建議

推薦 **A 番茄暖橘**：與品牌名和 v1 草圖一致，插隊用桃紅與工作橘也分得開。若想更低調耐看，B 是次選。

---

## D shadcn/ui 鋅灰（zinc）

**個性**：shadcn/ui 的極簡中性風格，黑白灰為主，番茄橘只用在點綴（進度環、時段色條、tray、狀態點），看起來像現代 SaaS／開發者工具。檔案：`tokens-D.css`、`D.png`（01 快速輸入、02 計時、05 今天，各有淺／深）、`demo-D.css`、`gen-D.py`。

- **Token**：沿用 shadcn 官方名稱與 zinc 預設 HSL triplet，用法為 `hsl(var(--primary))`。包含 `--background --foreground --card --popover --primary --secondary --muted --accent --destructive --border --input --ring`（各自附 `-foreground`），`--radius: 0.5rem`。切換方式為 `.dark` class 或 `[data-theme="dark"]`，兩者都支援。
- **App 語意色（light → dark）**：work #C2410C → #FB923C、interrupt #BE185D → #F472B6、meeting #6D28D9 → #A78BFA、lunch #0F766E → #2DD4BF、gap/warning #9A5B06 → #FACC15、success #15803D → #4ADE80，另有 `--destructive-text`（#C62828 → #F87171）作為紅字專用。
- **對比**：所有語意色在背景上 ≥ 4.8:1（light 最低為 work 4.9、muted-foreground 4.8）。
  - 和官方值不同的地方：light 的 `--destructive` 由官方 `0 84.2% 60.2%` 改為 `0 72.2% 50.6%`（#DC2626），白字對比從 3.8 提升到 4.6:1，以符合 AA。
  - 官方 muted-foreground 放在 muted 底上只有 4.4:1，只用於 ≥ 14px 的輔助字或在白底上使用。
- **字體**：Noto Sans TC（備援 Inter），計時數字用 JetBrains Mono／Geist Mono 加 tabular-nums。
- **元件（shadcn 慣用法）**：
  - **Button**：default（主動作「結束」）、secondary、outline（暫停）、ghost（工具列）、destructive（刪除，需再確認）。插隊鍵為 outline 變體，加 interrupt 虛線框與 8% 底色。
  - **Input**：高 36（h-9），focus 時顯示 ring。
  - **Badge**：最近任務的 chip 用 secondary，區塊類型用 outline。
  - **Card**：圓角為 radius + 4px，配淡陰影。
  - **Tabs**：計時／今天，放在 muted 底的 segmented 樣式內。
  - **主題切換**：header 右側的 outline icon button，淺色時顯示 ☾、深色時顯示 ☀，可在 light、dark、system 間循環。
  - **Command（cmdk）**：用於 Ctrl+Alt+T 快速輸入，結構為搜尋列、群組（最近任務／快速區塊）、選中項用 accent 底，底部列出鍵盤提示。
  - **Dialog**：popover 底，遮罩為黑 50%。
  - **Toast（sonner 風格）**：右下浮動卡，內容為標題與描述，附「復原」按鈕，4 秒後消失。
  - **Timeline slot**：border 卡片加左側 3px 語意色條；gap 用虛線；插隊縮排。
- **實作提示**：shadcn-svelte 可直接用於 Astro + Svelte islands（bits-ui、Tailwind、cmdk 對應 `Command` 元件，sonner 對應 `svelte-sonner`）。這份 token 可直接貼進 `app.css`。

**更新建議**：如果使用者喜歡 shadcn 的感覺，D 最適合當實作基準，因為元件現成、維護成本最低。

---

## D-mono 純單色 shadcn（zinc，無番茄色）

**個性**：完全黑白灰，像 shadcn 文件本身，安靜、專業、不搶注意力。類型靠 **icon＋標籤＋灰階的填色／外框／虛線** 區分，不靠色相。檔案：`tokens-D-mono.css`、`D-mono.png`（01、02、03、05，各有淺／深）、`demo-D-mono.css`、`gen-D-mono.py`。

- **Token**：shadcn 官方 zinc 名稱與值（同 D，light 的 destructive 一樣調成 AA 版）。移除 work／interrupt 的色相，只新增 `--warning`。
- **類型編碼（lucide icon＋Item 變體）**：

| 類型       | Icon          | Item variant                              | ItemMedia                                       |
| ---------- | ------------- | ----------------------------------------- | ----------------------------------------------- |
| 工作       | briefcase     | outline                                   | **實心**（primary 底、反白 icon），視覺權重最高 |
| 插隊       | zap           | outline，縮排 26px，加「插隊」badge       | 外框                                            |
| 會議       | users         | muted                                     | 灰底                                            |
| 午休       | coffee        | muted，標題也用 muted-foreground          | 灰底                                            |
| 未記錄空檔 | circle-dashed | **虛線**框                                | 虛線                                            |
| 進行中     | —             | outline 加 default（實心）badge「進行中」 | 實心                                            |

- **只在兩處用顏色，並說明理由**：
  1. **destructive**：刪除等不可逆動作，紅色是使用者都懂的警示，不可省略。
  2. **warning（琥珀 #915108 / #FBBF24，只用在「未記錄 N 分」這段文字）**：空檔是整個 app 唯一「需要使用者採取行動」的狀態（下班前要補登）。灰階虛線在一長串灰色 Item 裡容易被略過，所以只把文字染一點色，背景與框線仍是灰的。

  除此之外，進度環、主按鈕、計時數字都用 foreground 色。

- **對比**：foreground ≈ 19.9:1；muted-foreground 在 background 上 4.8:1（dark 7.8:1）；warning 在白底上 6.4:1、在 muted 底上 5.8:1、dark 為 11.8:1；primary-foreground 在 primary 上 17:1；destructive 4.6:1。全部達到 AA。類型辨識不依賴顏色，對色盲友善。
- **Item 用法**：
  - 最近任務、快速區塊（Command palette 內）用 size sm，選中項用 accent 底。
  - 「接下來」清單用 outline／muted。
  - 時間軸的每個 slot 都是一個 Item：ItemMedia 放 icon，ItemTitle 為任務名，ItemDescription 為時間，ItemActions 放時長 badge 或「補登」按鈕。
  - 下班前檢查（gap review）同樣是一列 Item，dashed 變體，ItemActions 為「補登」與「忽略」。
- **其他元件**：
  - 進度環：foreground 弧線，配 muted 軌道。
  - 插隊鍵：outline dashed 加 zap icon。
  - 03 插隊畫面：上方是被暫停的工作（muted Item），中間大 mono 計時，下方兩個選填欄位，主按鈕為「回到原本工作」。
  - Toast（sonner 風格）：check icon 加「復原」outline 按鈕。
  - 主題切換按鈕：淺色顯示 moon、深色顯示 sun。
- **Tray**：同樣單色，用圖示形狀區分：實心圓加分鐘數＝工作；閃電＝插隊；虛線圓＝未記錄或閒置；杯子＝午休。
- **實作**：shadcn-svelte 已有 Item 元件，可用於 Astro + Svelte islands，icon 用 `@lucide/svelte`。

**更新建議**：使用者偏好單色的話，以 D-mono 為準，設計最一致、最耐看；D（有番茄點綴）作為備案。
