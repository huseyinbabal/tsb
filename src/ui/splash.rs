use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

const SPRING_GREEN: Color = Color::Rgb(80, 200, 50); // bright accent for UI elements

/// State for the animated splash screen.
#[derive(Debug, Clone)]
pub struct SplashState {
    pub current_step: usize,
    pub total_steps: usize,
    pub current_message: String,
    pub spinner_frame: usize,
    pub done: bool,
}

impl Default for SplashState {
    fn default() -> Self {
        Self {
            current_step: 0,
            total_steps: 5,
            current_message: "Initializing TSB...".into(),
            spinner_frame: 0,
            done: false,
        }
    }
}

fn render_big_logo(f: &mut Frame, area: Rect) {
    let logo_lines = vec![
        Line::from(Span::styled(
            r"████████╗███████╗██████╗ ",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"╚══██╔══╝██╔════╝██╔══██╗",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"   ██║   ███████╗██████╔╝",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"   ██║   ╚════██║██╔══██╗",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"   ██║   ███████║██████╔╝",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            r"   ╚═╝   ╚══════╝╚═════╝ ",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Terminal UI for Spring Boot",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            format!("v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(logo_lines).alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

pub fn render(f: &mut Frame, splash_state: &SplashState) {
    let area = f.area();

    // Vertical layout: 25% top padding, Min(15) center, 30% bottom padding
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Min(15),
            Constraint::Percentage(30),
        ])
        .split(area);

    let center = outer[1];

    // Horizontal centering
    let h_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(center);

    let content_area = h_layout[1];

    // Inner layout: logo / spacer / progress bar / spacer / status message
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9), // Big logo
            Constraint::Length(1), // spacer
            Constraint::Length(3), // progress bar
            Constraint::Length(1), // spacer
            Constraint::Length(1), // status message
        ])
        .split(content_area);

    // -- Logo --
    render_big_logo(f, inner[0]);

    // -- Progress bar --
    let progress = if splash_state.total_steps > 0 {
        (splash_state.current_step as f64) / (splash_state.total_steps as f64)
    } else {
        0.0
    };

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .gauge_style(Style::default().fg(SPRING_GREEN).bg(Color::Black))
        .ratio(progress.min(1.0))
        .label(format!(
            "{}/{}",
            splash_state.current_step, splash_state.total_steps
        ));

    f.render_widget(gauge, inner[2]);

    // -- Status message --
    let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner = spinner_chars[splash_state.spinner_frame % spinner_chars.len()];

    let status_text = if splash_state.done {
        Line::from(Span::styled(
            "✓ Ready",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ))
    } else {
        Line::from(vec![
            Span::styled(format!("{} ", spinner), Style::default().fg(SPRING_GREEN)),
            Span::styled(
                &splash_state.current_message,
                Style::default().fg(Color::Gray),
            ),
        ])
    };

    let status_widget = Paragraph::new(status_text).alignment(Alignment::Center);
    f.render_widget(status_widget, inner[4]);
}
