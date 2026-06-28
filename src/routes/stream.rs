use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::HeaderMap,
    response::Response,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::state::AppState;

/// Handler for WebSocket `/api/v1/stream`.
/// Authenticates the client during the HTTP upgrade handshake and handles active WebSocket stream.
pub async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Response, AppError> {
    // 1. Authenticate pre-upgrade from Authorization or X-Assertion-Token headers
    let token = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .or_else(|| {
            headers
                .get("X-Assertion-Token")
                .and_then(|h| h.to_str().ok())
        })
        .ok_or_else(|| {
            AppError::Unauthorized("Missing authentication token for WebSocket upgrade".to_owned())
        })?;

    // 2. Verify the session token
    let device_id = crate::crypto::session::verify_session_token(&state.hmac_key, token)?;

    // 3. Perform the protocol upgrade
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, device_id)))
}

/// Active WebSocket session loop. Handles ping/pong keepalive and routes messages.
async fn handle_socket(mut socket: WebSocket, device_id: Uuid) {
    tracing::info!(device_id = %device_id, "WebSocket stream connection upgraded");

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));

    loop {
        tokio::select! {
            // Periodic ping to keep the connection open and detect dead peers
            _ = interval.tick() => {
                tracing::debug!(device_id = %device_id, "Sending keepalive Ping");
                if let Err(e) = socket.send(Message::Ping(bytes::Bytes::new())).await {
                    tracing::error!(device_id = %device_id, "Failed to send keepalive Ping: {e}");
                    break;
                }
            }
            // Recv next message from the client
            msg_res = socket.recv() => {
                match msg_res {
                    Some(Ok(Message::Text(text))) => {
                        tracing::info!(device_id = %device_id, "Received text frame: {text}");
                        // Simple echo response for connection testing
                        let response_text = format!("Echo: {}", text);
                        if let Err(e) = socket.send(Message::Text(response_text.into())).await {
                            tracing::error!(device_id = %device_id, "Failed to send echo response: {e}");
                            break;
                        }
                    }
                    Some(Ok(Message::Binary(bin))) => {
                        tracing::info!(device_id = %device_id, "Received binary frame: {} bytes", bin.len());
                        // Zero-Knowledge stream just routes data, we do not inspect it.
                    }
                    Some(Ok(Message::Ping(data))) => {
                        tracing::debug!(device_id = %device_id, "Received Ping, replying with Pong");
                        if let Err(e) = socket.send(Message::Pong(data)).await {
                            tracing::error!(device_id = %device_id, "Failed to reply with Pong: {e}");
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        tracing::debug!(device_id = %device_id, "Received keepalive Pong response");
                    }
                    Some(Ok(Message::Close(frame))) => {
                        tracing::info!(device_id = %device_id, "Connection closed by client: {:?}", frame);
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::error!(device_id = %device_id, "WebSocket connection error: {e}");
                        break;
                    }
                    None => {
                        tracing::info!(device_id = %device_id, "WebSocket stream finished");
                        break;
                    }
                }
            }
        }
    }

    tracing::info!(device_id = %device_id, "WebSocket stream connection closed");
}
