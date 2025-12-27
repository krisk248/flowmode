use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// App definition for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedApp {
    pub name: String,
    pub match_type: MatchType,
    pub pattern: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchType {
    WindowClass,  // Match by WM_CLASS
    WindowTitle,  // Match by window title contains
    Process,      // Match by process name
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub idle_timeout_secs: u64,
    pub poll_interval_secs: u64,
    pub apps: Vec<TrackedApp>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            idle_timeout_secs: 300, // 5 minutes
            poll_interval_secs: 5,   // Check every 5 seconds
            apps: vec![
                // Browsers
                TrackedApp {
                    name: "Brave".into(),
                    match_type: MatchType::WindowClass,
                    pattern: "brave".into(),
                    category: "Browser".into(),
                },
                // Communication
                TrackedApp {
                    name: "Teams".into(),
                    match_type: MatchType::WindowTitle,
                    pattern: "Teams".into(),
                    category: "Communication".into(),
                },
                // Terminals
                TrackedApp {
                    name: "Ghostty".into(),
                    match_type: MatchType::WindowClass,
                    pattern: "ghostty".into(),
                    category: "Terminal".into(),
                },
                TrackedApp {
                    name: "Terminus".into(),
                    match_type: MatchType::WindowClass,
                    pattern: "terminus".into(),
                    category: "Terminal".into(),
                },
                // Editors & IDEs
                TrackedApp {
                    name: "Claude Code".into(),
                    match_type: MatchType::WindowTitle,
                    pattern: "Claude".into(),
                    category: "Development".into(),
                },
                TrackedApp {
                    name: "VS Code".into(),
                    match_type: MatchType::WindowClass,
                    pattern: "code".into(),
                    category: "Development".into(),
                },
                // Notes
                TrackedApp {
                    name: "Obsidian".into(),
                    match_type: MatchType::WindowClass,
                    pattern: "obsidian".into(),
                    category: "Notes".into(),
                },
                // Office
                TrackedApp {
                    name: "OnlyOffice".into(),
                    match_type: MatchType::WindowClass,
                    pattern: "onlyoffice".into(),
                    category: "Office".into(),
                },
                // File Manager
                TrackedApp {
                    name: "Dolphin".into(),
                    match_type: MatchType::WindowClass,
                    pattern: "dolphin".into(),
                    category: "Files".into(),
                },
            ],
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("flowmode")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn data_dir() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("flowmode")
    }

    pub fn db_path() -> PathBuf {
        Self::data_dir().join("activity.db")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        std::fs::create_dir_all(path.parent().unwrap())?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Find matching app for given window class and title
    pub fn match_window(&self, window_class: &str, window_title: &str) -> Option<&TrackedApp> {
        let class_lower = window_class.to_lowercase();
        let title_lower = window_title.to_lowercase();

        self.apps.iter().find(|app| {
            let pattern_lower = app.pattern.to_lowercase();
            match app.match_type {
                MatchType::WindowClass => class_lower.contains(&pattern_lower),
                MatchType::WindowTitle => title_lower.contains(&pattern_lower),
                MatchType::Process => class_lower.contains(&pattern_lower),
            }
        })
    }
}
