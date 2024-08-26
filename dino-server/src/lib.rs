use std::collections::HashMap;

use axum::body::Bytes;
use axum::extract::{Host, Query, State};
use axum::http::request::Parts;
use axum::response::IntoResponse;
use axum::routing::any;
use axum::{Json, Router};
use dashmap::DashMap;
use indexmap::IndexMap;
use serde_json::json;
use tokio::net::TcpListener;
use tracing::info;

mod config;
mod error;
mod router;

pub use self::config::*;
pub use self::error::AppError;
pub use self::router::*;

type ProjectRoutes = IndexMap<String, Vec<ProjectRoute>>;

#[derive(Clone)]
pub struct AppState {
    routers: DashMap<String, SwappableAppRouter>,
}

pub async fn start_server(
    port: u16,
    routers: DashMap<String, SwappableAppRouter>,
) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(addr).await?;

    info!("Listening on {}", listener.local_addr()?);
    let state = AppState::new(routers);
    let app = Router::new()
        .route("/*path", any(handler))
        .with_state(state);

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

async fn handler(
    State(state): State<AppState>,
    parts: Parts,
    Host(mut host): Host,
    Query(query): Query<serde_json::Value>,
    body: Option<Bytes>,
) -> Result<impl IntoResponse, AppError> {
    info!("host: {:?}", host);
    _ = host.split_off(host.find(':').unwrap_or(host.len()));
    let router: AppRouter = state
        .routers
        .get(&host)
        .ok_or(AppError::HostNotFound(host))?
        .load();

    let matched = router.match_it(parts.method, parts.uri.path())?;
    let handler = matched.value;
    let params: HashMap<String, String> = matched
        .params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let body = if let Some(body) = body {
        serde_json::from_slice(&body)?
    } else {
        serde_json::Value::Null
    };

    Ok(Json(json!({
        "handler": handler,
        "params": params,
        "query": query,
        "body": body,
    })))
}

impl AppState {
    pub fn new(routers: DashMap<String, SwappableAppRouter>) -> Self {
        Self { routers }
    }
}
