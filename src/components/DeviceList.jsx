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
            gap: 8,
            fontSize: 13,
          }}
        >
          <span
            style={{
              width: 7,
              height: 7,
              borderRadius: "50%",
              background: "#1D9E75",
              flexShrink: 0,
            }}
          />
          <span style={{ fontFamily: "var(--font-mono)" }}>{id}</span>
        </li>
      ))}
    </ul>
  );
}