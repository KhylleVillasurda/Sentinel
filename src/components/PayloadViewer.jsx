import { useState, useEffect } from "react";
import { getDecryptedPayloads } from "../lib/bridge";

function formatTimestamp(ts) {
  return new Date(ts * 1000).toLocaleString([], {
    month: "short",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function PayloadViewer() {
  const [payloads, setPayloads] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const [autoRefresh, setAutoRefresh] = useState(false);

  const fetchPayloads = async () => {
    setLoading(true);
    try {
      const data = await getDecryptedPayloads(50);
      setPayloads(data);
      setError(null);
    } catch (err) {
      console.error("Failed to fetch payloads:", err);
      setError(err.toString());
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchPayloads();
  }, []);

  useEffect(() => {
    let interval;
    if (autoRefresh) {
      interval = setInterval(fetchPayloads, 3000);
    }
    return () => clearInterval(interval);
  }, [autoRefresh]);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <h3 style={{ margin: 0, fontSize: 14, color: "var(--color-text-primary)" }}>
          Payload Decryption Viewer
        </h3>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 12, cursor: "pointer" }}>
            <input 
              type="checkbox" 
              checked={autoRefresh} 
              onChange={(e) => setAutoRefresh(e.target.checked)} 
            />
            Auto-refresh (3s)
          </label>
          <button 
            onClick={fetchPayloads} 
            disabled={loading}
            style={buttonStyle}
          >
            {loading ? "Refreshing..." : "Refresh Now"}
          </button>
        </div>
      </div>

      {error && (
        <div style={{ color: "#e24b4a", fontSize: 12, padding: 8, background: "rgba(226, 75, 74, 0.1)", borderRadius: 4 }}>
          Error: {error}
        </div>
      )}

      <div style={terminalContainerStyle}>
        {payloads.length === 0 && !loading && (
          <div style={{ color: "#555", textAlign: "center", padding: "20px" }}>
            No payloads found in database.
          </div>
        )}
        
        {payloads.map((p) => (
          <div key={p.id} style={payloadRowStyle}>
            <div style={payloadHeaderStyle}>
              <span style={{ color: "#888" }}>[{formatTimestamp(p.received_at)}]</span>
              <span style={{ color: "#1d9e75", fontWeight: "bold" }}>{p.device_id}</span>
              <span style={{ color: "#555", fontSize: 10 }}>ID: {p.id}</span>
            </div>
            <div style={payloadDataStyle}>
              {p.decrypted_data}
            </div>
          </div>
        ))}
      </div>
      <p style={{ fontSize: 11, color: "var(--color-text-tertiary)", margin: 0 }}>
        * Only the most recent 50 payloads are shown. Decryption performed on-the-fly using the master key.
      </p>
    </div>
  );
}

const buttonStyle = {
  background: "var(--color-background-secondary)",
  border: "1px solid var(--color-border-tertiary)",
  color: "var(--color-text-primary)",
  borderRadius: 4,
  padding: "4px 10px",
  cursor: "pointer",
  fontSize: 12
};

const terminalContainerStyle = {
  background: "#0a0a0a",
  borderRadius: 8,
  padding: "12px",
  fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
  fontSize: 13,
  height: "400px",
  overflowY: "auto",
  border: "1px solid var(--color-border-tertiary)",
  display: "flex",
  flexDirection: "column",
  gap: 8
};

const payloadRowStyle = {
  borderBottom: "1px solid #1a1a1a",
  paddingBottom: 8,
  display: "flex",
  flexDirection: "column",
  gap: 4
};

const payloadHeaderStyle = {
  display: "flex",
  gap: 12,
  alignItems: "center",
  fontSize: 11
};

const payloadDataStyle = {
  color: "#ddd",
  wordBreak: "break-all",
  whiteSpace: "pre-wrap",
  padding: "4px 8px",
  background: "#121212",
  borderRadius: 4,
  borderLeft: "2px solid #1d9e75"
};
