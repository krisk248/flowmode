use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline},
    Frame, Terminal,
};
use std::io;

use crate::storage::{AppSummary, HourlyActivity, Storage};
use crate::tray::format_duration;

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
    loop {
        // Get data
        let summaries = storage.get_today_summary().unwrap_or_default();
        let total_secs = storage.get_today_total_secs().unwrap_or(0);
        let hourly = storage.get_today_hourly().unwrap_or_default();
        let week = storage.get_week_summary().unwrap_or_default();

        terminal.draw(|f| {
            ui(f, &summaries, total_secs, &hourly);
        })?;

        // Handle input
        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        _ => {}
                    }
                }
            }
        }
    }
}

fn ui(f: &mut Frame, summaries: &[AppSummary], total_secs: i64, hourly: &[HourlyActivity]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(3),  // Today's total
            Constraint::Min(10),    // App breakdown
            Constraint::Length(8),  // Hourly chart
            Constraint::Length(2),  // Footer
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled("FlowMode", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" - Activity Tracker"),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header, chunks[0]);

    // Today's total with gauge
    let hours_worked = total_secs as f64 / 3600.0;
    let target_hours = 8.0;
    let percent = ((hours_worked / target_hours) * 100.0).min(100.0) as u16;

    let gauge = Gauge::default()
        .block(Block::default().title("Today's Progress").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green))
        .percent(percent)
        .label(format!("{} / {}h target", format_duration(total_secs), target_hours as u32));
    f.render_widget(gauge, chunks[1]);

    // App breakdown
    let total = summaries.iter().map(|s| s.total_secs).sum::<i64>().max(1);
    let items: Vec<ListItem> = summaries
        .iter()
        .map(|s| {
            let pct = (s.total_secs as f64 / total as f64 * 100.0) as u32;
            let bar_len = (pct as usize / 2).min(25);
            let bar: String = "█".repeat(bar_len);
            let color = category_color(&s.category);

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:<15}", s.app_name),
                    Style::default().fg(color).add_modifier(Modifier::BOLD)
                ),
                Span::styled(
                    format!("{:<8}", format_duration(s.total_secs)),
                    Style::default().fg(Color::White)
                ),
                Span::styled(
                    format!("{:>3}% ", pct),
                    Style::default().fg(Color::DarkGray)
                ),
                Span::styled(bar, Style::default().fg(color)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title("App Breakdown").borders(Borders::ALL));
    f.render_widget(list, chunks[2]);

    // Hourly activity sparkline
    let mut hourly_data: [u64; 24] = [0; 24];
    for h in hourly {
        if (h.hour as usize) < 24 {
            hourly_data[h.hour as usize] = h.total_secs as u64;
        }
    }

    let sparkline = Sparkline::default()
        .block(Block::default().title("Hourly Activity (0-23h)").borders(Borders::ALL))
        .data(&hourly_data)
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(sparkline, chunks[3]);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Press ", Style::default().fg(Color::DarkGray)),
        Span::styled("q", Style::default().fg(Color::Cyan)),
        Span::styled(" to quit", Style::default().fg(Color::DarkGray)),
    ]));
    f.render_widget(footer, chunks[4]);
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
