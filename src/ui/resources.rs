use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;

const SPRING_GREEN: Color = Color::Rgb(80, 200, 50);

/// Centered popup helper — returns a Rect centered in `area` with the given
/// percentage width and height.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
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
    let area = centered_rect(50, 40, f.area());

    // Clear background behind popup
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SPRING_GREEN))
        .title(Span::styled(
            " Command Palette ",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split inner into: input (3 rows) + suggestions list (rest)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(inner);

    // -- Input box --
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " : ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

    // Build input line with ghost text autocomplete
    let mut input_spans = vec![Span::styled(
        &app.command_text,
        Style::default().fg(Color::White),
    )];

    // Ghost text: show the rest of the selected command as dimmed text
    if !app.command_text.is_empty() {
        if let Some(selected) = app.get_selected_command() {
            let cmd_lower = selected.command.to_lowercase();
            let text_lower = app.command_text.to_lowercase();
            if cmd_lower.starts_with(&text_lower) {
                let ghost = &selected.command[app.command_text.len()..];
                if !ghost.is_empty() {
                    input_spans.push(Span::styled(
                        ghost.to_string(),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }
        }
    }

    let input_paragraph = Paragraph::new(Line::from(input_spans)).block(input_block);
    f.render_widget(input_paragraph, chunks[0]);

    // -- Suggestions list --
    let items: Vec<ListItem> = app
        .command_suggestions
        .iter()
        .map(|&idx| {
            let resource = &app.resources[idx];
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<14}", resource.command),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<14}", resource.name),
                    Style::default().fg(Color::White),
                ),
                Span::styled(&resource.description, Style::default().fg(Color::DarkGray)),
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
    list_state.select(Some(app.command_suggestion_selected));
    f.render_stateful_widget(list, chunks[1], &mut list_state);
}
