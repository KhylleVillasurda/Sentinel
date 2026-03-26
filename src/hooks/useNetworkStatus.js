// hooks/useNetworkStatus.js
// Polls the Rust network monitor every 5 seconds.
// Returns the latest status so any component can read it without
// each one setting up its own polling interval.

import { useState, useEffect } from "react";
import { getNetworkStatus } from "../lib/bridge";

const POLL_INTERVAL_MS = 5000;

/**
 * @returns {{
 *   status: "Stable"|"Degraded"|"Offline"|"Unknown",
 *   error: string|null
 * }}
 */
export function useNetworkStatus() {
  const [data, setData] = useState({
    status: "Unknown",
    error: null,
  });

  useEffect(() => {
    let cancelled = false;

    const poll = async () => {
      try {
        const res = await getNetworkStatus();
        if (!cancelled) {
          setData({ status: res.status, error: null });
        }
      } catch (err) {
        if (!cancelled) setData((prev) => ({ ...prev, error: String(err) }));
      }
    };

    poll(); // immediate first fetch
    const id = setInterval(poll, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  return data;
}
