use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

const SPRING_GREEN: Color = Color::Rgb(80, 200, 50);

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(
            " TSB ",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // 5-column layout
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20), // Context
            Constraint::Percentage(25), // Stats
            Constraint::Percentage(20), // Keybindings 1 (context-sensitive)
            Constraint::Percentage(20), // Keybindings 2 (global)
            Constraint::Percentage(15), // Logo
        ])
        .split(inner);

    // -- Column 1: Context --
    let server_name = app.current_server_name();
    let context_lines = vec![
        Line::from(vec![
            Span::styled(" Server: ", Style::default().fg(Color::Gray)),
            Span::styled(
                &server_name,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Resource: ", Style::default().fg(Color::Gray)),
            Span::styled(
                &app.active_resource,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];
    let context_widget = Paragraph::new(context_lines);
    f.render_widget(context_widget, columns[0]);

    // -- Column 2: Stats --
    let app_count = app.apps.len();
    let version = env!("CARGO_PKG_VERSION");
    let stats_lines = vec![
        Line::from(vec![
            Span::styled(" Apps: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", app_count),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Version: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("v{}", version), Style::default().fg(Color::White)),
        ]),
    ];
    let stats_widget = Paragraph::new(stats_lines);
    f.render_widget(stats_widget, columns[1]);

    // -- Column 3: Keybindings (context-sensitive) --
    let keybindings_context = match app.active_resource.as_str() {
        "apps" => vec![
            Line::from(vec![
                Span::styled(" Enter", Style::default().fg(Color::Yellow)),
                Span::styled(" Connect", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled(" a", Style::default().fg(Color::Yellow)),
                Span::styled(" Add Server  ", Style::default().fg(Color::DarkGray)),
                Span::styled("^d", Style::default().fg(Color::Yellow)),
                Span::styled(" Delete", Style::default().fg(Color::DarkGray)),
            ]),
        ],
        "loggers" => vec![
            Line::from(vec![
                Span::styled(" e", Style::default().fg(Color::Yellow)),
                Span::styled(" Edit Level", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled(" Enter", Style::default().fg(Color::Yellow)),
                Span::styled(" Describe", Style::default().fg(Color::DarkGray)),
            ]),
        ],
        "threaddump" => vec![
            Line::from(vec![
                Span::styled(" t", Style::default().fg(Color::Yellow)),
                Span::styled(" ThreadDump", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled(" Enter", Style::default().fg(Color::Yellow)),
                Span::styled(" Describe", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled(" /", Style::default().fg(Color::Yellow)),
                Span::styled(" Filter", Style::default().fg(Color::DarkGray)),
            ]),
        ],
        "heapdump" => vec![
            Line::from(vec![
                Span::styled(" h", Style::default().fg(Color::Yellow)),
                Span::styled(" HeapDump  ", Style::default().fg(Color::DarkGray)),
                Span::styled("v", Style::default().fg(Color::Yellow)),
                Span::styled(" VisualVM", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled(" m", Style::default().fg(Color::Yellow)),
                Span::styled(" Eclipse MAT", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled(" Enter", Style::default().fg(Color::Yellow)),
                Span::styled(" Describe  ", Style::default().fg(Color::DarkGray)),
                Span::styled("/", Style::default().fg(Color::Yellow)),
                Span::styled(" Filter", Style::default().fg(Color::DarkGray)),
            ]),
        ],
        _ => vec![
            Line::from(vec![
                Span::styled(" Enter", Style::default().fg(Color::Yellow)),
                Span::styled(" Describe", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled(" /", Style::default().fg(Color::Yellow)),
                Span::styled(" Filter", Style::default().fg(Color::DarkGray)),
            ]),
        ],
    };
    let keybindings1_widget = Paragraph::new(keybindings_context);
    f.render_widget(keybindings1_widget, columns[2]);

    // -- Column 4: Global keybindings --
    let keybindings_global = vec![
        Line::from(vec![
            Span::styled(" :", Style::default().fg(Color::Yellow)),
            Span::styled(" Command", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" q", Style::default().fg(Color::Yellow)),
            Span::styled(" Quit", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" ctrl-c", Style::default().fg(Color::Yellow)),
            Span::styled(" Force Quit", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    let keybindings2_widget = Paragraph::new(keybindings_global);
    f.render_widget(keybindings2_widget, columns[3]);

    // -- Column 5: Logo --
    let logo = vec![
        Line::from(Span::styled(
            r" _____ ___ ___ ",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"|_   _/ __| _ )",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"  | | \__ \ _ \",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"  |_| |___/___/",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("         v{}", version),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let logo_widget = Paragraph::new(logo).alignment(Alignment::Right);
    f.render_widget(logo_widget, columns[4]);
}
