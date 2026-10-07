//! Playback query picker overlay.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::{state::AppState, ui::theme};

/// Render the playback query overlay on top of the detail screen.
pub fn render_overlay(frame: &mut Frame, state: &AppState) {
    let height = (state.playback_queries.len() as u16)
        .saturating_add(9)
        .min(22);
    let area = theme::popup(frame.area(), 72, height);
    frame.render_widget(Clear, area);
    let panel = theme::panel("Choose a title");
    let inner = panel.inner(area);
    frame.render_widget(panel, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new("Pick the matching title to start playback.")
            .style(Style::default().fg(theme::MUTED).bg(theme::SURFACE)),
        chunks[0],
    );
    let items: Vec<ListItem> = state
        .playback_queries
        .iter()
        .map(|query| ListItem::new(format!(" {query}")))
        .collect();
    let mut list_state = ListState::default().with_selected(Some(state.playback_query_cursor));
    frame.render_stateful_widget(
        List::new(items)
            .style(Style::default().fg(theme::TEXT).bg(theme::SURFACE))
            .highlight_style(
                Style::default()
                    .fg(theme::ACCENT)
                    .bg(theme::PANEL)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("› "),
        chunks[1],
        &mut list_state,
    );
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Enter", Style::default().fg(theme::ACCENT)),
        Span::raw(" play    "),
        Span::styled("j/k", Style::default().fg(theme::ACCENT)),
        Span::raw(" choose    "),
        Span::styled("Esc", Style::default().fg(theme::ACCENT)),
        Span::raw(" back"),
    ]))
    .style(Style::default().fg(theme::MUTED).bg(theme::SURFACE));
    frame.render_widget(footer, chunks[2]);
}
