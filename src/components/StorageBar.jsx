// components/StorageBar.jsx
// Shows local DB disk usage and the number of rows waiting to sync.
// Reads from useStorageStats — no props needed.

import { useStorageStats } from "../hooks/useStorageStats";

const MAX_DISPLAY_KB = 102400; // treat 100 MB as "full" for the bar

export function StorageBar() {
  const { totalRows, unsyncedRows, sizeKb } = useStorageStats();

  const fillPct = Math.min((sizeKb / MAX_DISPLAY_KB) * 100, 100);
  const sizeLabel = sizeKb >= 1024
    ? `${(sizeKb / 1024).toFixed(1)} MB`
    : `${sizeKb} KB`;

  return (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 6 }}>
        <span style={{ fontSize: 13, color: "var(--color-text-secondary)" }}>
          Local storage
        </span>
        <span style={{ fontSize: 13, fontWeight: 500 }}>{sizeLabel}</span>
      </div>

      {/* Progress bar */}
      <div
        style={{
          height: 6,
          background: "var(--color-border-tertiary)",
          borderRadius: 3,
          overflow: "hidden",
        }}
      >
        <div
          style={{
            height: "100%",
            width: `${fillPct}%`,
            background: fillPct > 80 ? "#E24B4A" : "#1D9E75",
            borderRadius: 3,
            transition: "width 0.4s ease",
          }}
        />
      </div>

      <div style={{ display: "flex", justifyContent: "space-between", marginTop: 6 }}>
        <span style={{ fontSize: 12, color: "var(--color-text-secondary)" }}>
          {totalRows.toLocaleString()} rows total
        </span>
        {unsyncedRows > 0 && (
          <span
            style={{
              fontSize: 11,
              fontWeight: 500,
              padding: "1px 8px",
              borderRadius: 99,
              background: "#FAEEDA",
              color: "#633806",
            }}
          >
            {unsyncedRows.toLocaleString()} pending sync
          </span>
        )}
      </div>
    </div>
  );
}
