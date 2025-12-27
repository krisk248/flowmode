use anyhow::{Result, anyhow};
use std::process::Command;
use tracing::{debug, warn};

/// Information about the currently active window
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub window_id: String,
    pub window_class: String,
    pub window_title: String,
}

/// Get the currently active window information
pub fn get_active_window() -> Result<WindowInfo> {
    // Get active window ID using xdotool
    let window_id = Command::new("xdotool")
        .arg("getactivewindow")
        .output()?;

    if !window_id.status.success() {
        return Err(anyhow!("Failed to get active window"));
    }

    let window_id = String::from_utf8_lossy(&window_id.stdout)
        .trim()
        .to_string();

    if window_id.is_empty() {
        return Err(anyhow!("No active window"));
    }

    // Get window name (title)
    let window_title = Command::new("xdotool")
        .args(["getwindowname", &window_id])
        .output()?;

    let window_title = String::from_utf8_lossy(&window_title.stdout)
        .trim()
        .to_string();

    // Get window class using xprop
    let window_class = get_window_class(&window_id)?;

    debug!("Active window: {} - {} ({})", window_class, window_title, window_id);

    Ok(WindowInfo {
        window_id,
        window_class,
        window_title,
    })
}

/// Get window class using xprop
fn get_window_class(window_id: &str) -> Result<String> {
    let output = Command::new("xprop")
        .args(["-id", window_id, "WM_CLASS"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse: WM_CLASS(STRING) = "instance", "class"
    if let Some(start) = stdout.find('"') {
        let rest = &stdout[start + 1..];
        if let Some(end) = rest.find('"') {
            return Ok(rest[..end].to_string());
        }
    }

    // Fallback: try second quoted string (the actual class)
    let parts: Vec<&str> = stdout.split('"').collect();
    if parts.len() >= 4 {
        return Ok(parts[3].to_string());
    }

    Ok("unknown".to_string())
}

/// Get idle time in seconds using xprintidle
pub fn get_idle_time_secs() -> Result<u64> {
    let output = Command::new("xprintidle")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let ms: u64 = String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse()
                .unwrap_or(0);
            Ok(ms / 1000)
        }
        Ok(_) => {
            warn!("xprintidle failed, assuming active");
            Ok(0)
        }
        Err(_) => {
            // xprintidle not installed, fall back to assuming active
            debug!("xprintidle not available, assuming active");
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_active_window() {
        // This will only work in a graphical environment
        if std::env::var("DISPLAY").is_ok() {
            let result = get_active_window();
            println!("Active window: {:?}", result);
        }
    }
}
