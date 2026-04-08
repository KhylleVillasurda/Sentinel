// config.rs
// Persisted application configuration.
//
// Settings the user can change at runtime are stored here.
// The config is saved as `sentinel_config.json` in the app data directory
// alongside the encrypted database. On first launch, defaults are written.
//
// Design rules:
//   - Config is loaded once at startup and stored in AppState
//   - All mutations go through `Config::save()` so disk stays in sync
//   - Every field has a documented default — nothing is silently undefined

use serde::{Deserialize, Serialize};
use std::path::Path;

/// File name for the persisted config, written next to the DB.
const CONFIG_FILE: &str = "sentinel_config.json";

/// All user-tunable settings.
///
/// Adding a new field: give it a `serde(default = ...)` so older
/// config files (which don't have the field) deserialize cleanly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// HTTP endpoint for cloud sync POST requests.
    /// Default: the bundled local-cloud mock server.
    #[serde(default = "default_cloud_endpoint")]
    pub cloud_endpoint: String,

    /// How often (seconds) the sync engine wakes to drain unsynced rows.
    /// Minimum enforced: 5s — below that hammers the DB unnecessarily.
    #[serde(default = "default_sync_interval_secs")]
    pub sync_interval_secs: u64,

    /// Host address the WebSocket ingestion server binds to.
    /// Use "0.0.0.0" to accept connections from any network interface,
    /// or a specific IP to limit to one interface.
    #[serde(default = "default_ws_host")]
    pub ws_host: String,

    /// Port the WebSocket ingestion server listens on.
    #[serde(default = "default_ws_port")]
    pub ws_port: u16,

    /// Maximum number of log entries kept in memory (rolling cap).
    #[serde(default = "default_log_max_entries")]
    pub log_max_entries: usize,
}

// ---------------------------------------------------------------------------
// Default value functions — required by serde(default = "fn_name")
// ---------------------------------------------------------------------------

fn default_cloud_endpoint() -> String {
    "http://127.0.0.1:9000/ingest".to_string()
}

fn default_sync_interval_secs() -> u64 {
    10
}

fn default_ws_host() -> String {
    "0.0.0.0".to_string()
}

fn default_ws_port() -> u16 {
    6767
}

fn default_log_max_entries() -> usize {
    100
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cloud_endpoint: default_cloud_endpoint(),
            sync_interval_secs: default_sync_interval_secs(),
            ws_host: default_ws_host(),
            ws_port: default_ws_port(),
            log_max_entries: default_log_max_entries(),
        }
    }
}

impl Config {
    /// Loads config from `{app_data_dir}/sentinel_config.json`.
    ///
    /// If the file does not exist, `Config::default()` is returned **and**
    /// written to disk so subsequent runs start from a known file.
    ///
    /// If the file exists but is malformed, returns an error rather than
    /// silently falling back — better to surface corruption early.
    pub fn load(app_data_dir: &Path) -> Result<Self, String> {
        let path = app_data_dir.join(CONFIG_FILE);

        if !path.exists() {
            // First launch — write defaults and return them
            let defaults = Config::default();
            defaults.save(app_data_dir)?;
            return Ok(defaults);
        }

        let raw =
            std::fs::read_to_string(&path).map_err(|e| format!("Failed to read config: {e}"))?;

        serde_json::from_str(&raw).map_err(|e| format!("Failed to parse config: {e}"))
    }

    /// Persists the current config to `{app_data_dir}/sentinel_config.json`.
    ///
    /// Uses pretty-printed JSON so the file is human-readable and easy to
    /// inspect or manually edit.
    pub fn save(&self, app_data_dir: &Path) -> Result<(), String> {
        let path = app_data_dir.join(CONFIG_FILE);
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("Failed to write config: {e}"))
    }

    /// Returns the validated sync interval, enforcing the 5-second minimum.
    pub fn sync_interval_secs_validated(&self) -> u64 {
        self.sync_interval_secs.max(5)
    }

    /// Returns the full WebSocket bind address string.
    pub fn ws_bind_addr(&self) -> String {
        format!("{}:{}", self.ws_host, self.ws_port)
    }
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
            "sentinel_config_test_{:?}_{}",
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
    fn load_creates_default_config_on_first_run() {
        let dir = temp_dir();

        let config = Config::load(&dir).expect("load must succeed on first run");

        assert_eq!(config.cloud_endpoint, "http://127.0.0.1:9000/ingest");
        assert_eq!(config.sync_interval_secs, 10);
        assert_eq!(config.ws_port, 6767);
        assert_eq!(config.ws_host, "0.0.0.0");
        assert_eq!(config.log_max_entries, 100);

        // File must have been created
        assert!(
            dir.join(CONFIG_FILE).exists(),
            "config file should be created on first load"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_and_reload_roundtrip() {
        let dir = temp_dir();

        let mut config = Config::default();
        config.cloud_endpoint = "http://192.168.1.50:9000/ingest".to_string();
        config.sync_interval_secs = 30;
        config.ws_port = 7777;
        config.log_max_entries = 200;

        config.save(&dir).expect("save must succeed");

        let reloaded = Config::load(&dir).expect("reload must succeed");

        assert_eq!(reloaded.cloud_endpoint, "http://192.168.1.50:9000/ingest");
        assert_eq!(reloaded.sync_interval_secs, 30);
        assert_eq!(reloaded.ws_port, 7777);
        assert_eq!(reloaded.log_max_entries, 200);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_returns_error_on_malformed_json() {
        let dir = temp_dir();
        let path = dir.join(CONFIG_FILE);
        fs::write(&path, b"this is not json").unwrap();

        let result = Config::load(&dir);
        assert!(result.is_err(), "malformed JSON should return an error");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_tolerates_missing_fields_with_defaults() {
        let dir = temp_dir();
        let path = dir.join(CONFIG_FILE);
        // Write a config that is missing the newer fields — simulates an older
        // config file where only cloud_endpoint was stored.
        fs::write(&path, r#"{"cloud_endpoint": "http://old-server/ingest"}"#).unwrap();

        let config = Config::load(&dir).expect("partial config should deserialize with defaults");

        assert_eq!(config.cloud_endpoint, "http://old-server/ingest");
        // Missing fields must fall back to defaults
        assert_eq!(config.sync_interval_secs, 10);
        assert_eq!(config.ws_port, 6767);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sync_interval_validated_enforces_minimum() {
        let mut config = Config::default();

        config.sync_interval_secs = 0;
        assert_eq!(
            config.sync_interval_secs_validated(),
            5,
            "0s should clamp to 5s minimum"
        );

        config.sync_interval_secs = 3;
        assert_eq!(
            config.sync_interval_secs_validated(),
            5,
            "3s should clamp to 5s minimum"
        );

        config.sync_interval_secs = 5;
        assert_eq!(
            config.sync_interval_secs_validated(),
            5,
            "5s is the minimum, no change"
        );

        config.sync_interval_secs = 60;
        assert_eq!(
            config.sync_interval_secs_validated(),
            60,
            "60s should pass through unchanged"
        );
    }

    #[test]
    fn ws_bind_addr_combines_host_and_port() {
        let mut config = Config::default();
        config.ws_host = "10.0.0.1".to_string();
        config.ws_port = 8080;

        assert_eq!(config.ws_bind_addr(), "10.0.0.1:8080");
    }

    #[test]
    fn default_ws_bind_addr_is_correct() {
        let config = Config::default();
        assert_eq!(config.ws_bind_addr(), "0.0.0.0:6767");
    }
}
