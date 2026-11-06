use crate::{
    app_state::AppState,
    error::AppResult,
    events::{OpenDeskWsEvent, WsEnvelope},
};
use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut receiver = state.events.subscribe();

    if send_initial(&mut socket, &state).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            event = receiver.recv() => {
                match event {
                    Ok(envelope) => {
                        if send_json(&mut socket, &envelope).await.is_err() {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        let envelope = WsEnvelope {
                            event_id: 0,
                            event: OpenDeskWsEvent::ResyncRequired {
                                reason: format!("client lagged behind by {skipped} events"),
                            },
                        };
                        if send_json(&mut socket, &envelope).await.is_err() {
                            break;
                        }
                    }
                    Err(RecvError::Closed) => break,
                }
            }
            message = socket.recv() => {
                match message {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
        }
    }
}

async fn send_initial(socket: &mut WebSocket, state: &AppState) -> Result<(), ()> {
    let status = state.daemon.status().await;
    let envelope = WsEnvelope {
        event_id: 0,
        event: OpenDeskWsEvent::DaemonStatusChanged {
            online: status.online,
            port: status.port,
            active_jobs: status.active_jobs,
            state: status.state,
            message: status.message,
            version: status.version,
            mode: status.mode,
            binary_path: status.binary_path,
        },
    };
    send_json(socket, &envelope).await
}

async fn send_json(socket: &mut WebSocket, envelope: &WsEnvelope) -> Result<(), ()> {
    let payload = serde_json::to_string(envelope).map_err(|_| ())?;
    socket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|_| ())
}

#[allow(dead_code)]
pub fn _assert_send(_: AppResult<()>) {}
