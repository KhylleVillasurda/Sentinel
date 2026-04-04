use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// Persisted application settings, saved as JSON in the app data directory.
///
/// Every field has a sensible default so first-run works without user input.
/// Unknown keys in the JSON file are ignored (serde `deny_unknown_fields` is
/// intentionally NOT set so future fields don't break old installs).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// Cloud endpoint URL that receives batched encrypted payloads.
    /// Changed from the hardcoded `CLOUD_ENDPOINT` const in sync.rs.
    pub cloud_endpoint: String,

    /// WebSocket server bind address (host:port).
    /// Changed from the hardcoded addr in ws.rs.
    pub ws_bind_address: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cloud_endpoint: "http://127.0.0.1:9000/ingest".to_string(),
            ws_bind_address: "0.0.0.0:6767".to_string(),
        }
    }
}

impl Settings {
    /// Loads settings from `<dir>/settings.json`.
    ///
    /// Returns `Settings::default()` silently when:
    ///   - the file does not exist (first run)
    ///   - the file cannot be read
    ///   - the JSON is malformed or missing fields
    ///
    /// This means the app always starts even if the config file is corrupted.
    pub fn load(dir: &Path) -> Self {
        let path = Self::file_path(dir);
        let contents = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => return Self::default(),
        };
        serde_json::from_str(&contents).unwrap_or_default()
    }

    /// Saves the current settings to `<dir>/settings.json`.
    ///
    /// Creates the file if it does not exist; overwrites if it does.
    /// Returns `Err(String)` on serialization or I/O failure.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = Self::file_path(dir);
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("Failed to write settings.json: {e}"))?;
        Ok(())
    }

    /// Returns the canonical path to the settings file inside `dir`.
    fn file_path(dir: &Path) -> PathBuf {
        dir.join("settings.json")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Creates a unique temp directory for each test so tests never share state.
    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sentinel_settings_test_{:?}_{}",
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_settings_have_correct_values() {
        let s = Settings::default();
        assert_eq!(s.cloud_endpoint, "http://127.0.0.1:9000/ingest");
        assert_eq!(s.ws_bind_address, "0.0.0.0:6767");
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = temp_dir();
        let original = Settings {
            cloud_endpoint: "http://192.168.1.50:9000/ingest".to_string(),
            ws_bind_address: "0.0.0.0:7000".to_string(),
        };

        original.save(&dir).expect("save must succeed");
        let loaded = Settings::load(&dir);

        assert_eq!(loaded, original);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = temp_dir();
        // No settings.json written — should silently fall back to defaults
        let loaded = Settings::load(&dir);
        assert_eq!(loaded, Settings::default());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_returns_default_when_file_malformed() {
        let dir = temp_dir();
        // Write garbage JSON — should not panic, just fall back to defaults
        fs::write(dir.join("settings.json"), b"{ this is not valid json }")
            .expect("write must succeed");

        let loaded = Settings::load(&dir);
        assert_eq!(loaded, Settings::default());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_overwrites_existing_file() {
        let dir = temp_dir();

        let first = Settings {
            cloud_endpoint: "http://first.example.com/ingest".to_string(),
            ws_bind_address: "0.0.0.0:6767".to_string(),
        };
        first.save(&dir).expect("first save must succeed");

        let second = Settings {
            cloud_endpoint: "http://second.example.com/ingest".to_string(),
            ws_bind_address: "0.0.0.0:8888".to_string(),
        };
        second.save(&dir).expect("second save must succeed");

        let loaded = Settings::load(&dir);
        assert_eq!(loaded, second, "second save must overwrite first");

        fs::remove_dir_all(&dir).ok();
    }
}
