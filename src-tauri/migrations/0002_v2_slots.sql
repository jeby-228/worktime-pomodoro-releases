-- V2 Migration: Simplified time slot model
-- Version: 2

-- Drop old tables (preserving events as historical log if needed for migration)
-- We'll keep events table for now but stop using it

-- Time slots: the new source of truth
CREATE TABLE IF NOT EXISTS time_slots (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  uuid          TEXT NOT NULL UNIQUE,
  task_name     TEXT NOT NULL,           -- Simple task name (no complex hierarchy)
  slot_type     TEXT NOT NULL,           -- 'work', 'interrupt', 'meeting', 'lunch'
  start_ts      TEXT NOT NULL,           -- ISO8601 timestamp
  end_ts        TEXT,                    -- NULL for ongoing/planned slots
  note          TEXT,
  parent_id     INTEGER,                 -- For interrupts: points to the work slot being interrupted
  created_at    TEXT NOT NULL DEFAULT (datetime('now','localtime')),
  updated_at    TEXT NOT NULL DEFAULT (datetime('now','localtime')),
  
  FOREIGN KEY (parent_id) REFERENCES time_slots(id) ON DELETE SET NULL,
  CHECK (slot_type IN ('work', 'interrupt', 'meeting', 'lunch'))
);

CREATE INDEX IF NOT EXISTS ix_slots_start ON time_slots(start_ts);
CREATE INDEX IF NOT EXISTS ix_slots_end ON time_slots(end_ts);
CREATE INDEX IF NOT EXISTS ix_slots_type ON time_slots(slot_type);
CREATE INDEX IF NOT EXISTS ix_slots_parent ON time_slots(parent_id);

-- Recent people for interrupt quick input
CREATE TABLE IF NOT EXISTS recent_people (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  name       TEXT NOT NULL UNIQUE,
  last_used  TEXT NOT NULL,
  use_count  INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS ix_recent_people_used ON recent_people(last_used DESC);

-- Recent tasks for quick input
CREATE TABLE IF NOT EXISTS recent_tasks (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  task_name  TEXT NOT NULL UNIQUE,
  last_used  TEXT NOT NULL,
  use_count  INTEGER NOT NULL DEFAULT 1,
  slot_type  TEXT NOT NULL DEFAULT 'work'
);

CREATE INDEX IF NOT EXISTS ix_recent_tasks_used ON recent_tasks(last_used DESC);

-- App state: current active slot, settings
CREATE TABLE IF NOT EXISTS app_state (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- Migrate existing settings
INSERT OR IGNORE INTO app_state (key, value)
SELECT key, value FROM settings;

-- User preferences
INSERT OR IGNORE INTO app_state (key, value) VALUES 
  ('lunch_start', '12:00'),
  ('lunch_end', '13:00'),
  ('work_end', '17:20'),
  ('eod_check_enabled', 'true'),
  ('pomodoro_minutes', '25'),
  ('theme', 'system'),
  ('mini_mode', 'none'),
  ('pet_id', 'tomato-kun');
