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

    let filtered: Vec<(usize, &crate::model::EnvProperty)> = app
        .env_props
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            if filter.is_empty() {
                return true;
            }
            e.name.to_lowercase().contains(&filter)
                || e.value.to_lowercase().contains(&filter)
                || e.source.to_lowercase().contains(&filter)
        })
        .collect();

    let title = if filter.is_empty() {
        format!(" Environment ({}) ", app.env_props.len())
    } else {
        format!(" Environment ({}/{}) ", filtered.len(), app.env_props.len())
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
        let msg = if app.env_props.is_empty() {
            "  No environment properties loaded. Connect to a server and run :env"
        } else {
            "  No matching properties."
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
        Cell::from("VALUE"),
        Cell::from("SOURCE"),
    ])
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
            let source_style = Style::default().fg(Color::DarkGray);
            Row::new(vec![
                Cell::from(highlight_text(&e.name, &filter, base)),
                Cell::from(highlight_text(&e.value, &filter, base)),
                Cell::from(highlight_text(&e.source, &filter, source_style)),
            ])
        })
        .collect();

    let widths = [
        ratatui::layout::Constraint::Percentage(30),
        ratatui::layout::Constraint::Percentage(40),
        ratatui::layout::Constraint::Percentage(30),
    ];

    let table = Table::new(rows, widths).header(header).row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let selected_pos = filtered
        .iter()
        .position(|(idx, _)| *idx == app.selected_env_index)
        .unwrap_or(0);

    let mut state = TableState::default();
    state.select(Some(selected_pos));
    f.render_stateful_widget(table, inner_area, &mut state);
}
