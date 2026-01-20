use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    body::Bytes,
    extract::State,
    http::{self, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use bytes::BytesMut;
use sha2::{Digest, Sha256};
use tokio::{fs, sync::RwLock, time};
use tower::ServiceBuilder;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use tracing::{error, info};

// Embedded fallback for /local/people when local file is missing
static EMBEDDED_LOCAL_PEOPLE: &[u8] = include_bytes!("../assets/people.sample.json");
// Embedded example data served at /example
static EMBEDDED_EXAMPLE: &[u8] = include_bytes!("../assets/example.json");

#[derive(Clone)]
struct AppState {
    local_cache: Cached,
    remote_url: String,
    remote_cache: Arc<RwLock<Option<Cached>>>,
}

#[derive(Clone, Debug)]
struct Cached {
    bytes: Bytes,
    etag: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let port = std::env::var("PORT").unwrap_or_else(|_| "9090".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    // Default to reading from assets/people.json; can be overridden via LOCAL_PATH
    let local_path = std::env::var("LOCAL_PATH").unwrap_or_else(|_| "assets/people.json".to_string());
    let remote_url = std::env::var("REMOTE_URL").unwrap_or_else(|_| "https://raw.githubusercontent.com/cncf/people/refs/heads/main/people.json".to_string());
    let refresh = std::env::var("REFRESH_INTERVAL").ok().and_then(|s| humantime::parse_duration(&s).ok()).unwrap_or_else(|| Duration::from_secs(600));

    let local_cache = load_local_cache(&local_path).await;

    let state = AppState {
        local_cache,
        remote_url: remote_url.clone(),
        remote_cache: Arc::new(RwLock::new(None)),
    };

    // Kick off background refresher for remote cache
    tokio::spawn(refresh_task(state.clone(), refresh));

    let app = Router::new()
        .route("/healthz", get(|| async { (StatusCode::OK, "ok") }))
        .route("/local/people", get(local_people))
        .route("/people", get(remote_people))
        .route("/example", get(example_json))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new()),
        );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn local_people(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let etag = state.local_cache.etag.clone();
    if let Some(inm) = headers.get(http::header::IF_NONE_MATCH) {
        if let Ok(s) = inm.to_str() { if s == etag { return StatusCode::NOT_MODIFIED.into_response(); } }
    }

    let mut h = HeaderMap::new();
    h.insert("Content-Type", HeaderValue::from_static("application/json; charset=utf-8"));
    h.insert("Cache-Control", HeaderValue::from_static("public, max-age=30"));
    h.insert(http::header::ETAG, HeaderValue::from_str(&etag).unwrap());
    (h, state.local_cache.bytes.clone()).into_response()
}

async fn remote_people(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(c) = state.remote_cache.read().await.clone() {
        // Conditional GET support
        if let Some(inm) = headers.get(http::header::IF_NONE_MATCH) {
            if let Ok(s) = inm.to_str() { if s == c.etag { return StatusCode::NOT_MODIFIED.into_response(); } }
        }
        let mut h = HeaderMap::new();
        h.insert("Content-Type", HeaderValue::from_static("application/json; charset=utf-8"));
        h.insert("Cache-Control", HeaderValue::from_static("public, max-age=30"));
        h.insert(http::header::ETAG, HeaderValue::from_str(&c.etag).unwrap());
        return (h, c.bytes).into_response();
    }
    // Fallback to local if remote cache is empty
    let resp = local_people(State(state), headers).await;
    resp.into_response()
}

async fn refresh_task(state: AppState, interval: Duration) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .pool_max_idle_per_host(10)
        .use_rustls_tls()
        .build()
        .expect("client");

    // Initial warm
    if let Err(e) = refresh_once(&client, &state).await { error!(?e, "initial remote refresh failed"); }

    let mut ticker = time::interval(interval);
    loop {
        ticker.tick().await;
        if let Err(e) = refresh_once(&client, &state).await { error!(?e, "remote refresh failed"); }
    }
}

async fn refresh_once(client: &reqwest::Client, state: &AppState) -> anyhow::Result<()> {
    let current_etag = state.remote_cache.read().await.as_ref().map(|c| c.etag.clone());
    let mut req = client.get(&state.remote_url);
    if let Some(et) = current_etag.as_ref() { req = req.header("If-None-Match", et); }
    let resp = req.send().await?;
    match resp.status() {
        reqwest::StatusCode::OK => {
            let etag = resp.headers().get("ETag").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
            let bytes = resp.bytes().await?;
            let etag = etag.unwrap_or_else(|| strong_etag(&bytes));
            let cached = Cached { bytes, etag };
            *state.remote_cache.write().await = Some(cached);
            info!("remote cache refreshed");
        }
        reqwest::StatusCode::NOT_MODIFIED => {
            // no-op
            info!("remote not modified");
        }
        s => {
            anyhow::bail!("unexpected status: {}", s);
        }
    }
    Ok(())
}

fn strong_etag(b: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b);
    let sum = hasher.finalize();
    let mut out = BytesMut::with_capacity(2 + sum.len() * 2);
    out.extend_from_slice(b"\"");
    out.extend_from_slice(hex::encode(sum).as_bytes());
    out.extend_from_slice(b"\"");
    String::from_utf8(out.to_vec()).unwrap()
}

fn init_tracing() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=warn".into());
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(filter))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

async fn load_local_cache(path: &str) -> Cached {
    match fs::read(path).await {
        Ok(bytes) => {
            let bytes = Bytes::from(bytes);
            let etag = strong_etag(&bytes);
            info!(path, "loaded local people.json into memory");
            Cached { bytes, etag }
        }
        Err(err) => {
            error!(?err, path, "failed to read local file; using embedded sample");
            let bytes = Bytes::from_static(EMBEDDED_LOCAL_PEOPLE);
            let etag = strong_etag(&bytes);
            Cached { bytes, etag }
        }
    }
}

async fn example_json(headers: HeaderMap) -> impl IntoResponse {
    let etag = strong_etag(EMBEDDED_EXAMPLE);
    if let Some(inm) = headers.get(http::header::IF_NONE_MATCH) {
        if let Ok(s) = inm.to_str() { if s == etag { return StatusCode::NOT_MODIFIED.into_response(); } }
    }
    let mut h = HeaderMap::new();
    h.insert("Content-Type", HeaderValue::from_static("application/json; charset=utf-8"));
    h.insert("Cache-Control", HeaderValue::from_static("public, max-age=30"));
    h.insert(http::header::ETAG, HeaderValue::from_str(&etag).unwrap());
    (h, Bytes::from_static(EMBEDDED_EXAMPLE)).into_response()
}
