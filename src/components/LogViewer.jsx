import { useState, useEffect, useRef, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { getLogBuffer, isLoggingEnabled, setLoggingEnabled } from "../lib/bridge";

const MAX_BUFFER_SIZE = 500;

const COLORS = {
  trace: "#71717a",
  debug: "#a1a1aa",
  info: "#1d9e75",
  warn: "#f59e0b",
  error: "#e24b4a",
};

const SUBSYSTEMS = ["WS", "SYNC", "DB", "NETWORK", "CONFIG", "AUTH", "GENERAL"];
const LEVELS = ["trace", "debug", "info", "warn", "error"];

function formatTimestamp(ts) {
  return new Date(ts * 1000).toLocaleTimeString([], {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function LogViewer() {
  const [logs, setLogs] = useState([]);
  const [isEnabled, setIsEnabled] = useState(true);
  const [filterSubsystem, setFilterSubsystem] = useState("ALL");
  const [filterLevel, setFilterLevel] = useState("info"); // Default to info and above
  const [searchQuery, setSearchQuery] = useState("");
  const [autoScroll, setAutoScroll] = useState(true);
  
  const scrollRef = useRef(null);

  useEffect(() => {
    // Initial load
    isLoggingEnabled().then(setIsEnabled);
    getLogBuffer().then(setLogs);

    // Listen for real-time events
    const unlisten = listen("new-log-event", (event) => {
      setLogs((prev) => [event.payload, ...prev].slice(0, MAX_BUFFER_SIZE));
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = 0; // Since we are using flex-direction: column-reverse or just prepending
    }
  }, [logs, autoScroll]);

  const handleToggle = async () => {
    const next = !isEnabled;
    await setLoggingEnabled(next);
    setIsEnabled(next);
  };

  const filteredLogs = useMemo(() => {
    return logs.filter((log) => {
      const matchesSubsystem = filterSubsystem === "ALL" || log.subsystem === filterSubsystem;
      
      const levelIdx = LEVELS.indexOf(log.level);
      const minLevelIdx = LEVELS.indexOf(filterLevel);
      const matchesLevel = levelIdx >= minLevelIdx;

      const matchesSearch = log.message.toLowerCase().includes(searchQuery.toLowerCase()) ||
                            log.subsystem.toLowerCase().includes(searchQuery.toLowerCase());

      return matchesSubsystem && matchesLevel && matchesSearch;
    });
  }, [logs, filterSubsystem, filterLevel, searchQuery]);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "400px" }}>
      {/* Controls */}
      <div style={{ 
        display: "flex", 
        gap: 12, 
        marginBottom: 12, 
        flexWrap: "wrap",
        alignItems: "center",
        fontSize: 12 
      }}>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <label style={{ color: "var(--color-text-tertiary)" }}>Subsystem:</label>
          <select 
            value={filterSubsystem} 
            onChange={(e) => setFilterSubsystem(e.target.value)}
            style={selectStyle}
          >
            <option value="ALL">All</option>
            {SUBSYSTEMS.map(s => <option key={s} value={s}>{s}</option>)}
          </select>
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <label style={{ color: "var(--color-text-tertiary)" }}>Min Level:</label>
          <select 
            value={filterLevel} 
            onChange={(e) => setFilterLevel(e.target.value)}
            style={selectStyle}
          >
            {LEVELS.map(l => <option key={l} value={l}>{l.toUpperCase()}</option>)}
          </select>
        </div>

        <input 
          type="text" 
          placeholder="Search logs..." 
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          style={{
            background: "var(--color-background-secondary)",
            border: "1px solid var(--color-border-tertiary)",
            borderRadius: 4,
            padding: "4px 8px",
            color: "var(--color-text-primary)",
            fontSize: 12,
            flex: 1,
            minWidth: "150px"
          }}
        />

        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <button 
            onClick={() => setLogs([])}
            style={buttonStyle}
          >
            Clear
          </button>
          <label style={{ display: "flex", alignItems: "center", gap: 4, cursor: "pointer" }}>
            <input 
              type="checkbox" 
              checked={isEnabled} 
              onChange={handleToggle}
            />
            Logging
          </label>
        </div>
      </div>

      {/* Terminal View */}
      <div 
        ref={scrollRef}
        style={{
          flex: 1,
          background: "#000000",
          borderRadius: 8,
          padding: "12px",
          fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
          fontSize: 12,
          overflowY: "auto",
          border: "1px solid var(--color-border-tertiary)",
          display: "flex",
          flexDirection: "column", // Newer logs at top might be better for this height
        }}
      >
        {filteredLogs.map((log, i) => (
          <div key={i} style={{ 
            display: "flex", 
            gap: 8, 
            padding: "2px 0",
            borderBottom: "1px solid #1a1a1a",
            whiteSpace: "pre-wrap"
          }}>
            <span style={{ color: "#555", flexShrink: 0 }}>
              {formatTimestamp(log.timestamp)}
            </span>
            <span style={{ 
              color: COLORS[log.level], 
              fontWeight: "bold",
              minWidth: "45px",
              flexShrink: 0
            }}>
              {log.level.toUpperCase()}
            </span>
            <span style={{ 
              color: "#888", 
              minWidth: "70px",
              flexShrink: 0 
            }}>
              [{log.subsystem}]
            </span>
            <span style={{ color: "#ddd" }}>
              {log.message}
            </span>
          </div>
        ))}
        {filteredLogs.length === 0 && (
          <div style={{ color: "var(--color-text-tertiary)", textAlign: "center", marginTop: 20 }}>
            No logs matching filters
          </div>
        )}
      </div>
    </div>
  );
}

const selectStyle = {
  background: "var(--color-background-secondary)",
  border: "1px solid var(--color-border-tertiary)",
  borderRadius: 4,
  padding: "3px 6px",
  color: "var(--color-text-primary)",
  fontSize: 12,
  cursor: "pointer"
};

const buttonStyle = {
  background: "none",
  border: "1px solid var(--color-border-tertiary)",
  color: "var(--color-text-secondary)",
  borderRadius: 4,
  padding: "4px 8px",
  cursor: "pointer",
  fontSize: 11
};
