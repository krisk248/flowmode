use anyhow::Result;
use chrono::{Local, Timelike};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline, Tabs, Wrap},
    Frame, Terminal,
};
use std::io;

use crate::storage::{AppSummary, HourlyActivity, Storage};
use crate::tray::format_duration;

/// Available tabs in the TUI
#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Summary,
    Detailed,
    Timeline,
}

impl Tab {
    fn titles() -> Vec<&'static str> {
        vec!["[1] Summary", "[2] Detailed", "[3] Timeline"]
    }

    fn index(&self) -> usize {
        match self {
            Tab::Summary => 0,
            Tab::Detailed => 1,
            Tab::Timeline => 2,
        }
    }

    fn from_index(idx: usize) -> Self {
        match idx {
            0 => Tab::Summary,
            1 => Tab::Detailed,
            2 => Tab::Timeline,
            _ => Tab::Summary,
        }
    }

    fn next(&self) -> Self {
        Tab::from_index((self.index() + 1) % 3)
    }

    fn prev(&self) -> Self {
        Tab::from_index((self.index() + 2) % 3)
    }
}

/// App state for the TUI
struct AppState {
    current_tab: Tab,
    scroll_offset: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_tab: Tab::Summary,
            scroll_offset: 0,
        }
    }
}

/// Run the TUI application
pub fn run_tui(storage: &Storage) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let result = run_app(&mut terminal, storage);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    storage: &Storage,
) -> Result<()> {
    let mut state = AppState::default();

    loop {
        // Get data
        let summaries = storage.get_today_summary().unwrap_or_default();
        let total_secs = storage.get_today_total_secs().unwrap_or(0);
        let hourly = storage.get_today_hourly().unwrap_or_default();
        let detailed = storage.get_today_detailed().unwrap_or_default();

        terminal.draw(|f| {
            ui(f, &state, &summaries, total_secs, &hourly, &detailed);
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('1') => state.current_tab = Tab::Summary,
                        KeyCode::Char('2') => state.current_tab = Tab::Detailed,
                        KeyCode::Char('3') => state.current_tab = Tab::Timeline,
                        KeyCode::Tab | KeyCode::Right => {
                            state.current_tab = state.current_tab.next();
                            state.scroll_offset = 0;
                        }
                        KeyCode::BackTab | KeyCode::Left => {
                            state.current_tab = state.current_tab.prev();
                            state.scroll_offset = 0;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            state.scroll_offset = state.scroll_offset.saturating_add(1);
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            state.scroll_offset = state.scroll_offset.saturating_sub(1);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn ui(
    f: &mut Frame,
    state: &AppState,
    summaries: &[AppSummary],
    total_secs: i64,
    hourly: &[HourlyActivity],
    detailed: &[(String, String, String, i64)],
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Header with tabs
            Constraint::Length(3),  // Today's progress
            Constraint::Min(10),    // Main content
            Constraint::Length(2),  // Footer
        ])
        .split(f.area());

    // Header with date/time and tabs
    render_header(f, chunks[0], state.current_tab);

    // Progress gauge
    render_progress(f, chunks[1], total_secs);

    // Tab content
    match state.current_tab {
        Tab::Summary => render_summary_tab(f, chunks[2], summaries, total_secs),
        Tab::Detailed => render_detailed_tab(f, chunks[2], detailed, state.scroll_offset),
        Tab::Timeline => render_timeline_tab(f, chunks[2], hourly, summaries),
    }

    // Footer
    render_footer(f, chunks[3], state.current_tab);
}

fn render_header(f: &mut Frame, area: Rect, current_tab: Tab) {
    let now = Local::now();
    let date_str = now.format("%a, %b %d").to_string();
    let time_str = now.format("%H:%M:%S").to_string();

    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(25),  // Logo
            Constraint::Min(30),     // Tabs
            Constraint::Length(20),  // Date/time
        ])
        .split(area);

    // Logo
    let logo = Paragraph::new(Line::from(vec![
        Span::styled("⏱ ", Style::default().fg(Color::Yellow)),
        Span::styled("FlowMode", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]));
    f.render_widget(logo, header_layout[0]);

    // Tabs
    let titles: Vec<Line> = Tab::titles()
        .iter()
        .map(|t| Line::from(*t))
        .collect();
    let tabs = Tabs::new(titles)
        .select(current_tab.index())
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .divider(" │ ");
    f.render_widget(tabs, header_layout[1]);

    // Date/time
    let datetime = Paragraph::new(Line::from(vec![
        Span::styled(&date_str, Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
        Span::styled(&time_str, Style::default().fg(Color::Yellow)),
    ]))
    .alignment(Alignment::Right);
    f.render_widget(datetime, header_layout[2]);
}

fn render_progress(f: &mut Frame, area: Rect, total_secs: i64) {
    let hours_worked = total_secs as f64 / 3600.0;
    let target_hours = 8.0;
    let percent = ((hours_worked / target_hours) * 100.0).min(100.0) as u16;

    let color = if percent >= 100 {
        Color::Green
    } else if percent >= 75 {
        Color::Yellow
    } else if percent >= 50 {
        Color::Cyan
    } else {
        Color::Blue
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL))
        .gauge_style(Style::default().fg(color))
        .percent(percent)
        .label(format!(
            "{} / {}h target ({}%)",
            format_duration(total_secs),
            target_hours as u32,
            percent
        ));
    f.render_widget(gauge, area);
}

fn render_summary_tab(f: &mut Frame, area: Rect, summaries: &[AppSummary], _total_secs: i64) {
    if summaries.is_empty() {
        let empty = Paragraph::new("No activity recorded yet. Start working!")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().title("App Breakdown").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(empty, area);
        return;
    }

    let total = summaries.iter().map(|s| s.total_secs).sum::<i64>().max(1);
    let max_secs = summaries.iter().map(|s| s.total_secs).max().unwrap_or(1);

    let items: Vec<ListItem> = summaries
        .iter()
        .map(|s| {
            let pct = (s.total_secs as f64 / total as f64 * 100.0) as u32;
            let bar_width = ((s.total_secs as f64 / max_secs as f64) * 30.0) as usize;
            let bar: String = "█".repeat(bar_width);
            let color = category_color(&s.category);

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<12}", s.app_name),
                    Style::default().fg(color).add_modifier(Modifier::BOLD)
                ),
                Span::styled(
                    format!("{:>8}", format_duration(s.total_secs)),
                    Style::default().fg(Color::White)
                ),
                Span::styled(
                    format!(" {:>3}% ", pct),
                    Style::default().fg(Color::DarkGray)
                ),
                Span::styled(bar, Style::default().fg(color)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default()
            .title(format!(" Apps ({}) ", summaries.len()))
            .borders(Borders::ALL));
    f.render_widget(list, area);
}

fn render_detailed_tab(
    f: &mut Frame,
    area: Rect,
    detailed: &[(String, String, String, i64)],
    scroll_offset: usize,
) {
    if detailed.is_empty() {
        let empty = Paragraph::new("No detailed activity yet. Start working!")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().title("Window Titles").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(empty, area);
        return;
    }

    // Group by app
    let mut lines: Vec<Line> = Vec::new();
    let mut current_app = String::new();

    for (app_name, category, window_title, secs) in detailed {
        if *app_name != current_app {
            if !current_app.is_empty() {
                lines.push(Line::from(""));
            }
            let color = category_color(category);
            lines.push(Line::from(vec![
                Span::styled(
                    format!("▸ {}", app_name),
                    Style::default().fg(color).add_modifier(Modifier::BOLD)
                ),
            ]));
            current_app = app_name.clone();
        }

        // Truncate long titles
        let max_width = area.width.saturating_sub(15) as usize;
        let title = if window_title.len() > max_width {
            format!("{}...", &window_title[..max_width.saturating_sub(3)])
        } else {
            window_title.clone()
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:>7}  ", format_duration(*secs)),
                Style::default().fg(Color::DarkGray)
            ),
            Span::styled(title, Style::default().fg(Color::White)),
        ]));
    }

    // Apply scroll
    let visible_lines: Vec<Line> = lines
        .into_iter()
        .skip(scroll_offset)
        .collect();

    let paragraph = Paragraph::new(visible_lines)
        .block(Block::default()
            .title(format!(" Window Titles ({} entries) ", detailed.len()))
            .borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_timeline_tab(
    f: &mut Frame,
    area: Rect,
    hourly: &[HourlyActivity],
    summaries: &[AppSummary],
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Hourly chart
            Constraint::Min(5),     // Category breakdown
        ])
        .split(area);

    // Hourly activity sparkline
    let mut hourly_data: [u64; 24] = [0; 24];
    let mut max_activity: u64 = 0;
    for h in hourly {
        if (h.hour as usize) < 24 {
            hourly_data[h.hour as usize] = h.total_secs as u64;
            max_activity = max_activity.max(h.total_secs as u64);
        }
    }

    // Create hour labels
    let current_hour = Local::now().hour() as usize;
    let mut hour_labels = String::new();
    for h in 0..24 {
        if h % 3 == 0 {
            hour_labels.push_str(&format!("{:2} ", h));
        } else {
            hour_labels.push_str("   ");
        }
    }

    let sparkline_block = Block::default()
        .title(" Hourly Activity ")
        .borders(Borders::ALL);

    let inner_area = sparkline_block.inner(chunks[0]);
    f.render_widget(sparkline_block, chunks[0]);

    let sparkline_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner_area);

    let sparkline = Sparkline::default()
        .data(&hourly_data)
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(sparkline, sparkline_layout[0]);

    // Hour markers
    let markers = Paragraph::new(hour_labels)
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(markers, sparkline_layout[1]);

    // Current hour indicator
    let indicator = format!("{}▲ Now ({}:00)", " ".repeat(current_hour * 3), current_hour);
    let indicator_para = Paragraph::new(indicator)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(indicator_para, sparkline_layout[2]);

    // Category breakdown
    let mut categories: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for s in summaries {
        *categories.entry(s.category.clone()).or_insert(0) += s.total_secs;
    }

    let mut cat_list: Vec<_> = categories.into_iter().collect();
    cat_list.sort_by(|a, b| b.1.cmp(&a.1));

    let cat_items: Vec<ListItem> = cat_list
        .iter()
        .map(|(cat, secs)| {
            let color = category_color(cat);
            ListItem::new(Line::from(vec![
                Span::styled("● ", Style::default().fg(color)),
                Span::styled(
                    format!("{:<15}", cat),
                    Style::default().fg(color).add_modifier(Modifier::BOLD)
                ),
                Span::styled(
                    format_duration(*secs),
                    Style::default().fg(Color::White)
                ),
            ]))
        })
        .collect();

    let cat_list_widget = List::new(cat_items)
        .block(Block::default()
            .title(" Categories ")
            .borders(Borders::ALL));
    f.render_widget(cat_list_widget, chunks[1]);
}

fn render_footer(f: &mut Frame, area: Rect, current_tab: Tab) {
    let nav_hint = match current_tab {
        Tab::Summary => "↑↓ scroll",
        Tab::Detailed => "↑↓/jk scroll",
        Tab::Timeline => "view only",
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("1/2/3", Style::default().fg(Color::Cyan)),
        Span::styled(" tabs  ", Style::default().fg(Color::DarkGray)),
        Span::styled("←→/Tab", Style::default().fg(Color::Cyan)),
        Span::styled(" switch  ", Style::default().fg(Color::DarkGray)),
        Span::styled(nav_hint, Style::default().fg(Color::DarkGray)),
        Span::styled("  ", Style::default()),
        Span::styled("q", Style::default().fg(Color::Cyan)),
        Span::styled(" quit", Style::default().fg(Color::DarkGray)),
    ]));
    f.render_widget(footer, area);
}

fn category_color(category: &str) -> Color {
    match category {
        "Development" => Color::Cyan,
        "Communication" => Color::Magenta,
        "Browser" => Color::Blue,
        "Terminal" => Color::Green,
        "Notes" => Color::Yellow,
        "Office" => Color::Red,
        "Files" => Color::White,
        _ => Color::Gray,
    }
}

/// Print simple CLI stats (no TUI)
pub fn print_stats(storage: &Storage) -> Result<()> {
    let summaries = storage.get_today_summary()?;
    let total_secs = storage.get_today_total_secs()?;

    println!();
    println!("  FlowMode - Today's Activity");
    println!("  ════════════════════════════════════════");
    println!();
    println!("  Total tracked: {}", format_duration(total_secs));
    println!();

    if summaries.is_empty() {
        println!("  No activity recorded today.");
    } else {
        let max_secs = summaries.iter().map(|s| s.total_secs).max().unwrap_or(1);

        for summary in &summaries {
            let bar_len = ((summary.total_secs as f64 / max_secs as f64) * 20.0) as usize;
            let bar: String = "█".repeat(bar_len);
            let pct = (summary.total_secs as f64 / total_secs as f64 * 100.0) as u32;

            println!(
                "  {:<15} {:>8} {:>3}% {}",
                summary.app_name,
                format_duration(summary.total_secs),
                pct,
                bar
            );
        }
    }

    println!();
    Ok(())
}

/// Print detailed stats with window titles (tabs, folders, etc.)
pub fn print_detailed_stats(storage: &Storage) -> Result<()> {
    let detailed = storage.get_today_detailed()?;
    let total_secs = storage.get_today_total_secs()?;

    println!();
    println!("  FlowMode - Detailed Activity");
    println!("  ════════════════════════════════════════════════════════════════");
    println!();
    println!("  Total tracked: {}", format_duration(total_secs));
    println!();

    if detailed.is_empty() {
        println!("  No activity recorded today.");
        println!();
        return Ok(());
    }

    let mut current_app = String::new();

    for (app_name, _category, window_title, secs) in &detailed {
        // Print app header when it changes
        if *app_name != current_app {
            if !current_app.is_empty() {
                println!();
            }
            println!("  {} ─────────────────────────────────", app_name);
            current_app = app_name.clone();
        }

        // Truncate long titles
        let title = if window_title.len() > 50 {
            format!("{}...", &window_title[..47])
        } else {
            window_title.clone()
        };

        println!(
            "    {:>8}  {}",
            format_duration(*secs),
            title
        );
    }

    println!();
    println!("  ────────────────────────────────────────────────────────────────");
    println!("  Tip: Window titles show tabs (YouTube - Brave) and folders");
    println!();

    Ok(())
}
