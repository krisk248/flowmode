use axum::{
    extract::State,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use rust_embed::RustEmbed;
use serde::Serialize;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};

use crate::storage::Storage;
use crate::tray::format_duration;

/// Embedded static files from the web folder
#[derive(RustEmbed)]
#[folder = "web/dist"]
struct Assets;

/// Shared state for the web server - just the db path
#[derive(Clone)]
pub struct AppState {
    pub db_path: PathBuf,
}

/// API response for today's summary
#[derive(Serialize)]
pub struct TodaySummary {
    pub total_secs: i64,
    pub total_formatted: String,
    pub apps: Vec<AppStat>,
    pub hourly: Vec<HourlyStat>,
}

#[derive(Serialize)]
pub struct AppStat {
    pub name: String,
    pub category: String,
    pub secs: i64,
    pub formatted: String,
    pub percent: u32,
}

#[derive(Serialize)]
pub struct HourlyStat {
    pub hour: u32,
    pub secs: i64,
}

#[derive(Serialize)]
pub struct DetailedEntry {
    pub app_name: String,
    pub category: String,
    pub window_title: String,
    pub secs: i64,
    pub formatted: String,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub version: String,
    pub tracking: bool,
}

#[derive(Serialize)]
pub struct HistoryDay {
    pub date: String,
    pub total_secs: i64,
    pub formatted: String,
}

/// Create the web server router
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // API routes
        .route("/api/today", get(api_today))
        .route("/api/today/detailed", get(api_today_detailed))
        .route("/api/today/hourly", get(api_today_hourly))
        .route("/api/status", get(api_status))
        .route("/api/history", get(api_history))
        .route("/api/tracking/pause", post(api_pause))
        .route("/api/tracking/resume", post(api_resume))
        // Static files (Svelte app)
        .fallback(static_handler)
        .layer(cors)
        .with_state(state)
}

/// Serve static files from embedded assets
async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Default to index.html for SPA routing
    let path = if path.is_empty() || !path.contains('.') {
        "index.html"
    } else {
        path
    };

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => {
            // Try index.html for SPA routes
            match Assets::get("index.html") {
                Some(content) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html")],
                    content.data.into_owned(),
                )
                    .into_response(),
                None => (StatusCode::NOT_FOUND, "Not found").into_response(),
            }
        }
    }
}

/// GET /api/today - Today's summary
async fn api_today(State(state): State<AppState>) -> impl IntoResponse {
    let storage = match Storage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Json(TodaySummary {
            total_secs: 0,
            total_formatted: "0m".to_string(),
            apps: vec![],
            hourly: vec![],
        }),
    };

    let total_secs = storage.get_today_total_secs().unwrap_or(0);
    let summaries = storage.get_today_summary().unwrap_or_default();
    let hourly = storage.get_today_hourly().unwrap_or_default();

    let total = summaries.iter().map(|s| s.total_secs).sum::<i64>().max(1);

    let apps: Vec<AppStat> = summaries
        .iter()
        .map(|s| AppStat {
            name: s.app_name.clone(),
            category: s.category.clone(),
            secs: s.total_secs,
            formatted: format_duration(s.total_secs),
            percent: ((s.total_secs as f64 / total as f64) * 100.0) as u32,
        })
        .collect();

    let hourly_stats: Vec<HourlyStat> = hourly
        .iter()
        .map(|h| HourlyStat {
            hour: h.hour,
            secs: h.total_secs,
        })
        .collect();

    Json(TodaySummary {
        total_secs,
        total_formatted: format_duration(total_secs),
        apps,
        hourly: hourly_stats,
    })
}

/// GET /api/today/detailed - Detailed window titles
async fn api_today_detailed(State(state): State<AppState>) -> impl IntoResponse {
    let storage = match Storage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Json(Vec::<DetailedEntry>::new()),
    };

    let detailed = storage.get_today_detailed().unwrap_or_default();

    let entries: Vec<DetailedEntry> = detailed
        .iter()
        .map(|(app, cat, title, secs)| DetailedEntry {
            app_name: app.clone(),
            category: cat.clone(),
            window_title: title.clone(),
            secs: *secs,
            formatted: format_duration(*secs),
        })
        .collect();

    Json(entries)
}

/// GET /api/today/hourly - Hourly breakdown
async fn api_today_hourly(State(state): State<AppState>) -> impl IntoResponse {
    let storage = match Storage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Json(Vec::<HourlyStat>::new()),
    };

    let hourly = storage.get_today_hourly().unwrap_or_default();

    // Return all 24 hours
    let mut data: Vec<HourlyStat> = (0u32..24)
        .map(|h| HourlyStat { hour: h, secs: 0 })
        .collect();

    for h in hourly {
        if (h.hour as usize) < 24 {
            data[h.hour as usize].secs = h.total_secs;
        }
    }

    Json(data)
}

/// GET /api/status - Daemon status
async fn api_status() -> impl IntoResponse {
    Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        tracking: true,
    })
}

/// GET /api/history - Past 30 days
async fn api_history(State(state): State<AppState>) -> impl IntoResponse {
    let storage = match Storage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Json(Vec::<HistoryDay>::new()),
    };

    let history = storage.get_history_days(30).unwrap_or_default();

    let days: Vec<HistoryDay> = history
        .iter()
        .map(|(date, secs)| HistoryDay {
            date: date.to_string(),
            total_secs: *secs,
            formatted: format_duration(*secs),
        })
        .collect();

    Json(days)
}

/// POST /api/tracking/pause
async fn api_pause() -> impl IntoResponse {
    Json(serde_json::json!({"status": "paused"}))
}

/// POST /api/tracking/resume
async fn api_resume() -> impl IntoResponse {
    Json(serde_json::json!({"status": "resumed"}))
}

/// Start the web server
pub async fn start_web_server(db_path: PathBuf, port: u16) -> anyhow::Result<()> {
    let state = AppState { db_path };
    let app = create_router(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("Web dashboard at http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
