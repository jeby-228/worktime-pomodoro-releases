use anyhow::{Context, Result};
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

const MAX_BACKUPS: usize = 5;

/// Database migrations manager
pub struct MigrationManager {
    migrations: Migrations<'static>,
}

impl MigrationManager {
    /// Create a new migration manager with embedded migrations
    pub fn new() -> Result<Self> {
        let migrations = Migrations::new(vec![
            M::up(include_str!("../migrations/0001_init.sql")),
            M::up(include_str!("../migrations/0002_v2_slots.sql")),
            M::up(include_str!("../migrations/0003_categories.sql")),
        ]);

        Ok(Self { migrations })
    }

    /// Get the current schema version the app knows about
    pub fn latest_version(&self) -> usize {
        3
    }

    /// Run migrations on a database connection
    pub fn migrate(&self, conn: &mut Connection, db_path: Option<&Path>) -> Result<()> {
        // Get current user_version
        let current_version: usize = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .context("Failed to read user_version")?;

        let latest_version = self.latest_version();

        info!(
            "Database version: {}, App supports up to: {}",
            current_version, latest_version
        );

        // Check if database is from a newer version (user downgraded)
        if current_version > latest_version {
            error!(
                "Database schema version {} is newer than app supports ({}). Please update the application.",
                current_version, latest_version
            );
            anyhow::bail!(
                "Database version {} is too new. This app only supports up to version {}. Please update the application and do not downgrade.",
                current_version,
                latest_version
            );
        }

        // No migration needed
        if current_version == latest_version {
            info!("Database is up to date");
            return Ok(());
        }

        // Backup before migration. Checkpoint first: recent rows may still be in
        // the WAL file, which a plain file copy would miss.
        if let Some(path) = db_path {
            if let Err(err) = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())) {
                warn!("WAL checkpoint before backup failed: {}", err);
            }
            self.backup_database(path)?;
        }

        // Run migrations
        info!(
            "Migrating database from version {} to {}...",
            current_version, latest_version
        );

        self.migrations
            .to_latest(conn)
            .context("Failed to apply migrations")?;

        let new_version: usize = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .context("Failed to read user_version after migration")?;

        info!(
            "Migration complete. Database now at version {}",
            new_version
        );

        Ok(())
    }

    /// Create a timestamped backup of the database file
    fn backup_database(&self, db_path: &Path) -> Result<()> {
        if !db_path.exists() {
            return Ok(()); // Nothing to backup
        }

        let db_dir = db_path.parent().unwrap_or_else(|| Path::new("."));
        let db_filename = db_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("worktime.db");

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S%.3f");
        let backup_filename = format!("{}.backup_{}", db_filename, timestamp);
        let backup_path = db_dir.join(&backup_filename);

        info!("Creating database backup: {}", backup_path.display());

        fs::copy(db_path, &backup_path)
            .with_context(|| format!("Failed to backup database to {}", backup_path.display()))?;

        // Clean up old backups (keep last MAX_BACKUPS)
        self.cleanup_old_backups(db_dir, db_filename)?;

        Ok(())
    }

    /// Remove old backup files, keeping only the last MAX_BACKUPS
    fn cleanup_old_backups(&self, db_dir: &Path, db_filename: &str) -> Result<()> {
        let backup_prefix = format!("{}.backup_", db_filename);

        let mut backups: Vec<PathBuf> = fs::read_dir(db_dir)?
            .filter_map(|entry| entry.ok())
            .map(|e| e.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.starts_with(&backup_prefix))
                    .unwrap_or(false)
            })
            .collect();

        if backups.len() <= MAX_BACKUPS {
            return Ok(());
        }

        // Sort by modification time (oldest first)
        backups.sort_by_key(|path| {
            fs::metadata(path)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });

        // Remove oldest backups
        let to_remove = backups.len() - MAX_BACKUPS;
        for path in backups.iter().take(to_remove) {
            info!("Removing old backup: {}", path.display());
            if let Err(e) = fs::remove_file(path) {
                warn!("Failed to remove old backup {}: {}", path.display(), e);
            }
        }

        Ok(())
    }

    /// Check if a database can be opened (for read-only access when version is too new)
    pub fn check_version_compatibility(db_path: &Path) -> Result<VersionCheck> {
        let conn =
            Connection::open(db_path).context("Failed to open database for version check")?;

        let current_version: usize = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .context("Failed to read user_version")?;

        let manager = Self::new()?;
        let latest_version = manager.latest_version();

        Ok(VersionCheck {
            current: current_version,
            supported: latest_version,
            compatible: current_version <= latest_version,
        })
    }
}

impl Default for MigrationManager {
    fn default() -> Self {
        Self::new().expect("Failed to initialize migrations")
    }
}

#[derive(Debug, Clone)]
pub struct VersionCheck {
    pub current: usize,
    pub supported: usize,
    pub compatible: bool,
}

impl VersionCheck {
    pub fn needs_upgrade(&self) -> bool {
        self.current < self.supported
    }

    pub fn is_too_new(&self) -> bool {
        self.current > self.supported
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::TempDir;

    #[test]
    fn test_fresh_database_migration() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut conn = Connection::open(&db_path).unwrap();
        let manager = MigrationManager::new().unwrap();

        manager.migrate(&mut conn, Some(&db_path)).unwrap();

        // Check that tables exist
        let table_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('events', 'tasks_cache', 'segments', 'settings')",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(table_count, 4);

        // Check version
        let version: usize = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();

        assert_eq!(version, manager.latest_version());
    }

    #[test]
    fn test_migration_preserves_data() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create initial database
        let mut conn = Connection::open(&db_path).unwrap();
        let manager = MigrationManager::new().unwrap();
        manager.migrate(&mut conn, Some(&db_path)).unwrap();

        // Insert some test data
        conn.execute(
            "INSERT INTO events (uuid, ts, mono_ms, boot_id, kind, source) VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params!["test-uuid", "2026-10-21T10:00:00+08:00", 0, "boot-1", "start", "user"],
        ).unwrap();

        drop(conn);

        // Re-open and "migrate" again (should be no-op)
        let mut conn = Connection::open(&db_path).unwrap();
        manager.migrate(&mut conn, Some(&db_path)).unwrap();

        // Check data still exists
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();

        assert_eq!(count, 1);
    }

    fn database_at_v2(db_path: &Path) -> Connection {
        let conn = Connection::open(db_path).unwrap();
        conn.execute_batch(include_str!("../migrations/0001_init.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../migrations/0002_v2_slots.sql"))
            .unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
        conn
    }

    /// Builds a database as a v1.1.4 install left it, then upgrades it.
    fn upgrade_from_v2(db_path: &Path) -> Connection {
        let mut conn = database_at_v2(db_path);
        conn.execute(
            "INSERT INTO time_slots (id, uuid, task_name, slot_type, start_ts, end_ts, note)
             VALUES (1, 'u-1', '舊任務', 'work', '2026-10-21T09:00:00+08:00',
                     '2026-10-21T10:00:00+08:00', '舊備註'),
                    (2, 'u-2', '午休', 'lunch', '2026-10-21T12:00:00+08:00',
                     '2026-10-21T13:00:00+08:00', NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO app_state (key, value) VALUES ('pomodoro_minutes', '40')",
            [],
        )
        .unwrap();

        MigrationManager::new()
            .unwrap()
            .migrate(&mut conn, Some(db_path))
            .unwrap();
        conn
    }

    #[test]
    fn test_upgrade_keeps_old_slots_uncategorized() {
        let temp_dir = TempDir::new().unwrap();
        let conn = upgrade_from_v2(&temp_dir.path().join("test.db"));

        let version: usize = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 3);

        let (name, note, category): (String, String, Option<i64>) = conn
            .query_row(
                "SELECT task_name, note, category_id FROM time_slots WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "舊任務");
        assert_eq!(note, "舊備註");
        assert_eq!(category, None);

        let categorized: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM time_slots WHERE category_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(categorized, 0);

        let minutes: String = conn
            .query_row(
                "SELECT value FROM app_state WHERE key = 'pomodoro_minutes'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(minutes, "40");
    }

    #[test]
    fn test_upgrade_seeds_default_categories_in_order() {
        let temp_dir = TempDir::new().unwrap();
        let conn = upgrade_from_v2(&temp_dir.path().join("test.db"));

        let mut stmt = conn
            .prepare("SELECT name FROM categories WHERE hidden = 0 ORDER BY sort_order")
            .unwrap();
        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            names,
            ["實作", "問題處理", "規格討論", "資料調整", "文件維護"]
        );
    }

    #[test]
    fn test_upgrade_backup_includes_rows_still_in_the_wal() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut conn = Connection::open(&db_path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        drop(conn);
        conn = database_at_v2(&db_path);
        conn.execute(
            "INSERT INTO time_slots (uuid, task_name, slot_type, start_ts)
             VALUES ('u-wal', 'WAL 內的紀錄', 'work', '2026-10-21T09:00:00+08:00')",
            [],
        )
        .unwrap();

        MigrationManager::new()
            .unwrap()
            .migrate(&mut conn, Some(&db_path))
            .unwrap();
        drop(conn);

        let backup = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .find(|path| path.to_string_lossy().contains(".backup_"))
            .unwrap();
        let backup = Connection::open(backup).unwrap();
        let count: i64 = backup
            .query_row(
                "SELECT COUNT(*) FROM time_slots WHERE uuid = 'u-wal'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_newer_version_rejection() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut conn = Connection::open(&db_path).unwrap();
        let manager = MigrationManager::new().unwrap();

        // Set a future version
        let future_version = manager.latest_version() + 10;
        conn.pragma_update(None, "user_version", future_version)
            .unwrap();

        // Attempt to migrate should fail
        let result = manager.migrate(&mut conn, Some(&db_path));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too new"));
    }

    #[test]
    fn test_backup_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create and migrate initial database (creates backup #1)
        let mut conn = Connection::open(&db_path).unwrap();
        let manager = MigrationManager::new().unwrap();
        manager.migrate(&mut conn, Some(&db_path)).unwrap();

        // Insert data
        conn.execute(
            "INSERT INTO events (uuid, ts, mono_ms, boot_id, kind, source) VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params!["test-uuid", "2026-10-21T10:00:00+08:00", 0, "boot-1", "start", "user"],
        ).unwrap();
        drop(conn);

        // Create a second backup manually (backup #2)
        manager.backup_database(&db_path).unwrap();

        // Check backups exist
        let backups: Vec<_> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.contains(".backup_"))
                    .unwrap_or(false)
            })
            .collect();

        // Should have 2 backups: one from migration, one manual
        assert_eq!(backups.len(), 2);
    }

    #[test]
    fn test_backup_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut conn = Connection::open(&db_path).unwrap();
        let manager = MigrationManager::new().unwrap();
        manager.migrate(&mut conn, Some(&db_path)).unwrap();
        drop(conn);

        // Create more than MAX_BACKUPS backups
        for _ in 0..MAX_BACKUPS + 3 {
            manager.backup_database(&db_path).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10)); // Ensure different timestamps
        }

        // Count backups
        let backup_count = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|s| s.contains(".backup_"))
                    .unwrap_or(false)
            })
            .count();

        assert_eq!(backup_count, MAX_BACKUPS);
    }

    #[test]
    fn test_version_check() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let mut conn = Connection::open(&db_path).unwrap();
        let manager = MigrationManager::new().unwrap();
        manager.migrate(&mut conn, Some(&db_path)).unwrap();
        drop(conn);

        let check = MigrationManager::check_version_compatibility(&db_path).unwrap();
        assert!(check.compatible);
        assert!(!check.needs_upgrade());
        assert!(!check.is_too_new());
    }
}
