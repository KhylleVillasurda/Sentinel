// App.jsx
// Root layout. Composes all dashboard components into one view.
// No data fetching here — each component owns its own polling via hooks.
// Add new sections here as phases complete.
import { useState } from "react"; // Add useState
import { SettingsModal } from "./components/SettingsModal";
import { StatusBadge } from "./components/StatusBadge";
import { StorageBar } from "./components/StorageBar";
import { DeviceList } from "./components/DeviceList";
import { LogViewer } from "./components/LogViewer";
import { PayloadViewer } from "./components/PayloadViewer";

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
  const [activeTab, setActiveTab] = useState("logs");

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

      {/* Edge Diagnostics (Log Viewer) — full width */}
      <div style={card}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
          <p style={{ ...label, marginBottom: 0 }}>Edge Diagnostics</p>
          <div style={{ display: "flex", gap: 8 }}>
            <button 
              onClick={() => setActiveTab("logs")}
              style={{
                ...tabButtonStyle,
                color: activeTab === "logs" ? "var(--color-text-primary)" : "var(--color-text-tertiary)",
                borderBottom: activeTab === "logs" ? "2px solid #1d9e75" : "none",
              }}
            >
              System Logs
            </button>
            <button 
              onClick={() => setActiveTab("payloads")}
              style={{
                ...tabButtonStyle,
                color: activeTab === "payloads" ? "var(--color-text-primary)" : "var(--color-text-tertiary)",
                borderBottom: activeTab === "payloads" ? "2px solid #1d9e75" : "none",
              }}
            >
              Decrypted Payloads
            </button>
          </div>
        </div>
        
        {activeTab === "logs" ? <LogViewer /> : <PayloadViewer />}
      </div>
      <SettingsModal
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
      />
    </div>
  );
}

const tabButtonStyle = {
  background: "none",
  border: "none",
  fontSize: 11,
  fontWeight: 600,
  textTransform: "uppercase",
  cursor: "pointer",
  padding: "4px 8px",
  transition: "all 0.2s",
};
