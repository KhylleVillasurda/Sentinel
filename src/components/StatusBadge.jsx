// components/StatusBadge.jsx
// Renders a colour-coded pill showing the current network health.
// Reads from useNetworkStatus — no props needed, self-contained.

import { useNetworkStatus } from "../hooks/useNetworkStatus";

const STYLES = {
  Stable:   { bg: "#EAF3DE", color: "#27500A", icon: "bi-check-circle-fill" },
  Degraded: { bg: "#FAEEDA", color: "#633806", icon: "bi-exclamation-triangle-fill" },
  Offline:  { bg: "#FCEBEB", color: "#501313", icon: "bi-wifi-off" },
  Unknown:  { bg: "#F1EFE8", color: "#2C2C2A", icon: "bi-question-circle-fill" },
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
        fontWeight: 600,
        padding: "4px 12px",
        borderRadius: 99,
        textTransform: "uppercase",
        letterSpacing: "0.02em",
      }}
    >
      <i className={s.icon} style={{ fontSize: 13 }} />
      {status}
    </span>
  );
}