// lib/bridge.js
// Thin wrapper around Tauri's invoke().
// All React code imports from here — never calls invoke() directly.

import { invoke } from "@tauri-apps/api/core";

/** @returns {{ status: "Stable"|"Degraded"|"Offline"|"Unknown" }} */
export const getNetworkStatus = () => invoke("get_network_status");

/** @returns {{ total_rows: number, unsynced_rows: number, size_kb: number }} */
export const getStorageStats = () => invoke("get_storage_stats");

/** @returns {string[]} — list of device IDs currently connected via WebSocket */
export const getConnectedDevices = () => invoke("get_connected_devices");

/** @returns {{ message: string, timestamp: number }[]} */
export const getSyncLog = () => invoke("get_sync_log");

export const getSettings = () => invoke("get_settings");
export const saveSettings = (settings) => invoke("save_settings", { settings });
export const forceSync = () => invoke("force_sync");

/** @returns {Promise<boolean>} */
export const isLoggingEnabled = () => invoke("is_logging_enabled");

/** @param {boolean} enabled */
export const setLoggingEnabled = (enabled) => invoke("set_logging_enabled", { enabled });

/** @returns {Promise<{ timestamp: number, level: string, subsystem: string, message: string }[]>} */
export const getLogBuffer = () => invoke("get_log_buffer");

/** @param {boolean} active @returns {Promise<number>} - expiry timestamp */
export const togglePairingMode = (active) => invoke("toggle_pairing_mode", { active });

/** @returns {Promise<boolean>} */
export const isPairingModeActive = () => invoke("is_pairing_mode_active");

/** @returns {Promise<number>} */
export const getPairingExpiry = () => invoke("get_pairing_expiry");

/** @returns {Promise<{ device_id: string, friendly_name: string, created_at: number, last_seen: number }[]>} */
export const getRegisteredDevices = () => invoke("get_registered_devices");

/** @param {string} deviceId */
export const revokeDevice = (deviceId) => invoke("revoke_device", { device_id: deviceId });

/** @param {number} limit @returns {Promise<{ id: number, device_id: string, decrypted_data: string, received_at: number }[]>} */
export const getDecryptedPayloads = (limit = 50) => invoke("get_decrypted_payloads", { limit });
