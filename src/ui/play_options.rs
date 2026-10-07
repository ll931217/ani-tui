//! Playback options overlay.

use crate::ui::theme;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::state::AppState;

const QUALITY_LABELS: [&str; 5] = ["best", "1080p", "720p", "480p", "360p"];

/// Render the playback options overlay on top of the detail screen.
pub fn render_overlay(frame: &mut Frame, state: &AppState) {
    let area = theme::popup(frame.area(), 62, 18);
    frame.render_widget(Clear, area);
    let panel = theme::panel("Playback options");
    let inner = panel.inner(area);
    frame.render_widget(panel, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(inner);

    let can_dub = state
        .selected_anime
        .as_ref()
        .map(|anime| anime.has_dub())
        .unwrap_or(false);
    let audio_label = if !can_dub {
        "Sub only"
    } else if state.pending_dub {
        "Dub"
    } else {
        "Sub"
    };
    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            "Choose quality before launch",
            Style::default()
                .fg(theme::TEXT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("Audio", Style::default().fg(theme::MUTED)),
            Span::raw(" "),
            Span::styled(
                audio_label,
                Style::default()
                    .fg(theme::TEXT)
                    .bg(theme::PANEL)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ]);
    frame.render_widget(header, chunks[0]);

    let items: Vec<ListItem> = QUALITY_LABELS
        .iter()
        .enumerate()
        .map(|(i, quality)| {
            let style = if i == state.playback_quality_cursor {
                Style::default()
                    .fg(theme::TEXT)
                    .bg(theme::PANEL)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::MUTED)
            };
            let prefix = if i == state.playback_quality_cursor {
                "▶ "
            } else {
                "  "
            };
            ListItem::new(Line::from(Span::styled(
                format!("{prefix}{quality}"),
                style,
            )))
        })
        .collect();

    let mut list_state = ListState::default().with_selected(Some(state.playback_quality_cursor));
    frame.render_stateful_widget(List::new(items), chunks[1], &mut list_state);

    let footer = Paragraph::new(vec![
        Line::from(Span::styled(
            "Enter play  ·  j/k quality  ·  Esc cancel",
            Style::default().fg(theme::MUTED),
        )),
        Line::from(Span::styled(
            if can_dub {
                "h/l switch between sub and dub"
            } else {
                "This title is available with subtitles only"
            },
            Style::default().fg(theme::MUTED),
        )),
    ]);
    frame.render_widget(footer, chunks[2]);
}
