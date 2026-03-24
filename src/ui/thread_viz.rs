use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::app::App;

const SPRING_GREEN: Color = Color::Rgb(80, 200, 50);

/// Map thread state to a color matching VisualVM conventions.
fn state_color(state: &str) -> Color {
    match state.to_uppercase().as_str() {
        "RUNNABLE" => SPRING_GREEN,
        "SLEEPING" | "TIMED_WAITING" => Color::Rgb(255, 165, 0), // orange
        "WAITING" => Color::Yellow,
        "BLOCKED" => Color::Red,
        "NEW" => Color::Cyan,
        "TERMINATED" => Color::DarkGray,
        _ => Color::Gray,
    }
}

/// State label for display.
fn state_label(state: &str) -> &str {
    match state.to_uppercase().as_str() {
        "RUNNABLE" => "Running",
        "TIMED_WAITING" => "Sleeping",
        "WAITING" => "Wait",
        "BLOCKED" => "Blocked",
        "NEW" => "New",
        "TERMINATED" => "Dead",
        _ => state,
    }
}

/// Render a colored bar for the thread state.
fn state_bar(state: &str, width: usize) -> Vec<Span<'static>> {
    let color = state_color(state);
    vec![Span::styled("█".repeat(width), Style::default().fg(color))]
}

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let threads = &app.parsed_threads;

    // Layout: summary bar (3) | thread table (fill) | legend (1)
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // summary
            Constraint::Min(5),    // thread table
            Constraint::Length(1), // legend
        ])
        .split(area);

    // -- Summary panel --
    let total = threads.len();
    let running = threads
        .iter()
        .filter(|t| t.state.to_uppercase() == "RUNNABLE")
        .count();
    let waiting = threads
        .iter()
        .filter(|t| t.state.to_uppercase() == "WAITING")
        .count();
    let timed_waiting = threads
        .iter()
        .filter(|t| t.state.to_uppercase() == "TIMED_WAITING")
        .count();
    let blocked = threads
        .iter()
        .filter(|t| t.state.to_uppercase() == "BLOCKED")
        .count();
    let daemon = threads.iter().filter(|t| t.daemon).count();

    let summary_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            format!(" {} ", app.thread_viz_title),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let summary_lines = vec![
        Line::from(vec![
            Span::styled(" Live threads: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", total),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("    Daemon: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", daemon), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(" Running: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", running), Style::default().fg(SPRING_GREEN)),
            Span::styled("  Waiting: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", waiting), Style::default().fg(Color::Yellow)),
            Span::styled("  Sleeping: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", timed_waiting),
                Style::default().fg(Color::Rgb(255, 165, 0)),
            ),
            Span::styled("  Blocked: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", blocked), Style::default().fg(Color::Red)),
        ]),
    ];

    let summary = Paragraph::new(summary_lines).block(summary_block);
    f.render_widget(summary, layout[0]);

    // -- Thread table --
    let table_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " Threads ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = table_block.inner(layout[1]);
    f.render_widget(table_block, layout[1]);

    if threads.is_empty() {
        let empty = Paragraph::new(Span::styled(
            "  No threads found.",
            Style::default().fg(Color::DarkGray),
        ));
        f.render_widget(empty, inner);
    } else {
        let header = Row::new(vec![
            Cell::from(""),
            Cell::from("Name"),
            Cell::from("State"),
            Cell::from(""),
        ])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

        // Calculate bar width based on available space
        let bar_width = (inner.width as usize).saturating_sub(60).clamp(8, 30);

        let rows: Vec<Row> = threads
            .iter()
            .map(|t| {
                let color = state_color(&t.state);
                Row::new(vec![
                    Cell::from(Span::styled(
                        if t.daemon { "d" } else { " " },
                        Style::default().fg(Color::DarkGray),
                    )),
                    Cell::from(Span::styled(
                        t.name.clone(),
                        Style::default().fg(Color::White),
                    )),
                    Cell::from(Line::from(state_bar(&t.state, bar_width))),
                    Cell::from(Span::styled(
                        state_label(&t.state).to_string(),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    )),
                ])
            })
            .collect();

        let widths = [
            Constraint::Length(2),
            Constraint::Percentage(40),
            Constraint::Min(10),
            Constraint::Length(12),
        ];

        let table = Table::new(rows, widths).header(header).row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

        let mut state = TableState::default();
        state.select(Some(app.thread_viz_scroll));
        *state.offset_mut() = app
            .thread_viz_scroll
            .saturating_sub(inner.height as usize / 2);
        f.render_stateful_widget(table, inner, &mut state);
    }

    // -- Legend bar --
    let legend = Line::from(vec![
        Span::styled(" ██", Style::default().fg(SPRING_GREEN)),
        Span::styled(" Running  ", Style::default().fg(Color::DarkGray)),
        Span::styled("██", Style::default().fg(Color::Rgb(255, 165, 0))),
        Span::styled(" Sleeping  ", Style::default().fg(Color::DarkGray)),
        Span::styled("██", Style::default().fg(Color::Yellow)),
        Span::styled(" Wait  ", Style::default().fg(Color::DarkGray)),
        Span::styled("██", Style::default().fg(Color::Red)),
        Span::styled(" Blocked  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "  j/k Navigate  Esc Back",
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    f.render_widget(
        Paragraph::new(legend).style(Style::default().bg(Color::Black)),
        layout[2],
    );
}
