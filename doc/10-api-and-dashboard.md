# RESTful API 与 Web Dashboard

## API 概览

使用 `axum` 框架构建 RESTful API，提供运行时管理和监控能力。

## API 端点

```
GET    /                              # 服务信息
GET    /version                       # 版本信息

GET    /logs                          # WebSocket: 实时日志流
GET    /traffic                       # WebSocket: 实时流量统计

GET    /configs                       # 获取当前运行配置
PATCH  /configs                       # 部分更新配置
PUT    /configs                       # 完整重载配置

GET    /proxies                       # 列出所有代理和代理组
GET    /proxies/:name                 # 获取单个代理信息 (含延迟历史)
PUT    /proxies/:name                 # Selector 组切换代理
GET    /proxies/:name/delay           # 触发延迟测试

GET    /rules                         # 列出所有活跃规则

GET    /connections                   # 列出所有活跃连接
DELETE /connections                   # 关闭所有连接
DELETE /connections/:id               # 关闭指定连接

GET    /providers/proxies             # 列出代理 Provider
PUT    /providers/proxies/:name       # 触发 Provider 更新
GET    /providers/rules               # 列出规则 Provider
PUT    /providers/rules/:name         # 触发规则 Provider 更新

GET    /dns/query?name=xxx&type=A     # 手动 DNS 查询 (调试)
```

## API 实现

```rust
use axum::{
    Router, Json,
    extract::{Path, Query, State, WebSocketUpgrade, ws::WebSocket},
    response::IntoResponse,
    routing::{get, put, patch, delete},
    http::StatusCode,
};
use std::sync::Arc;
use serde::{Serialize, Deserialize};

/// API 共享状态
pub struct ApiState {
    pub runtime: Arc<Runtime>,
    pub traffic_counter: Arc<TrafficCounter>,
    pub connection_tracker: Arc<ConnectionTracker>,
    pub log_broadcaster: tokio::sync::broadcast::Sender<LogEntry>,
}

/// 构建路由
pub fn build_router(state: Arc<ApiState>) -> Router {
    Router::new()
        // 基本信息
        .route("/", get(get_hello))
        .route("/version", get(get_version))

        // 日志和流量 (WebSocket)
        .route("/logs", get(ws_logs))
        .route("/traffic", get(ws_traffic))

        // 配置
        .route("/configs", get(get_configs).patch(patch_configs).put(put_configs))

        // 代理
        .route("/proxies", get(get_proxies))
        .route("/proxies/:name", get(get_proxy).put(change_proxy))
        .route("/proxies/:name/delay", get(get_proxy_delay))

        // 规则
        .route("/rules", get(get_rules))

        // 连接
        .route("/connections", get(get_connections).delete(close_all_connections))
        .route("/connections/:id", delete(close_connection))

        // Provider
        .route("/providers/proxies", get(get_proxy_providers))
        .route("/providers/proxies/:name", put(update_proxy_provider))

        // DNS
        .route("/dns/query", get(dns_query))

        .with_state(state)
}

/// 启动 API 服务器
pub async fn start_api_server(
    addr: &str,
    state: Arc<ApiState>,
    secret: Option<String>,
) -> Result<(), Error> {
    let app = build_router(state);

    // 如果配置了 secret, 添加认证中间件
    let app = if let Some(secret) = secret {
        app.layer(axum::middleware::from_fn(move |req, next| {
            auth_middleware(req, next, secret.clone())
        }))
    } else {
        app
    };

    // 添加 CORS 支持 (Web Dashboard 需要)
    let app = app.layer(
        tower_http::cors::CorsLayer::permissive()
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("API server listening on {}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}
```

## 关键端点实现

### 获取代理列表

```rust
#[derive(Serialize)]
struct ProxyResponse {
    name: String,
    #[serde(rename = "type")]
    proxy_type: String,
    alive: bool,
    delay: u16,
    history: Vec<DelayEntry>,
    /// 代理组特有: 可选代理列表
    #[serde(skip_serializing_if = "Option::is_none")]
    all: Option<Vec<String>>,
    /// 代理组特有: 当前选中
    #[serde(skip_serializing_if = "Option::is_none")]
    now: Option<String>,
}

async fn get_proxies(
    State(state): State<Arc<ApiState>>,
) -> Json<HashMap<String, ProxyResponse>> {
    let runtime = state.runtime.state();
    let mut result = HashMap::new();

    for (name, proxy) in &runtime.proxies {
        result.insert(name.clone(), proxy_to_response(proxy));
    }

    Json(result)
}
```

### 延迟测试

```rust
#[derive(Deserialize)]
struct DelayQuery {
    url: Option<String>,
    timeout: Option<u64>,
}

async fn get_proxy_delay(
    State(state): State<Arc<ApiState>>,
    Path(name): Path<String>,
    Query(query): Query<DelayQuery>,
) -> Result<Json<DelayResult>, ApiError> {
    let runtime = state.runtime.state();
    let proxy = runtime.proxies.get(&name)
        .ok_or(ApiError::NotFound)?;

    let url = query.url.unwrap_or_else(|| {
        "http://www.gstatic.com/generate_204".to_string()
    });
    let timeout = Duration::from_millis(query.timeout.unwrap_or(5000));

    let start = tokio::time::Instant::now();
    let result = tokio::time::timeout(
        timeout,
        url_test(proxy.as_ref(), &url),
    ).await;

    match result {
        Ok(Ok(_)) => {
            let delay = start.elapsed().as_millis() as u16;
            Ok(Json(DelayResult { delay }))
        }
        _ => Err(ApiError::TestFailed),
    }
}
```

### WebSocket 实时流量

```rust
async fn ws_traffic(
    State(state): State<Arc<ApiState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_traffic_ws(socket, state))
}

async fn handle_traffic_ws(mut socket: WebSocket, state: Arc<ApiState>) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));

    loop {
        interval.tick().await;

        let snapshot = state.traffic_counter.snapshot();
        let msg = serde_json::json!({
            "up": snapshot.upload_speed,     // bytes/s
            "down": snapshot.download_speed, // bytes/s
        });

        if socket.send(axum::extract::ws::Message::Text(
            msg.to_string()
        )).await.is_err() {
            break; // 客户端断开
        }
    }
}
```

### WebSocket 实时日志

```rust
async fn ws_logs(
    State(state): State<Arc<ApiState>>,
    ws: WebSocketUpgrade,
    Query(params): Query<LogQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_logs_ws(socket, state, params.level))
}

async fn handle_logs_ws(
    mut socket: WebSocket,
    state: Arc<ApiState>,
    level: Option<String>,
) {
    let mut rx = state.log_broadcaster.subscribe();
    let min_level = parse_level(&level.unwrap_or_else(|| "info".into()));

    while let Ok(entry) = rx.recv().await {
        if entry.level >= min_level {
            let msg = serde_json::json!({
                "type": entry.level.to_string(),
                "payload": entry.message,
            });
            if socket.send(axum::extract::ws::Message::Text(
                msg.to_string()
            )).await.is_err() {
                break;
            }
        }
    }
}
```

### 连接管理

```rust
/// 连接追踪器
pub struct ConnectionTracker {
    connections: DashMap<String, ConnectionInfo>,
    id_counter: AtomicU64,
}

#[derive(Serialize, Clone)]
pub struct ConnectionInfo {
    pub id: String,
    pub metadata: MetadataInfo,
    pub upload: u64,
    pub download: u64,
    pub start: chrono::DateTime<chrono::Utc>,
    pub chains: Vec<String>,
    pub rule: String,
    pub rule_payload: String,
}

async fn get_connections(
    State(state): State<Arc<ApiState>>,
) -> Json<ConnectionsResponse> {
    let connections: Vec<ConnectionInfo> = state.connection_tracker
        .connections
        .iter()
        .map(|entry| entry.value().clone())
        .collect();

    let snapshot = state.traffic_counter.total();

    Json(ConnectionsResponse {
        download_total: snapshot.download,
        upload_total: snapshot.upload,
        connections,
    })
}

async fn close_connection(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> StatusCode {
    if state.connection_tracker.close(&id) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}
```

## 认证中间件

```rust
use axum::{
    middleware::Next,
    http::{Request, header},
    body::Body,
};

async fn auth_middleware(
    req: Request<Body>,
    next: Next,
    secret: String,
) -> Result<impl IntoResponse, StatusCode> {
    // Bearer token 认证
    if let Some(auth) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth.to_str() {
            if auth_str == format!("Bearer {}", secret) {
                return Ok(next.run(req).await);
            }
        }
    }

    // Query 参数认证 (兼容 Web Dashboard)
    if let Some(query) = req.uri().query() {
        if query.contains(&format!("token={}", secret)) {
            return Ok(next.run(req).await);
        }
    }

    // WebSocket 升级请求跳过认证检查 (已在握手时验证)
    if req.headers().contains_key(header::UPGRADE) {
        return Ok(next.run(req).await);
    }

    Err(StatusCode::UNAUTHORIZED)
}
```

## 流量统计实现

```rust
use std::sync::atomic::{AtomicU64, Ordering};

pub struct TrafficCounter {
    upload_total: AtomicU64,
    download_total: AtomicU64,
    last_upload: AtomicU64,
    last_download: AtomicU64,
}

impl TrafficCounter {
    pub fn add_upload(&self, bytes: u64) {
        self.upload_total.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_download(&self, bytes: u64) {
        self.download_total.fetch_add(bytes, Ordering::Relaxed);
    }

    /// 快照: 计算当前速率
    pub fn snapshot(&self) -> TrafficSnapshot {
        let upload = self.upload_total.load(Ordering::Relaxed);
        let download = self.download_total.load(Ordering::Relaxed);
        let last_up = self.last_upload.swap(upload, Ordering::Relaxed);
        let last_down = self.last_download.swap(download, Ordering::Relaxed);

        TrafficSnapshot {
            upload_speed: upload - last_up,
            download_speed: download - last_down,
            upload_total: upload,
            download_total: download,
        }
    }
}
```
