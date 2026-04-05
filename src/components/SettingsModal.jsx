import { useState, useEffect } from "react";
import { getSettings, saveSettings } from "../lib/bridge";

const TABS = {
  NETWORK: "Network",
  SYNC: "Sync",
  DIAGNOSTICS: "Diagnostics",
};

export function SettingsModal({ isOpen, onClose }) {
  const [activeTab, setActiveTab] = useState(TABS.NETWORK);
  const [config, setConfig] = useState({
    cloud_endpoint: "",
    ws_host: "0.0.0.0",
    ws_port: 6767,
    logging_enabled: true,
  });
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (isOpen) {
      getSettings().then((loadedConfig) => {
        setConfig(loadedConfig);
      });
    }
  }, [isOpen]);

  const handleSave = async () => {
    setSaving(true);
    try {
      // Validate Port before sending
      if (config.ws_port < 1024 || config.ws_port > 65535) {
        throw new Error("Invalid Port. Use 1024-65535.");
      }
      await saveSettings(config);
      onClose();
    } catch (e) {
      alert("Configuration Error: " + e.message);
    } finally {
      setSaving(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div style={modalOverlay}>
      <div style={modalContent}>
        {/* MODAL HEADER: Title + Tab Bar */}
        <div style={header}>
          <h2 style={{ fontSize: 16, margin: 0, fontWeight: 500 }}>
            Gateway Configuration
          </h2>

          <div style={tabBar}>
            {Object.values(TABS).map((tab) => (
              <button
                key={tab}
                onClick={() => setActiveTab(tab)}
                style={activeTab === tab ? activeTabBtn : tabBtn}
              >
                {tab}
              </button>
            ))}
          </div>
        </div>

        {/* MODAL BODY: Displays the active tab's form fields */}
        <div style={formBody}>
          {/* NETWORK TAB */}
          {activeTab === TABS.NETWORK && (
            <>
              <div style={inputGroup}>
                <label style={label}>WebSocket Bind Host (IP)</label>
                <input
                  style={input}
                  placeholder="e.g. 0.0.0.0"
                  value={config.ws_host}
                  onChange={(e) =>
                    setConfig({ ...config, ws_host: e.target.value })
                  }
                />
              </div>
              <div style={inputGroup}>
                <label style={label}>WebSocket Bind Port</label>
                <input
                  style={input}
                  type="number"
                  placeholder="e.g. 6767"
                  min="1024"
                  max="65535"
                  value={config.ws_port}
                  onChange={(e) =>
                    setConfig({
                      ...config,
                      ws_port: parseInt(e.target.value) || 0,
                    })
                  }
                />
              </div>
              <p style={restartNote}>Requires restart to rebind port.</p>
            </>
          )}

          {/* SYNC TAB */}
          {activeTab === TABS.SYNC && (
            <div style={inputGroup}>
              <label style={label}>Cloud Ingestion Endpoint</label>
              <input
                style={input}
                placeholder="http://my-cloud.com/api/ingest"
                value={config.cloud_endpoint}
                onChange={(e) =>
                  setConfig({ ...config, cloud_endpoint: e.target.value })
                }
              />
            </div>
          )}

          {/* DIAGNOSTICS TAB */}
          {activeTab === TABS.DIAGNOSTICS && (
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                alignItems: "center",
                marginTop: 16,
              }}
            >
              <span
                style={{ fontSize: 13, color: "var(--color-text-secondary)" }}
              >
                Enable High-Frequency System Logging
              </span>
              <input
                type="checkbox"
                checked={config.logging_enabled}
                onChange={(e) =>
                  setConfig({ ...config, logging_enabled: e.target.checked })
                }
                style={{
                  cursor: "pointer",
                  accentColor: "var(--color-accent-green)",
                }}
              />
            </div>
          )}
        </div>

        {/* MODAL FOOTER: Action Buttons */}
        <div style={footer}>
          <button onClick={handleSave} disabled={saving} style={primaryBtn}>
            {saving ? "Applying..." : "Save Changes"}
          </button>
          <button onClick={onClose} style={secondaryBtn}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

// --- Styles Section ---
const modalOverlay = {
  position: "fixed",
  top: 0,
  left: 0,
  right: 0,
  bottom: 0,
  background: "rgba(0,0,0,0.8)",
  display: "flex",
  justifyContent: "center",
  alignItems: "center",
  zIndex: 1000,
};
const modalContent = {
  background: "var(--color-bg-panel)",
  padding: "28px 32px",
  borderRadius: 12,
  width: "460px",
  border: "1px solid var(--color-border-tertiary)",
  display: "flex",
  flexDirection: "column",
};
const header = { marginBottom: 0 };
const tabBar = {
  display: "flex",
  gap: 24,
  marginTop: 16,
  borderBottom: "1px solid var(--color-border-tertiary)",
  paddingBottom: 12,
};
const tabBtn = {
  background: "none",
  border: "none",
  color: "var(--color-text-secondary)",
  cursor: "pointer",
  fontSize: 13,
  transition: "all 0.2s",
  padding: 0,
};
const activeTabBtn = {
  ...tabBtn,
  color: "var(--color-accent-green)",
  fontWeight: 500,
};
const formBody = {
  minHeight: "150px",
  paddingTop: "24px",
  display: "flex",
  flexDirection: "column",
  justifyContent: "flex-start",
};
const inputGroup = { marginBottom: 16 };
const label = {
  display: "block",
  fontSize: 11,
  textTransform: "uppercase",
  color: "var(--color-text-tertiary)",
  marginBottom: 8,
  letterSpacing: "0.5px",
};
const input = {
  width: "100%",
  background: "var(--color-bg-main)",
  border: "1px solid var(--color-border-tertiary)",
  borderRadius: 6,
  padding: "10px 12px",
  color: "var(--color-text-main)",
  fontSize: 13,
  boxSizing: "border-box",
};
const restartNote = {
  fontSize: 12,
  color: "var(--color-text-tertiary)",
  marginTop: -4,
  marginBottom: 0,
};
const footer = {
  display: "flex",
  gap: 12,
  marginTop: 32,
  justifyContent: "flex-start",
};
const primaryBtn = {
  background: "var(--color-accent-green)",
  border: "none",
  color: "#FFF",
  padding: "8px 16px",
  borderRadius: 6,
  cursor: "pointer",
  fontSize: 13,
  fontWeight: 500,
};
const secondaryBtn = {
  background: "none",
  border: "none",
  color: "var(--color-text-secondary)",
  padding: "8px 16px",
  borderRadius: 6,
  cursor: "pointer",
  fontSize: 13,
};
