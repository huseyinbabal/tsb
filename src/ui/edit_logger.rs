use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{EditLoggerState, LOG_LEVELS};

const SPRING_GREEN: Color = Color::Rgb(80, 200, 50);

/// Centered popup with fixed width and height.
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

pub fn render(f: &mut Frame, state: &EditLoggerState) {
    let height = (LOG_LEVELS.len() as u16 + 6).min(14);
    let area = centered_rect(50, height, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SPRING_GREEN))
        .title(Span::styled(
            " Edit Logger Level ",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Logger name
            Constraint::Length(1), // Current level
            Constraint::Length(1), // Spacer
            Constraint::Min(1),    // Level list
            Constraint::Length(1), // Error
        ])
        .split(inner);

    // Logger name
    let name_line = Line::from(vec![
        Span::styled("  Logger: ", Style::default().fg(Color::Gray)),
        Span::styled(
            &state.logger_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(Paragraph::new(name_line), chunks[0]);

    // Current level
    let current_line = Line::from(vec![
        Span::styled("  Current: ", Style::default().fg(Color::Gray)),
        Span::styled(&state.current_level, Style::default().fg(Color::Yellow)),
    ]);
    f.render_widget(Paragraph::new(current_line), chunks[1]);

    // Level list
    let items: Vec<ListItem> = LOG_LEVELS
        .iter()
        .map(|&level| {
            let level_style = match level {
                "ERROR" => Style::default().fg(Color::Red),
                "WARN" => Style::default().fg(Color::Yellow),
                "INFO" => Style::default().fg(Color::Green),
                "DEBUG" => Style::default().fg(Color::Cyan),
                "TRACE" => Style::default().fg(Color::Magenta),
                "OFF" => Style::default().fg(Color::DarkGray),
                _ => Style::default().fg(Color::White),
            };

            let marker = if level == state.current_level {
                " * "
            } else {
                "   "
            };
            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(SPRING_GREEN)),
                Span::styled(level, level_style.add_modifier(Modifier::BOLD)),
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
    list_state.select(Some(state.selected_level_index));
    f.render_stateful_widget(list, chunks[3], &mut list_state);

    // Error
    if let Some(ref err) = state.error {
        let err_paragraph =
            Paragraph::new(Span::styled(err.as_str(), Style::default().fg(Color::Red)))
                .alignment(Alignment::Center);
        f.render_widget(err_paragraph, chunks[4]);
    }
}
