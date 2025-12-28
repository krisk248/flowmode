use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, debug, Level};
use tracing_subscriber::FmtSubscriber;

mod config;
mod storage;
mod tracker;
mod tray;
mod tui;

use config::Config;
use storage::Storage;
use tray::{start_tray_service, TrayCommand, TrayHandles, format_duration};

#[derive(Parser)]
#[command(name = "flowmode")]
#[command(about = "Privacy-focused activity tracker for Linux")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start FlowMode daemon (background tracking)
    Start,

    /// Show today's activity stats (summary)
    Stats,

    /// Show detailed stats with window titles (tabs, folders)
    Detailed,

    /// Show live TUI dashboard
    Dashboard,

    /// List tracked apps
    Apps,

    /// Stop the daemon
    Stop,

    /// Reset today's data (start fresh)
    Reset,

    /// Generate default config
    Init,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    let level = if cli.verbose { Level::DEBUG } else { Level::INFO };
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .compact()
        .init();

    // Ensure data directory exists
    std::fs::create_dir_all(Config::data_dir())?;

    match cli.command {
        Some(Commands::Start) | None => {
            start_daemon().await
        }
        Some(Commands::Stats) => {
            show_stats()
        }
        Some(Commands::Detailed) => {
            show_detailed_stats()
        }
        Some(Commands::Dashboard) => {
            show_dashboard()
        }
        Some(Commands::Apps) => {
            list_apps()
        }
        Some(Commands::Stop) => {
            stop_daemon()
        }
        Some(Commands::Reset) => {
            reset_today()
        }
        Some(Commands::Init) => {
            init_config()
        }
    }
}

/// Start the activity tracking daemon
async fn start_daemon() -> Result<()> {
    info!("Starting FlowMode activity tracker...");

    // Load config
    let config = Config::load().unwrap_or_default();
    info!("Tracking {} apps", config.apps.len());

    // Open database
    let storage = Arc::new(Storage::open(&Config::db_path())?);

    // Close any orphaned sessions from previous runs
    storage.close_open_sessions()?;

    // Start system tray
    let (tray_service, mut tray_rx, handles) = start_tray_service()?;
    let TrayHandles { tracking: is_tracking, is_idle, idle_secs: idle_secs_handle, today_time } = handles;

    // Spawn tray in separate thread
    std::thread::spawn(move || {
        let _ = tray_service.run();
    });

    info!("System tray started");

    // Current tracking state
    let current_session: Arc<RwLock<Option<i64>>> = Arc::new(RwLock::new(None));

    // Update today's time initially
    if let Ok(total) = storage.get_today_total_secs() {
        if let Ok(mut time) = today_time.write() {
            *time = format_duration(total);
        }
    }

    info!("FlowMode is running. Check the system tray.");

    // Main tracking loop
    let poll_interval = std::time::Duration::from_secs(config.poll_interval_secs);
    let idle_timeout = config.idle_timeout_secs;

    loop {
        tokio::select! {
            // Handle tray commands
            Some(cmd) = tray_rx.recv() => {
                match cmd {
                    TrayCommand::ShowStats => {
                        info!("Opening dashboard...");
                        // Spawn TUI in separate process to not block
                        let _ = std::process::Command::new("flowmode")
                            .arg("dashboard")
                            .spawn();
                    }
                    TrayCommand::Pause => {
                        info!("Tracking paused");
                        // End current session
                        let mut session = current_session.write().await;
                        if let Some(id) = session.take() {
                            storage.end_activity(id)?;
                        }
                    }
                    TrayCommand::Resume => {
                        info!("Tracking resumed");
                    }
                    TrayCommand::Quit => {
                        info!("Shutting down...");
                        // End current session
                        let session = current_session.read().await;
                        if let Some(id) = *session {
                            storage.end_activity(id)?;
                        }
                        break;
                    }
                }
            }

            // Tracking tick
            _ = tokio::time::sleep(poll_interval) => {
                if !is_tracking.load(Ordering::Relaxed) {
                    continue;
                }

                // Check idle
                let idle_secs = tracker::get_idle_time_secs().unwrap_or(0);
                if idle_secs > idle_timeout {
                    debug!("User idle for {}s", idle_secs);
                    // Update tray idle status
                    is_idle.store(true, Ordering::Relaxed);
                    idle_secs_handle.store(idle_secs, Ordering::Relaxed);
                    // End current session if any
                    let mut session = current_session.write().await;
                    if let Some(id) = session.take() {
                        storage.end_activity(id)?;
                    }
                    continue;
                } else {
                    // Not idle - clear idle status
                    is_idle.store(false, Ordering::Relaxed);
                    idle_secs_handle.store(0, Ordering::Relaxed);
                }

                // Get active window
                match tracker::get_active_window() {
                    Ok(window) => {
                        // Check if it matches a tracked app
                        if let Some(app) = config.match_window(&window.window_class, &window.window_title) {
                            let mut session = current_session.write().await;

                            // Check if we need to start new session
                            let need_new_session = match storage.get_active_session() {
                                Ok(Some(active)) => active.app_name != app.name,
                                Ok(None) => true,
                                Err(_) => true,
                            };

                            if need_new_session {
                                // End previous session
                                if let Some(id) = session.take() {
                                    storage.end_activity(id)?;
                                }

                                // Start new session
                                let id = storage.start_activity(
                                    &app.name,
                                    &app.category,
                                    &window.window_title
                                )?;
                                *session = Some(id);

                                info!("Tracking: {} ({})", app.name, app.category);
                            }
                        } else {
                            // Not a tracked app - end session
                            let mut session = current_session.write().await;
                            if let Some(id) = session.take() {
                                storage.end_activity(id)?;
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Failed to get active window: {}", e);
                    }
                }

                // Update today's time in tray
                if let Ok(total) = storage.get_today_total_secs() {
                    if let Ok(mut time) = today_time.write() {
                        *time = format_duration(total);
                    }
                }
            }

            // Handle Ctrl+C
            _ = tokio::signal::ctrl_c() => {
                info!("Received Ctrl+C, shutting down...");
                // End current session
                let session = current_session.read().await;
                if let Some(id) = *session {
                    storage.end_activity(id)?;
                }
                break;
            }
        }
    }

    info!("FlowMode stopped.");
    Ok(())
}

/// Show today's stats in CLI
fn show_stats() -> Result<()> {
    let storage = Storage::open(&Config::db_path())?;
    tui::print_stats(&storage)
}

/// Show live TUI dashboard
fn show_dashboard() -> Result<()> {
    let storage = Storage::open(&Config::db_path())?;
    tui::run_tui(&storage)
}

/// List tracked apps
fn list_apps() -> Result<()> {
    let config = Config::load().unwrap_or_default();

    println!();
    println!("  FlowMode - Tracked Applications");
    println!("  ════════════════════════════════════════");
    println!();

    for app in &config.apps {
        println!("  {:<15} [{:<12}] matches: {}",
            app.name,
            app.category,
            app.pattern
        );
    }

    println!();
    println!("  Edit ~/.config/flowmode/config.toml to customize");
    println!();

    Ok(())
}

/// Stop the daemon
fn stop_daemon() -> Result<()> {
    use std::process::Command;

    let output = Command::new("pkill")
        .args(["-f", "flowmode start"])
        .status();

    match output {
        Ok(status) if status.success() => {
            println!("FlowMode daemon stopped.");
        }
        _ => {
            println!("FlowMode daemon is not running.");
        }
    }

    Ok(())
}

/// Generate default config
fn init_config() -> Result<()> {
    let config = Config::default();
    config.save()?;

    println!("Created config at: {:?}", Config::config_path());
    println!("Edit it to customize tracked apps!");

    Ok(())
}

/// Show detailed stats with window titles
fn show_detailed_stats() -> Result<()> {
    let storage = Storage::open(&Config::db_path())?;
    tui::print_detailed_stats(&storage)
}

/// Reset today's data
fn reset_today() -> Result<()> {
    let storage = Storage::open(&Config::db_path())?;
    storage.reset_today()?;

    println!("Today's activity data has been reset.");
    println!("Start fresh tracking now!");

    Ok(())
}
