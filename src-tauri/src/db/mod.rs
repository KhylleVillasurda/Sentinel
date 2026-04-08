// db/mod.rs
// SQLCipher-encrypted database initialisation and schema migrations.

use rusqlite::Connection;
use std::path::Path;

use crate::crypto::load_or_create_key;
use crate::error::SentinelError;

pub mod queries;

/// Owned handle to the SQLCipher-encrypted SQLite database.
///
/// Holds both the open connection and the 32-byte key that was used to
/// unlock it — so callers never have to pass the key around separately.
pub struct Db {
    pub conn: Connection,
    pub key: [u8; 32],
}

impl Db {
    /// Opens (or creates) the encrypted database at `path`.
    ///
    /// Steps performed in order:
    /// 1. Open the file with rusqlite (created if absent).
    /// 2. Apply the SQLCipher key via `PRAGMA key` — must happen before any
    ///    other read/write on an encrypted file.
    /// 3. Enable `cipher_memory_security` to wipe key material from memory
    ///    when the connection closes.
    /// 4. Run schema migrations (idempotent `CREATE TABLE IF NOT EXISTS`).
    ///
    /// `path` is a parameter so this function is testable without Tauri.
    /// In production, pass the Tauri app data dir resolved in `main.rs`.
    pub fn open(path: &Path) -> Result<Self, SentinelError> {
        let db_path = path.join("sentinel.db");
        println!("[db] Opening database at: {:?}", db_path);

        let conn = Connection::open(&db_path)?;

        // SQLCipher requires the key before any other operation.
        // Pass it as a raw hex blob: PRAGMA key = \"x'<hex>'\";
        println!("[db] Loading encryption key from system keychain...");
        let key = load_or_create_key();
        let hex_key = hex::encode(key);
        
        println!("[db] Applying SQLCipher key...");
        if let Err(e) = conn.execute_batch(&format!("PRAGMA key = \"x'{hex_key}'\";\n")) {
            println!("[db] CRITICAL ERROR: Database is locked or key is incorrect. Error: {:?}", e);
            println!("[db] ACTION REQUIRED: If you manually deleted your system keychain or changed passwords, you must delete 'sentinel.db' to reset the gateway.");
            return Err(e.into());
        }

        // Test if the key worked by running a simple query
        if let Err(e) = conn.query_row("SELECT 1", [], |_| Ok(())) {
            println!("[db] CRITICAL ERROR: Key accepted by PRAGMA but database check failed. File might be corrupted or encrypted with a different key. Error: {:?}", e);
            return Err(e.into());
        }

        // Scrub SQLCipher key pages from memory on connection close.
        conn.execute_batch("PRAGMA cipher_memory_security = ON;\n")?;

        println!("[db] Key applied successfully. Running migrations...");
        run_migrations(&conn)?;

        println!("[db] Database initialization complete.");
        Ok(Self { conn, key })
    }
}

// ---------------------------------------------------------------------------
// Schema migrations
// ---------------------------------------------------------------------------

/// Runs all schema migrations in order. Every statement is idempotent
/// (`CREATE TABLE IF NOT EXISTS`, etc.) so this safely re-runs on every startup.
fn run_migrations(conn: &Connection) -> Result<(), SentinelError> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS payloads (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            device_id       TEXT    NOT NULL,
            encrypted_blob  BLOB    NOT NULL,
            received_at     INTEGER NOT NULL,
            synced          INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS devices (
            device_id       TEXT    PRIMARY KEY,
            friendly_name   TEXT    NOT NULL,
            token_hash      TEXT    NOT NULL,
            created_at      INTEGER NOT NULL,
            last_seen       INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_payloads_synced
            ON payloads (synced, received_at);
        ",
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sentinel_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn open_creates_db_file() {
        let dir = temp_dir();
        let db = Db::open(&dir).expect("Db::open must succeed");
        drop(db);
        assert!(
            dir.join("sentinel.db").exists(),
            "sentinel.db must exist after Db::open"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn migrations_are_idempotent() {
        let dir = temp_dir();
        Db::open(&dir).expect("first open must succeed");
        Db::open(&dir).expect("second open (idempotent migrations) must succeed");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn payloads_table_exists_after_open() {
        let dir = temp_dir();
        let db = Db::open(&dir).expect("Db::open must succeed");
        let result = db.conn.execute(
            "INSERT INTO payloads (device_id, encrypted_blob, received_at)
             VALUES (?1, ?2, ?3)",
            rusqlite::params!["test-device", b"blob".to_vec(), 0i64],
        );
        assert!(
            result.is_ok(),
            "INSERT into payloads must succeed: {result:?}"
        );
        fs::remove_dir_all(&dir).ok();
    }
}
