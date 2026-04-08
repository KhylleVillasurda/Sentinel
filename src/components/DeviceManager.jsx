import { useState, useEffect } from "react";
import { 
  getRegisteredDevices, 
  revokeDevice, 
  togglePairingMode, 
  isPairingModeActive,
  getPairingExpiry 
} from "../lib/bridge";

export function DeviceManager() {
  const [devices, setDevices] = useState([]);
  const [isPairing, setIsPairing] = useState(false);
  const [timeLeft, setTimeLeft] = useState(0);

  const refreshDevices = () => {
    getRegisteredDevices().then(setDevices).catch(console.error);
  };

  useEffect(() => {
    refreshDevices();
    isPairingModeActive().then(setIsPairing);
    getPairingExpiry().then(expiry => {
      if (expiry > 0) {
        const now = Math.floor(Date.now() / 1000);
        setTimeLeft(Math.max(0, expiry - now));
      }
    });
    
    const interval = setInterval(refreshDevices, 5000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    let timer;
    if (isPairing && timeLeft > 0) {
      timer = setInterval(() => {
        setTimeLeft(prev => {
          if (prev <= 1) {
            setIsPairing(false);
            return 0;
          }
          return prev - 1;
        });
      }, 1000);
    }
    return () => clearInterval(timer);
  }, [isPairing, timeLeft]);

  const handleStartPairing = async () => {
    const expiry = await togglePairingMode(true);
    const now = Math.floor(Date.now() / 1000);
    setTimeLeft(Math.max(0, expiry - now));
    setIsPairing(true);
  };

  const handleRevoke = async (id) => {
    if (confirm(`Are you sure you want to revoke device ${id}?`)) {
      await revokeDevice(id);
      refreshDevices();
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div style={{ 
        display: "flex", 
        justifyContent: "space-between", 
        alignItems: "center",
        padding: "12px",
        background: "var(--color-background-secondary)",
        borderRadius: 8,
        border: "1px solid var(--color-border-tertiary)"
      }}>
        <div>
          <h4 style={{ margin: 0, fontSize: 14 }}>Pairing Mode</h4>
          <p style={{ margin: "4px 0 0", fontSize: 12, color: "var(--color-text-secondary)" }}>
            {isPairing 
              ? `Accepting new devices for ${timeLeft}s...` 
              : "Disabled. Enable to register new devices."}
          </p>
        </div>
        <button
          onClick={handleStartPairing}
          disabled={isPairing}
          style={{
            padding: "6px 12px",
            borderRadius: 6,
            border: "none",
            background: isPairing ? "var(--color-text-tertiary)" : "var(--color-accent-green)",
            color: "white",
            fontSize: 12,
            cursor: isPairing ? "default" : "pointer",
            fontWeight: 500
          }}
        >
          {isPairing ? "Pairing Active" : "Enable Pairing"}
        </button>
      </div>

      <div>
        <h4 style={{ fontSize: 12, color: "var(--color-text-tertiary)", textTransform: "uppercase", marginBottom: 8 }}>
          Registered Devices ({devices.length})
        </h4>
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {devices.map(dev => (
            <div key={dev.device_id} style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              padding: "10px",
              border: "1px solid var(--color-border-tertiary)",
              borderRadius: 8,
              fontSize: 13
            }}>
              <div>
                <div style={{ fontWeight: 500 }}>{dev.friendly_name}</div>
                <div style={{ fontSize: 11, color: "var(--color-text-secondary)" }}>ID: {dev.device_id}</div>
              </div>
              <button 
                onClick={() => handleRevoke(dev.device_id)}
                style={{
                  padding: "4px 8px",
                  fontSize: 11,
                  background: "none",
                  border: "1px solid var(--color-accent-red)",
                  color: "var(--color-accent-red)",
                  borderRadius: 4,
                  cursor: "pointer"
                }}
              >
                Revoke
              </button>
            </div>
          ))}
          {devices.length === 0 && (
            <p style={{ fontSize: 13, color: "var(--color-text-secondary)", textAlign: "center", padding: "20px 0" }}>
              No devices registered yet.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
