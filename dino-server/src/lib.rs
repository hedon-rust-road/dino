use std::collections::HashMap;

use axum::body::Bytes;
use axum::extract::{Host, Query, State};
use axum::http::request::Parts;
use axum::http::Response;
use axum::response::IntoResponse;
use axum::routing::any;
use axum::Router;
use dashmap::DashMap;
use indexmap::IndexMap;
use matchit::Match;
use middleware::ServerTimeLayer;
use tokio::net::TcpListener;
use tracing::info;

mod config;
mod engine;
mod error;
mod middleware;
mod router;
mod worker_pool;

pub use self::config::*;
pub use self::engine::*;
pub use self::error::AppError;
pub use self::router::*;
pub use self::worker_pool::*;

type ProjectRoutes = IndexMap<String, Vec<ProjectRoute>>;

#[derive(Clone)]
pub struct AppState {
    routers: DashMap<String, SwappableAppRouter>,
    worker_pools: DashMap<String, SwappableWorkerPool>,
}

#[derive(Clone)]
pub struct TenentRouter {
    host: String,
    router: SwappableAppRouter,
}

#[derive(Clone)]
pub struct TenentWorkerPool {
    host: String,
    pool: SwappableWorkerPool,
}

pub async fn start_server(
    port: u16,
    routers: Vec<TenentRouter>,
    worker_pools: Vec<TenentWorkerPool>,
) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(addr).await?;

    info!("Listening on {}", listener.local_addr()?);

    let routes = DashMap::new();
    for TenentRouter { host, router } in routers {
        routes.insert(host, router);
    }
    let pools = DashMap::new();
    for TenentWorkerPool { host, pool } in worker_pools {
        pools.insert(host, pool);
    }
    let state = AppState::new(routes, pools);
    let app = Router::new()
        .route("/*path", any(handler))
        .layer(ServerTimeLayer)
        .with_state(state);

    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

async fn handler(
    State(state): State<AppState>,
    parts: Parts,
    Host(host): Host,
    Query(query): Query<HashMap<String, String>>,
    body: Option<Bytes>,
) -> Result<impl IntoResponse, AppError> {
    let router = get_router_by_host(host.clone(), state.clone())?;
    let matched = router.match_it(parts.method.clone(), parts.uri.path())?;
    let req = assemble_req(&matched, &parts, query, body)?;

    // TODO: build a worker pool, and send req via mpsc channel and get res from oneshot channel
    // but if code changed we need to recreate the worker pool
    // let worker = JsWorker::try_new(&router.code)?;
    let handler = matched.value;

    let worker_pool = get_worker_pool_by_host(host, state)?;
    let res = worker_pool.run(handler, req).await?;
    // let res = worker.run(handler, req)?;
    Ok(Response::from(res))
}

impl AppState {
    pub fn new(
        routers: DashMap<String, SwappableAppRouter>,
        pools: DashMap<String, SwappableWorkerPool>,
    ) -> Self {
        Self {
            routers,
            worker_pools: pools,
        }
    }
}

impl TenentRouter {
    pub fn new(host: impl Into<String>, router: SwappableAppRouter) -> Self {
        Self {
            host: host.into(),
            router,
        }
    }
}

impl TenentWorkerPool {
    pub fn new(host: impl Into<String>, pool: SwappableWorkerPool) -> Self {
        Self {
            host: host.into(),
            pool,
        }
    }
}

fn get_router_by_host(mut host: String, state: AppState) -> Result<AppRouter, AppError> {
    _ = host.split_off(host.find(':').unwrap_or(host.len()));
    info!("host: {:?}", host);
    let router = state
        .routers
        .get(&host)
        .ok_or(AppError::HostNotFound(host))?
        .load();
    Ok(router)
}

fn get_worker_pool_by_host(mut host: String, state: AppState) -> Result<WorkerPool, AppError> {
    _ = host.split_off(host.find(':').unwrap_or(host.len()));
    info!("host: {:?}", host);
    let pool = state
        .worker_pools
        .get(&host)
        .ok_or(AppError::HostNotFound(host))?
        .load();
    Ok(pool)
}

fn assemble_req(
    matched: &Match<&str>,
    parts: &Parts,
    query: HashMap<String, String>,
    body: Option<Bytes>,
) -> Result<Req, AppError> {
    let params: HashMap<String, String> = matched
        .params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let headers: HashMap<String, String> = parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
        .collect();

    let body = body.and_then(|v| String::from_utf8(v.to_vec()).ok());

    let req = Req::builder()
        .method(parts.method.to_string())
        .url(parts.uri.to_string())
        .headers(headers)
        .params(params)
        .query(query)
        .body(body)
        .build();

    Ok(req)
}
