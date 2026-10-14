//! The explorer over HTTP: the page at `/` and the JSON API under `/api/`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use overrules::{ErrorKind, Network};
use serde_json::{Value, json};

use crate::{Error, api, assets};

struct App {
    net: Network,
    /// The whole network as GeoJSON, serialised once at start-up.
    network_geojson: Vec<u8>,
    stats: Value,
}

type Shared = Arc<App>;

/// The routes, ready to be served or nested in a larger application.
pub fn router(net: Network) -> Router {
    let app = Arc::new(App {
        network_geojson: api::network_geojson(&net).into_bytes(),
        stats: api::stats(&net),
        net,
    });
    let mut router = Router::new();
    for (path, content_type, body) in assets::FILES {
        router = router.route(
            &format!("/{path}"),
            get(move || async move { ([(header::CONTENT_TYPE, content_type)], body) }),
        );
    }
    router
        .route("/api/stats", get(stats))
        .route("/api/network", get(network))
        .route("/api/find", get(find))
        .route("/api/resolve", get(resolve))
        .with_state(app)
}

/// Serves the explorer on `addr` until the process receives Ctrl-C.
pub async fn serve(net: Network, addr: SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(net))
        .with_graceful_shutdown(async {
            // If the signal handler cannot be installed, run until killed.
            if tokio::signal::ctrl_c().await.is_err() {
                std::future::pending::<()>().await;
            }
        })
        .await
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match self.kind() {
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::InvalidInput => StatusCode::BAD_REQUEST,
            ErrorKind::Data => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({"error": self.to_string()}))).into_response()
    }
}

async fn stats(State(app): State<Shared>) -> Json<Value> {
    Json(app.stats.clone())
}

async fn network(State(app): State<Shared>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/geo+json"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        app.network_geojson.clone(),
    )
        .into_response()
}

async fn find(State(app): State<Shared>, Query(p): Query<HashMap<String, String>>) -> Json<Value> {
    Json(api::find(&app.net, p.get("q").map_or("", String::as_str)))
}

async fn resolve(
    State(app): State<Shared>,
    Query(p): Query<HashMap<String, String>>,
) -> Result<Json<Value>, Error> {
    api::resolve(&app.net, &p).map(Json)
}
