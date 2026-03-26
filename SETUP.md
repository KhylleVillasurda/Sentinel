# SENTINEL — Setup Guide

This guide covers everything needed to get SENTINEL running on a fresh Windows machine. macOS and Linux steps are noted where they differ.

---

## Prerequisites

### 1. Rust

Install from https://rustup.rs — accept all defaults.

Verify:

```bash
rustc --version
cargo --version
```

### 2. Node.js

Install v18 or later from https://nodejs.org.

Verify:

```bash
node --version
npm --version
```

### 3. Tauri CLI

```bash
cargo install tauri-cli
```

### 4. Strawberry Perl _(Windows only)_

Required to compile SQLCipher from source.

Download the 64-bit installer from https://strawberryperl.com and install with all defaults. It adds `perl` to your PATH automatically.

Verify (in a **new** terminal after installing):

```bash
perl --version
```

### 5. WebView2 _(Windows only)_

Usually pre-installed on Windows 11. If missing, download from:
https://developer.microsoft.com/en-us/microsoft-edge/webview2/

---

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/sentinel.git
cd sentinel

# Install frontend dependencies
npm install
```

---

## Development

Open **three terminals** from the project root:

**Terminal 1 — Local cloud receiver** _(optional but recommended for testing)_

```bash
node local-cloud.cjs
```

Listens on `http://127.0.0.1:9000/ingest` and logs all received payloads to `received-payloads.json`.

**Terminal 2 — SENTINEL app**

```bash
cargo tauri dev
```

> ⚠️ First run compiles SQLCipher and OpenSSL from source — expect 5–20 minutes. All subsequent runs take seconds.

**Terminal 3 — Laptop sensor agent** _(optional)_

```bash
node sentinel-agent.cjs
```

Sends real CPU and memory metrics to SENTINEL every 5 seconds.

---

## Running Tests

All tests are in the Rust library crate. Always run from `src-tauri/`:

```bash
cd src-tauri

# Run all tests
cargo test --lib

# Run tests for a specific module
cargo test --lib crypto
cargo test --lib db
cargo test --lib ws
cargo test --lib network
cargo test --lib sync
cargo test --lib commands
```

Expected output: **30 tests, all passing.**

---

## Phone as a Sensor

To send real phone sensor data (GPS, accelerometer, battery) to SENTINEL:

**Step 1 — Find your laptop IP**

```bash
ipconfig     # Windows
ifconfig     # macOS / Linux
```

Look for the IPv4 address on your WiFi adapter, e.g. `192.168.1.105`.

**Step 2 — Allow external connections**

In `src-tauri/src/ws.rs` change:

```rust
let addr = format!("127.0.0.1:{WS_PORT}");
// to:
let addr = format!("0.0.0.0:{WS_PORT}");
```

**Step 3 — Open firewall port** _(Windows, run as Administrator)_

```powershell
New-NetFirewallRule -DisplayName "SENTINEL WebSocket" -Direction Inbound -Protocol TCP -LocalPort 6767 -Action Allow
```

**Step 4 — Serve the phone agent page**

```bash
npx serve . -p 8080
```

**Step 5 — Open on your phone**

Make sure your phone and laptop are on the same WiFi, then open:

```
http://192.168.1.105:8080/phone-agent.html
```

Enter your laptop IP, tap **Connect**, and your phone will start sending sensor data to SENTINEL.

---

## Building for Production

```bash
cargo tauri build
```

The installer is generated at:

```
src-tauri/target/release/bundle/nsis/SENTINEL_0.1.0_x64-setup.exe   # Windows
src-tauri/target/release/bundle/dmg/SENTINEL_0.1.0_x64.dmg          # macOS
src-tauri/target/release/bundle/deb/sentinel_0.1.0_amd64.deb        # Linux
```

---

## Environment Variables

| Variable      | Value                            | Required on |
| ------------- | -------------------------------- | ----------- |
| `OPENSSL_DIR` | `C:\Program Files\OpenSSL-Win64` | Windows     |

Set permanently in PowerShell:

```powershell
[System.Environment]::SetEnvironmentVariable("OPENSSL_DIR","C:\Program Files\OpenSSL-Win64","User")
```

---

## Project Structure

```
sentinel/
├── src/                     React frontend
│   ├── lib/bridge.js        Single Tauri invoke() import point
│   ├── hooks/               useNetworkStatus, useStorageStats
│   └── components/          StatusBadge, DeviceList, StorageBar, SyncLog
├── src-tauri/
│   ├── Cargo.toml
│   ├── build.rs             Required for Tauri generate_context!()
│   └── src/
│       ├── main.rs          App entry — wires everything together
│       ├── lib.rs           Module registry for cargo test
│       ├── state.rs         Shared AppState struct
│       ├── commands.rs      Tauri invoke() endpoints
│       ├── crypto.rs        AES-256-GCM encrypt / decrypt
│       ├── network.rs       Ping loop → Stable / Degraded / Offline
│       ├── sync.rs          Drains unsynced rows when Stable
│       ├── ws.rs            WebSocket server on port 6767
│       └── db/
│           ├── mod.rs       SQLCipher init + migrations
│           └── queries.rs   INSERT / SELECT / UPDATE
├── sentinel-agent.cjs       Laptop IoT sensor agent
├── local-cloud.cjs          Local cloud endpoint for testing
└── phone-agent.html         Phone browser sensor agent
```

---

## Troubleshooting

**`OUT_DIR env var is not set`**
→ Use `cargo test --lib` not `cargo test`. Never run cargo from inside `src/`.

**`Blocking waiting for file lock on build directory`**

```bash
taskkill /F /IM cargo.exe
taskkill /F /IM rustc.exe
```

**`vite is not recognized`**
→ Change `beforeDevCommand` in `tauri.conf.json` to `npm run dev`.

**SQLCipher compile fails on Windows**
→ Install Strawberry Perl from https://strawberryperl.com, open a new terminal, verify with `perl --version`.

**Phone can't connect to SENTINEL**
→ Check both devices are on the same WiFi. Verify `ws.rs` binds to `0.0.0.0`. Check firewall rule is added.
