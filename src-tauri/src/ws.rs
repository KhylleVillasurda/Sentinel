use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use serde::{Deserialize, Serialize};

use crate::crypto::encrypt_payload;
use crate::db::queries::{insert_payload, get_device, register_device, update_last_seen, DeviceRow};
use crate::logging::{LogLevel, LogSubsystem, LogManager};
use crate::log_event;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthMessage {
    Register {
        device_id: String,
        friendly_name: String,
    },
    Auth {
        device_id: String,
        token: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthResponse {
    Registered {
        token: String,
    },
    Authenticated,
    Error {
        message: String,
    },
}

/// Starts the WebSocket server and listens for incoming IoT device connections.
pub async fn start_server(state: Arc<Mutex<AppState>>) {
    let (addr, logging) = {
        let s = state.lock().expect("Lock poisoned");
        (format!("0.0.0.0:{}", s.config.ws_port), s.logging.clone())
    };

    let listener = TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to {}", addr));

    log_event!(logging, LogLevel::Info, LogSubsystem::WS, "Sentinel ingestion server listening on ws://{}", addr);

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                let state = Arc::clone(&state);
                let logging = logging.clone();
                tokio::spawn(handle_connection(stream, state, logging, peer_addr.to_string()));
            }
            Err(e) => {
                log_event!(logging, LogLevel::Error, LogSubsystem::WS, "Accept error: {}", e);
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    state: Arc<Mutex<AppState>>,
    logging: Arc<LogManager>,
    peer_addr: String,
) {
    let mut ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            log_event!(logging, LogLevel::Error, LogSubsystem::WS, "Handshake failed for {}: {}", peer_addr, e);
            return;
        }
    };

    // --- Authentication Phase ---
    let mut authorized_device_id = None;

    if let Some(msg_result) = ws_stream.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                if let Ok(auth_msg) = serde_json::from_str::<AuthMessage>(&text) {
                    match auth_msg {
                        AuthMessage::Register { device_id, friendly_name } => {
                            let is_pairing = {
                                let s = state.lock().unwrap();
                                s.pairing_mode.load(Ordering::SeqCst)
                            };

                            if is_pairing {
                                let token = generate_token();
                                let token_hash = hash_token(&token);
                                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
                                
                                let device = DeviceRow {
                                    device_id: device_id.clone(),
                                    friendly_name,
                                    token_hash,
                                    created_at: now,
                                    last_seen: now,
                                };

                                let res = {
                                    let s = state.lock().unwrap();
                                    register_device(&s.db.conn, &device)
                                };

                                match res {
                                    Ok(_) => {
                                        let _ = ws_stream.send(Message::Text(serde_json::to_string(&AuthResponse::Registered { token }).unwrap())).await;
                                        authorized_device_id = Some(device_id);
                                        log_event!(logging, LogLevel::Info, LogSubsystem::Auth, "New device registered: {}", authorized_device_id.as_ref().unwrap());
                                    }
                                    Err(e) => {
                                        let _ = ws_stream.send(Message::Text(serde_json::to_string(&AuthResponse::Error { message: format!("Registration failed: {}", e) }).unwrap())).await;
                                    }
                                }
                            } else {
                                let _ = ws_stream.send(Message::Text(serde_json::to_string(&AuthResponse::Error { message: "Pairing mode is disabled".into() }).unwrap())).await;
                            }
                        }
                        AuthMessage::Auth { device_id, token } => {
                            let device = {
                                let s = state.lock().unwrap();
                                get_device(&s.db.conn, &device_id)
                            };

                            match device {
                                Ok(Some(dev)) => {
                                    if dev.token_hash == hash_token(&token) {
                                        let _ = ws_stream.send(Message::Text(serde_json::to_string(&AuthResponse::Authenticated).unwrap())).await;
                                        authorized_device_id = Some(device_id);
                                        
                                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
                                        let _ = {
                                            let s = state.lock().unwrap();
                                            update_last_seen(&s.db.conn, authorized_device_id.as_ref().unwrap(), now)
                                        };
                                        log_event!(logging, LogLevel::Info, LogSubsystem::Auth, "Device authenticated: {}", authorized_device_id.as_ref().unwrap());
                                    } else {
                                        let _ = ws_stream.send(Message::Text(serde_json::to_string(&AuthResponse::Error { message: "Invalid token".into() }).unwrap())).await;
                                    }
                                }
                                _ => {
                                    let _ = ws_stream.send(Message::Text(serde_json::to_string(&AuthResponse::Error { message: "Device not found".into() }).unwrap())).await;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let device_id = match authorized_device_id {
        Some(id) => id,
        None => {
            log_event!(logging, LogLevel::Warn, LogSubsystem::Auth, "Closing unauthenticated connection from {}", peer_addr);
            let _ = ws_stream.close(None).await;
            return;
        }
    };

    // --- Data Ingestion Phase ---
    {
        let mut s = state.lock().expect("AppState lock poisoned on device connect");
        s.connected_devices.push(device_id.clone());
    }

    let (mut sender, mut receiver) = ws_stream.split();

    while let Some(msg_result) = receiver.next().await {
        match msg_result {
            Ok(Message::Binary(payload)) => {
                if let Err(e) = process_payload(&state, &device_id, &payload) {
                    log_event!(logging, LogLevel::Error, LogSubsystem::WS, "Failed to process payload from {}: {}", device_id, e);
                } else {
                    let _ = sender.send(Message::Binary(vec![0x01])).await;
                }
            }
            Ok(Message::Text(text)) => {
                let payload = text.into_bytes();
                if let Err(e) = process_payload(&state, &device_id, &payload) {
                    log_event!(logging, LogLevel::Error, LogSubsystem::WS, "Failed to process text payload from {}: {}", device_id, e);
                } else {
                    let _ = sender.send(Message::Binary(vec![0x01])).await;
                }
            }
            Ok(Message::Close(_)) => {
                log_event!(logging, LogLevel::Info, LogSubsystem::WS, "Device closed connection: {}", device_id);
                break;
            }
            Ok(Message::Ping(data)) => {
                let _ = sender.send(Message::Pong(data)).await;
            }
            Err(e) => {
                log_event!(logging, LogLevel::Error, LogSubsystem::WS, "Connection error from {}: {}", device_id, e);
                break;
            }
            _ => {}
        }
    }

    {
        let mut s = state.lock().expect("AppState lock poisoned on device disconnect");
        s.connected_devices.retain(|d| d != &device_id);
    }
}

fn generate_token() -> String {
    use rand::{thread_rng, Rng};
    use rand::distributions::Alphanumeric;
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn hash_token(token: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Encrypts a raw payload and inserts it into the database.
fn process_payload(
    state: &Arc<Mutex<AppState>>,
    device_id: &str,
    raw: &[u8],
) -> Result<(), crate::error::SentinelError> {
    let s = state.lock()?;
    let encrypted = encrypt_payload(raw, &s.encryption_key)?;
    let received_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    insert_payload(&s.db.conn, device_id, &encrypted, received_at)?;
    Ok(())
}
