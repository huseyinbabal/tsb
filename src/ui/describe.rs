use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.describe_title.is_empty() {
        " Describe ".to_string()
    } else {
        format!(" {} ", app.describe_title)
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

    let content = if app.describe_content.is_empty() {
        "No details available. Select an item and press Enter to describe it.".to_string()
    } else {
        app.describe_content.clone()
    };

    let paragraph = Paragraph::new(content)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false })
        .scroll((app.describe_scroll, 0));

    f.render_widget(paragraph, inner_area);
}
