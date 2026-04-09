use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, put},
    Json, Router,
};
use redog_tunnel::tunnel::Mode;
use redog_tunnel::Tunnel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub struct ApiState {
    pub tunnel: Arc<Tunnel>,
}

pub async fn start_api_server(
    addr: &str,
    tunnel: Arc<Tunnel>,
    _secret: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(ApiState { tunnel });

    let app = Router::new()
        .route("/", get(get_hello))
        .route("/version", get(get_version))
        .route("/traffic", get(get_traffic))
        .route("/configs", get(get_configs).patch(patch_configs))
        .route("/proxies", get(get_proxies))
        .route("/proxies/{name}", get(get_proxy).put(change_proxy))
        .route("/rules", get(get_rules))
        .route(
            "/connections",
            get(get_connections).delete(close_all_connections),
        )
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("API server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}

// ── Handlers ──

async fn get_hello() -> &'static str {
    "redog"
}

async fn get_version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "meta": true,
    }))
}

async fn get_traffic(State(state): State<Arc<ApiState>>) -> Json<serde_json::Value> {
    let snapshot = state.tunnel.traffic.snapshot();
    Json(serde_json::json!({
        "up": snapshot.upload_speed,
        "down": snapshot.download_speed,
    }))
}

#[derive(Deserialize)]
struct ConfigPatch {
    mode: Option<String>,
}

async fn get_configs(State(state): State<Arc<ApiState>>) -> Json<serde_json::Value> {
    let mode = match state.tunnel.mode() {
        Mode::Rule => "rule",
        Mode::Global => "global",
        Mode::Direct => "direct",
    };
    Json(serde_json::json!({
        "mode": mode,
    }))
}

async fn patch_configs(
    State(state): State<Arc<ApiState>>,
    Json(patch): Json<ConfigPatch>,
) -> StatusCode {
    if let Some(mode) = patch.mode {
        match mode.as_str() {
            "rule" => state.tunnel.set_mode(Mode::Rule),
            "global" => state.tunnel.set_mode(Mode::Global),
            "direct" => state.tunnel.set_mode(Mode::Direct),
            _ => return StatusCode::BAD_REQUEST,
        }
    }
    StatusCode::NO_CONTENT
}

async fn get_proxies(State(state): State<Arc<ApiState>>) -> Json<serde_json::Value> {
    let names = state.tunnel.proxy_names();
    let mut proxies = serde_json::Map::new();
    for name in names {
        if let Some(proxy) = state.tunnel.get_proxy(&name) {
            proxies.insert(
                name.clone(),
                serde_json::json!({
                    "name": proxy.name(),
                    "type": proxy.adapter_type().to_string(),
                    "alive": proxy.alive(),
                    "udp": proxy.support_udp(),
                }),
            );
        }
    }
    Json(serde_json::json!({ "proxies": proxies }))
}

async fn get_proxy(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let proxy = state
        .tunnel
        .get_proxy(&name)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!({
        "name": proxy.name(),
        "type": proxy.adapter_type().to_string(),
        "alive": proxy.alive(),
        "udp": proxy.support_udp(),
    })))
}

#[derive(Deserialize)]
struct ChangeProxyBody {
    name: String,
}

async fn change_proxy(
    State(state): State<Arc<ApiState>>,
    Path(group_name): Path<String>,
    Json(body): Json<ChangeProxyBody>,
) -> StatusCode {
    // TODO: implement selector group switching
    tracing::info!(
        "switch proxy group '{}' to '{}'",
        group_name,
        body.name
    );
    StatusCode::NO_CONTENT
}

async fn get_rules(State(state): State<Arc<ApiState>>) -> Json<serde_json::Value> {
    let rules: Vec<serde_json::Value> = state
        .tunnel
        .rules_info()
        .into_iter()
        .map(|(rule_type, payload, adapter)| {
            serde_json::json!({
                "type": rule_type,
                "payload": payload,
                "proxy": adapter,
            })
        })
        .collect();
    Json(serde_json::json!({ "rules": rules }))
}

async fn get_connections(State(state): State<Arc<ApiState>>) -> Json<serde_json::Value> {
    let snapshot = state.tunnel.traffic.snapshot();
    let conns = state.tunnel.connections.list();
    Json(serde_json::json!({
        "downloadTotal": snapshot.download_total,
        "uploadTotal": snapshot.upload_total,
        "connections": conns,
    }))
}

async fn close_all_connections(State(state): State<Arc<ApiState>>) -> StatusCode {
    let count = state.tunnel.connections.close_all();
    tracing::info!("closed {} connections", count);
    StatusCode::NO_CONTENT
}
