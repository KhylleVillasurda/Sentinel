// db/queries.rs
// All SQL against the `payloads` and `devices` tables lives here.

use rusqlite::Connection;
use crate::error::SentinelError;

/// A single row from the `payloads` table.
#[derive(Debug, PartialEq)]
pub struct PayloadRow {
    pub id: i64,
    pub device_id: String,
    pub encrypted_blob: Vec<u8>,
    pub received_at: i64,
}

/// A single row from the `devices` table.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceRow {
    pub device_id: String,
    pub friendly_name: String,
    pub token_hash: String,
    pub created_at: i64,
    pub last_seen: i64,
}

// --- Payload Queries ---

pub fn insert_payload(
    conn: &Connection,
    device_id: &str,
    encrypted_blob: &[u8],
    received_at: i64,
) -> Result<i64, SentinelError> {
    conn.execute(
        "INSERT INTO payloads (device_id, encrypted_blob, received_at, synced)
         VALUES (?1, ?2, ?3, 0)",
        rusqlite::params![device_id, encrypted_blob, received_at],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn fetch_unsynced(conn: &Connection) -> Result<Vec<PayloadRow>, SentinelError> {
    let mut stmt = conn.prepare(
        "SELECT id, device_id, encrypted_blob, received_at
         FROM payloads
         WHERE synced = 0
         ORDER BY received_at ASC",
    )?;

    let rows = stmt
        .query_map([], |row| {
            Ok(PayloadRow {
                id: row.get(0)?,
                device_id: row.get(1)?,
                encrypted_blob: row.get(2)?,
                received_at: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(rows)
}

pub fn mark_synced(conn: &Connection, id: i64) -> Result<(), SentinelError> {
    let updated = conn.execute(
        "UPDATE payloads SET synced = 1 WHERE id = ?1",
        rusqlite::params![id],
    )?;

    if updated == 0 {
        return Err(SentinelError::Other(format!(
            "mark_synced: no row found with id {id}"
        )));
    }

    Ok(())
}

// --- Device Queries ---

pub fn register_device(
    conn: &Connection,
    device: &DeviceRow,
) -> Result<(), SentinelError> {
    conn.execute(
        "INSERT INTO devices (device_id, friendly_name, token_hash, created_at, last_seen)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            device.device_id,
            device.friendly_name,
            device.token_hash,
            device.created_at,
            device.last_seen
        ],
    )?;
    Ok(())
}

pub fn get_device(
    conn: &Connection,
    device_id: &str,
) -> Result<Option<DeviceRow>, SentinelError> {
    let mut stmt = conn.prepare(
        "SELECT device_id, friendly_name, token_hash, created_at, last_seen FROM devices WHERE device_id = ?1",
    )?;
    
    let mut rows = stmt.query_map(rusqlite::params![device_id], |row| {
        Ok(DeviceRow {
            device_id: row.get(0)?,
            friendly_name: row.get(1)?,
            token_hash: row.get(2)?,
            created_at: row.get(3)?,
            last_seen: row.get(4)?,
        })
    })?;

    if let Some(res) = rows.next() {
        Ok(Some(res?))
    } else {
        Ok(None)
    }
}

pub fn update_last_seen(conn: &Connection, device_id: &str, timestamp: i64) -> Result<(), SentinelError> {
    conn.execute(
        "UPDATE devices SET last_seen = ?1 WHERE device_id = ?2",
        rusqlite::params![timestamp, device_id],
    )?;
    Ok(())
}

pub fn list_devices(conn: &Connection) -> Result<Vec<DeviceRow>, SentinelError> {
    let mut stmt = conn.prepare(
        "SELECT device_id, friendly_name, token_hash, created_at, last_seen FROM devices ORDER BY created_at DESC",
    )?;

    let rows = stmt
        .query_map([], |row| {
            Ok(DeviceRow {
                device_id: row.get(0)?,
                friendly_name: row.get(1)?,
                token_hash: row.get(2)?,
                created_at: row.get(3)?,
                last_seen: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(rows)
}

pub fn delete_device(conn: &Connection, device_id: &str) -> Result<(), SentinelError> {
    conn.execute(
        "DELETE FROM devices WHERE device_id = ?1",
        rusqlite::params![device_id],
    )?;
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
    fn device_registration_cycle() {
        let (db, dir) = temp_db();
        let dev = DeviceRow {
            device_id: "test-dev".into(),
            friendly_name: "Test Sensor".into(),
            token_hash: "hashed-token".into(),
            created_at: 100,
            last_seen: 100,
        };

        register_device(&db.conn, &dev).unwrap();
        
        let fetched = get_device(&db.conn, "test-dev").unwrap().unwrap();
        assert_eq!(fetched.friendly_name, "Test Sensor");

        let all = list_devices(&db.conn).unwrap();
        assert_eq!(all.len(), 1);

        update_last_seen(&db.conn, "test-dev", 200).unwrap();
        let updated = get_device(&db.conn, "test-dev").unwrap().unwrap();
        assert_eq!(updated.last_seen, 200);

        delete_device(&db.conn, "test-dev").unwrap();
        assert!(get_device(&db.conn, "test-dev").unwrap().is_none());

        fs::remove_dir_all(&dir).ok();
    }
}
