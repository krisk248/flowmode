use chrono::Local;
use ksni::{Tray, TrayService};
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use tokio::sync::mpsc;

/// Commands from tray menu
#[derive(Debug, Clone)]
pub enum TrayCommand {
    ShowStats,
    Pause,
    Resume,
    Quit,
}

/// FlowMode system tray
pub struct FlowModeTray {
    is_tracking: Arc<AtomicBool>,
    is_idle: Arc<AtomicBool>,
    idle_secs: Arc<AtomicU64>,
    today_time: Arc<std::sync::RwLock<String>>,
    tx: mpsc::Sender<TrayCommand>,
}

impl FlowModeTray {
    pub fn new(tx: mpsc::Sender<TrayCommand>) -> Self {
        Self {
            is_tracking: Arc::new(AtomicBool::new(true)),
            is_idle: Arc::new(AtomicBool::new(false)),
            idle_secs: Arc::new(AtomicU64::new(0)),
            today_time: Arc::new(std::sync::RwLock::new("0m".into())),
            tx,
        }
    }

    pub fn set_today_time(&self, time: &str) {
        if let Ok(mut today) = self.today_time.write() {
            *today = time.to_string();
        }
    }

    pub fn set_idle(&self, idle: bool, secs: u64) {
        self.is_idle.store(idle, Ordering::Relaxed);
        self.idle_secs.store(secs, Ordering::Relaxed);
    }

    pub fn is_tracking(&self) -> bool {
        self.is_tracking.load(Ordering::Relaxed)
    }

    pub fn tracking_handle(&self) -> Arc<AtomicBool> {
        self.is_tracking.clone()
    }

    pub fn idle_handle(&self) -> Arc<AtomicBool> {
        self.is_idle.clone()
    }

    pub fn idle_secs_handle(&self) -> Arc<AtomicU64> {
        self.idle_secs.clone()
    }

    pub fn today_time_handle(&self) -> Arc<std::sync::RwLock<String>> {
        self.today_time.clone()
    }
}

impl Tray for FlowModeTray {
    fn id(&self) -> String {
        "flowmode".into()
    }

    fn icon_name(&self) -> String {
        if self.is_idle.load(Ordering::Relaxed) {
            "user-idle".into()
        } else if self.is_tracking.load(Ordering::Relaxed) {
            "chronometer".into()
        } else {
            "media-playback-pause".into()
        }
    }

    fn title(&self) -> String {
        let time = self.today_time.read()
            .map(|t| t.clone())
            .unwrap_or_else(|_| "0m".into());

        if self.is_idle.load(Ordering::Relaxed) {
            format!("⏸ {}", time)
        } else if self.is_tracking.load(Ordering::Relaxed) {
            format!("▶ {}", time)
        } else {
            format!("⏹ {}", time)
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let time = self.today_time.read()
            .map(|t| t.clone())
            .unwrap_or_else(|_| "0m".into());
        let date = Local::now().format("%a, %b %d").to_string();

        let status = if self.is_idle.load(Ordering::Relaxed) {
            let idle_mins = self.idle_secs.load(Ordering::Relaxed) / 60;
            format!("Idle ({}m)", idle_mins)
        } else if self.is_tracking.load(Ordering::Relaxed) {
            "Working".into()
        } else {
            "Paused".into()
        };

        ksni::ToolTip {
            icon_name: "chronometer".into(),
            title: "FlowMode".into(),
            description: format!(
                "<b>{}</b><br/>\
                 <b>Status:</b> {}<br/>\
                 <b>Today:</b> {}",
                date, status, time
            ),
            icon_pixmap: Vec::new(),
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        let is_tracking = self.is_tracking.load(Ordering::Relaxed);
        let is_idle = self.is_idle.load(Ordering::Relaxed);
        let date = Local::now().format("%a, %b %d").to_string();

        let status_label = if is_idle {
            let idle_mins = self.idle_secs.load(Ordering::Relaxed) / 60;
            format!("⏸ Idle ({}m)", idle_mins)
        } else if is_tracking {
            "▶ Working".into()
        } else {
            "⏹ Paused".into()
        };

        vec![
            // Date header
            StandardItem {
                label: format!("📅 {}", date),
                enabled: false,
                ..Default::default()
            }.into(),

            // Today's time
            StandardItem {
                label: format!("⏱ Today: {}",
                    self.today_time.read()
                        .map(|t| t.clone())
                        .unwrap_or_else(|_| "0m".into())
                ),
                enabled: false,
                ..Default::default()
            }.into(),

            // Status
            StandardItem {
                label: status_label,
                enabled: false,
                ..Default::default()
            }.into(),

            MenuItem::Separator,

            // Pause/Resume
            if is_tracking {
                StandardItem {
                    label: "⏸ Pause".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.is_tracking.store(false, Ordering::Relaxed);
                        let _ = tray.tx.blocking_send(TrayCommand::Pause);
                    }),
                    ..Default::default()
                }.into()
            } else {
                StandardItem {
                    label: "▶ Resume".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.is_tracking.store(true, Ordering::Relaxed);
                        let _ = tray.tx.blocking_send(TrayCommand::Resume);
                    }),
                    ..Default::default()
                }.into()
            },

            MenuItem::Separator,

            // Quit
            StandardItem {
                label: "✕ Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.blocking_send(TrayCommand::Quit);
                }),
                ..Default::default()
            }.into(),
        ]
    }
}

/// Handles returned from tray service
pub struct TrayHandles {
    pub tracking: Arc<AtomicBool>,
    pub is_idle: Arc<AtomicBool>,
    pub idle_secs: Arc<AtomicU64>,
    pub today_time: Arc<std::sync::RwLock<String>>,
}

/// Start the tray service
pub fn start_tray_service() -> anyhow::Result<(
    TrayService<FlowModeTray>,
    mpsc::Receiver<TrayCommand>,
    TrayHandles,
)> {
    let (tx, rx) = mpsc::channel(100);
    let tray = FlowModeTray::new(tx);

    let handles = TrayHandles {
        tracking: tray.tracking_handle(),
        is_idle: tray.idle_handle(),
        idle_secs: tray.idle_secs_handle(),
        today_time: tray.today_time_handle(),
    };

    let service = TrayService::new(tray);

    Ok((service, rx, handles))
}

/// Format seconds as "Xh Ym"
pub fn format_duration(secs: i64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;

    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}
