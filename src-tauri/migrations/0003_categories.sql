-- Work categories shared by work, meeting and interrupt slots
-- Version: 3

CREATE TABLE IF NOT EXISTS categories (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  name         TEXT NOT NULL UNIQUE,
  report_code  TEXT,                     -- Optional code written to exports
  sort_order   INTEGER NOT NULL DEFAULT 0,
  hidden       INTEGER NOT NULL DEFAULT 0, -- Hidden categories keep their history
  created_at   TEXT NOT NULL DEFAULT (datetime('now','localtime')),
  updated_at   TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS ix_categories_order ON categories(sort_order);

INSERT OR IGNORE INTO categories (name, sort_order) VALUES
  ('實作', 1),
  ('問題處理', 2),
  ('規格討論', 3),
  ('資料調整', 4),
  ('文件維護', 5);

-- Existing slots stay NULL, which the app shows as uncategorized.
ALTER TABLE time_slots ADD COLUMN category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS ix_slots_category ON time_slots(category_id);
