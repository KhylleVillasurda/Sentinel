use std::collections::HashSet;
use std::collections::VecDeque;

use crate::db::Db;
use crate::settings::Settings;

// ---------------------------------------------------------------------------
// NetworkStatus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum NetworkStatus {
    Unknown,
    Stable,
    Degraded,
    Offline,
}

// ---------------------------------------------------------------------------
// SyncEvent
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
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

    /// Current internet health (updated by network::start_monitor)
    pub network_status: NetworkStatus,

    /// Device IDs currently connected via WebSocket.
    /// HashSet prevents duplicates if a device reconnects before cleanup.
    /// Updated live by ws.rs on connect/disconnect.
    pub connected_devices: HashSet<String>,

    /// Rolling log of recent sync events for the dashboard.
    /// VecDeque gives O(1) front-removal when capping at 100 entries.
    pub sync_log: VecDeque<SyncEvent>,

    /// User-configurable settings (cloud endpoint, WS bind address).
    /// Loaded from settings.json at startup; persisted on save_settings command.
    pub settings: Settings,
}

impl AppState {
    /// Creates a new AppState from an open Db handle and loaded settings.
    pub fn new(db: Db, settings: Settings) -> Self {
        let encryption_key = db.key;
        Self {
            db,
            encryption_key,
            network_status: NetworkStatus::Unknown,
            connected_devices: HashSet::new(),
            sync_log: VecDeque::new(),
            settings,
        }
    }
}
