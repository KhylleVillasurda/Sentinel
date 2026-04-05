// App.jsx
// Root layout. Composes all dashboard components into one view.
// No data fetching here — each component owns its own polling via hooks.
// Add new sections here as phases complete.
import { useState } from "react"; // Add useState
import { SettingsModal } from "./components/SettingsModal";
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
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);

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
          <p
            style={{
              fontSize: 13,
              color: "var(--color-text-secondary)",
              margin: "2px 0 0",
            }}
          >
            Local-first IoT edge gateway
          </p>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          {/* Clean up the button style to match the dashboard */}
          <button
            onClick={() => setIsSettingsOpen(true)}
            style={{
              background: "none",
              border: "1px solid var(--color-border-tertiary)",
              color: "var(--color-text-secondary)",
              borderRadius: 6,
              padding: "4px 10px", // added subtle padding
              cursor: "pointer",
              fontSize: 12,
              transition: "all 0.2s", // added subtle hover transition
            }}
            onMouseOver={(e) =>
              (e.currentTarget.style.borderColor =
                "var(--color-text-secondary)")
            }
            onMouseOut={(e) =>
              (e.currentTarget.style.borderColor =
                "var(--color-border-tertiary)")
            }
          >
            Settings
          </button>
          <StatusBadge />
        </div>
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
      <SettingsModal
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
      />
    </div>
  );
}
