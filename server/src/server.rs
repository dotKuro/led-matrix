use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::{
    Router,
    body::Bytes,
    extract::State,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    http::StatusCode,
    response::{Json, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tokio::sync::Notify;
use tower_http::services::{ServeDir, ServeFile};

use crate::matrix::Color;
use crate::pattern::{self, AVAILABLE, Display, DisplayState, Params};
use crate::sim::FrameBroadcast;

#[derive(Clone)]
struct AppState {
    display: DisplayState,
    frames: FrameBroadcast,
    matrix_width: usize,
    matrix_height: usize,
}

#[derive(Deserialize)]
struct RunRequest {
    name: String,
    #[serde(default)]
    params: Params,
}

pub async fn serve(
    display: DisplayState,
    frames: FrameBroadcast,
    matrix_width: usize,
    matrix_height: usize,
    shutdown: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,
) {
    let state = AppState {
        display,
        frames,
        matrix_width,
        matrix_height,
    };

    let static_files =
        ServeDir::new("static").not_found_service(ServeFile::new("static/index.html"));

    let app = Router::new()
        .route("/api/pattern/run", post(run_pattern))
        .route("/api/pattern/stop", post(stop_pattern))
        .route("/api/pattern/status", get(get_status))
        .route("/api/patterns", get(list_patterns))
        .route("/api/image", post(upload_image))
        .route("/api/sim/stream", get(sim_stream))
        .fallback_service(static_files)
        .with_state(state);

    let listener = match tokio::net::TcpListener::bind("0.0.0.0:3000").await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind failed: {e}");
            shutdown.store(true, Ordering::SeqCst);
            return;
        }
    };
    eprintln!("listening on http://0.0.0.0:3000");

    let result = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_notify.notified().await;
        })
        .await;

    if let Err(e) = result {
        eprintln!("server error: {e}");
    }
    shutdown.store(true, Ordering::SeqCst);
}

async fn run_pattern(
    State(state): State<AppState>,
    Json(req): Json<RunRequest>,
) -> StatusCode {
    if !AVAILABLE.contains(&req.name.as_str()) {
        return StatusCode::NOT_FOUND;
    }
    *state.display.lock().unwrap() = Some(Display::Pattern(req.name, req.params));
    StatusCode::NO_CONTENT
}

async fn stop_pattern(State(state): State<AppState>) -> StatusCode {
    *state.display.lock().unwrap() = None;
    StatusCode::NO_CONTENT
}

async fn get_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    match state.display.lock().unwrap().as_ref() {
        Some(Display::Pattern(name, params)) => {
            Json(serde_json::json!({ "running": name, "params": params }))
        }
        _ => Json(serde_json::json!({ "running": null, "params": {} })),
    }
}

async fn list_patterns() -> Json<Vec<serde_json::Value>> {
    Json(pattern::infos())
}

async fn upload_image(State(state): State<AppState>, body: Bytes) -> StatusCode {
    let expected = state.matrix_width * state.matrix_height * 3;
    if body.len() != expected {
        return StatusCode::BAD_REQUEST;
    }
    let mut pixels = Vec::with_capacity(state.matrix_width * state.matrix_height);
    for chunk in body.chunks_exact(3) {
        pixels.push(Color::new(chunk[0], chunk[1], chunk[2]));
    }
    *state.display.lock().unwrap() = Some(Display::Image(pixels));
    StatusCode::NO_CONTENT
}

async fn sim_stream(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    let (latest, rx) = state.frames.subscribe();
    ws.on_upgrade(move |socket| handle_sim_socket(socket, latest, rx))
}

async fn handle_sim_socket(
    mut socket: WebSocket,
    latest: Option<crate::sim::Frame>,
    mut rx: tokio::sync::broadcast::Receiver<crate::sim::Frame>,
) {
    if let Some(frame) = latest {
        if socket
            .send(Message::Binary(frame.as_ref().clone()))
            .await
            .is_err()
        {
            return;
        }
    }
    while let Ok(frame) = rx.recv().await {
        if socket
            .send(Message::Binary(frame.as_ref().clone()))
            .await
            .is_err()
        {
            break;
        }
    }
}
