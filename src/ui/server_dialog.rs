use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{ServerDialogPhase, ServerDialogState};
use crate::model::AppStatus;

const SPRING_GREEN: Color = Color::Rgb(80, 200, 50);

/// Centered popup with fixed width percentage and row count.
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let v_pad = area.height.saturating_sub(height) / 2;
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(v_pad),
            Constraint::Length(height),
            Constraint::Length(v_pad),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn render(f: &mut Frame, state: &ServerDialogState) {
    match state.phase {
        ServerDialogPhase::ChooseMethod => render_choose_method(f, state),
        ServerDialogPhase::ManualEntry => render_manual_entry(f, state),
        ServerDialogPhase::Scanning => render_scanning(f, state),
        ServerDialogPhase::ScanResults => render_scan_results(f, state),
    }
}

// ---------------------------------------------------------------------------
// Phase 1: Choose between Manual Entry and Scan Local
// ---------------------------------------------------------------------------

fn render_choose_method(f: &mut Frame, state: &ServerDialogState) {
    let area = centered_rect(50, 10, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SPRING_GREEN))
        .title(Span::styled(
            " Add Server ",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Prompt text
            Constraint::Length(1), // Spacer
            Constraint::Min(1),    // Options list
        ])
        .split(inner);

    // Prompt
    let prompt = Paragraph::new(Line::from(Span::styled(
        "How would you like to add a server?",
        Style::default().fg(Color::White),
    )))
    .alignment(Alignment::Center);
    f.render_widget(prompt, chunks[0]);

    // Options
    let options = vec![
        ListItem::new(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("Manual Entry", Style::default().fg(Color::White)),
            Span::styled(
                "  — Enter name and URL manually",
                Style::default().fg(Color::DarkGray),
            ),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("Scan Local", Style::default().fg(Color::White)),
            Span::styled(
                "  — Scan local ports for Spring Boot apps",
                Style::default().fg(Color::DarkGray),
            ),
        ])),
    ];

    let list = List::new(options).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let mut list_state = ListState::default();
    list_state.select(Some(state.method_selected));
    f.render_stateful_widget(list, chunks[2], &mut list_state);
}

// ---------------------------------------------------------------------------
// Phase 2a: Manual entry form (name + URL)
// ---------------------------------------------------------------------------

fn render_manual_entry(f: &mut Frame, state: &ServerDialogState) {
    let area = centered_rect(60, 10, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SPRING_GREEN))
        .title(Span::styled(
            " Add Server — Manual Entry ",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Name field
            Constraint::Length(3), // URL field
            Constraint::Length(1), // Error message
            Constraint::Min(0),    // padding
        ])
        .split(inner);

    // -- Name field --
    let name_border_color = if state.active_field == 0 {
        SPRING_GREEN
    } else {
        Color::DarkGray
    };

    let name_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(name_border_color))
        .title(Span::styled(
            " Name ",
            Style::default()
                .fg(if state.active_field == 0 {
                    SPRING_GREEN
                } else {
                    Color::Gray
                })
                .add_modifier(Modifier::BOLD),
        ));

    let name_text = if state.name.is_empty() && state.active_field != 0 {
        Span::styled("e.g. My App", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(&state.name, Style::default().fg(Color::White))
    };

    let name_paragraph = Paragraph::new(Line::from(name_text)).block(name_block);
    f.render_widget(name_paragraph, chunks[0]);

    // -- URL field --
    let url_border_color = if state.active_field == 1 {
        SPRING_GREEN
    } else {
        Color::DarkGray
    };

    let url_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(url_border_color))
        .title(Span::styled(
            " URL ",
            Style::default()
                .fg(if state.active_field == 1 {
                    SPRING_GREEN
                } else {
                    Color::Gray
                })
                .add_modifier(Modifier::BOLD),
        ));

    let url_text = if state.url.is_empty() && state.active_field != 1 {
        Span::styled(
            "e.g. http://localhost:8080",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        Span::styled(&state.url, Style::default().fg(Color::White))
    };

    let url_paragraph = Paragraph::new(Line::from(url_text)).block(url_block);
    f.render_widget(url_paragraph, chunks[1]);

    // -- Error message --
    if let Some(ref err) = state.error {
        let err_paragraph =
            Paragraph::new(Span::styled(err.as_str(), Style::default().fg(Color::Red)))
                .alignment(Alignment::Center);
        f.render_widget(err_paragraph, chunks[2]);
    }
}

// ---------------------------------------------------------------------------
// Phase 2b: Scanning in progress
// ---------------------------------------------------------------------------

fn render_scanning(f: &mut Frame, state: &ServerDialogState) {
    let area = centered_rect(50, 8, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SPRING_GREEN))
        .title(Span::styled(
            " Scanning Local Ports ",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Spinner + message
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Found count
            Constraint::Min(0),    // padding
        ])
        .split(inner);

    // Spinner + progress message
    let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let frame = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        / 100) as usize;
    let spinner = spinner_chars[frame % spinner_chars.len()];

    let progress_line = Line::from(vec![
        Span::styled(format!("  {} ", spinner), Style::default().fg(SPRING_GREEN)),
        Span::styled(&state.scan_progress, Style::default().fg(Color::Gray)),
    ]);
    let progress_widget = Paragraph::new(progress_line);
    f.render_widget(progress_widget, chunks[0]);

    // Found count
    let found_count = state.discovered_apps.len();
    let found_line = Line::from(vec![
        Span::styled("  Found: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{} app(s)", found_count),
            Style::default()
                .fg(if found_count > 0 {
                    Color::Green
                } else {
                    Color::DarkGray
                })
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let found_widget = Paragraph::new(found_line);
    f.render_widget(found_widget, chunks[2]);
}

// ---------------------------------------------------------------------------
// Phase 3: Scan results — select discovered apps
// ---------------------------------------------------------------------------

fn render_scan_results(f: &mut Frame, state: &ServerDialogState) {
    let height = (state.discovered_apps.len() as u16 + 6).clamp(8, 20);
    let area = centered_rect(60, height, f.area());
    f.render_widget(Clear, area);

    let found_count = state.discovered_apps.len();
    let title = format!(" Scan Results — {} app(s) found ", found_count);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SPRING_GREEN))
        .title(Span::styled(
            title,
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.discovered_apps.is_empty() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(inner);

        let msg = Paragraph::new(Line::from(Span::styled(
            "  No Spring Boot applications found on local ports.",
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(msg, chunks[0]);

        let hint = Paragraph::new(Line::from(Span::styled(
            "  Press Esc to go back.",
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(hint, chunks[1]);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Hint
            Constraint::Min(1),    // List
        ])
        .split(inner);

    let hint = Paragraph::new(Line::from(Span::styled(
        "  Select an app and press Enter to add it:",
        Style::default().fg(Color::Gray),
    )));
    f.render_widget(hint, chunks[0]);

    let items: Vec<ListItem> = state
        .discovered_apps
        .iter()
        .map(|app| {
            let status_style = match app.status {
                AppStatus::Up => Style::default().fg(Color::Green),
                AppStatus::Down => Style::default().fg(Color::Red),
                AppStatus::Unknown => Style::default().fg(Color::Yellow),
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("  :{:<6}", app.port),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<30}", app.url),
                    Style::default().fg(Color::White),
                ),
                Span::styled(format!(" {}", app.status), status_style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let mut list_state = ListState::default();
    list_state.select(Some(state.scan_selected_index));
    f.render_stateful_widget(list, chunks[1], &mut list_state);
}
