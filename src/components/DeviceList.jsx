// components/DeviceList.jsx
// Shows devices currently connected over WebSocket.
// Polls every 3 seconds — connection list changes quickly.

import { useState, useEffect } from "react";
import { getConnectedDevices } from "../lib/bridge";

const POLL_INTERVAL_MS = 3000;

export function DeviceList() {
  const [devices, setDevices] = useState([]);

  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      try {
        const list = await getConnectedDevices();
        if (!cancelled) setDevices(list);
      } catch (_) {}
    };
    poll();
    const id = setInterval(poll, POLL_INTERVAL_MS);
    return () => { cancelled = true; clearInterval(id); };
  }, []);

  if (devices.length === 0) {
    return (
      <p style={{ fontSize: 13, color: "var(--color-text-secondary)", margin: 0 }}>
        No devices connected
      </p>
    );
  }

  return (
    <ul style={{ listStyle: "none", margin: 0, padding: 0, display: "flex", flexDirection: "column", gap: 6 }}>
      {devices.map((id) => (
        <li
          key={id}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            fontSize: 13,
            padding: "8px 12px",
            background: "var(--color-background-secondary)",
            borderRadius: 8,
            border: "0.5px solid var(--color-border-tertiary)",
          }}
        >
          <i className="bi bi-cpu" style={{ color: "#1D9E75", fontSize: 16 }} />
          <span style={{ fontFamily: "var(--font-mono)", fontWeight: 500 }}>{id}</span>
        </li>
      ))}
    </ul>
  );
}