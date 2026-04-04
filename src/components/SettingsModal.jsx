import { useState, useEffect } from "react";
import { getSettings, saveSettings } from "../lib/bridge";

export function SettingsModal({ isOpen, onClose }) {
  const [config, setConfig] = useState({ cloud_endpoint: "", ws_bind_address: "" });
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (isOpen) getSettings().then(setConfig);
  }, [isOpen]);

  const handleSave = async () => {
    setSaving(true);
    try {
      await saveSettings(config);
      onClose();
    } catch (e) {
      alert("Failed to save: " + e);
    } finally {
      setSaving(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div style={modalOverlay}>
      <div style={modalContent}>
        <h2 style={{ fontSize: 16, marginBottom: 20 }}>Gateway Configuration</h2>
        
        <div style={inputGroup}>
          <label style={label}>Cloud Ingestion Endpoint</label>
          <input 
            style={input}
            value={config.cloud_endpoint}
            onChange={e => setConfig({...config, cloud_endpoint: e.target.value})}
          />
        </div>

        <div style={inputGroup}>
          <label style={label}>WebSocket Bind Address</label>
          <input 
            style={input}
            value={config.ws_bind_address}
            onChange={e => setConfig({...config, ws_bind_address: e.target.value})}
          />
          <p style={{fontSize: 10, color: 'var(--color-text-tertiary)', marginTop: 4}}>
            Requires restart to rebind port.
          </p>
        </div>

        <div style={{ display: 'flex', gap: 12, marginTop: 24 }}>
          <button onClick={handleSave} disabled={saving} style={primaryBtn}>
            {saving ? "Saving..." : "Save Changes"}
          </button>
          <button onClick={onClose} style={secondaryBtn}>Cancel</button>
        </div>
      </div>
    </div>
  );
}

// Minimalist inline styles to match your App.jsx theme
const modalOverlay = { position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.8)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 100 };
const modalContent = { background: 'var(--color-background-primary)', padding: 24, borderRadius: 12, width: 400, border: '1px solid var(--color-border-tertiary)' };
const inputGroup = { marginBottom: 16 };
const label = { display: 'block', fontSize: 11, color: 'var(--color-text-tertiary)', marginBottom: 8, textTransform: 'uppercase' };
const input = { width: '100%', background: 'var(--color-background-tertiary)', border: '1px solid var(--color-border-tertiary)', padding: '8px 12px', color: 'white', borderRadius: 6 };
const primaryBtn = { background: 'var(--color-accent-green)', color: 'white', border: 'none', padding: '8px 16px', borderRadius: 6, cursor: 'pointer', fontWeight: 500 };
const secondaryBtn = { background: 'transparent', color: 'var(--color-text-secondary)', border: '1px solid var(--color-border-tertiary)', padding: '8px 16px', borderRadius: 6, cursor: 'pointer' };