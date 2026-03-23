pub mod apps_table;
pub mod beans_table;
pub mod describe;
pub mod dialog;
pub mod dumps_table;
pub mod edit_logger;
pub mod endpoints_table;
pub mod env_table;
pub mod header;
pub mod loggers_table;
pub mod mappings_table;
pub mod new_project;
pub mod resources;
pub mod server_dialog;
pub mod splash;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, Mode};

const SPRING_GREEN: Color = Color::Rgb(80, 200, 50);

/// Build a `Line` from `text` with all occurrences of `filter` (case-insensitive)
/// highlighted. When `filter` is empty the text is returned with `base_style`.
/// `hl_style` is applied to the matching portions.
/// All spans own their data so no lifetime issues with temporaries.
pub fn highlight_text(text: &str, filter: &str, base_style: Style) -> Line<'static> {
    if filter.is_empty() {
        return Line::from(Span::styled(text.to_string(), base_style));
    }

    let hl_style = base_style
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let lower_text = text.to_lowercase();
    let lower_filter = filter.to_lowercase();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut last_end = 0;

    for (start, _) in lower_text.match_indices(&lower_filter) {
        if start > last_end {
            spans.push(Span::styled(text[last_end..start].to_string(), base_style));
        }
        spans.push(Span::styled(
            text[start..start + lower_filter.len()].to_string(),
            hl_style,
        ));
        last_end = start + lower_filter.len();
    }
    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }
    if spans.is_empty() {
        return Line::from(Span::styled(text.to_string(), base_style));
    }
    Line::from(spans)
}

/// Top-level render dispatch — called from the main loop on every frame.
pub fn render(f: &mut Frame, app: &App) {
    match app.mode {
        Mode::Splash => {
            splash::render(f, &app.splash_state);
            return;
        }
        Mode::ServerDialog => {
            // Render the normal UI underneath, then overlay the dialog.
            render_main(f, app);
            server_dialog::render(f, &app.server_dialog_state);
            return;
        }
        Mode::NewProject => {
            // Full-screen wizard overlay — render main underneath for context.
            render_main(f, app);
            new_project::render(f, &app.new_project_state);
            return;
        }
        _ => {}
    }

    render_main(f, app);

    // Overlay rendering for modal modes
    match app.mode {
        Mode::Confirm => {
            dialog::render(f, app);
        }
        Mode::Resources => {
            resources::render(f, app);
        }
        Mode::EditLogger => {
            edit_logger::render(f, &app.edit_logger_state);
        }
        _ => {}
    }
}

/// Renders the main layout: header + content + footer.
fn render_main(f: &mut Frame, app: &App) {
    let size = f.area();

    // Vertical layout: header(6) | content(fill) | footer(1)
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // header
            Constraint::Min(1),    // content
            Constraint::Length(1), // footer
        ])
        .split(size);

    let header_area = layout[0];
    let content_area = layout[1];
    let footer_area = layout[2];

    // -- Header --
    header::render(f, app, header_area);

    // -- Content: dispatch based on active_resource and mode --
    match app.mode {
        Mode::Describe => {
            describe::render(f, app, content_area);
        }
        _ => match app.active_resource.as_str() {
            "apps" => apps_table::render(f, app, content_area),
            "endpoints" => endpoints_table::render(f, app, content_area),
            "beans" => beans_table::render(f, app, content_area),
            "loggers" => loggers_table::render(f, app, content_area),
            "mappings" => mappings_table::render(f, app, content_area),
            "env" => env_table::render(f, app, content_area),
            "threaddump" => dumps_table::render_thread_dumps(f, app, content_area),
            "heapdump" => dumps_table::render_heap_dumps(f, app, content_area),
            _ => apps_table::render(f, app, content_area),
        },
    }

    // -- Footer --
    render_footer(f, app, footer_area);
}

/// Renders the footer bar with filter text or status info.
fn render_footer(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let footer_spans = if app.filter_active {
        vec![
            Span::styled(
                " Filter: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&app.filter_text, Style::default().fg(Color::White)),
            Span::styled(
                "█",
                Style::default().fg(Color::White), // cursor
            ),
        ]
    } else if let Some(ref err) = app.error_message {
        vec![Span::styled(
            format!(" Error: {}", err),
            Style::default().fg(Color::Red),
        )]
    } else {
        let resource_label = app.active_resource.as_str();
        let count = match resource_label {
            "apps" => app.apps.len(),
            "endpoints" => app.endpoints.len(),
            "beans" => app.beans.len(),
            "loggers" => app.loggers.len(),
            "mappings" => app.mappings.len(),
            "env" => app.env_props.len(),
            "threaddump" => app.saved_thread_dumps.len(),
            "heapdump" => app.saved_heap_dumps.len(),
            _ => 0,
        };
        vec![
            Span::styled(
                format!(" {} ", resource_label),
                Style::default()
                    .fg(Color::Black)
                    .bg(SPRING_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {} items", count),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "  Press ':' for command palette  ",
                Style::default().fg(Color::DarkGray),
            ),
        ]
    };

    let footer_line = Line::from(footer_spans);
    let footer_widget = Paragraph::new(footer_line).style(Style::default().bg(Color::Black));
    f.render_widget(footer_widget, area);
}
