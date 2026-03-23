use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

/// Centered popup: fixed width percentage and row count.
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

pub fn render(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 9, f.area());

    // Clear the area behind the dialog
    f.render_widget(Clear, area);

    let title = if app.modal_title.is_empty() {
        " Error ".to_string()
    } else {
        format!(" {} ", app.modal_title)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(Span::styled(
            title,
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Inner layout: message + spacer + OK button
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // message
            Constraint::Length(1), // spacer
            Constraint::Length(1), // OK button
            Constraint::Length(1), // bottom padding
        ])
        .split(inner);

    // -- Message --
    let message = if app.modal_msg.is_empty() {
        "An error occurred.".to_string()
    } else {
        app.modal_msg.clone()
    };

    let msg_paragraph = Paragraph::new(message)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
    f.render_widget(msg_paragraph, chunks[0]);

    // -- OK button --
    let ok_line = Line::from(Span::styled(
        "[ OK ]",
        Style::default()
            .fg(Color::Black)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    let ok_paragraph = Paragraph::new(ok_line).alignment(Alignment::Center);
    f.render_widget(ok_paragraph, chunks[2]);
}
