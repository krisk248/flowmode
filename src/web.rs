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
use std::sync::LazyLock;
use tower_http::cors::{Any, CorsLayer};

use crate::pomodoro::{PomodoroTimer, SharedPomodoro};
use crate::storage::Storage;
use crate::title_parser::parse_title;
use crate::tray::format_duration;

/// Global Pomodoro timer instance
static POMODORO: LazyLock<SharedPomodoro> = LazyLock::new(|| {
    std::sync::Arc::new(PomodoroTimer::new())
});

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
    pub active_secs: i64,
    pub passive_secs: i64,
    pub active_percent: u32,
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
    pub active_secs: i64,
    pub passive_secs: i64,
    pub active_percent: u32,
}

#[derive(Serialize)]
pub struct HourlyStat {
    pub hour: u32,
    pub secs: i64,
    pub active_secs: i64,
    pub passive_secs: i64,
}

#[derive(Serialize)]
pub struct DetailedEntry {
    pub app_name: String,
    pub category: String,
    pub window_title: String,
    pub parsed_display: String,
    pub context_type: String,
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

/// Analytics summary with insights
#[derive(Serialize)]
pub struct AnalyticsSummary {
    pub best_hour: Option<u32>,
    pub best_hour_secs: i64,
    pub most_used_app: String,
    pub most_used_secs: i64,
    pub focus_streak_mins: i64,
    pub total_apps_today: usize,
    pub active_percent: u32,
}

/// Daily trend data point
#[derive(Serialize)]
pub struct TrendDay {
    pub date: String,
    pub total_secs: i64,
    pub active_secs: i64,
    pub passive_secs: i64,
}

/// Burnout risk assessment
#[derive(Serialize)]
pub struct BurnoutAssessment {
    pub level: String,        // "low", "medium", "high", "critical"
    pub weekly_hours: f64,
    pub consecutive_long_days: u32,
    pub trend_direction: String, // "increasing", "stable", "decreasing"
    pub recommendation: String,
}

/// Pomodoro timer status
#[derive(Serialize)]
pub struct PomodoroStatus {
    pub state: String,           // "idle", "working", "short_break", "long_break", "paused"
    pub remaining_secs: u64,
    pub remaining_formatted: String,
    pub completed_pomodoros: u64,
    pub enabled: bool,
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
        .route("/api/analytics/summary", get(api_analytics_summary))
        .route("/api/analytics/trends", get(api_analytics_trends))
        .route("/api/analytics/burnout", get(api_analytics_burnout))
        .route("/api/tracking/pause", post(api_pause))
        .route("/api/tracking/resume", post(api_resume))
        // Pomodoro routes
        .route("/api/pomodoro/status", get(api_pomodoro_status))
        .route("/api/pomodoro/start", post(api_pomodoro_start))
        .route("/api/pomodoro/pause", post(api_pomodoro_pause))
        .route("/api/pomodoro/resume", post(api_pomodoro_resume))
        .route("/api/pomodoro/reset", post(api_pomodoro_reset))
        .route("/api/pomodoro/skip", post(api_pomodoro_skip))
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
            active_secs: 0,
            passive_secs: 0,
            active_percent: 0,
            apps: vec![],
            hourly: vec![],
        }),
    };

    let total_secs = storage.get_today_total_secs().unwrap_or(0);
    let summaries = storage.get_today_summary().unwrap_or_default();
    let hourly = storage.get_today_hourly_detailed().unwrap_or_default();

    let total = summaries.iter().map(|s| s.total_secs).sum::<i64>().max(1);
    let total_active: i64 = summaries.iter().map(|s| s.active_secs).sum();
    let total_passive: i64 = summaries.iter().map(|s| s.passive_secs).sum();

    let apps: Vec<AppStat> = summaries
        .iter()
        .map(|s| {
            let app_total = s.total_secs.max(1);
            AppStat {
                name: s.app_name.clone(),
                category: s.category.clone(),
                secs: s.total_secs,
                formatted: format_duration(s.total_secs),
                percent: ((s.total_secs as f64 / total as f64) * 100.0) as u32,
                active_secs: s.active_secs,
                passive_secs: s.passive_secs,
                active_percent: ((s.active_secs as f64 / app_total as f64) * 100.0) as u32,
            }
        })
        .collect();

    let hourly_stats: Vec<HourlyStat> = hourly
        .iter()
        .map(|h| HourlyStat {
            hour: h.hour,
            secs: h.active_secs + h.passive_secs,
            active_secs: h.active_secs,
            passive_secs: h.passive_secs,
        })
        .collect();

    let overall_active_percent = if total_secs > 0 {
        ((total_active as f64 / total_secs as f64) * 100.0) as u32
    } else {
        0
    };

    Json(TodaySummary {
        total_secs,
        total_formatted: format_duration(total_secs),
        active_secs: total_active,
        passive_secs: total_passive,
        active_percent: overall_active_percent,
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
        .map(|(app, cat, title, secs)| {
            let parsed = parse_title(app, cat, title);
            DetailedEntry {
                app_name: app.clone(),
                category: cat.clone(),
                window_title: title.clone(),
                parsed_display: parsed.display,
                context_type: parsed.context_type,
                secs: *secs,
                formatted: format_duration(*secs),
            }
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

    let hourly = storage.get_today_hourly_detailed().unwrap_or_default();

    // Return all 24 hours with active/passive breakdown
    let mut data: Vec<HourlyStat> = (0u32..24)
        .map(|h| HourlyStat { hour: h, secs: 0, active_secs: 0, passive_secs: 0 })
        .collect();

    for h in hourly {
        if (h.hour as usize) < 24 {
            data[h.hour as usize].secs = h.active_secs + h.passive_secs;
            data[h.hour as usize].active_secs = h.active_secs;
            data[h.hour as usize].passive_secs = h.passive_secs;
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

/// GET /api/analytics/summary - Today's insights
async fn api_analytics_summary(State(state): State<AppState>) -> impl IntoResponse {
    let storage = match Storage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Json(AnalyticsSummary {
            best_hour: None,
            best_hour_secs: 0,
            most_used_app: String::new(),
            most_used_secs: 0,
            focus_streak_mins: 0,
            total_apps_today: 0,
            active_percent: 0,
        }),
    };

    let hourly = storage.get_today_hourly().unwrap_or_default();
    let summaries = storage.get_today_summary().unwrap_or_default();

    // Find best hour
    let best_hour = hourly.iter().max_by_key(|h| h.total_secs);
    let (best_hour_num, best_hour_secs) = best_hour
        .map(|h| (Some(h.hour), h.total_secs))
        .unwrap_or((None, 0));

    // Find most used app
    let most_used = summaries.iter().max_by_key(|s| s.total_secs);
    let (most_app, most_secs) = most_used
        .map(|s| (s.app_name.clone(), s.total_secs))
        .unwrap_or((String::new(), 0));

    // Calculate active percent
    let total_active: i64 = summaries.iter().map(|s| s.active_secs).sum();
    let total_secs: i64 = summaries.iter().map(|s| s.total_secs).sum();
    let active_pct = if total_secs > 0 {
        ((total_active as f64 / total_secs as f64) * 100.0) as u32
    } else {
        0
    };

    // Simple focus streak: find longest consecutive hour block
    let mut max_streak = 0i64;
    let mut current_streak = 0i64;
    for hour in 0..24u32 {
        let secs = hourly.iter().find(|h| h.hour == hour).map(|h| h.total_secs).unwrap_or(0);
        if secs > 300 { // At least 5 minutes in the hour
            current_streak += secs;
        } else {
            max_streak = max_streak.max(current_streak);
            current_streak = 0;
        }
    }
    max_streak = max_streak.max(current_streak);

    Json(AnalyticsSummary {
        best_hour: best_hour_num,
        best_hour_secs,
        most_used_app: most_app,
        most_used_secs: most_secs,
        focus_streak_mins: max_streak / 60,
        total_apps_today: summaries.len(),
        active_percent: active_pct,
    })
}

/// GET /api/analytics/trends - 7 and 30 day trends
async fn api_analytics_trends(State(state): State<AppState>) -> impl IntoResponse {
    let storage = match Storage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Json(Vec::<TrendDay>::new()),
    };

    let history = storage.get_history_days(30).unwrap_or_default();

    // For now, we don't have active/passive breakdown in history
    // Future: add get_history_with_activity() that returns active/passive
    let trends: Vec<TrendDay> = history
        .iter()
        .map(|(date, secs)| TrendDay {
            date: date.to_string(),
            total_secs: *secs,
            active_secs: *secs, // Placeholder - assume all active for historical
            passive_secs: 0,
        })
        .collect();

    Json(trends)
}

/// GET /api/analytics/burnout - Burnout risk assessment
async fn api_analytics_burnout(State(state): State<AppState>) -> impl IntoResponse {
    let storage = match Storage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Json(BurnoutAssessment {
            level: "unknown".to_string(),
            weekly_hours: 0.0,
            consecutive_long_days: 0,
            trend_direction: "stable".to_string(),
            recommendation: "Unable to calculate".to_string(),
        }),
    };

    let history = storage.get_history_days(14).unwrap_or_default();

    // Calculate weekly hours (last 7 days)
    let weekly_secs: i64 = history.iter().take(7).map(|(_, secs)| secs).sum();
    let weekly_hours = weekly_secs as f64 / 3600.0;

    // Count consecutive long days (>10 hours)
    let long_day_threshold = 10 * 3600; // 10 hours
    let mut consecutive = 0u32;
    for (_, secs) in history.iter().take(7) {
        if *secs > long_day_threshold {
            consecutive += 1;
        } else {
            break;
        }
    }

    // Calculate trend direction (simple: compare last 7 days to previous 7)
    let recent_avg: f64 = if history.len() >= 7 {
        history.iter().take(7).map(|(_, s)| *s as f64).sum::<f64>() / 7.0
    } else {
        0.0
    };
    let older_avg: f64 = if history.len() >= 14 {
        history.iter().skip(7).take(7).map(|(_, s)| *s as f64).sum::<f64>() / 7.0
    } else {
        recent_avg
    };

    let trend = if recent_avg > older_avg * 1.1 {
        "increasing"
    } else if recent_avg < older_avg * 0.9 {
        "decreasing"
    } else {
        "stable"
    };

    // Determine burnout level and recommendation
    let (level, recommendation) = match (weekly_hours as u32, consecutive) {
        (w, c) if w > 60 || c >= 5 => (
            "critical",
            "Take a break! Consider taking time off to recover."
        ),
        (w, c) if w > 50 || c >= 3 => (
            "high",
            "Warning: Working too many hours. Plan shorter days this week."
        ),
        (w, _) if w > 45 => (
            "medium",
            "Approaching limits. Try to wrap up earlier today."
        ),
        _ => (
            "low",
            "Good balance! Keep maintaining healthy work hours."
        ),
    };

    Json(BurnoutAssessment {
        level: level.to_string(),
        weekly_hours,
        consecutive_long_days: consecutive,
        trend_direction: trend.to_string(),
        recommendation: recommendation.to_string(),
    })
}

/// POST /api/tracking/pause
async fn api_pause() -> impl IntoResponse {
    Json(serde_json::json!({"status": "paused"}))
}

/// POST /api/tracking/resume
async fn api_resume() -> impl IntoResponse {
    Json(serde_json::json!({"status": "resumed"}))
}

/// GET /api/pomodoro/status - Get current Pomodoro timer state
async fn api_pomodoro_status() -> impl IntoResponse {
    let state = POMODORO.get_state().await;
    Json(PomodoroStatus {
        state: state.as_str().to_string(),
        remaining_secs: POMODORO.get_remaining_secs(),
        remaining_formatted: POMODORO.format_remaining(),
        completed_pomodoros: POMODORO.get_completed_pomodoros(),
        enabled: POMODORO.is_enabled(),
    })
}

/// POST /api/pomodoro/start - Start a work session
async fn api_pomodoro_start() -> impl IntoResponse {
    POMODORO.start_work().await;
    Json(serde_json::json!({"status": "started", "message": "Work session started"}))
}

/// POST /api/pomodoro/pause - Pause the timer
async fn api_pomodoro_pause() -> impl IntoResponse {
    POMODORO.pause().await;
    Json(serde_json::json!({"status": "paused", "message": "Timer paused"}))
}

/// POST /api/pomodoro/resume - Resume the timer
async fn api_pomodoro_resume() -> impl IntoResponse {
    POMODORO.resume().await;
    Json(serde_json::json!({"status": "resumed", "message": "Timer resumed"}))
}

/// POST /api/pomodoro/reset - Reset the timer
async fn api_pomodoro_reset() -> impl IntoResponse {
    POMODORO.reset().await;
    Json(serde_json::json!({"status": "reset", "message": "Timer reset"}))
}

/// POST /api/pomodoro/skip - Skip current session
async fn api_pomodoro_skip() -> impl IntoResponse {
    POMODORO.skip().await;
    Json(serde_json::json!({"status": "skipped", "message": "Session skipped"}))
}

/// Start the web server
pub async fn start_web_server(db_path: PathBuf, port: u16) -> anyhow::Result<()> {
    let state = AppState { db_path };
    let app = create_router(state);

    // Start Pomodoro timer tick task
    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            let completed = POMODORO.tick().await;
            if completed {
                tracing::info!("Pomodoro session completed!");
                // Could send notification here in the future
            }
        }
    });

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("Web dashboard at http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
