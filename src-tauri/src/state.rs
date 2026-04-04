// state.rs
// Shared application state — wrapped in Arc<Mutex<AppState>> everywhere.
//
// Design notes:
//   - sync_log uses VecDeque so oldest entries can be dropped in O(1) with
//     pop_front() instead of the O(n) drain(0..n) that Vec required.
//   - connected_devices uses HashSet so insert/remove/contains are all O(1).
//     Vec gave O(n) contains checks and O(n) removes by value.

use std::collections::{HashSet, VecDeque};

use crate::db::Db;

// ---------------------------------------------------------------------------
// NetworkStatus
// ---------------------------------------------------------------------------

/// Represents the current internet connection health.
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

/// A single entry in the rolling sync event log shown on the dashboard.
#[derive(Debug, Clone)]
pub struct SyncEvent {
    pub message: String,
    pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

/// Shared application state — wrapped in `Arc<Mutex<AppState>>` everywhere.
///
/// All fields are `pub` so Tauri commands and background tasks can read them
/// directly after acquiring the lock. All writes go through the functions in
/// their respective modules.
pub struct AppState {
    /// Encrypted SQLite database handle — all DB access via `db::queries`.
    pub db: Db,

    /// AES-256-GCM key loaded at startup via `crypto::load_or_create_key()`.
    pub encryption_key: [u8; 32],

    /// Current internet health (updated every 5 s by `network::start_monitor`).
    pub network_status: NetworkStatus,

    /// Device IDs currently connected via WebSocket.
    ///
    /// HashSet instead of Vec:
    ///   - O(1) insert on connect, O(1) remove on disconnect
    ///   - O(1) contains check (Vec was O(n))
    ///   - Naturally deduplicates reconnects without extra logic
    pub connected_devices: HashSet<String>,

    /// Rolling log of recent sync events for the dashboard.
    ///
    /// VecDeque instead of Vec:
    ///   - push_back for new entries: O(1) amortised (same as Vec)
    ///   - pop_front to drop oldest beyond the 100-entry cap: O(1)
    ///     (Vec::drain(0..n) was O(n) due to element shifting)
    pub sync_log: VecDeque<SyncEvent>,
}

impl AppState {
    /// Creates a new `AppState` from an open `Db` handle.
    /// The encryption key is copied from the `Db` so it stays in sync.
    pub fn new(db: Db) -> Self {
        let encryption_key = db.key;
        Self {
            db,
            encryption_key,
            network_status: NetworkStatus::Unknown,
            connected_devices: HashSet::new(),
            sync_log: VecDeque::new(),
        }
    }
}
