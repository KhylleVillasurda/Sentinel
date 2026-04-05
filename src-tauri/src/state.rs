use crate::config::Config;
use crate::db::Db;

// ---------------------------------------------------------------------------
// Phase 3 placeholder — will be moved to network.rs
// ---------------------------------------------------------------------------

/// Represents the current internet connection health.
/// TODO (Phase 3): move this enum into network.rs and re-export from here.
#[derive(Debug, Clone, PartialEq)]
pub enum NetworkStatus {
    Unknown,
    Stable,
    Degraded,
    Offline,
}

// ---------------------------------------------------------------------------
// Phase 4 placeholder — will be moved to sync.rs
// ---------------------------------------------------------------------------

/// A single entry in the rolling sync event log shown on the dashboard.
/// TODO (Phase 4): move this struct into sync.rs and re-export from here.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncEvent {
    pub message: String,
    pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

/// Shared application state — wrapped in Arc<Mutex<AppState>> everywhere.
///
/// All fields are pub so Tauri commands can read them directly.
/// All writes go through the functions in their respective modules.
pub struct AppState {
    /// Encrypted SQLite database handle — all DB access via db::queries
    pub db: Db,

    /// AES-256-GCM key loaded at startup via crypto::load_or_create_key()
    pub encryption_key: [u8; 32],

    /// Current internet health (updated by network::start_monitor, Phase 3)
    pub network_status: NetworkStatus,

    /// Device IDs currently connected via WebSocket (Phase 2)
    pub connected_devices: Vec<String>,

    /// Rolling log of recent sync events for the dashboard (Phase 4)
    pub sync_log: Vec<SyncEvent>,

    /// User-tunable settings — loaded at startup, mutated via save_config command
    pub config: Config,

    /// App data directory path — needed by save_config to persist changes
    pub app_data_dir: std::path::PathBuf,
}

impl AppState {
    /// Creates a new AppState from an open Db handle, loaded config, and data directory.
    pub fn new(db: Db, config: Config, app_data_dir: std::path::PathBuf) -> Self {
        let encryption_key = db.key;
        Self {
            db,
            encryption_key,
            network_status: NetworkStatus::Unknown,
            connected_devices: Vec::new(),
            sync_log: Vec::new(),
            config,
            app_data_dir,
        }
    }
}
