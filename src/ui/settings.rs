//! Settings overlay — persistent global preferences.

use crate::ui::theme;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

use crate::{config, state::AppState};

const PLAYER_CHOICES: [&str; 3] = ["mpv", "iina", "vlc"];
const QUALITY_CHOICES: [&str; 5] = ["best", "1080p", "720p", "480p", "360p"];
const AUDIO_CHOICES: [&str; 2] = ["sub", "dub"];

pub fn render_overlay(frame: &mut Frame, state: &AppState, cfg: &config::Config) {
    let area = theme::popup(frame.area(), 76, 21);
    frame.render_widget(Clear, area);

    let block = theme::panel("Settings");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "Your playback defaults",
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "j/k select  ·  h/l or Enter change  ·  Esc close",
                Style::default().fg(theme::MUTED),
            )),
        ]),
        rows[0],
    );

    render_setting_row(
        rows[1],
        "Player",
        &PLAYER_CHOICES,
        player_index(&cfg.player),
        state.settings_cursor == 0,
        frame,
    );
    render_setting_row(
        rows[2],
        "Quality",
        &QUALITY_CHOICES,
        quality_index(&cfg.quality),
        state.settings_cursor == 1,
        frame,
    );
    render_setting_row(
        rows[3],
        "Audio",
        &AUDIO_CHOICES,
        audio_index(&cfg.audio_mode),
        state.settings_cursor == 2,
        frame,
    );

    let preferred_ready = match cfg.player {
        config::Player::Mpv => state.has_mpv || state.has_iina,
        config::Player::Iina => state.has_iina,
        config::Player::Vlc => state.has_vlc,
    };
    let footer = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                " Status ",
                Style::default()
                    .fg(theme::BG)
                    .bg(if preferred_ready {
                        theme::SUCCESS
                    } else {
                        theme::WARNING
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                if preferred_ready {
                    "Ready for playback"
                } else {
                    "Current player missing, fallback may be used"
                },
                Style::default().fg(theme::MUTED),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Changes are saved automatically.",
            Style::default().fg(theme::MUTED),
        )),
    ])
    .block(Block::default().style(Style::default().bg(theme::SURFACE)));
    frame.render_widget(footer, rows[4]);
}

fn render_setting_row(
    area: Rect,
    label: &str,
    values: &[&str],
    selected: usize,
    focused: bool,
    frame: &mut Frame,
) {
    let row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(12), Constraint::Min(0)])
        .split(area);

    let label_style = if focused {
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::MUTED)
    };
    frame.render_widget(
        Paragraph::new(Span::styled(format!("{label}:"), label_style)),
        row[0],
    );

    let mut spans = Vec::new();
    for (idx, value) in values.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw("  "));
        }
        let active = idx == selected;
        spans.push(Span::styled(
            format!(" {} ", value),
            Style::default()
                .fg(if active && focused {
                    theme::BG
                } else {
                    theme::TEXT
                })
                .bg(if active {
                    if focused {
                        theme::ACCENT
                    } else {
                        theme::PANEL
                    }
                } else {
                    theme::SURFACE
                })
                .add_modifier(if active {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .block(Block::default().style(Style::default().bg(theme::SURFACE))),
        row[1],
    );
}

fn player_index(player: &config::Player) -> usize {
    match player {
        config::Player::Mpv => 0,
        config::Player::Iina => 1,
        config::Player::Vlc => 2,
    }
}

fn quality_index(quality: &config::Quality) -> usize {
    match quality {
        config::Quality::Best => 0,
        config::Quality::P1080 => 1,
        config::Quality::P720 => 2,
        config::Quality::P480 => 3,
        config::Quality::P360 => 4,
    }
}

fn audio_index(audio: &config::AudioMode) -> usize {
    match audio {
        config::AudioMode::Sub => 0,
        config::AudioMode::Dub => 1,
    }
}
