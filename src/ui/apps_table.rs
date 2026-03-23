use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use super::highlight_text;
use crate::app::App;
use crate::model::AppStatus;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let filter = app.filter_text.to_lowercase();

    let filtered: Vec<(usize, &crate::model::SpringApp)> = app
        .apps
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            if filter.is_empty() {
                return true;
            }
            a.name.to_lowercase().contains(&filter)
                || a.url.to_lowercase().contains(&filter)
                || a.status.to_string().to_lowercase().contains(&filter)
        })
        .collect();

    let title = if filter.is_empty() {
        format!(" Applications ({}) ", app.apps.len())
    } else {
        format!(" Applications ({}/{}) ", filtered.len(), app.apps.len())
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
        let msg = if app.apps.is_empty() {
            "  No applications configured. Press 'a' to add a server."
        } else {
            "  No matching applications."
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
        Cell::from("URL"),
        Cell::from("STATUS"),
        Cell::from("THREADS"),
        Cell::from("HEAPS"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let rows: Vec<Row> = filtered
        .iter()
        .map(|(_, a)| {
            let status_style = match a.status {
                AppStatus::Up => Style::default().fg(Color::Green),
                AppStatus::Down => Style::default().fg(Color::Red),
                AppStatus::Unknown => Style::default().fg(Color::Yellow),
            };
            let base = Style::default().fg(Color::White);

            let thread_count = app
                .saved_thread_dumps
                .iter()
                .filter(|d| d.app_url == a.url)
                .count();
            let heap_count = app
                .saved_heap_dumps
                .iter()
                .filter(|d| d.app_url == a.url)
                .count();

            let thread_str = if thread_count > 0 {
                format!("{}", thread_count)
            } else {
                "—".into()
            };
            let heap_str = if heap_count > 0 {
                format!("{}", heap_count)
            } else {
                "—".into()
            };

            let dump_style = Style::default().fg(Color::Cyan);

            Row::new(vec![
                Cell::from(highlight_text(&a.name, &filter, base)),
                Cell::from(highlight_text(&a.url, &filter, base)),
                Cell::from(highlight_text(&a.status.to_string(), &filter, status_style)),
                Cell::from(Span::styled(thread_str, dump_style)),
                Cell::from(Span::styled(heap_str, dump_style)),
            ])
        })
        .collect();

    let widths = [
        ratatui::layout::Constraint::Percentage(22),
        ratatui::layout::Constraint::Percentage(38),
        ratatui::layout::Constraint::Percentage(15),
        ratatui::layout::Constraint::Percentage(12),
        ratatui::layout::Constraint::Percentage(13),
    ];

    let table = Table::new(rows, widths).header(header).row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    // Map the global selected index to position within filtered list
    let selected_pos = filtered
        .iter()
        .position(|(idx, _)| *idx == app.selected_app_index)
        .unwrap_or(0);

    let mut state = TableState::default();
    state.select(Some(selected_pos));
    f.render_stateful_widget(table, inner_area, &mut state);
}
