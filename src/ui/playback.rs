//! Playback screen — ani-cli log stream + player controls.

use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph},
    Frame,
};

use crate::{state::AppState, ui::theme};

/// Render the playback screen.
pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(theme::BG)), area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        }));
    let title = state.now_playing.as_deref().unwrap_or("Starting playback…");
    frame.render_widget(
        Paragraph::new(title)
            .style(
                Style::default()
                    .fg(theme::TEXT)
                    .bg(theme::SURFACE)
                    .add_modifier(Modifier::BOLD),
            )
            .block(theme::panel("Now playing")),
        chunks[0],
    );
    render_logs(frame, chunks[1], state);
    let controls = Paragraph::new(Line::from(vec![
        Span::styled("q", Style::default().fg(theme::ACCENT)),
        Span::raw(" stop    "),
        Span::styled("n", Style::default().fg(theme::ACCENT)),
        Span::raw(" next episode    "),
        Span::styled("Esc", Style::default().fg(theme::ACCENT)),
        Span::raw(" return"),
    ]))
    .style(Style::default().fg(theme::MUTED).bg(theme::BG));
    frame.render_widget(controls, chunks[2]);
}

fn render_logs(frame: &mut Frame, area: Rect, state: &AppState) {
    let panel = theme::panel("Playback activity");
    let visible = panel.inner(area).height as usize;
    let logs = &state.playback_logs;
    let start = logs.len().saturating_sub(visible);
    let items: Vec<ListItem> = logs[start..]
        .iter()
        .map(|line| {
            let style = if line.to_ascii_lowercase().contains("error") {
                Style::default().fg(theme::WARNING)
            } else if line.contains('›') || line.starts_with('[') {
                Style::default().fg(theme::ACCENT)
            } else {
                Style::default().fg(theme::MUTED)
            };
            ListItem::new(Line::from(Span::styled(line.as_str(), style)))
        })
        .collect();
    frame.render_widget(
        List::new(items)
            .style(Style::default().fg(theme::TEXT).bg(theme::SURFACE))
            .block(panel),
        area,
    );
}
