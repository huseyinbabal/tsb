use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::{NewProjectWizardState, WizardStep};

const SPRING_GREEN: Color = Color::Rgb(80, 200, 50);

/// Centered popup — percentage-based width and fixed height.
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let v_pad = area.height.saturating_sub(height) / 2;
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(v_pad),
            Constraint::Length(height),
            Constraint::Length(v_pad),
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

pub fn render(f: &mut Frame, state: &NewProjectWizardState) {
    match state.step {
        WizardStep::ProjectInfo => render_project_info(f, state),
        WizardStep::Dependencies => render_dependencies(f, state),
        WizardStep::Confirm => render_confirm(f, state),
        WizardStep::Generating => render_generating(f, state),
    }
}

// ---------------------------------------------------------------------------
// Step 1: Project Info
// ---------------------------------------------------------------------------

fn render_project_info(f: &mut Frame, state: &NewProjectWizardState) {
    let area = centered_rect(70, 28, f.area());
    f.render_widget(Clear, area);

    let step_label = " New Project — Step 1/3: Project Info ";
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SPRING_GREEN))
        .title(Span::styled(
            step_label,
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Loading state
    if state.loading_metadata {
        let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            / 100) as usize;
        let spinner = spinner_chars[frame % spinner_chars.len()];
        let loading = Paragraph::new(Line::from(vec![
            Span::styled(format!("  {} ", spinner), Style::default().fg(SPRING_GREEN)),
            Span::styled("Loading metadata...", Style::default().fg(Color::Gray)),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(loading, inner);
        return;
    }

    // Error state
    if let Some(ref err) = state.error {
        if state.metadata.is_none() {
            let err_p = Paragraph::new(vec![
                Line::from(Span::styled(err.as_str(), Style::default().fg(Color::Red))),
                Line::from(""),
                Line::from(Span::styled(
                    "Press Esc to go back",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .alignment(Alignment::Center);
            f.render_widget(err_p, inner);
            return;
        }
    }

    let meta = match &state.metadata {
        Some(m) => m,
        None => return,
    };

    // Layout: 10 field rows (each 1 line) + hint + spacer
    let mut constraints: Vec<Constraint> = Vec::new();
    for _ in 0..10 {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1)); // spacer
    constraints.push(Constraint::Length(2)); // hints
    constraints.push(Constraint::Min(0)); // padding

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    // Helper to render a select field
    let render_select_field =
        |f: &mut Frame, area: Rect, label: &str, value: &str, active: bool| {
            let label_style = if active {
                Style::default()
                    .fg(SPRING_GREEN)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let value_style = if active {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let arrow_style = if active {
                Style::default().fg(SPRING_GREEN)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let line = Line::from(vec![
                Span::styled(format!("  {:<16}", label), label_style),
                Span::styled("◂ ", arrow_style),
                Span::styled(value, value_style),
                Span::styled(" ▸", arrow_style),
            ]);
            f.render_widget(Paragraph::new(line), area);
        };

    // Helper to render a text field
    let render_text_field = |f: &mut Frame, area: Rect, label: &str, value: &str, active: bool| {
        let label_style = if active {
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let value_style = if active {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let mut spans = vec![
            Span::styled(format!("  {:<16}", label), label_style),
            Span::styled(value, value_style),
        ];
        if active {
            spans.push(Span::styled("█", Style::default().fg(Color::White)));
        }
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    };

    // Boot Version (0)
    let bv_name = meta
        .boot_versions
        .get(state.boot_version_idx)
        .map(|v| v.name.as_str())
        .unwrap_or("?");
    render_select_field(
        f,
        chunks[0],
        "Boot Version",
        bv_name,
        state.active_field == 0,
    );

    // Language (1)
    let lang_name = meta
        .languages
        .get(state.language_idx)
        .map(|v| v.name.as_str())
        .unwrap_or("?");
    render_select_field(f, chunks[1], "Language", lang_name, state.active_field == 1);

    // Packaging (2)
    let pkg_name = meta
        .packagings
        .get(state.packaging_idx)
        .map(|v| v.name.as_str())
        .unwrap_or("?");
    render_select_field(f, chunks[2], "Packaging", pkg_name, state.active_field == 2);

    // Java Version (3)
    let jv_name = meta
        .java_versions
        .get(state.java_version_idx)
        .map(|v| v.name.as_str())
        .unwrap_or("?");
    render_select_field(
        f,
        chunks[3],
        "Java Version",
        jv_name,
        state.active_field == 3,
    );

    // Project Type (4)
    let pt_name = meta
        .project_types
        .get(state.project_type_idx)
        .map(|v| v.name.as_str())
        .unwrap_or("?");
    render_select_field(
        f,
        chunks[4],
        "Project Type",
        pt_name,
        state.active_field == 4,
    );

    // Group Id (5)
    render_text_field(
        f,
        chunks[5],
        "Group",
        &state.group_id,
        state.active_field == 5,
    );

    // Artifact Id (6)
    render_text_field(
        f,
        chunks[6],
        "Artifact",
        &state.artifact_id,
        state.active_field == 6,
    );

    // Name (7)
    render_text_field(f, chunks[7], "Name", &state.name, state.active_field == 7);

    // Description (8)
    render_text_field(
        f,
        chunks[8],
        "Description",
        &state.description,
        state.active_field == 8,
    );

    // Package Name (9)
    render_text_field(
        f,
        chunks[9],
        "Package",
        &state.package_name,
        state.active_field == 9,
    );

    // Hints
    let hints = Paragraph::new(vec![Line::from(vec![
        Span::styled("  Tab/↓↑", Style::default().fg(Color::DarkGray)),
        Span::styled(" navigate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("◂/▸", Style::default().fg(Color::DarkGray)),
        Span::styled(" cycle selects  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::DarkGray)),
        Span::styled(" next step  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
    ])]);
    f.render_widget(hints, chunks[11]);
}

// ---------------------------------------------------------------------------
// Step 2: Dependencies
// ---------------------------------------------------------------------------

fn render_dependencies(f: &mut Frame, state: &NewProjectWizardState) {
    let area = centered_rect(80, 30, f.area());
    f.render_widget(Clear, area);

    let selected_count = state.selected_deps.len();
    let title = format!(
        " New Project — Step 2/3: Dependencies ({} selected) ",
        selected_count
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SPRING_GREEN))
        .title(Span::styled(
            title,
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let meta = match &state.metadata {
        Some(m) => m,
        None => return,
    };

    // Layout: filter bar | dep list | selected summary | hints
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // filter bar
            Constraint::Min(3),    // dep list
            Constraint::Length(2), // selected summary
            Constraint::Length(1), // hints
        ])
        .split(inner);

    // -- Filter bar --
    if state.dep_filter_active {
        let filter_line = Line::from(vec![
            Span::styled(
                "  Filter: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&state.dep_filter, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::White)),
        ]);
        f.render_widget(Paragraph::new(filter_line), chunks[0]);
    } else if !state.dep_filter.is_empty() {
        let filter_line = Line::from(vec![
            Span::styled("  Filter: ", Style::default().fg(Color::Yellow)),
            Span::styled(&state.dep_filter, Style::default().fg(Color::White)),
        ]);
        f.render_widget(Paragraph::new(filter_line), chunks[0]);
    } else {
        let filter_hint = Paragraph::new(Line::from(Span::styled(
            "  Press / to filter dependencies",
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(filter_hint, chunks[0]);
    }

    // -- Flat dependency list with group headers --
    let flat = flatten_all_deps_for_render(meta, &state.dep_filter);

    let items: Vec<ListItem> = flat
        .iter()
        .map(|entry| match entry {
            FlatDepEntry::GroupHeader(name) => ListItem::new(Line::from(Span::styled(
                format!("  ── {} ──", name),
                Style::default()
                    .fg(SPRING_GREEN)
                    .add_modifier(Modifier::BOLD),
            ))),
            FlatDepEntry::Dep(dep) => {
                let is_selected = state.selected_deps.contains(&dep.id);
                let checkbox = if is_selected { "[x]" } else { "[ ]" };
                let check_style = if is_selected {
                    Style::default()
                        .fg(SPRING_GREEN)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let mut spans = vec![
                    Span::styled(format!("  {} ", checkbox), check_style),
                    Span::styled(&dep.name, Style::default().fg(Color::White)),
                ];
                if !dep.description.is_empty() {
                    spans.push(Span::styled(
                        format!(" — {}", truncate_str(&dep.description, 50)),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                ListItem::new(Line::from(spans))
            }
        })
        .collect();

    if items.is_empty() {
        let empty_msg = Paragraph::new(Line::from(Span::styled(
            "  No dependencies match the filter",
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(empty_msg, chunks[1]);
    } else {
        let list = List::new(items).highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

        let mut list_state = ListState::default();
        list_state.select(Some(state.dep_item_idx.min(flat.len().saturating_sub(1))));
        f.render_stateful_widget(list, chunks[1], &mut list_state);
    }

    // -- Selected summary --
    if state.selected_deps.is_empty() {
        let summary = Paragraph::new(Line::from(Span::styled(
            "  No dependencies selected",
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(summary, chunks[2]);
    } else {
        let dep_names: Vec<String> = state.selected_deps.iter().take(8).cloned().collect();
        let mut summary_text = dep_names.join(", ");
        if state.selected_deps.len() > 8 {
            summary_text.push_str(&format!(" +{} more", state.selected_deps.len() - 8));
        }
        let summary = Paragraph::new(vec![Line::from(vec![
            Span::styled(
                "  Selected: ",
                Style::default()
                    .fg(SPRING_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(summary_text, Style::default().fg(Color::White)),
        ])]);
        f.render_widget(summary, chunks[2]);
    }

    // -- Hints --
    let hints = Paragraph::new(Line::from(vec![
        Span::styled("  j/k", Style::default().fg(Color::DarkGray)),
        Span::styled(" nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Space", Style::default().fg(Color::DarkGray)),
        Span::styled(" toggle  ", Style::default().fg(Color::DarkGray)),
        Span::styled("/", Style::default().fg(Color::DarkGray)),
        Span::styled(" filter  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::DarkGray)),
        Span::styled(" next  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::styled(" back", Style::default().fg(Color::DarkGray)),
    ]));
    f.render_widget(hints, chunks[3]);
}

// ---------------------------------------------------------------------------
// Step 3: Confirm & Generate
// ---------------------------------------------------------------------------

fn render_confirm(f: &mut Frame, state: &NewProjectWizardState) {
    let area = centered_rect(70, 22, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SPRING_GREEN))
        .title(Span::styled(
            " New Project — Step 3/3: Review & Generate ",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let meta = match &state.metadata {
        Some(m) => m,
        None => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // summary
            Constraint::Length(3), // output dir
            Constraint::Length(1), // error
            Constraint::Length(1), // hints
        ])
        .split(inner);

    // -- Summary --
    let bv = meta
        .boot_versions
        .get(state.boot_version_idx)
        .map(|v| v.name.as_str())
        .unwrap_or("?");
    let lang = meta
        .languages
        .get(state.language_idx)
        .map(|v| v.name.as_str())
        .unwrap_or("?");
    let pkg = meta
        .packagings
        .get(state.packaging_idx)
        .map(|v| v.name.as_str())
        .unwrap_or("?");
    let jv = meta
        .java_versions
        .get(state.java_version_idx)
        .map(|v| v.name.as_str())
        .unwrap_or("?");
    let pt = meta
        .project_types
        .get(state.project_type_idx)
        .map(|v| v.name.as_str())
        .unwrap_or("?");

    let dep_list = if state.selected_deps.is_empty() {
        "(none)".to_string()
    } else {
        state.selected_deps.join(", ")
    };

    let summary_lines = vec![
        Line::from(vec![
            Span::styled("  Boot Version:  ", Style::default().fg(Color::Gray)),
            Span::styled(bv, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Language:      ", Style::default().fg(Color::Gray)),
            Span::styled(lang, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Packaging:     ", Style::default().fg(Color::Gray)),
            Span::styled(pkg, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Java Version:  ", Style::default().fg(Color::Gray)),
            Span::styled(jv, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Project Type:  ", Style::default().fg(Color::Gray)),
            Span::styled(pt, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Group:         ", Style::default().fg(Color::Gray)),
            Span::styled(&state.group_id, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Artifact:      ", Style::default().fg(Color::Gray)),
            Span::styled(&state.artifact_id, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Name:          ", Style::default().fg(Color::Gray)),
            Span::styled(&state.name, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Package:       ", Style::default().fg(Color::Gray)),
            Span::styled(&state.package_name, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Dependencies:  ", Style::default().fg(Color::Gray)),
            Span::styled(&dep_list, Style::default().fg(Color::Cyan)),
        ]),
    ];

    let summary = Paragraph::new(summary_lines).wrap(Wrap { trim: false });
    f.render_widget(summary, chunks[0]);

    // -- Output directory --
    let dir_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SPRING_GREEN))
        .title(Span::styled(
            " Output Directory ",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ));
    let dir_text = Paragraph::new(Line::from(vec![
        Span::styled(&state.output_dir, Style::default().fg(Color::White)),
        Span::styled("█", Style::default().fg(Color::White)),
    ]))
    .block(dir_block);
    f.render_widget(dir_text, chunks[1]);

    // -- Error --
    if let Some(ref err) = state.error {
        let err_p = Paragraph::new(Span::styled(err.as_str(), Style::default().fg(Color::Red)))
            .alignment(Alignment::Center);
        f.render_widget(err_p, chunks[2]);
    }

    // -- Hints --
    let hints = Paragraph::new(Line::from(vec![
        Span::styled("  Enter", Style::default().fg(Color::DarkGray)),
        Span::styled(" generate  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::styled(" back to deps", Style::default().fg(Color::DarkGray)),
    ]));
    f.render_widget(hints, chunks[3]);
}

// ---------------------------------------------------------------------------
// Step 4: Generating
// ---------------------------------------------------------------------------

fn render_generating(f: &mut Frame, state: &NewProjectWizardState) {
    let area = centered_rect(60, 10, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SPRING_GREEN))
        .title(Span::styled(
            " Generating Project ",
            Style::default()
                .fg(SPRING_GREEN)
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // status
            Constraint::Length(1), // spacer
            Constraint::Min(1),    // result
        ])
        .split(inner);

    if state.gen_done {
        // Success
        let success_icon = Paragraph::new(Line::from(vec![
            Span::styled(
                "  ✓ ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Project generated successfully!",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        f.render_widget(success_icon, chunks[0]);

        if let Some(ref path) = state.gen_result_path {
            let path_line = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("  Path: ", Style::default().fg(Color::Gray)),
                    Span::styled(path.as_str(), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  Press Enter or Esc to close",
                    Style::default().fg(Color::DarkGray),
                )),
            ]);
            f.render_widget(path_line, chunks[2]);
        }
    } else {
        // In progress
        let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            / 100) as usize;
        let spinner = spinner_chars[frame % spinner_chars.len()];

        let progress = Paragraph::new(Line::from(vec![
            Span::styled(format!("  {} ", spinner), Style::default().fg(SPRING_GREEN)),
            Span::styled(&state.gen_progress, Style::default().fg(Color::Gray)),
        ]));
        f.render_widget(progress, chunks[0]);
    }
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

enum FlatDepEntry<'a> {
    GroupHeader(String),
    Dep(&'a crate::model::InitializrDependency),
}

fn flatten_all_deps_for_render<'a>(
    meta: &'a crate::model::InitializrMetadata,
    filter: &str,
) -> Vec<FlatDepEntry<'a>> {
    let f = filter.to_lowercase();
    let mut result = Vec::new();

    for group in &meta.dependency_groups {
        let deps: Vec<&crate::model::InitializrDependency> = if filter.is_empty() {
            group.values.iter().collect()
        } else {
            group
                .values
                .iter()
                .filter(|d| {
                    d.name.to_lowercase().contains(&f)
                        || d.id.to_lowercase().contains(&f)
                        || d.description.to_lowercase().contains(&f)
                })
                .collect()
        };

        if !deps.is_empty() {
            result.push(FlatDepEntry::GroupHeader(group.name.clone()));
            for dep in deps {
                result.push(FlatDepEntry::Dep(dep));
            }
        }
    }

    result
}

/// Check if a flat dep entry at the given index is a selectable dependency (not a header).
pub fn flat_dep_is_selectable(
    meta: &crate::model::InitializrMetadata,
    filter: &str,
    idx: usize,
) -> bool {
    let flat = flatten_all_deps_for_render(meta, filter);
    matches!(flat.get(idx), Some(FlatDepEntry::Dep(_)))
}

/// Get the dependency ID at the given flat index, if it's a Dep entry.
pub fn flat_dep_id_at(
    meta: &crate::model::InitializrMetadata,
    filter: &str,
    idx: usize,
) -> Option<String> {
    let flat = flatten_all_deps_for_render(meta, filter);
    match flat.get(idx) {
        Some(FlatDepEntry::Dep(dep)) => Some(dep.id.clone()),
        _ => None,
    }
}

/// Total number of entries in the flat list.
pub fn flat_dep_count(meta: &crate::model::InitializrMetadata, filter: &str) -> usize {
    flatten_all_deps_for_render(meta, filter).len()
}
