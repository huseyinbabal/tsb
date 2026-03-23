use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

use super::highlight_text;
use crate::app::App;
use crate::model::SavedDump;

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn format_timestamp(ts: &str) -> String {
    // Input: "20260312_143022" → "2026-03-12 14:30:22"
    if ts.len() == 15 {
        format!(
            "{}-{}-{} {}:{}:{}",
            &ts[0..4],
            &ts[4..6],
            &ts[6..8],
            &ts[9..11],
            &ts[11..13],
            &ts[13..15],
        )
    } else {
        ts.to_string()
    }
}

fn render_dump_table(
    f: &mut Frame,
    area: Rect,
    title_label: &str,
    dumps: &[SavedDump],
    selected_index: usize,
    filter: &str,
) {
    let filtered: Vec<(usize, &SavedDump)> = dumps
        .iter()
        .enumerate()
        .filter(|(_, d)| {
            if filter.is_empty() {
                return true;
            }
            let f = filter.to_lowercase();
            d.path.to_lowercase().contains(&f)
                || d.timestamp.to_lowercase().contains(&f)
                || d.app_name.to_lowercase().contains(&f)
        })
        .collect();

    let title = if filter.is_empty() {
        format!(" {} ({}) ", title_label, dumps.len())
    } else {
        format!(" {} ({}/{}) ", title_label, filtered.len(), dumps.len())
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
        let msg = if dumps.is_empty() {
            format!(
                "  No {} saved. Use 't' or 'h' on an app to capture one.",
                title_label.to_lowercase()
            )
        } else {
            "  No matching dumps.".to_string()
        };
        let empty = ratatui::widgets::Paragraph::new(Span::styled(
            msg,
            Style::default().fg(Color::DarkGray),
        ));
        f.render_widget(empty, inner_area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("TIMESTAMP"),
        Cell::from("APP"),
        Cell::from("SIZE"),
        Cell::from("PATH"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let rows: Vec<Row> = filtered
        .iter()
        .map(|(_, d)| {
            let base = Style::default().fg(Color::White);
            let dim = Style::default().fg(Color::DarkGray);
            let ts = format_timestamp(&d.timestamp);
            let size = format_size(d.size_bytes);
            let app_display = if d.app_name.is_empty() {
                "—".to_string()
            } else {
                d.app_name.clone()
            };
            Row::new(vec![
                Cell::from(highlight_text(&ts, filter, base)),
                Cell::from(highlight_text(&app_display, filter, base)),
                Cell::from(Span::styled(size, base)),
                Cell::from(highlight_text(&d.path, filter, dim)),
            ])
        })
        .collect();

    let widths = [
        ratatui::layout::Constraint::Percentage(22),
        ratatui::layout::Constraint::Percentage(20),
        ratatui::layout::Constraint::Percentage(12),
        ratatui::layout::Constraint::Percentage(46),
    ];

    let table = Table::new(rows, widths).header(header).row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let selected_pos = filtered
        .iter()
        .position(|(idx, _)| *idx == selected_index)
        .unwrap_or(0);

    let mut state = TableState::default();
    state.select(Some(selected_pos));
    f.render_stateful_widget(table, inner_area, &mut state);
}

pub fn render_thread_dumps(f: &mut Frame, app: &App, area: Rect) {
    render_dump_table(
        f,
        area,
        "Thread Dumps",
        &app.saved_thread_dumps,
        app.selected_thread_dump_index,
        &app.filter_text,
    );
}

pub fn render_heap_dumps(f: &mut Frame, app: &App, area: Rect) {
    render_dump_table(
        f,
        area,
        "Heap Dumps",
        &app.saved_heap_dumps,
        app.selected_heap_dump_index,
        &app.filter_text,
    );
}
