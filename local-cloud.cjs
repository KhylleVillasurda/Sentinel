// local-cloud.cjs
// Simulates the cloud endpoint locally.
// Receives encrypted payload batches from SENTINEL's sync engine,
// logs them to the console, and appends them to received-payloads.json.
//
// Run with: node local-cloud.cjs
// SENTINEL should point to: http://127.0.0.1:9000/ingest

const http = require("http");
const fs = require("fs");
const path = require("path");

const PORT = 9000;
const ENDPOINT = "/ingest";
const OUTPUT_FILE = path.join(__dirname, "received-payloads.json");

// Initialize output file if it doesn't exist
if (!fs.existsSync(OUTPUT_FILE)) {
  fs.writeFileSync(OUTPUT_FILE, JSON.stringify([], null, 2));
}

const server = http.createServer((req, res) => {
  // Health check
  if (req.method === "GET" && req.url === "/health") {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ status: "ok" }));
    return;
  }

  // Only accept POST to /ingest
  if (req.method !== "POST" || req.url !== ENDPOINT) {
    res.writeHead(404);
    res.end();
    return;
  }

  let body = "";
  req.on("data", (chunk) => (body += chunk));
  req.on("end", () => {
    try {
      const batch = JSON.parse(body);
      const payloads = batch.payloads ?? [];
      const receivedAt = new Date().toISOString();

      // --- Console log ---
      console.log("\n─────────────────────────────────────────");
      console.log(`[cloud] ✓ Batch received at ${receivedAt}`);
      console.log(`[cloud]   ${payloads.length} payload(s)`);
      payloads.forEach((p, i) => {
        console.log(
          `[cloud]   [${i + 1}] device=${p.device_id} ` +
            `received_at=${new Date(p.received_at * 1000).toLocaleTimeString()} ` +
            `blob_len=${p.encrypted_blob?.length ?? 0} chars`,
        );
      });
      console.log("─────────────────────────────────────────");

      // --- Append to JSON file ---
      const existing = JSON.parse(fs.readFileSync(OUTPUT_FILE, "utf8"));
      existing.push({
        received_at: receivedAt,
        payload_count: payloads.length,
        payloads,
      });
      fs.writeFileSync(OUTPUT_FILE, JSON.stringify(existing, null, 2));

      // --- Respond 200 so SENTINEL marks rows as synced ---
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: true, received: payloads.length }));
    } catch (err) {
      console.error("[cloud] Failed to parse batch:", err.message);
      res.writeHead(400);
      res.end(JSON.stringify({ error: "Invalid JSON" }));
    }
  });
});

server.listen(PORT, "127.0.0.1", () => {
  console.log("╔══════════════════════════════════════════╗");
  console.log("║       SENTINEL Local Cloud Endpoint      ║");
  console.log("╠══════════════════════════════════════════╣");
  console.log(`║  Listening on http://127.0.0.1:${PORT}/ingest  ║`);
  console.log(`║  Saving to: received-payloads.json       ║`);
  console.log("╚══════════════════════════════════════════╝");
});
