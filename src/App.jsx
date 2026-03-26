// App.jsx
// Root layout. Composes all dashboard components into one view.
// No data fetching here — each component owns its own polling via hooks.
// Add new sections here as phases complete.

import { StatusBadge } from "./components/StatusBadge";
import { StorageBar } from "./components/StorageBar";
import { DeviceList } from "./components/DeviceList";
import { SyncLog } from "./components/SyncLog";

const card = {
  background: "var(--color-background-primary)",
  border: "0.5px solid var(--color-border-tertiary)",
  borderRadius: 12,
  padding: "16px 20px",
};

const label = {
  fontSize: 11,
  fontWeight: 500,
  color: "var(--color-text-tertiary)",
  textTransform: "uppercase",
  letterSpacing: "0.06em",
  marginBottom: 12,
};

export default function App() {
  return (
    <div
      style={{
        minHeight: "100vh",
        background: "var(--color-background-tertiary)",
        padding: 24,
        fontFamily: "var(--font-sans)",
        color: "var(--color-text-primary)",
      }}
    >
      {/* Header */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: 24,
        }}
      >
        <div>
          <h1 style={{ fontSize: 20, fontWeight: 500, margin: 0 }}>SENTINEL</h1>
          <p style={{ fontSize: 13, color: "var(--color-text-secondary)", margin: "2px 0 0" }}>
            Local-first IoT edge gateway
          </p>
        </div>
        <StatusBadge />
      </div>

      {/* Top row — storage + devices */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: 16,
          marginBottom: 16,
        }}
      >
        <div style={card}>
          <p style={label}>Local storage</p>
          <StorageBar />
        </div>

        <div style={card}>
          <p style={label}>Connected devices</p>
          <DeviceList />
        </div>
      </div>

      {/* Sync log — full width */}
      <div style={card}>
        <p style={label}>Sync log</p>
        <SyncLog />
      </div>
    </div>
  );
}
