use rusqlite::{Connection, Result as RusqliteResult};

/// A single row returned from the `payloads` table.
/// Used by `fetch_unsynced` so callers never write raw SQL outside this file.
#[derive(Debug, PartialEq)]
pub struct PayloadRow {
    pub id: i64,
    pub device_id: String,
    pub encrypted_blob: Vec<u8>,
    pub received_at: i64,
}

/// Inserts an encrypted payload blob into the `payloads` table.
///
/// `encrypted_blob` is the direct output of `crypto::encrypt_payload()` —
/// this function stores it as-is with no further transformation.
///
/// Returns the `rowid` of the newly inserted row.
pub fn insert_payload(
    conn: &Connection,
    device_id: &str,
    encrypted_blob: &[u8],
    received_at: i64,
) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO payloads (device_id, encrypted_blob, received_at, synced)
         VALUES (?1, ?2, ?3, 0)",
        rusqlite::params![device_id, encrypted_blob, received_at],
    )
    .map_err(|e| format!("insert_payload failed: {e}"))?;

    Ok(conn.last_insert_rowid())
}

/// Returns all rows where `synced = 0`, ordered oldest-first.
///
/// The sync engine (Phase 4) drains this list when the network is Stable.
pub fn fetch_unsynced(conn: &Connection) -> Result<Vec<PayloadRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, device_id, encrypted_blob, received_at
             FROM payloads
             WHERE synced = 0
             ORDER BY received_at ASC",
        )
        .map_err(|e| format!("fetch_unsynced prepare failed: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(PayloadRow {
                id: row.get(0)?,
                device_id: row.get(1)?,
                encrypted_blob: row.get(2)?,
                received_at: row.get(3)?,
            })
        })
        .map_err(|e| format!("fetch_unsynced query failed: {e}"))?
        .collect::<RusqliteResult<Vec<_>>>()
        .map_err(|e| format!("fetch_unsynced row mapping failed: {e}"))?;

    Ok(rows)
}

/// Marks a single payload row as synced.
///
/// Called by the sync engine (Phase 4) after a successful cloud upload.
/// Returns an error if no row with the given `id` exists.
pub fn mark_synced(conn: &Connection, id: i64) -> Result<(), String> {
    let updated = conn
        .execute(
            "UPDATE payloads SET synced = 1 WHERE id = ?1",
            rusqlite::params![id],
        )
        .map_err(|e| format!("mark_synced failed: {e}"))?;

    if updated == 0 {
        return Err(format!("mark_synced: no row found with id {id}"));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use std::fs;

    fn temp_db() -> (Db, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "sentinel_queries_test_{:?}_{}",
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir).expect("Db::open must succeed");
        (db, dir)
    }

    #[test]
    fn insert_and_fetch_unsynced() {
        let (db, dir) = temp_db();

        let id = insert_payload(&db.conn, "device-1", b"encrypted-blob", 1000)
            .expect("insert must succeed");

        let rows = fetch_unsynced(&db.conn).expect("fetch must succeed");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].device_id, "device-1");
        assert_eq!(rows[0].encrypted_blob, b"encrypted-blob");
        assert_eq!(rows[0].received_at, 1000);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mark_synced_removes_row_from_unsynced() {
        let (db, dir) = temp_db();

        let id = insert_payload(&db.conn, "device-2", b"blob", 2000).expect("insert must succeed");

        mark_synced(&db.conn, id).expect("mark_synced must succeed");

        let rows = fetch_unsynced(&db.conn).expect("fetch must succeed");
        assert!(
            rows.is_empty(),
            "synced row must not appear in fetch_unsynced"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fetch_unsynced_orders_oldest_first() {
        let (db, dir) = temp_db();

        insert_payload(&db.conn, "device-3", b"blob-c", 3000).unwrap();
        insert_payload(&db.conn, "device-3", b"blob-a", 1000).unwrap();
        insert_payload(&db.conn, "device-3", b"blob-b", 2000).unwrap();

        let rows = fetch_unsynced(&db.conn).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].received_at, 1000);
        assert_eq!(rows[1].received_at, 2000);
        assert_eq!(rows[2].received_at, 3000);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mark_synced_unknown_id_returns_error() {
        let (db, dir) = temp_db();

        let result = mark_synced(&db.conn, 9999);
        assert!(result.is_err(), "marking unknown id must return error");

        fs::remove_dir_all(&dir).ok();
    }
}
