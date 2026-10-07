//! Dependency / onboarding overlay.

use crate::ui::theme;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

use crate::{config, state::AppState};

pub fn render_overlay(frame: &mut Frame, state: &AppState, cfg: &config::Config) {
    let area = theme::popup(frame.area(), 84, 25);
    frame.render_widget(Clear, area);

    let block = theme::panel("Playback setup");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Length(5),
            Constraint::Min(0),
        ])
        .split(inner);

    let ready = state.has_ani_cli && state.has_any_player();
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                if ready {
                    "You are ready to watch."
                } else {
                    "Install the missing tools to start watching."
                },
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "r refresh checks  ·  s settings  ·  Esc close",
                Style::default().fg(theme::MUTED),
            )),
        ]),
        rows[0],
    );

    let preferred = cfg.player.as_str();
    let lines = vec![
        status_line(
            "ani-cli",
            state.has_ani_cli,
            "Finds streams and launches your player",
        ),
        status_line("mpv", state.has_mpv, "Default playback engine"),
        status_line("iina", state.has_iina, "macOS-native player option"),
        status_line("vlc", state.has_vlc, "Cross-platform fallback player"),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " Preferred ",
                Style::default().fg(theme::BG).bg(theme::SUCCESS),
            ),
            Span::raw(" "),
            Span::styled(
                preferred,
                Style::default()
                    .fg(theme::TEXT)
                    .bg(theme::PANEL)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().style(Style::default().bg(theme::SURFACE))),
        rows[1],
    );

    let suggestions = Paragraph::new(vec![
        Line::from(Span::styled(
            "Installation",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            if cfg!(target_os = "macos") {
                "brew install curl grep aria2 ffmpeg git fzf yt-dlp"
            } else {
                "Install ani-cli and mpv with your package manager."
            },
            Style::default().fg(theme::MUTED),
        )),
        Line::from(Span::styled(
            if cfg!(target_os = "macos") {
                "brew install --cask iina"
            } else {
                "Alternatives: vlc (Linux / Windows), iina (macOS)."
            },
            Style::default().fg(theme::MUTED),
        )),
        Line::from(Span::styled(
            "ani-cli must also be installed and available in PATH.",
            Style::default().fg(theme::MUTED),
        )),
    ])
    .block(Block::default().style(Style::default().bg(theme::SURFACE)));
    frame.render_widget(suggestions, rows[2]);

    let footer = Paragraph::new(vec![Line::from(vec![
        Span::styled(
            " Ready ",
            Style::default().fg(theme::BG).bg(if ready {
                theme::SUCCESS
            } else {
                theme::WARNING
            }),
        ),
        Span::raw(" "),
        Span::styled(
            if ready {
                "You can close this and start watching."
            } else {
                "Browsing is ready. Install the missing tools for playback."
            },
            Style::default().fg(theme::MUTED),
        ),
    ])])
    .block(Block::default().style(Style::default().bg(theme::SURFACE)));
    frame.render_widget(footer, rows[3]);
}

fn status_line(name: &str, ok: bool, note: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            if ok { " OK " } else { " Missing " },
            Style::default()
                .fg(theme::BG)
                .bg(if ok { theme::SUCCESS } else { theme::WARNING })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{name:<8}"),
            Style::default()
                .fg(theme::TEXT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(note.to_string(), Style::default().fg(theme::MUTED)),
    ])
}
