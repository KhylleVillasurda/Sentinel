use rusqlite::Connection;
use std::path::Path;

use crate::crypto::load_or_create_key;

/// Owned handle to the SQLCipher-encrypted SQLite database.
///
/// Holds both the open connection and the 32-byte key that was used to
/// unlock it — so callers never have to pass the key around separately.
///
pub mod queries;
pub struct Db {
    pub conn: Connection,
    pub key: [u8; 32],
}

impl Db {
    /// Opens (or creates) the encrypted database at `path`.
    ///
    /// Steps performed in order:
    /// 1. Open the file with rusqlite (file is created if absent).
    /// 2. Immediately apply the SQLCipher key via `PRAGMA key` —
    ///    any read/write before this would fail on an encrypted file.
    /// 3. Run `PRAGMA cipher_memory_security = ON` — wipes key material
    ///    from memory when the connection is closed.
    /// 4. Run schema migrations (idempotent CREATE TABLE IF NOT EXISTS).
    ///
    /// The `path` is intentionally a parameter so this function is testable
    /// without a Tauri app handle. In production, pass the Tauri app data dir
    /// resolved in `main.rs` or `commands.rs`.
    pub fn open(path: &Path) -> Result<Self, String> {
        // --- 1. Resolve the full DB file path ---
        let db_path = path.join("sentinel.db");

        // --- 2. Open connection ---
        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database at {db_path:?}: {e}"))?;

        // --- 3. Load key and apply it via PRAGMA ---
        // SQLCipher requires the key to be set before any other operation.
        // We pass it as a raw hex blob: PRAGMA key = "x'<hex>'";
        let key = load_or_create_key();
        let hex_key = encode_hex(&key);

        conn.execute_batch(&format!("PRAGMA key = \"x'{hex_key}'\";"))
            .map_err(|e| format!("Failed to apply SQLCipher key: {e}"))?;

        // --- 4. Harden memory security ---
        // Scrubs SQLCipher key pages from memory on connection close.
        conn.execute_batch("PRAGMA cipher_memory_security = ON;")
            .map_err(|e| format!("Failed to set cipher_memory_security: {e}"))?;

        // --- 5. Run migrations ---
        run_migrations(&conn)?;

        Ok(Self { conn, key })
    }
}

// ---------------------------------------------------------------------------
// Schema migrations
// ---------------------------------------------------------------------------

/// Runs all schema migrations in order. Every statement must be idempotent
/// (CREATE TABLE IF NOT EXISTS, etc.) so this can safely run on every startup.
///
/// TODO (Phase 4): add a migrations table and version-gate statements if the
/// schema grows complex enough to warrant it.
fn run_migrations(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS payloads (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            device_id       TEXT    NOT NULL,
            encrypted_blob  BLOB    NOT NULL,   -- output of crypto::encrypt_payload()
            received_at     INTEGER NOT NULL,   -- Unix timestamp (seconds)
            synced          INTEGER NOT NULL DEFAULT 0  -- 0 = pending, 1 = synced
        );

        CREATE INDEX IF NOT EXISTS idx_payloads_synced
            ON payloads (synced, received_at);
        ",
    )
    .map_err(|e| format!("Migration failed: {e}"))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Encodes a byte slice as a lowercase hex string without pulling in the
/// `hex` crate — keeps dependencies lean.
fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Creates a fresh temporary directory for each test and cleans it up
    /// afterward. This keeps tests hermetic — no shared state between runs.
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

        // Open twice — second open re-runs migrations on the existing file.
        // If migrations are not idempotent this will panic.
        Db::open(&dir).expect("first open must succeed");
        Db::open(&dir).expect("second open (idempotent migrations) must succeed");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn payloads_table_exists_after_open() {
        let dir = temp_dir();
        let db = Db::open(&dir).expect("Db::open must succeed");

        // Try to INSERT a dummy row — if the table doesn't exist this panics.
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
