// components/SyncLog.jsx
// Displays the last N sync events pushed by the sync engine.
// Polls every 10 seconds — sync events are infrequent.

import { useState, useEffect } from "react";
import { getSyncLog } from "../lib/bridge";

const POLL_INTERVAL_MS = 10000;
const MAX_VISIBLE = 8;

// Formats a Unix timestamp (seconds) into a readable local time string
function formatTimestamp(ts) {
  return new Date(ts * 1000).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

// A sync event is considered a failure if the message contains "fail" or "reject"
function isFailure(message) {
  return /fail|reject/i.test(message);
}

export function SyncLog() {
  const [events, setEvents] = useState([]);

  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      try {
        const log = await getSyncLog();
        if (!cancelled) setEvents(log.slice(0, MAX_VISIBLE));
      } catch (_) {}
    };
    poll();
    const id = setInterval(poll, POLL_INTERVAL_MS);
    return () => { cancelled = true; clearInterval(id); };
  }, []);

  if (events.length === 0) {
    return (
      <p style={{ fontSize: 13, color: "var(--color-text-secondary)", margin: 0 }}>
        No sync events yet
      </p>
    );
  }

  return (
    <ul style={{ listStyle: "none", margin: 0, padding: 0, display: "flex", flexDirection: "column", gap: 4 }}>
      {events.map((ev, i) => {
        const failed = isFailure(ev.message);
        return (
          <li
            key={i}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 10,
              fontSize: 12,
              padding: "4px 0",
              borderBottom: "0.5px solid var(--color-border-tertiary)",
            }}
          >
            <span
              style={{
                width: 7,
                height: 7,
                borderRadius: "50%",
                background: failed ? "#E24B4A" : "#1D9E75",
                flexShrink: 0,
              }}
            />
            <span style={{ color: "var(--color-text-secondary)", minWidth: 80, flexShrink: 0 }}>
              {formatTimestamp(ev.timestamp)}
            </span>
            <span style={{ color: failed ? "#E24B4A" : "inherit" }}>
              {ev.message}
            </span>
          </li>
        );
      })}
    </ul>
  );
}