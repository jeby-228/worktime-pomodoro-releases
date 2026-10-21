-- Initial schema for worktime pomodoro
-- Version: 1

-- 事件：只新增，不修改、不刪除
CREATE TABLE IF NOT EXISTS events (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  uuid         TEXT NOT NULL UNIQUE,
  ts           TEXT NOT NULL,
  mono_ms      INTEGER NOT NULL,
  boot_id      TEXT NOT NULL,
  kind         TEXT NOT NULL,
  task_key     TEXT,
  ref_event_id INTEGER,
  note         TEXT,
  source       TEXT NOT NULL,
  synced       INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS ix_events_ts ON events(ts);
CREATE INDEX IF NOT EXISTS ix_events_synced ON events(synced);

-- 任務快取（從 API 撈或手動建立）
CREATE TABLE IF NOT EXISTS tasks_cache (
  task_key     TEXT PRIMARY KEY,
  plan_name    TEXT,
  stage_name   TEXT,
  process_name TEXT,
  task_name    TEXT,
  task_id      INTEGER,
  is_adhoc     INTEGER NOT NULL DEFAULT 0,
  is_closed    INTEGER NOT NULL DEFAULT 0,
  last_used_at TEXT,
  fetched_at   TEXT
);

-- 推導結果（可隨時刪除重算）
CREATE TABLE IF NOT EXISTS segments (
  id           INTEGER PRIMARY KEY,
  task_key     TEXT,
  start_ts     TEXT NOT NULL,
  end_ts       TEXT,
  minutes      INTEGER,
  status       TEXT NOT NULL,
  synced       INTEGER NOT NULL DEFAULT 0
);

-- 設定
CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
