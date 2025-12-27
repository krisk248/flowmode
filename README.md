# FlowMode

A privacy-focused activity tracker for Linux, written in Rust.

Unlike cloud-based alternatives (Wakatime, RescueTime), **all your data stays local**. Track only the apps you choose, see exactly where your time goes.

## Features

- **Privacy-first**: All data stored locally in SQLite
- **Whitelist-based**: Only tracks apps you explicitly configure
- **Window titles**: See which browser tabs, terminal folders, or files you worked on
- **System tray**: Live tracking indicator with pause/resume
- **TUI Dashboard**: Beautiful terminal UI with charts
- **Idle detection**: Automatically pauses when you're away
- **Lightweight**: ~3MB binary, minimal resource usage

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
```

### Commands

| Command | Description |
|---------|-------------|
| `flowmode start` | Start the tracking daemon |
| `flowmode stop` | Stop the daemon |
| `flowmode stats` | Show today's activity summary |
| `flowmode detailed` | Show detailed stats with window titles |
| `flowmode dashboard` | Open live TUI dashboard |
| `flowmode apps` | List configured apps |
| `flowmode reset` | Clear today's data |
| `flowmode init` | Generate default config |

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
name = "Claude Code"
match_type = "windowtitle"
pattern = "Claude"
category = "Development"
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

## License

MIT License - see LICENSE file

## Contributing

Issues and PRs welcome at https://github.com/krisk248/flowmode
