//! Tiny HTTP server: the resolver as JSON (see [`crate::api`]), plus the
//! embedded single-page map demo.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::{Query as Params, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::api;
use crate::index::Network;

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_JS: &str = include_str!("../web/app.js");
const BACKEND_JS: &str = include_str!("../web/backend.js");
const STYLE_CSS: &str = include_str!("../web/style.css");

struct App {
    net: Network,
    /// Whole network as GeoJSON, serialised once at start-up.
    network_geojson: Vec<u8>,
    stats: Value,
}

type Shared = Arc<App>;

pub async fn run(net: Network, port: u16) -> Result<()> {
    let app = Arc::new(App {
        network_geojson: api::network_geojson(&net).into_bytes(),
        stats: api::stats(&net),
        net,
    });
    let router = Router::new()
        .route(
            "/",
            get(|| async { static_file(INDEX_HTML, "text/html; charset=utf-8") }),
        )
        .route(
            "/app.js",
            get(|| async { static_file(APP_JS, "application/javascript; charset=utf-8") }),
        )
        .route(
            "/backend.js",
            get(|| async { static_file(BACKEND_JS, "application/javascript; charset=utf-8") }),
        )
        .route(
            "/style.css",
            get(|| async { static_file(STYLE_CSS, "text/css; charset=utf-8") }),
        )
        .route("/api/stats", get(api_stats))
        .route("/api/network", get(api_network))
        .route("/api/find", get(api_find))
        .route("/api/resolve", get(api_resolve))
        .with_state(app);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .with_context(|| format!("binding 127.0.0.1:{port}"))?;
    eprintln!("gers-lr demo → http://127.0.0.1:{port}");
    axum::serve(listener, router).await?;
    Ok(())
}

fn static_file(body: &'static str, content_type: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type)], body).into_response()
}

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": self.0.to_string()})),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError(e)
    }
}

async fn api_stats(State(app): State<Shared>) -> Json<Value> {
    Json(app.stats.clone())
}

async fn api_network(State(app): State<Shared>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/geo+json"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        app.network_geojson.clone(),
    )
        .into_response()
}

async fn api_find(
    State(app): State<Shared>,
    Params(p): Params<HashMap<String, String>>,
) -> Json<Value> {
    Json(api::find(&app.net, p.get("q").map_or("", String::as_str)))
}

async fn api_resolve(
    State(app): State<Shared>,
    Params(p): Params<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(api::resolve(&app.net, &p)?))
}
