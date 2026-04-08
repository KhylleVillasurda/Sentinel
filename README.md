# <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/shield-shaded.svg" width="32" height="32"> SENTINEL

### Hardened, Local-First IoT Edge Gateway

> A capstone thesis project — built with Rust, Tauri 2, and React.

SENTINEL acts as a secure middleman between IoT devices and the cloud. Instead of pushing raw sensor data directly to the internet, it receives all incoming payloads over WebSocket, encrypts them locally, stores them in an encrypted database, and only syncs to the cloud when the network connection is stable.

---

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/question-circle.svg" width="20" height="20"> Why SENTINEL?

Most IoT systems push data directly to the cloud — no buffering, no encryption at the edge, no resilience against network outages. SENTINEL solves this by keeping data **local-first**:

- <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/shield-lock.svg" width="16" height="16"> **Secure Pairing** — Devices must register during a 60-second pairing window; no "open" ingestion allowed.
- <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/key.svg" width="16" height="16"> **OS Keychain Integration** — Master encryption keys are stored in the system keychain (Windows Credential Manager / macOS Keychain).
- <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/hdd-network.svg" width="16" height="16"> **No Data Loss** — Payloads queue locally during outages and sync when the connection recovers.
- <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/incognito.svg" width="16" height="16"> **Encrypted at Rest** — Every payload is AES-256-GCM encrypted (migrating to ChaCha20-Poly1305) before it touches disk.
- <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/database.svg" width="16" height="16"> **Encrypted on Disk** — The entire SQLite database is encrypted via SQLCipher.

---

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/diagram-3.svg" width="20" height="20"> Architecture

```
IoT Devices / Sensors
        │
        │  WebSocket + Auth Token (ws://localhost:6767)
        ▼
┌────────────────────────────────┐
│           SENTINEL             │
│                                │
│  ws.rs — authenticated ingest  │
│  crypto.rs — ChaCha20-Poly1305 │
│  db/ — SQLCipher + Devices DB  │
│  network.rs — health ping      │
│  sync.rs — batch uploader      │
│  commands.rs — Tauri API       │
│                                │
│  React Dashboard (Bento Grid)  │
└────────────────────────────────┘
        │
        │  HTTPS batch POST (when Stable)
        ▼
   Cloud Endpoint
```

---

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/stack.svg" width="20" height="20"> Tech Stack

| Layer      | Technology                                          |
| ---------- | --------------------------------------------------- |
| Backend    | Rust via Tauri 2                                    |
| Frontend   | React + Vite + Bootstrap Icons                      |
| Database   | SQLite + SQLCipher + `keyring` (OS Keychain)        |
| Encryption | AES-256-GCM / ChaCha20-Poly1305                     |
| Transport  | WebSocket (`tokio-tungstenite`)                     |
| HTTP Sync  | `reqwest` with `rustls-tls` — no OpenSSL dependency |
| Target     | Windows, macOS, Linux, Raspberry Pi                 |

---

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/stars.svg" width="20" height="20"> Features

- <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/cpu.svg" width="16" height="16"> **Command Center Dashboard** — Live network status, storage usage, and authenticated device management.
- <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/lightning-charge.svg" width="16" height="16"> **Secure Ingestion** — Only registered devices with valid tokens can stream data.
- <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/cloud-arrow-up.svg" width="16" height="16"> **Intelligent Sync** — Batches unsynced rows when network is "Stable"; gates activity when "Degraded".
- <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/activity.svg" width="16" height="16"> **Health Monitor** — Real-time latency tracking and connection classification.

---

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/window.svg" width="20" height="20"> Dashboard Components

| Component     | Description                                       | Icon |
| ------------- | ------------------------------------------------- | ---- |
| `StatusBadge` | Color-coded Stable / Degraded / Offline pill      | <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/wifi.svg" width="16" height="16"> |
| `StorageBar`  | DB size + unsynced row count                      | <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/database.svg" width="16" height="16"> |
| `DeviceList`  | Active connections + Registration status          | <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/pc-display.svg" width="16" height="16"> |
| `SyncLog`     | Rolling log of recent sync events with timestamps | <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/journal-text.svg" width="16" height="16"> |

---

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/kanban.svg" width="20" height="20"> Project Status

| Phase | Description                                     | Status      |
| ----- | ----------------------------------------------- | ----------- |
| 1     | SQLCipher local buffer + AES-256-GCM encryption | ✅ Complete |
| 2     | Secure Pairing + OS Keychain Integration        | ✅ Complete |
| 3     | Network health monitor + Sync Gating            | ✅ Complete |
| 4     | Intelligent sync engine (Batching)              | ✅ Complete |
| 5     | UI/UX Revamp (Bento Grid Dashboard)             | 🔄 In Progress |

---

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/gear.svg" width="20" height="20"> Setup

See [SETUP.md](./SETUP.md) for full installation and development instructions.

---

## <img src="https://raw.githubusercontent.com/twbs/icons/main/icons/file-earmark-text.svg" width="20" height="20"> License

Academic capstone project. All rights reserved.
