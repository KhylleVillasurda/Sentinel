# <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/tools.svg" width="32" height="32"> SENTINEL-X Setup Guide

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/router.svg" width="20" height="20"> 1. Network Configuration
To test the ingestion pipeline with physical mobile devices, ensure the development environment is bridged correctly.

### Option A: USB Tethering
1. Connect the mobile device via USB and enable USB Tethering.
2. Execute `ipconfig` and locate the Ethernet adapter IPv4 address.
3. In `phone-agent.html`, set the gateway to `ws://<ETHERNET_IP>:6767`.

### Option B: Mobile Hotspot
1. Enable Windows Mobile Hotspot on the host laptop.
2. Connect the secondary test device to the laptop's Wi-Fi network.
3. Use the Local Area Connection IPv4 address (typically 192.168.137.1) in `phone-agent.html`.

---

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/shield-plus.svg" width="20" height="20"> 2. Secure Pairing (Phase 2)
SENTINEL now uses a **Secure Pairing** model. Devices cannot stream data until they are registered.

1. Open the SENTINEL Dashboard.
2. Navigate to **Settings** > **Pairing Mode**.
3. Toggle "Enable Pairing Window" (60-second window).
4. Connect the mobile agent; it will receive and store a unique `auth_token` in `localStorage`.
5. Future connections will use this token automatically.

---

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/cloud-haze2.svg" width="20" height="20"> 3. Mock Cloud Endpoint
The sync engine requires an active listener to process the local SQLCipher buffer.

1. **Initialize Storage:** Confirm `received-payloads.ndjson` exists in the root directory.
2. **Launch Server:** `node local-cloud-v2.cjs`
3. **Troubleshooting EADDRINUSE:** If port 9000 is occupied, terminate the process:
   `taskkill /F /IM node.exe`

---

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/rust.svg" width="20" height="20"> 4. Backend (Rust/Tauri)
### Prerequisites
* **Rust (Stable Toolchain)**: `rustup update`
* **WebView2 Runtime**: (Windows only)
* **SQLCipher Libraries**: Ensure the build environment can locate encrypted SQLite headers.

### Initialization
```bash
cd src-tauri
cargo tauri dev
```

> **Note:** If a "file is not a database" error occurs, delete the local app data at `%APPDATA%\com.sentinel.gateway` to reset the encryption key synchronization.

---

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/phone.svg" width="20" height="20"> 5. Testing the Ingestion Flow
1. Open `phone-agent.html` in a mobile browser.
2. Enter the Gateway WebSocket URL identified in Step 1.
3. Select **Connect**.
4. Verify "Connected Devices" on the SENTINEL Dashboard and check the Mock Cloud terminal for "Batch Received" logs.
