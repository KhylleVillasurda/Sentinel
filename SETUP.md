# SENTINEL-X Setup Guide

## 1. Network Configuration (Multi-Device Testing)
To test the ingestion pipeline with physical mobile devices, ensure the development environment is bridged correctly.

### Option A: USB Tethering
1. Connect the mobile device via USB and enable USB Tethering.
2. Execute `ipconfig` and locate the Ethernet adapter IPv4 address.
3. In `phone-agent.html`, set the gateway to `ws://<ETHERNET_IP>:6767`.

### Option B: Mobile Hotspot
1. Enable Windows Mobile Hotspot on the host laptop.
2. Connect the secondary test device to the laptop's Wi-Fi network.
3. Use the Local Area Connection IPv4 address (typically 192.168.137.1) in `phone-agent.html`.

## 2. Mock Cloud Endpoint
The sync engine requires an active listener to process the local SQLCipher buffer.

1. **Initialize Storage:** Confirm `received-payloads.json` exists in the root directory and contains `[]`.
2. **Launch Server:** `node local-cloud.cjs`
3. **Troubleshooting EADDRINUSE:** If port 9000 is occupied, terminate the process:
   `taskkill /F /IM node.exe`

## 3. Backend (Rust/Tauri)
### Prerequisites
* Rust (Stable Toolchain)
* WebView2 Runtime
* SQLCipher Libraries: Ensure the build environment can locate encrypted SQLite headers.

### Initialization
`cd src-tauri`
`cargo tauri dev`

Note: If a "file is not a database" error occurs, delete the local app data at %APPDATA%\com.sentinel.gateway to reset the encryption key synchronization.

## 4. Testing the Ingestion Flow
1. Open `phone-agent.html` in a mobile browser.
2. Enter the Gateway WebSocket URL identified in Step 1.
3. Select Connect.
4. Verify "Connected Devices" on the SENTINEL Dashboard and check the Mock Cloud terminal for "Batch Received" logs.
