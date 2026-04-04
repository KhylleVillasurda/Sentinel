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

        let conn = Connection::open(&db_path)?;

        // SQLCipher requires the key before any other operation.
        // Pass it as a raw hex blob: PRAGMA key = "x'<hex>'";
        let key = load_or_create_key();
        let hex_key = encode_hex(&key);
        conn.execute_batch(&format!("PRAGMA key = \"x'{hex_key}'\";\n"))?;

        // Scrub SQLCipher key pages from memory on connection close.
        conn.execute_batch("PRAGMA cipher_memory_security = ON;\n")?;

        run_migrations(&conn)?;

        Ok(Self { conn, key })
    }
}

// ---------------------------------------------------------------------------
// Schema migrations
// ---------------------------------------------------------------------------

/// Runs all schema migrations in order. Every statement is idempotent
/// (`CREATE TABLE IF NOT EXISTS`, etc.) so this safely re-runs on every startup.
///
/// TODO (Phase 4): add a migrations table and version-gate statements if the
/// schema grows complex enough to warrant it.
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

        CREATE INDEX IF NOT EXISTS idx_payloads_synced
            ON payloads (synced, received_at);
        ",
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Encodes a byte slice as a lowercase hex string.
/// Avoids pulling in the `hex` crate — keeps dependencies lean.
fn encode_hex(bytes: &[u8]) -> String {
    // Pre-allocate exactly 2 chars per byte to avoid reallocation.
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        // write! into a String is infallible — the unwrap is unreachable.
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
    }
    out
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

    #[test]
    fn encode_hex_produces_correct_output() {
        assert_eq!(encode_hex(&[0x00, 0xFF, 0xAB]), "00ffab");
        assert_eq!(encode_hex(&[0u8; 4]), "00000000");
    }
}
