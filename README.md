# FlowMode

A privacy-focused activity tracker for Linux, written in Rust.

Unlike cloud-based alternatives (Wakatime, RescueTime), **all your data stays local**. Track only the apps you choose, see exactly where your time goes.

## Features

- **Privacy-first**: All data stored locally in SQLite
- **Whitelist-based**: Only tracks apps you explicitly configure
- **Window titles**: See which browser tabs, terminal folders, or files you worked on
- **Web Dashboard**: Beautiful Svelte-based dashboard at localhost:5555
- **System tray**: Live tracking indicator with "Open Dashboard" button
- **TUI Dashboard**: Multi-pane terminal UI with tabs (Summary, Detailed, Timeline)
- **Self-update**: Update with `flowmode update`
- **Idle detection**: Automatically pauses when you're away
- **Lightweight**: Single binary, minimal resource usage

## Installation

### Prerequisites

```bash
# Ubuntu/Debian
sudo apt install xdotool xprintidle

# Fedora
sudo dnf install xdotool xprintidle

# Arch
sudo pacman -S xdotool xprintidle
```

### Build from Source

```bash
git clone https://github.com/krisk248/flowmode.git
cd flowmode
cargo build --release

# Binary will be at ./target/release/flowmode
```

### Install Binary

```bash
# Copy to local bin
cp ./target/release/flowmode ~/.local/bin/

# Or system-wide
sudo cp ./target/release/flowmode /usr/local/bin/
```

## Usage

### Quick Start

```bash
# Generate default config
flowmode init

# Start tracking
flowmode start

# View stats
flowmode stats

# Open TUI dashboard
flowmode dashboard
```

### Commands

| Command | Description |
|---------|-------------|
| `flowmode start` | Start daemon + web server |
| `flowmode stop` | Stop the daemon |
| `flowmode web` | Open web dashboard in browser |
| `flowmode stats` | Show today's activity summary |
| `flowmode detailed` | Show detailed stats with window titles |
| `flowmode dashboard` | Open live TUI dashboard |
| `flowmode apps` | List configured apps |
| `flowmode reset` | Clear today's data |
| `flowmode init` | Generate default config |
| `flowmode update` | Self-update from GitHub |
| `flowmode version` | Show version info |

### TUI Dashboard

The dashboard has 3 tabs with keyboard navigation:

| Key | Action |
|-----|--------|
| `1` / `2` / `3` | Jump to Summary / Detailed / Timeline |
| `Tab` / `Arrow keys` | Cycle through tabs |
| `j` / `k` or `Up` / `Down` | Scroll in Detailed view |
| `q` / `Esc` | Quit |

**Tabs:**
- **Summary**: App breakdown with progress bars
- **Detailed**: Window titles grouped by app
- **Timeline**: Hourly activity chart + category breakdown

### Example Output

**Summary (`flowmode stats`):**
```
FlowMode - Today's Activity
════════════════════════════════════════

Total tracked: 4h 32m

Brave           2h 15m  49% ████████████████████
VS Code         1h 28m  32% █████████████
Ghostty            42m  15% ██████
Obsidian            7m   2% █
```

**Detailed (`flowmode detailed`):**
```
FlowMode - Detailed Activity
════════════════════════════════════════════════════════════════

Brave ─────────────────────────────────
   1h 12m  YouTube - Brave
      38m  GitHub - krisk248/flowmode - Brave
      25m  ChatGPT - Brave

Ghostty ─────────────────────────────────
      28m  ~/Projects/flowmode
      14m  ~/Documents
```

## Configuration

Config file: `~/.config/flowmode/config.toml`

```toml
idle_timeout_secs = 300    # 5 minutes
poll_interval_secs = 5     # Check every 5 seconds

[[apps]]
name = "Brave"
match_type = "windowclass"
pattern = "brave"
category = "Browser"

[[apps]]
name = "VS Code"
match_type = "windowclass"
pattern = "code"
category = "Development"

[[apps]]
name = "Ghostty"
match_type = "windowclass"
pattern = "ghostty"
category = "Terminal"

[[apps]]
name = "Teams"
match_type = "windowtitle"
pattern = "Microsoft Teams"
category = "Communication"
```

### Match Types

- `windowclass` - Match by WM_CLASS (most reliable)
- `windowtitle` - Match if window title contains pattern
- `process` - Match by process name

### Finding Window Class

```bash
# Click on a window to get its class
xprop WM_CLASS | grep -i class
```

## System Tray

The tray icon shows:
- **Date and time** in tooltip
- **Working/Idle/Paused** status
- **Today's tracked time**

Icons change based on status:
- Clock icon when working
- Idle icon when away
- Pause icon when paused

## Systemd Service (Auto-start)

### Create Service File

```bash
mkdir -p ~/.config/systemd/user

cat > ~/.config/systemd/user/flowmode.service << 'EOF'
[Unit]
Description=FlowMode Activity Tracker
After=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.local/bin/flowmode start
Restart=on-failure
RestartSec=5
Environment=DISPLAY=:0

[Install]
WantedBy=default.target
EOF
```

### Enable and Start

```bash
# Reload systemd
systemctl --user daemon-reload

# Enable auto-start on login
systemctl --user enable flowmode

# Start now
systemctl --user start flowmode

# Check status
systemctl --user status flowmode

# View logs
journalctl --user -u flowmode -f
```

### Stop Service

```bash
systemctl --user stop flowmode
systemctl --user disable flowmode
```

## Data Storage

- **Config**: `~/.config/flowmode/config.toml`
- **Database**: `~/.local/share/flowmode/activity.db`

All data is stored locally. Nothing is sent to any server.

### Backup Data

```bash
cp ~/.local/share/flowmode/activity.db ~/backup/
```

### Query Data Directly

```bash
sqlite3 ~/.local/share/flowmode/activity.db \
  "SELECT app_name, SUM(duration_secs)/60 as minutes
   FROM activity
   WHERE date(started_at) = date('now')
   GROUP BY app_name
   ORDER BY minutes DESC"
```

## Troubleshooting

### Tray icon not showing
- Ensure you have a system tray (KDE, GNOME with extension)
- Check if `ksni` is working: `systemctl --user status`

### Window detection not working
```bash
# Test xdotool
xdotool getactivewindow getwindowname

# Test xprop
xprop -id $(xdotool getactivewindow) WM_CLASS
```

### Idle detection not working
```bash
# Test xprintidle (returns milliseconds)
xprintidle
```

## Web Dashboard

When you run `flowmode start`, a web dashboard is available at **http://localhost:5555**

**Features:**
- Live-updating activity summary
- App breakdown with category colors
- Hourly activity chart
- Detailed window title view
- 30-day history
- Dark theme

Access via:
- System tray → "Open Dashboard"
- `flowmode web` command
- Direct browser to localhost:5555

## Changelog

### v0.4.0
- Web dashboard with Svelte frontend (localhost:5555)
- "Open Dashboard" in system tray opens browser
- Self-update: `flowmode update` downloads latest from GitHub
- `flowmode version` command
- Single binary with embedded web assets

### v0.3.0
- Multi-pane TUI dashboard with 3 tabs (Summary, Detailed, Timeline)
- Keyboard navigation (1/2/3, Tab, arrows, j/k)
- Improved system tray with date, idle status, and dynamic icons
- Hourly activity chart with current hour indicator
- Category breakdown view
- Better color-coded progress bars

### v0.2.0
- Initial release
- Window tracking with xdotool
- SQLite storage
- System tray with ksni
- Basic TUI dashboard
- Idle detection with xprintidle

## License

MIT License - see LICENSE file

## Contributing

Issues and PRs welcome at https://github.com/krisk248/flowmode
