# SENTINEL

### Hardened, Local-First IoT Edge Gateway

> A capstone thesis project — built with Rust, Tauri 2, and React.

SENTINEL acts as a secure middleman between IoT devices and the cloud. Instead of pushing raw sensor data directly to the internet, it receives all incoming payloads over WebSocket, encrypts them locally using AES-256-GCM, stores them in a SQLCipher-encrypted database, and only syncs to the cloud when the network connection is stable.

---

## Why SENTINEL?

Most IoT systems push data directly to the cloud — no buffering, no encryption at the edge, no resilience against network outages. SENTINEL solves this by keeping data **local-first**:

- **No data loss** during network outages — payloads queue locally and sync when the connection recovers
- **Encrypted at rest** — every payload is AES-256-GCM encrypted before it touches disk
- **Encrypted on disk** — the entire SQLite database is encrypted via SQLCipher
- **Network-aware** — a background health monitor classifies the connection as Stable, Degraded, or Offline and gates all sync activity accordingly

---

## Architecture

```
IoT Devices / Sensors
        │
        │  WebSocket (ws://localhost:6767)
        ▼
┌─────────────────────────────┐
│         SENTINEL            │
│                             │
│  ws.rs — ingestion server   │
│  crypto.rs — AES-256-GCM    │
│  db/ — SQLCipher storage    │
│  network.rs — health ping   │
│  sync.rs — batch uploader   │
│  commands.rs — Tauri API    │
│                             │
│  React Dashboard            │
└─────────────────────────────┘
        │
        │  HTTPS batch POST (when Stable)
        ▼
   Cloud Endpoint
```

---

## Tech Stack

| Layer      | Technology                                          |
| ---------- | --------------------------------------------------- |
| Backend    | Rust via Tauri 2                                    |
| Frontend   | React + Vite                                        |
| Database   | SQLite encrypted via SQLCipher                      |
| Encryption | AES-256-GCM (`aes-gcm` crate)                       |
| Transport  | WebSocket (`tokio-tungstenite`)                     |
| HTTP Sync  | `reqwest` with `rustls-tls` — no OpenSSL dependency |
| Target     | Windows, macOS, Linux, Raspberry Pi                 |

---

## Features

- **Real-time dashboard** — live network status badge, storage usage bar, connected device list, and rolling sync event log
- **WebSocket ingestion** — accepts binary and text frames from any device on the local network; responds with a `0x01` ACK on successful store
- **Intelligent sync engine** — batches all unsynced rows into a single POST; on failure, skips and logs the error so other rows are not blocked
- **Network health monitor** — consecutive-failure-based classification with configurable thresholds; drives sync gating
- **30 unit tests** — all pure logic, no flaky network or async tests

---

## Dashboard

The React dashboard polls the Rust backend via Tauri's `invoke()` bridge and displays:

| Component     | Description                                       |
| ------------- | ------------------------------------------------- |
| `StatusBadge` | Color-coded Stable / Degraded / Offline pill      |
| `StorageBar`  | DB size + unsynced row count                      |
| `DeviceList`  | Active WebSocket connections                      |
| `SyncLog`     | Rolling log of recent sync events with timestamps |

---

## Project Status

| Phase | Description                                     | Status      |
| ----- | ----------------------------------------------- | ----------- |
| 1     | SQLCipher local buffer + AES-256-GCM encryption | ✅ Complete |
| 2     | WebSocket ingestion server                      | ✅ Complete |
| 3     | Network health monitor                          | ✅ Complete |
| 4     | Intelligent sync engine                         | ✅ Complete |
| 5     | React dashboard                                 | ✅ Complete |

---

## Roadmap

- [ ] OS keychain key storage via `keyring` crate (replace dev placeholder)
- [ ] Connected devices live tracking in dashboard
- [ ] Raspberry Pi deployment guide
- [ ] Configurable cloud endpoint via `tauri.conf.json`
- [ ] Payload decryption viewer for debugging

---

## Setup

See [SETUP.md](./SETUP.md) for full installation and development instructions.

---

## License

Academic capstone project. All rights reserved.
