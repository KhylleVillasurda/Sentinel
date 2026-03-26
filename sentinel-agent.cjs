// sentinel-agent.js
// Simulates a real IoT device — reads live laptop metrics and
// sends them to SENTINEL's WebSocket every 5 seconds.
// Run with: node sentinel-agent.js

const WebSocket = require("ws");
const si = require("systeminformation");

const SENTINEL_URL = "ws://10.251.58.25:6767";
const SEND_INTERVAL_MS = 5000;

let ws = null;
let reconnectTimer = null;

function connect() {
  console.log(`[agent] Connecting to SENTINEL at ${SENTINEL_URL}...`);
  ws = new WebSocket(SENTINEL_URL);

  ws.on("open", () => {
    console.log("[agent] Connected to SENTINEL. Sending metrics every 5s.");
    startSending();
  });

  ws.on("message", (data) => {
    // 0x01 = ACK from SENTINEL
    if (data[0] === 1) {
      console.log("[agent] ✓ ACK received — payload stored and encrypted");
    }
  });

  ws.on("close", () => {
    console.log("[agent] Disconnected. Reconnecting in 5s...");
    stopSending();
    reconnectTimer = setTimeout(connect, 5000);
  });

  ws.on("error", (err) => {
    console.error("[agent] Connection error:", err.message);
  });
}

let sendTimer = null;

function startSending() {
  sendTimer = setInterval(async () => {
    try {
      const [cpu, mem, battery] = await Promise.all([
        si.currentLoad(),
        si.mem(),
        si.battery(),
      ]);

      const payload = JSON.stringify({
        device: "laptop",
        timestamp: Date.now(),
        cpu_load_pct: parseFloat(cpu.currentLoad.toFixed(1)),
        mem_used_mb: Math.round(mem.active / 1024 / 1024),
        mem_total_mb: Math.round(mem.total / 1024 / 1024),
        battery_pct: battery.percent ?? null,
        battery_charging: battery.isCharging ?? null,
      });

      if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(payload);
        console.log(`[agent] → Sent: ${payload}`);
      }
    } catch (err) {
      console.error("[agent] Failed to read metrics:", err.message);
    }
  }, SEND_INTERVAL_MS);
}

function stopSending() {
  if (sendTimer) {
    clearInterval(sendTimer);
    sendTimer = null;
  }
}

// Graceful shutdown on Ctrl+C
process.on("SIGINT", () => {
  console.log("\n[agent] Shutting down...");
  stopSending();
  if (ws) ws.close();
  if (reconnectTimer) clearTimeout(reconnectTimer);
  process.exit(0);
});

connect();
