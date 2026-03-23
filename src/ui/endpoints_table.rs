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

    let filtered: Vec<(usize, &crate::model::Endpoint)> = app
        .endpoints
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            if filter.is_empty() {
                return true;
            }
            e.name.to_lowercase().contains(&filter) || e.url.to_lowercase().contains(&filter)
        })
        .collect();

    let title = if filter.is_empty() {
        format!(" Endpoints ({}) ", app.endpoints.len())
    } else {
        format!(" Endpoints ({}/{}) ", filtered.len(), app.endpoints.len())
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
        let msg = if app.endpoints.is_empty() {
            "  No endpoints available. Connect to a server first."
        } else {
            "  No matching endpoints."
        };
        let empty = ratatui::widgets::Paragraph::new(Span::styled(
            msg,
            Style::default().fg(Color::DarkGray),
        ));
        f.render_widget(empty, inner_area);
        return;
    }

    let header = Row::new(vec![Cell::from("ENDPOINT"), Cell::from("URL")])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

    let rows: Vec<Row> = filtered
        .iter()
        .map(|(_, e)| {
            let base = Style::default().fg(Color::White);
            Row::new(vec![
                Cell::from(highlight_text(&e.name, &filter, base)),
                Cell::from(highlight_text(&e.url, &filter, base)),
            ])
        })
        .collect();

    let widths = [
        ratatui::layout::Constraint::Percentage(50),
        ratatui::layout::Constraint::Percentage(50),
    ];

    let table = Table::new(rows, widths).header(header).row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let selected_pos = filtered
        .iter()
        .position(|(idx, _)| *idx == app.selected_endpoint_index)
        .unwrap_or(0);

    let mut state = TableState::default();
    state.select(Some(selected_pos));
    f.render_stateful_widget(table, inner_area, &mut state);
}
