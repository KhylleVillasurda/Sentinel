// components/StatusBadge.jsx
// Renders a colour-coded pill showing the current network health.
// Reads from useNetworkStatus — no props needed, self-contained.

import { useNetworkStatus } from "../hooks/useNetworkStatus";

const STYLES = {
  Stable:   { bg: "#EAF3DE", color: "#27500A", dot: "#639922" },
  Degraded: { bg: "#FAEEDA", color: "#633806", dot: "#BA7517" },
  Offline:  { bg: "#FCEBEB", color: "#501313", dot: "#E24B4A" },
  Unknown:  { bg: "#F1EFE8", color: "#2C2C2A", dot: "#888780" },
};

export function StatusBadge() {
  const { status } = useNetworkStatus();
  const s = STYLES[status] ?? STYLES.Unknown;

  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        background: s.bg,
        color: s.color,
        fontSize: 12,
        fontWeight: 500,
        padding: "3px 10px",
        borderRadius: 99,
      }}
    >
      <span
        style={{
          width: 7,
          height: 7,
          borderRadius: "50%",
          background: s.dot,
          flexShrink: 0,
        }}
      />
      {status}
    </span>
  );
}