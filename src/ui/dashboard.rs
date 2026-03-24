use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

const SPRING_GREEN: Color = Color::Rgb(80, 200, 50);

/// Render a horizontal bar gauge: `[▓▓▓▓▓▓░░░░]`
fn bar_gauge(ratio: f64, width: usize) -> String {
    let filled = ((ratio.clamp(0.0, 1.0)) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "▓".repeat(filled), "░".repeat(empty))
}

/// Format seconds into a human-readable uptime string.
fn format_uptime(seconds: f64) -> String {
    let total = seconds as u64;
    let days = total / 86400;
    let hours = (total % 86400) / 3600;
    let mins = (total % 3600) / 60;
    if days > 0 {
        format!("{}d {}h {}m", days, hours, mins)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

/// Status indicator dot.
fn status_dot(status: &str) -> Span<'static> {
    let (symbol, color) = match status.to_uppercase().as_str() {
        "UP" => ("●", SPRING_GREEN),
        "DOWN" => ("●", Color::Red),
        _ => ("●", Color::Yellow),
    };
    Span::styled(format!("{} ", symbol), Style::default().fg(color))
}

fn make_panel<'a>(title: &str, lines: Vec<Line<'a>>) -> Paragraph<'a> {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    Paragraph::new(lines).block(block)
}

fn label_value<'a>(label: &str, value: String) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!(" {:<14}", label), Style::default().fg(Color::Gray)),
        Span::styled(value, Style::default().fg(Color::White)),
    ])
}

fn label_value_colored<'a>(label: &str, value: String, color: Color) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!(" {:<14}", label), Style::default().fg(Color::Gray)),
        Span::styled(value, Style::default().fg(color)),
    ])
}

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let d = &app.dashboard;

    // Overall layout: 3 rows
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // row 1: Health | JVM Memory | Threads
            Constraint::Length(8), // row 2: CPU | HTTP Requests | GC
            Constraint::Min(6),    // row 3: Info | Disk | Profiles
        ])
        .split(area);

    // =====================================================================
    // Row 1
    // =====================================================================
    let row1 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(rows[0]);

    // -- Health panel --
    let mut health_lines: Vec<Line> = vec![Line::from(vec![
        Span::raw(" "),
        status_dot(&d.app_status),
        Span::styled(
            d.app_status.clone(),
            Style::default()
                .fg(if d.app_status == "UP" {
                    SPRING_GREEN
                } else {
                    Color::Red
                })
                .add_modifier(Modifier::BOLD),
        ),
    ])];
    for comp in &d.health_components {
        let color = if comp.status == "UP" {
            SPRING_GREEN
        } else {
            Color::Red
        };
        health_lines.push(Line::from(vec![
            Span::raw("  "),
            status_dot(&comp.status),
            Span::styled(format!("{:<12}", comp.name), Style::default().fg(color)),
            Span::styled(
                if comp.details.is_empty() {
                    String::new()
                } else {
                    format!(" {}", comp.details)
                },
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }
    f.render_widget(make_panel("Health", health_lines), row1[0]);

    // -- JVM Memory panel --
    let heap_ratio = if d.heap_max_mb > 0.0 {
        d.heap_used_mb / d.heap_max_mb
    } else {
        0.0
    };
    let heap_color = if heap_ratio > 0.85 {
        Color::Red
    } else if heap_ratio > 0.7 {
        Color::Yellow
    } else {
        SPRING_GREEN
    };
    let mem_lines = vec![
        Line::from(vec![
            Span::styled(" Heap     ", Style::default().fg(Color::Gray)),
            Span::styled(bar_gauge(heap_ratio, 16), Style::default().fg(heap_color)),
            Span::styled(
                format!(" {:.0}/{:.0} MB", d.heap_used_mb, d.heap_max_mb),
                Style::default().fg(Color::White),
            ),
        ]),
        label_value("Non-heap:", format!("{:.0} MB", d.nonheap_used_mb)),
        label_value("Usage:", format!("{:.1}%", heap_ratio * 100.0)),
    ];
    f.render_widget(make_panel("JVM Memory", mem_lines), row1[1]);

    // -- Threads panel --
    let thread_lines = vec![
        label_value_colored("Live:", format!("{}", d.threads_live), Color::White),
        label_value("Peak:", format!("{}", d.threads_peak)),
        label_value("Daemon:", format!("{}", d.threads_daemon)),
        label_value(
            "Non-daemon:",
            format!("{}", d.threads_live.saturating_sub(d.threads_daemon)),
        ),
    ];
    f.render_widget(make_panel("Threads", thread_lines), row1[2]);

    // =====================================================================
    // Row 2
    // =====================================================================
    let row2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(rows[1]);

    // -- CPU panel --
    let sys_color = if d.cpu_system > 80.0 {
        Color::Red
    } else if d.cpu_system > 50.0 {
        Color::Yellow
    } else {
        SPRING_GREEN
    };
    let proc_color = if d.cpu_process > 80.0 {
        Color::Red
    } else if d.cpu_process > 50.0 {
        Color::Yellow
    } else {
        SPRING_GREEN
    };
    let cpu_lines = vec![
        Line::from(vec![
            Span::styled(" System   ", Style::default().fg(Color::Gray)),
            Span::styled(
                bar_gauge(d.cpu_system / 100.0, 16),
                Style::default().fg(sys_color),
            ),
            Span::styled(
                format!(" {:.1}%", d.cpu_system),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Process  ", Style::default().fg(Color::Gray)),
            Span::styled(
                bar_gauge(d.cpu_process / 100.0, 16),
                Style::default().fg(proc_color),
            ),
            Span::styled(
                format!(" {:.1}%", d.cpu_process),
                Style::default().fg(Color::White),
            ),
        ]),
    ];
    f.render_widget(make_panel("CPU", cpu_lines), row2[0]);

    // -- HTTP Requests panel --
    let avg_ms = if d.http_total_count > 0 {
        (d.http_total_time_s / d.http_total_count as f64) * 1000.0
    } else {
        0.0
    };
    let error_pct = if d.http_total_count > 0 {
        (d.http_error_count as f64 / d.http_total_count as f64) * 100.0
    } else {
        0.0
    };
    let err_color = if error_pct > 5.0 {
        Color::Red
    } else if error_pct > 1.0 {
        Color::Yellow
    } else {
        SPRING_GREEN
    };
    let http_lines = vec![
        label_value("Total:", format!("{}", d.http_total_count)),
        label_value("Avg:", format!("{:.1} ms", avg_ms)),
        label_value_colored(
            "Errors:",
            format!("{} ({:.2}%)", d.http_error_count, error_pct),
            err_color,
        ),
    ];
    f.render_widget(make_panel("HTTP Requests", http_lines), row2[1]);

    // -- GC panel --
    let avg_gc = if d.gc_pause_count > 0 {
        d.gc_pause_total_ms / d.gc_pause_count as f64
    } else {
        0.0
    };
    let gc_lines = vec![
        label_value("Pauses:", format!("{}", d.gc_pause_count)),
        label_value("Total:", format!("{:.1} ms", d.gc_pause_total_ms)),
        label_value("Avg:", format!("{:.2} ms", avg_gc)),
    ];
    f.render_widget(make_panel("Garbage Collection", gc_lines), row2[2]);

    // =====================================================================
    // Row 3
    // =====================================================================
    let row3 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(rows[2]);

    // -- Info panel --
    let mut info_lines = vec![label_value("Uptime:", format_uptime(d.uptime_seconds))];
    if !d.java_version.is_empty() {
        info_lines.push(label_value("Java:", d.java_version.clone()));
    }
    if !d.spring_boot_version.is_empty() {
        info_lines.push(label_value("Spring Boot:", d.spring_boot_version.clone()));
    }
    info_lines.push(label_value("Server:", app.current_server_name()));
    f.render_widget(make_panel("Info", info_lines), row3[0]);

    // -- Disk panel --
    let disk_ratio = if d.disk_total_gb > 0.0 {
        (d.disk_total_gb - d.disk_free_gb) / d.disk_total_gb
    } else {
        0.0
    };
    let disk_color = if disk_ratio > 0.9 {
        Color::Red
    } else if disk_ratio > 0.75 {
        Color::Yellow
    } else {
        SPRING_GREEN
    };
    let disk_lines = vec![
        Line::from(vec![
            Span::styled(" Used     ", Style::default().fg(Color::Gray)),
            Span::styled(bar_gauge(disk_ratio, 16), Style::default().fg(disk_color)),
            Span::styled(
                format!(" {:.1}%", disk_ratio * 100.0),
                Style::default().fg(Color::White),
            ),
        ]),
        label_value("Free:", format!("{:.1} GB", d.disk_free_gb)),
        label_value("Total:", format!("{:.1} GB", d.disk_total_gb)),
    ];
    f.render_widget(make_panel("Disk", disk_lines), row3[1]);

    // -- Profiles panel --
    let mut profile_lines: Vec<Line> = Vec::new();
    if d.active_profiles.is_empty() {
        profile_lines.push(Line::from(Span::styled(
            " (none)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for p in &d.active_profiles {
            profile_lines.push(Line::from(vec![
                Span::styled(" ● ", Style::default().fg(SPRING_GREEN)),
                Span::styled(p.clone(), Style::default().fg(Color::White)),
            ]));
        }
    }
    f.render_widget(make_panel("Active Profiles", profile_lines), row3[2]);
}
