// network.rs
// Ping-based internet health monitor.
// Runs a background loop that probes 1.1.1.1 every 5 seconds and updates
// AppState.network_status based on consecutive failure count.

use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};

use crate::state::{AppState, NetworkStatus};

const PING_INTERVAL_SECS: u64 = 5;
const PING_TARGET: &str = "https://1.1.1.1";

/// Consecutive failures before status transitions:
///   0 failures          → Stable
///   1–2 failures        → Degraded
///   3+ failures         → Offline
const DEGRADED_THRESHOLD: u32 = 1;
const OFFLINE_THRESHOLD: u32 = 3;

/// Starts the network health monitor loop.
///
/// Runs forever — spawn with `tokio::spawn` from `main.rs` at startup.
/// If the `AppState` lock is poisoned (a background thread panicked while
/// holding it), the monitor logs the event and exits cleanly rather than
/// propagating a panic across thread boundaries.
pub async fn start_monitor(state: Arc<Mutex<AppState>>) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(4)) // < PING_INTERVAL to avoid overlap
        .build()
        .expect("Failed to build reqwest client for network monitor");

    let mut ticker = interval(Duration::from_secs(PING_INTERVAL_SECS));
    let mut consecutive_failures: u32 = 0;

    loop {
        ticker.tick().await;

        let reachable = probe(&client).await;

        if reachable {
            consecutive_failures = 0;
        } else {
            consecutive_failures = consecutive_failures.saturating_add(1);
        }

        let new_status = classify(consecutive_failures);

        // Hold the lock only long enough to compare and update — never across
        // an await point.
        match state.lock() {
            Ok(mut s) => {
                if s.network_status != new_status {
                    println!(
                        "[network] Status changed: {:?} → {:?} (failures: {})",
                        s.network_status, new_status, consecutive_failures
                    );
                    s.network_status = new_status;
                }
            }
            Err(e) => {
                // Lock poisoned — another thread panicked while holding the lock.
                // We cannot safely read or write AppState; exit this task.
                eprintln!("[network] AppState lock poisoned: {e}; monitor shutting down");
                return;
            }
        }
    }
}

/// Sends a lightweight HEAD request to `PING_TARGET`.
///
/// Returns `true` on any HTTP response (even 4xx — the network path works).
/// Returns `false` on timeout, connection refused, or DNS failure.
async fn probe(client: &reqwest::Client) -> bool {
    client.head(PING_TARGET).send().await.is_ok()
}

/// Maps a consecutive failure count to a `NetworkStatus` variant.
/// Pure function — unit-testable without async or network I/O.
///
/// Both thresholds are explicit so the boundaries are visible here,
/// and both constants remain used in production code (not just tests).
fn classify(consecutive_failures: u32) -> NetworkStatus {
    if consecutive_failures == 0 {
        NetworkStatus::Stable
    } else if consecutive_failures >= DEGRADED_THRESHOLD && consecutive_failures < OFFLINE_THRESHOLD {
        NetworkStatus::Degraded
    } else {
        NetworkStatus::Offline
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_failures_is_stable() {
        assert_eq!(classify(0), NetworkStatus::Stable);
    }

    #[test]
    fn one_failure_is_degraded() {
        assert_eq!(classify(1), NetworkStatus::Degraded);
    }

    #[test]
    fn two_failures_is_degraded() {
        assert_eq!(classify(2), NetworkStatus::Degraded);
    }

    #[test]
    fn three_failures_is_offline() {
        assert_eq!(classify(3), NetworkStatus::Offline);
    }

    #[test]
    fn many_failures_stays_offline() {
        assert_eq!(classify(100), NetworkStatus::Offline);
    }

    #[test]
    fn classify_covers_all_thresholds() {
        assert_eq!(classify(DEGRADED_THRESHOLD - 1), NetworkStatus::Stable);
        assert_eq!(classify(DEGRADED_THRESHOLD), NetworkStatus::Degraded);
        assert_eq!(classify(OFFLINE_THRESHOLD - 1), NetworkStatus::Degraded);
        assert_eq!(classify(OFFLINE_THRESHOLD), NetworkStatus::Offline);
    }
}
