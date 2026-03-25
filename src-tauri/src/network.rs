use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};

use crate::state::{AppState, NetworkStatus};

/// How often the monitor checks network health.
const PING_INTERVAL_SECS: u64 = 5;

/// The host used to probe internet reachability.
/// 1.1.1.1 is Cloudflare's DNS — reliable, fast, and not blocked by most networks.
const PING_TARGET: &str = "https://1.1.1.1";

/// Consecutive failures before status transitions:
///   0 failures              → Stable
///   1–2 consecutive fails   → Degraded
///   3+ consecutive fails    → Offline
const DEGRADED_THRESHOLD: u32 = 1;
const OFFLINE_THRESHOLD: u32 = 3;

/// Starts the network health monitor loop.
///
/// Runs forever — spawn this with `tokio::spawn` from `main.rs` at startup.
/// Updates `AppState.network_status` every `PING_INTERVAL_SECS` seconds based
/// on consecutive HTTP probe failures against `PING_TARGET`.
///
/// Transition logic:
///   consecutive_failures == 0          → Stable
///   consecutive_failures in [1, 2]     → Degraded
///   consecutive_failures >= 3          → Offline
pub async fn start_monitor(state: Arc<Mutex<AppState>>) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(4)) // must be < PING_INTERVAL to avoid overlap
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

        // Only lock long enough to update — never hold the lock across await points
        {
            let mut s = state
                .lock()
                .expect("AppState lock poisoned in network monitor");
            if s.network_status != new_status {
                println!(
                    "[network] Status changed: {:?} → {:?} (failures: {})",
                    s.network_status, new_status, consecutive_failures
                );
                s.network_status = new_status;
            }
        }
    }
}

/// Sends a lightweight HEAD request to `PING_TARGET`.
///
/// Returns `true` if the server responds with any HTTP status — even a 4xx
/// counts as reachable since the network path is clearly working.
/// Returns `false` on timeout, connection refused, or DNS failure.
async fn probe(client: &reqwest::Client) -> bool {
    client.head(PING_TARGET).send().await.is_ok()
}

/// Maps a consecutive failure count to a `NetworkStatus` variant.
///
/// Extracted so it can be unit-tested without async or network I/O.
fn classify(consecutive_failures: u32) -> NetworkStatus {
    if consecutive_failures == 0 {
        NetworkStatus::Stable
    } else if consecutive_failures >= DEGRADED_THRESHOLD && consecutive_failures < OFFLINE_THRESHOLD
    {
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

    // classify() is pure — test it exhaustively without any async or I/O

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
        // Verify the boundary values precisely
        assert_eq!(classify(DEGRADED_THRESHOLD - 1), NetworkStatus::Stable);
        assert_eq!(classify(DEGRADED_THRESHOLD), NetworkStatus::Degraded);
        assert_eq!(classify(OFFLINE_THRESHOLD - 1), NetworkStatus::Degraded);
        assert_eq!(classify(OFFLINE_THRESHOLD), NetworkStatus::Offline);
    }
}
