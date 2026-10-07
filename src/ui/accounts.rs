//! Account connections and background progress synchronization.
use ratatui::{
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
    Frame,
};
use crate::{state::AppState, ui::theme};

pub const WIDTH: u16 = 88;
pub const HEIGHT: u16 = 24;

pub fn render_overlay(frame: &mut Frame, state: &mut AppState) {
    let area = theme::popup(frame.area(), WIDTH, HEIGHT);
    frame.render_widget(Clear, area);
    let panel = theme::panel("Accounts & progress");
    let inner = panel.inner(area);
    frame.render_widget(panel, area);
    let mut lines = vec![
        Line::from(Span::styled("Two-way progress · TUI, AniList & MyAnimeList", Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled("Local playback keeps working when a provider is offline.", Style::default().fg(theme::MUTED))),
        Line::from(""),
    ];
    for account in &state.tracking_accounts {
        let name = if account.provider == "anilist" { "AniList" } else { "MyAnimeList" };
        let (status, color) = if let Some(error) = &account.error {
            (error.clone(), theme::WARNING)
        } else if account.connected {
            (format!("Connected as {}", account.name), theme::SUCCESS)
        } else {
            ("Not connected".to_owned(), theme::MUTED)
        };
        lines.push(Line::from(vec![Span::styled(format!("{name:<14}"), Style::default().fg(theme::TEXT)), Span::styled(status, Style::default().fg(color))]));
    }
    lines.extend([
        Line::from(""),
        Line::from(Span::styled("Connect from a separate terminal", Style::default().fg(theme::TEXT))),
        Line::from(Span::styled("ani-tui accounts connect anilist CLIENT_ID", Style::default().fg(theme::ACCENT))),
        Line::from(Span::styled("ani-tui accounts connect mal CLIENT_ID", Style::default().fg(theme::ACCENT))),
        Line::from(Span::styled("Omit CLIENT_ID for registration instructions.", Style::default().fg(theme::MUTED))),
        Line::from(""),
        Line::from(Span::styled(if state.tracking_jobs > 0 { "Syncing progress in the background…" } else { "Progress sync" }, Style::default().fg(theme::TEXT))),
    ]);
    if state.tracking_messages.is_empty() {
        lines.push(Line::from(Span::styled("Import on startup and every 5 minutes; launches sync outward.", Style::default().fg(theme::MUTED))));
    } else {
        for message in state.tracking_messages.iter().rev().take(3).rev() {
            lines.push(Line::from(Span::styled(message, Style::default().fg(theme::MUTED))));
        }
    }
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).split(inner);
    let body = Paragraph::new(lines).style(Style::default().fg(theme::TEXT).bg(theme::SURFACE)).wrap(Wrap { trim: true });
    let max_scroll = body.line_count(chunks[0].width).saturating_sub(usize::from(chunks[0].height)).min(usize::from(u16::MAX)) as u16;
    state.accounts_scroll = state.accounts_scroll.min(max_scroll);
    frame.render_widget(body.scroll((state.accounts_scroll, 0)), chunks[0]);
    frame.render_widget(Paragraph::new(vec![Line::from("j/k Scroll · r Sync both ways"), Line::from("Esc Close")]).style(Style::default().fg(theme::ACCENT).bg(theme::SURFACE)), chunks[1]);
}
