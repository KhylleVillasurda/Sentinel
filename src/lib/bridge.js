// lib/bridge.js
// Thin wrapper around Tauri's invoke().
// All React code imports from here — never calls invoke() directly.
// This means if a command name changes, you fix it in one place.

import { invoke } from "@tauri-apps/api/core";

/** @returns {{ status: "Stable"|"Degraded"|"Offline"|"Unknown" }} */
export const getNetworkStatus = () => invoke("get_network_status");

/** @returns {{ total_rows: number, unsynced_rows: number, size_kb: number }} */
export const getStorageStats = () => invoke("get_storage_stats");

/** @returns {string[]} — list of device IDs currently connected via WebSocket */
export const getConnectedDevices = () => invoke("get_connected_devices");

/** @returns {{ message: string, timestamp: number }[]} */
export const getSyncLog = () => invoke("get_sync_log");
