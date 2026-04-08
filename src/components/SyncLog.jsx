// components/SyncLog.jsx
// Displays the last N sync events pushed by the sync engine.

import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { getSyncLog } from "../lib/bridge";

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
    // --- Load initial history on mount ---
    getSyncLog().then((log) => setEvents(log.slice(0, MAX_VISIBLE)));

    // --- Listen for "Live" updates ---
    const unlisten = listen("new-sync-event", (event) => {
      setEvents((prev) => [event.payload, ...prev].slice(0, MAX_VISIBLE));
    });

    return () => {
      unlisten.then((f) => f()); // Cleanup listener on unmount
    };
  }, []);

  if (events.length === 0) {
    return (
      <p style={{ fontSize: 13, color: "var(--color-text-secondary)", margin: 0 }}>
        No sync events yet
      </p>
    );
  }

  return (
    <ul
      style={{
        listStyle: "none",
        margin: 0,
        padding: 0,
        display: "flex",
        flexDirection: "column",
        gap: 4,
      }}
    >
      {events.map((ev, i) => {
        const failed = isFailure(ev.message);
        return (
          <li
            key={i}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 12,
              fontSize: 12,
              padding: "6px 0",
              borderBottom: "1px solid var(--color-border-tertiary)",
            }}
          >
            <i
              className={failed ? "bi bi-exclamation-circle-fill" : "bi bi-cloud-check-fill"}
              style={{
                color: failed ? "#E24B4A" : "#1D9E75",
                fontSize: 14,
                flexShrink: 0,
              }}
            />
            <span
              style={{
                color: "var(--color-text-secondary)",
                minWidth: 70,
                flexShrink: 0,
                fontWeight: 500,
              }}
            >
              {formatTimestamp(ev.timestamp)}
            </span>
            <span
              style={{
                color: failed ? "#E24B4A" : "var(--color-text-primary)",
                fontWeight: failed ? 500 : 400,
              }}
            >
              {ev.message}
            </span>
          </li>
        );
      })}
    </ul>
  );
}
