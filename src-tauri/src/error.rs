use serde::Serialize;
use std::fmt;
use std::sync::PoisonError;
use std::time::SystemTimeError;

// ---------------------------------------------------------------------------
// SentinelError
// ---------------------------------------------------------------------------

/// Unified error type used across all Sentinel backend modules.
///
/// Implements `serde::Serialize` so Tauri commands can return
/// `Result<T, SentinelError>` directly without an extra `.map_err()`.
///
/// Implements `From` for every error source used across the codebase so
/// the `?` operator works naturally in all modules.
#[derive(Debug, Serialize)]
pub enum SentinelError {
    /// SQLite / SQLCipher errors (rusqlite)
    Database(String),

    /// AES-256-GCM encrypt/decrypt failures (matches crypto.rs usage)
    Crypto(String),

    /// File I/O errors (settings.json, DB path, etc.)
    Io(String),

    /// JSON serialization / deserialization errors
    Serialization(String),

    /// Mutex lock poisoned — another thread panicked while holding the lock
    LockPoisoned(String),

    /// System clock error (duration_since UNIX_EPOCH)
    SystemTime(String),

    /// Catch-all for errors that don't fit a specific variant
    Other(String),
}

impl fmt::Display for SentinelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(msg) => write!(f, "Database error: {msg}"),
            Self::Crypto(msg) => write!(f, "Crypto error: {msg}"),
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
            Self::Serialization(msg) => write!(f, "Serialization error: {msg}"),
            Self::LockPoisoned(msg) => write!(f, "Lock poisoned: {msg}"),
            Self::SystemTime(msg) => write!(f, "System time error: {msg}"),
            Self::Other(msg) => write!(f, "Error: {msg}"),
        }
    }
}

impl std::error::Error for SentinelError {}

// ---------------------------------------------------------------------------
// From conversions — lets ? operator work naturally everywhere
// ---------------------------------------------------------------------------

impl From<rusqlite::Error> for SentinelError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Database(e.to_string())
    }
}

impl From<std::io::Error> for SentinelError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<serde_json::Error> for SentinelError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}

/// Covers every `state.lock()?` call regardless of what the Mutex holds.
/// T is the MutexGuard type — we only need the error message, not T itself.
impl<T> From<PoisonError<T>> for SentinelError {
    fn from(e: PoisonError<T>) -> Self {
        Self::LockPoisoned(e.to_string())
    }
}

/// Covers `SystemTime::now().duration_since(UNIX_EPOCH)?` in ws.rs
impl From<SystemTimeError> for SentinelError {
    fn from(e: SystemTimeError) -> Self {
        Self::SystemTime(e.to_string())
    }
}

/// Allows wrapping a plain `String` error from legacy map_err call sites.
impl From<String> for SentinelError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

/// Allows wrapping `&str` directly.
impl From<&str> for SentinelError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_database_variant() {
        let e = SentinelError::Database("no such table".to_string());
        assert_eq!(e.to_string(), "Database error: no such table");
    }

    #[test]
    fn display_crypto_variant() {
        let e = SentinelError::Crypto("nonce too short".to_string());
        assert_eq!(e.to_string(), "Crypto error: nonce too short");
    }

    #[test]
    fn display_io_variant() {
        let e = SentinelError::Io("file not found".to_string());
        assert_eq!(e.to_string(), "I/O error: file not found");
    }

    #[test]
    fn display_lock_poisoned_variant() {
        let e = SentinelError::LockPoisoned("mutex was poisoned".to_string());
        assert!(e.to_string().contains("Lock poisoned"));
    }

    #[test]
    fn display_system_time_variant() {
        let e = SentinelError::SystemTime("clock went backwards".to_string());
        assert!(e.to_string().contains("System time error"));
    }

    #[test]
    fn display_other_variant() {
        let e = SentinelError::Other("something unexpected".to_string());
        assert_eq!(e.to_string(), "Error: something unexpected");
    }

    #[test]
    fn from_string_wraps_as_other() {
        let e = SentinelError::from("raw string error".to_string());
        assert!(matches!(e, SentinelError::Other(_)));
        assert_eq!(e.to_string(), "Error: raw string error");
    }

    #[test]
    fn from_str_wraps_as_other() {
        let e = SentinelError::from("static str");
        assert!(matches!(e, SentinelError::Other(_)));
    }

    #[test]
    fn from_io_error_wraps_as_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing file");
        let e = SentinelError::from(io_err);
        assert!(matches!(e, SentinelError::Io(_)));
        assert!(e.to_string().contains("missing file"));
    }

    #[test]
    fn poison_error_via_real_mutex() {
        use std::sync::{Arc, Mutex};
        let mutex = Arc::new(Mutex::new(0u32));
        let mutex2 = Arc::clone(&mutex);
        let _ = std::thread::spawn(move || {
            let _guard = mutex2.lock().unwrap();
            panic!("intentional poison");
        })
        .join();

        let poison = mutex.lock().unwrap_err();
        let e = SentinelError::from(poison);
        assert!(matches!(e, SentinelError::LockPoisoned(_)));
    }

    #[test]
    fn serialize_produces_tagged_json() {
        let e = SentinelError::Database("test db error".to_string());
        let json = serde_json::to_string(&e).expect("serialize must succeed");
        assert!(json.contains("Database"));
        assert!(json.contains("test db error"));
    }
}
