use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use super::highlight_text;
use crate::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let filter = app.filter_text.to_lowercase();

    let filtered: Vec<(usize, &crate::model::Logger)> = app
        .loggers
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            if filter.is_empty() {
                return true;
            }
            l.name.to_lowercase().contains(&filter)
                || l.effective_level.to_lowercase().contains(&filter)
                || l.configured_level
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&filter)
        })
        .collect();

    let title = if filter.is_empty() {
        format!(" Loggers ({}) ", app.loggers.len())
    } else {
        format!(" Loggers ({}/{}) ", filtered.len(), app.loggers.len())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    if filtered.is_empty() {
        let msg = if app.loggers.is_empty() {
            "  No loggers loaded. Connect to a server and run :loggers"
        } else {
            "  No matching loggers."
        };
        let empty = ratatui::widgets::Paragraph::new(Span::styled(
            msg,
            Style::default().fg(Color::DarkGray),
        ));
        f.render_widget(empty, inner_area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("NAME"),
        Cell::from("CONFIGURED"),
        Cell::from("EFFECTIVE"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let rows: Vec<Row> = filtered
        .iter()
        .map(|(_, l)| {
            let configured = l.configured_level.as_deref().unwrap_or("-").to_string();
            let base = Style::default().fg(Color::White);

            let effective_style = match l.effective_level.as_str() {
                "ERROR" => Style::default().fg(Color::Red),
                "WARN" => Style::default().fg(Color::Yellow),
                "INFO" => Style::default().fg(Color::Green),
                "DEBUG" => Style::default().fg(Color::Cyan),
                "TRACE" => Style::default().fg(Color::Magenta),
                _ => Style::default().fg(Color::Gray),
            };

            Row::new(vec![
                Cell::from(highlight_text(&l.name, &filter, base)),
                Cell::from(highlight_text(&configured, &filter, base)),
                Cell::from(highlight_text(&l.effective_level, &filter, effective_style)),
            ])
        })
        .collect();

    let widths = [
        ratatui::layout::Constraint::Percentage(50),
        ratatui::layout::Constraint::Percentage(25),
        ratatui::layout::Constraint::Percentage(25),
    ];

    let table = Table::new(rows, widths).header(header).row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let selected_pos = filtered
        .iter()
        .position(|(idx, _)| *idx == app.selected_logger_index)
        .unwrap_or(0);

    let mut state = TableState::default();
    state.select(Some(selected_pos));
    f.render_stateful_widget(table, inner_area, &mut state);
}
