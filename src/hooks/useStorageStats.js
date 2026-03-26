// hooks/useStorageStats.js
// Polls local DB stats every 10 seconds.
// Used by StorageBar and any component that needs row counts.

import { useState, useEffect } from "react";
import { getStorageStats } from "../lib/bridge";

const POLL_INTERVAL_MS = 10000;

/**
 * @returns {{
 *   totalRows: number,
 *   unsyncedRows: number,
 *   sizeKb: number,
 *   error: string|null
 * }}
 */
export function useStorageStats() {
  const [data, setData] = useState({
    totalRows: 0,
    unsyncedRows: 0,
    sizeKb: 0,
    error: null,
  });

  useEffect(() => {
    let cancelled = false;

    const poll = async () => {
      try {
        const res = await getStorageStats();
        if (!cancelled) {
          setData({
            totalRows: res.total_rows,
            unsyncedRows: res.unsynced_rows,
            sizeKb: res.size_kb,
            error: null,
          });
        }
      } catch (err) {
        if (!cancelled) setData((prev) => ({ ...prev, error: String(err) }));
      }
    };

    poll();
    const id = setInterval(poll, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  return data;
}
