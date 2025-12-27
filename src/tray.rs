use ksni::{self, menu::StandardItem, Tray, TrayService};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
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
    current_app: Arc<std::sync::RwLock<String>>,
    today_time: Arc<std::sync::RwLock<String>>,
    tx: mpsc::Sender<TrayCommand>,
}

impl FlowModeTray {
    pub fn new(tx: mpsc::Sender<TrayCommand>) -> Self {
        Self {
            is_tracking: Arc::new(AtomicBool::new(true)),
            current_app: Arc::new(std::sync::RwLock::new("Starting...".into())),
            today_time: Arc::new(std::sync::RwLock::new("0h 0m".into())),
            tx,
        }
    }

    pub fn set_current_app(&self, app: &str) {
        if let Ok(mut current) = self.current_app.write() {
            *current = app.to_string();
        }
    }

    pub fn set_today_time(&self, time: &str) {
        if let Ok(mut today) = self.today_time.write() {
            *today = time.to_string();
        }
    }

    pub fn is_tracking(&self) -> bool {
        self.is_tracking.load(Ordering::Relaxed)
    }

    pub fn tracking_handle(&self) -> Arc<AtomicBool> {
        self.is_tracking.clone()
    }

    pub fn current_app_handle(&self) -> Arc<std::sync::RwLock<String>> {
        self.current_app.clone()
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
        // Use a standard icon - clock/time related
        "chronometer".into()
    }

    fn title(&self) -> String {
        let time = self.today_time.read()
            .map(|t| t.clone())
            .unwrap_or_else(|_| "0h".into());
        format!("FlowMode - {}", time)
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let app = self.current_app.read()
            .map(|a| a.clone())
            .unwrap_or_else(|_| "Unknown".into());
        let time = self.today_time.read()
            .map(|t| t.clone())
            .unwrap_or_else(|_| "0h 0m".into());

        let status = if self.is_tracking.load(Ordering::Relaxed) {
            "Tracking"
        } else {
            "Paused"
        };

        ksni::ToolTip {
            icon_name: "chronometer".into(),
            title: "FlowMode".into(),
            description: format!(
                "<b>Status:</b> {}<br/>\
                 <b>Current:</b> {}<br/>\
                 <b>Today:</b> {}",
                status, app, time
            ),
            icon_pixmap: Vec::new(),
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        let is_tracking = self.is_tracking.load(Ordering::Relaxed);

        vec![
            // Header showing current status
            StandardItem {
                label: format!("Today: {}",
                    self.today_time.read()
                        .map(|t| t.clone())
                        .unwrap_or_else(|_| "0h 0m".into())
                ),
                enabled: false,
                ..Default::default()
            }.into(),

            MenuItem::Separator,

            // Current app
            StandardItem {
                label: format!("Tracking: {}",
                    self.current_app.read()
                        .map(|a| a.clone())
                        .unwrap_or_else(|_| "None".into())
                ),
                enabled: false,
                ..Default::default()
            }.into(),

            MenuItem::Separator,

            // Show TUI stats
            StandardItem {
                label: "Show Stats (TUI)".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.blocking_send(TrayCommand::ShowStats);
                }),
                ..Default::default()
            }.into(),

            MenuItem::Separator,

            // Pause/Resume
            if is_tracking {
                StandardItem {
                    label: "Pause Tracking".into(),
                    activate: Box::new(|tray: &mut Self| {
                        tray.is_tracking.store(false, Ordering::Relaxed);
                        let _ = tray.tx.blocking_send(TrayCommand::Pause);
                    }),
                    ..Default::default()
                }.into()
            } else {
                StandardItem {
                    label: "Resume Tracking".into(),
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
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.blocking_send(TrayCommand::Quit);
                }),
                ..Default::default()
            }.into(),
        ]
    }
}

/// Start the tray service
pub fn start_tray_service() -> anyhow::Result<(
    TrayService<FlowModeTray>,
    mpsc::Receiver<TrayCommand>,
    Arc<AtomicBool>,
    Arc<std::sync::RwLock<String>>,
    Arc<std::sync::RwLock<String>>,
)> {
    let (tx, rx) = mpsc::channel(100);
    let tray = FlowModeTray::new(tx);

    let tracking = tray.tracking_handle();
    let current_app = tray.current_app_handle();
    let today_time = tray.today_time_handle();

    let service = TrayService::new(tray);

    Ok((service, rx, tracking, current_app, today_time))
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
