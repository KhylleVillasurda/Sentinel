use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::broadcast;
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LogSubsystem {
    WS,
    Sync,
    DB,
    Network,
    Config,
    General,
    Auth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub timestamp: i64,
    pub level: LogLevel,
    pub subsystem: LogSubsystem,
    pub message: String,
}

impl LogEvent {
    pub fn new(level: LogLevel, subsystem: LogSubsystem, message: String) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        
        Self {
            timestamp,
            level,
            subsystem,
            message,
        }
    }
}

/// Legacy event for backward compatibility with existing UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEvent {
    pub message: String,
    pub timestamp: i64,
}

pub struct LogManager {
    pub enabled: AtomicBool,
    pub buffer: Mutex<Vec<LogEvent>>,
    pub legacy_sync_log: Mutex<Vec<SyncEvent>>,
    pub sender: broadcast::Sender<LogEvent>,
    pub max_entries: usize,
}

impl LogManager {
    pub fn new(max_entries: usize) -> Self {
        let (sender, _) = broadcast::channel(100);
        Self {
            enabled: AtomicBool::new(true),
            buffer: Mutex::new(Vec::with_capacity(max_entries)),
            legacy_sync_log: Mutex::new(Vec::with_capacity(100)),
            sender,
            max_entries,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    /// Logs an event, handles internal buffers, and optionally emits Tauri events.
    pub fn log(&self, event: LogEvent, handle: Option<&AppHandle>) {
        if !self.is_enabled() {
            return;
        }

        let message = event.message.clone();
        let timestamp = event.timestamp;
        let subsystem = event.subsystem.clone();

        // 1. Update internal buffers (this has its own lock, independent of AppState)
        {
            if let Ok(mut b) = self.buffer.lock() {
                b.insert(0, event.clone());
                if b.len() > self.max_entries {
                    b.pop();
                }
            }

            if subsystem == LogSubsystem::Sync {
                if let Ok(mut s) = self.legacy_sync_log.lock() {
                    let sync_event = SyncEvent {
                        message: message.clone(),
                        timestamp,
                    };
                    s.insert(0, sync_event.clone());
                    if s.len() > 100 {
                        s.pop();
                    }
                    
                    // Emit legacy event if handle provided
                    if let Some(h) = handle {
                        let _ = h.emit("new-sync-event", sync_event);
                    }
                }
            }
        }

        // 2. Broadcast to real-time subscribers
        let _ = self.sender.send(event.clone());

        // 3. Emit global log event if handle provided
        if let Some(h) = handle {
            let _ = h.emit("new-log-event", event);
        }
    }
}

/// Macro for easy logging without Tauri event emission.
/// usage: log_event!(logging_arc, LogLevel::Info, LogSubsystem::WS, "Server started on {}", port);
#[macro_export]
macro_rules! log_event {
    ($logging:expr, $level:expr, $subsystem:expr, $($arg:tt)*) => {
        {
            let message = format!($($arg)*);
            let event = $crate::logging::LogEvent::new($level, $subsystem, message);
            $logging.log(event, None);
        }
    };
}

/// Macro for easy logging with Tauri event emission.
/// usage: log_event_emit!(logging_arc, handle, LogLevel::Info, LogSubsystem::WS, "Server started on {}", port);
#[macro_export]
macro_rules! log_event_emit {
    ($logging:expr, $handle:expr, $level:expr, $subsystem:expr, $($arg:tt)*) => {
        {
            let message = format!($($arg)*);
            let event = $crate::logging::LogEvent::new($level, $subsystem, message);
            $logging.log(event, Some($handle));
        }
    };
}
