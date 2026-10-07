//! Search overlay — floats centered over the current screen.

use crate::ui::theme;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::state::AppState;

/// Render the search overlay on top of whatever screen is below.
pub fn render_overlay(frame: &mut Frame, state: &AppState) {
    let area = theme::popup(frame.area(), 90, 26);

    frame.render_widget(Clear, area);
    let panel = theme::panel("Search anime");
    let inner = panel.inner(area);
    frame.render_widget(panel, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // query and shortcuts
            Constraint::Length(7), // focused result preview
            Constraint::Min(0),    // results list
        ])
        .split(inner);

    render_input(frame, chunks[0], state);
    render_preview(frame, chunks[1], state);
    render_results(frame, chunks[2], state);
}

/// The search input box.
fn render_input(frame: &mut Frame, area: Rect, state: &AppState) {
    let input = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme::BG).bg(theme::ACCENT)),
            Span::styled(
                format!("  {}▏", state.search_query),
                Style::default().fg(theme::TEXT),
            ),
        ]),
        Line::from(Span::styled(
            "Type to search  ·  ↑/↓ browse  ·  Enter open  ·  Esc close",
            Style::default().fg(theme::MUTED),
        )),
    ]);
    frame.render_widget(input, area);
}

/// The results list below the input.
fn render_results(frame: &mut Frame, area: Rect, state: &AppState) {
    if state.search_results.is_empty() {
        let msg = if state.search_query.is_empty() {
            "Type to search anime..."
        } else {
            "No results found."
        };
        let para = Paragraph::new(Span::styled(msg, Style::default().fg(theme::MUTED)));
        frame.render_widget(para, area);
        return;
    }

    let items: Vec<ListItem> = state
        .search_results
        .iter()
        .enumerate()
        .map(|(i, anime)| {
            let score = anime
                .score
                .map(|s| format!("{:.1}", s as f32 / 10.0))
                .unwrap_or_else(|| "N/A".to_string());
            let eps = anime
                .episodes
                .map(|e| format!("{} eps", e))
                .unwrap_or_else(|| "? eps".to_string());

            let style = if i == state.search_cursor {
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::TEXT)
            };

            let prefix = if i == state.search_cursor {
                "▶ "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{}{}", prefix, anime.display_title()), style),
                Span::styled(
                    format!("  ★{}  {}", score, eps),
                    Style::default().fg(theme::MUTED),
                ),
            ]))
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(state.search_cursor));
    frame.render_stateful_widget(List::new(items), area, &mut list_state);
}

fn render_preview(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(anime) = state.search_results.get(state.search_cursor) else {
        let empty = Paragraph::new(Span::styled(
            "Use ↑/↓ to focus a result.",
            Style::default().fg(theme::MUTED),
        ));
        frame.render_widget(empty, area);
        return;
    };

    let score = anime
        .score
        .map(|s| format!("★ {:.1}", s as f32 / 10.0))
        .unwrap_or_else(|| "★ N/A".to_string());
    let eps = anime
        .episodes
        .map(|e| format!("{} eps", e))
        .unwrap_or_else(|| "? eps".to_string());
    let format = anime.format.as_deref().unwrap_or("TV");
    let status = anime.status.as_deref().unwrap_or("Unknown");
    let desc = anime
        .description
        .as_deref()
        .unwrap_or("No description available.")
        .chars()
        .take(120)
        .collect::<String>();

    let preview = Paragraph::new(vec![
        Line::from(Span::styled(
            anime.display_title(),
            Style::default()
                .fg(theme::TEXT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("{}  ·  {}  ·  {}", score, eps, format),
            Style::default().fg(theme::ACCENT),
        )),
        Line::from(Span::styled(status, Style::default().fg(theme::MUTED))),
        Line::from(""),
        Line::from(Span::styled(desc, Style::default().fg(theme::MUTED))),
    ])
    .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(preview, area);
}
