//! Detail screen — full anime info + scrollable episode list.

use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
    Frame,
};

use crate::{
    db::cache::Anime,
    state::{AppState, DetailFocus},
    ui::{components::posters::PosterCache, theme},
};

/// Render the detail screen.
pub fn render(frame: &mut Frame, state: &mut AppState, posters: &mut PosterCache) {
    let Some(anime) = state.selected_anime.clone() else {
        return;
    };

    posters.set_overlay(state.screen != crate::state::Screen::Detail);
    posters.poll();
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(theme::BG)), area);
    let area = area.inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("Esc", Style::default().fg(theme::ACCENT)),
        Span::raw(" back    "),
        Span::styled("Enter", Style::default().fg(theme::ACCENT)),
        Span::raw(" play    "),
        Span::styled("+", Style::default().fg(theme::ACCENT)),
        Span::raw(" watchlist    "),
        Span::styled("Arrows", Style::default().fg(theme::ACCENT)),
        Span::raw(" episode    "),
        Span::styled("Tab", Style::default().fg(theme::ACCENT)),
        Span::raw(" related    "),
        Span::styled("?", Style::default().fg(theme::ACCENT)),
        Span::raw(" all keys"),
    ]))
    .style(Style::default().fg(theme::MUTED).bg(theme::BG));

    // Keep episode navigation available on short terminals before expanding metadata.
    let related_height = if state.detail_recommendations.is_empty() || area.height < 18 {
        0
    } else {
        6
    };
    let info_height = area.height.saturating_sub(related_height + 7).min(14);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(info_height),
            Constraint::Length(related_height),
            Constraint::Min(5),
        ])
        .split(area);

    frame.render_widget(hint, chunks[0]);
    render_info(frame, chunks[1], state, &anime, posters);
    if related_height > 0 {
        render_related(frame, chunks[2], state);
    }
    render_episodes(frame, chunks[3], state);
}

/// Top section: cover + metadata side by side.
fn render_info(frame: &mut Frame, area: Rect, state: &mut AppState, anime: &Anime, posters: &mut PosterCache) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20), // cover art
            Constraint::Min(0),     // metadata
        ])
        .split(area);

    let cover_frame = if cols[0].width > 4 && cols[0].height > 4 {
        cols[0].inner(Margin {
            horizontal: 1,
            vertical: 1,
        })
    } else {
        cols[0]
    };
    let cover_bg = Block::default().style(Style::default().bg(theme::SURFACE));
    frame.render_widget(cover_bg, cover_frame);
    let cover_inner = if cover_frame.width > 2 && cover_frame.height > 2 {
        cover_frame.inner(Margin {
            horizontal: 1,
            vertical: 1,
        })
    } else {
        cover_frame
    };

    posters.render(frame, cover_inner, anime);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme::BORDER)),
        cover_frame,
    );

    // Metadata
    render_metadata(frame, cols[1], state, anime);
}

fn render_metadata(frame: &mut Frame, area: Rect, state: &AppState, anime: &Anime) {
    let title = anime.display_title();
    let score = anime
        .score
        .map(|s| format!("★ {:.1}", s as f32 / 10.0))
        .unwrap_or_else(|| "★ N/A".to_string());
    let eps = anime
        .episodes
        .map(|e| format!("{} eps", e))
        .unwrap_or_else(|| "? eps".to_string());
    let fmt = anime.format.as_deref().unwrap_or("TV");
    let status = anime.status.as_deref().unwrap_or("Unknown");
    let year = anime
        .season_year
        .map(|y| y.to_string())
        .unwrap_or_else(|| "?".to_string());
    let season = anime.season.as_deref().unwrap_or("");
    let genres = anime.genre_list().join(" · ");
    let desc = theme::plain_text(
        anime
            .description
            .as_deref()
            .unwrap_or("No description available."),
    )
    .chars()
    .take(280)
    .collect::<String>();
    let dub_tag = if anime.has_dub() {
        "  Sub + Dub"
    } else {
        "  Sub only"
    };
    let play_label = match state.selected_episode {
        Some(1) => " Start E1 ".to_string(),
        Some(ep) => format!(" Continue E{ep} "),
        None if state.episodes_loading => " Checking episodes… ".to_string(),
        None => " No confirmed aired episodes ".to_string(),
    };
    let watchlist_label = if state.in_watchlist {
        " + Remove "
    } else {
        " + Watchlist "
    };

    let playback_status =
        if state.now_playing.is_some() && state.last_played_anime_id == Some(anime.id) {
            state.now_playing.as_deref()
        } else if state.last_played_anime_id == Some(anime.id) {
            state.last_played.as_deref()
        } else {
            None
        };

    let mut lines = Vec::new();
    if let Some(origin) = state.detail_origin_title.as_deref() {
        lines.push(Line::from(vec![
            Span::styled(" From ", Style::default().fg(theme::BG).bg(theme::ACCENT)),
            Span::raw(" "),
            Span::styled(origin, Style::default().fg(theme::MUTED)),
        ]));
        lines.push(Line::from(""));
    }

    lines.extend([
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(theme::TEXT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!(
                "{}  ·  {}  ·  {}  ·  {} {}  ·  {}{}",
                score, eps, fmt, season, year, status, dub_tag
            ),
            Style::default().fg(theme::MUTED),
        )),
        Line::from(Span::styled(genres, Style::default().fg(theme::ACCENT))),
        Line::from(""),
        Line::from(Span::styled(desc, Style::default().fg(theme::TEXT))),
    ]);
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            play_label,
            Style::default()
                .fg(if state.selected_episode.is_some() { theme::BG } else { theme::MUTED })
                .bg(if state.selected_episode.is_some() { theme::ACCENT } else { theme::SURFACE })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            watchlist_label,
            Style::default().fg(theme::TEXT).bg(theme::SURFACE),
        ),
    ]));

    if let Some(status_line) = playback_status {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(
                " Playback ",
                Style::default()
                    .fg(theme::BG)
                    .bg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(status_line, Style::default().fg(theme::TEXT)),
        ]));
    }

    let para = Paragraph::new(lines)
        .style(Style::default().fg(theme::TEXT).bg(theme::SURFACE))
        .block(Block::default().padding(Padding::new(2, 2, 1, 1)))
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(para, area);
}

fn render_related(frame: &mut Frame, area: Rect, state: &AppState) {
    let block = theme::panel("More Like This").padding(Padding::new(2, 2, 0, 0));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 4 || inner.width < 12 {
        return;
    }

    let card_width: u16 = 24;
    let gap: u16 = 1;
    let visible = (inner.width / (card_width + gap)).max(1) as usize;

    for (idx, anime) in state
        .detail_recommendations
        .iter()
        .skip(state.detail_related_offset)
        .take(visible)
        .enumerate()
    {
        let absolute_idx = state.detail_related_offset + idx;
        let x = inner.x + idx as u16 * (card_width + gap);
        if x + card_width > inner.x + inner.width {
            break;
        }

        let rect = Rect {
            x,
            y: inner.y,
            width: card_width,
            height: inner.height.min(4),
        };

        let reason = state
            .detail_recommendation_reasons
            .get(&anime.id)
            .map(String::as_str)
            .unwrap_or("Picked for you");

        let lines = vec![
            Line::from(Span::styled(
                anime.short_title(),
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(reason, Style::default().fg(theme::ACCENT))),
        ];

        let is_selected = state.detail_focus == DetailFocus::Related
            && absolute_idx == state.detail_related_cursor;
        let item = Paragraph::new(lines)
            .style(Style::default().fg(theme::TEXT).bg(theme::SURFACE))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(if is_selected {
                        Style::default().fg(theme::ACCENT)
                    } else {
                        Style::default().fg(theme::BORDER)
                    })
                    .style(Style::default().bg(theme::SURFACE)),
            )
            .wrap(ratatui::widgets::Wrap { trim: true });
        frame.render_widget(item, rect);
    }
}

/// Episode list section — horizontal scrolling pills.
fn render_episodes(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let episode_title = match (state.episode_count, state.selected_anime.as_ref().and_then(|anime| anime.episodes)) {
        (Some(aired), Some(total)) if i64::from(aired) < total => format!("Episodes · {aired} aired / {total} planned"),
        _ => "Episodes".to_owned(),
    };
    let block = theme::panel(&episode_title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.episode_list.is_empty() {
        let msg = Paragraph::new(state.episode_message())
            .style(Style::default().fg(theme::MUTED).bg(theme::SURFACE));
        frame.render_widget(msg, inner);
        return;
    }

    // Calculate how many pills fit per row
    let pill_width: u16 = 6; // " E99 "
    let pills_per_row = (inner.width / pill_width).max(1) as usize;
    state.update_episode_viewport(pills_per_row, inner.height.saturating_sub(1) as usize);
    let selected_ep = state.selected_episode.unwrap_or(1);

    // Render rows of episode pills
    let rows_needed = state.episode_list.len().div_ceil(pills_per_row);
    let visible_rows = inner.height.saturating_sub(1) as usize;
    let offset_rows = state.episode_offset / pills_per_row;

    for (y, row_idx) in (inner.y..).zip(offset_rows..(offset_rows + visible_rows).min(rows_needed))
    {
        if y >= inner.y + inner.height {
            break;
        }
        let start = row_idx * pills_per_row;
        let end = (start + pills_per_row).min(state.episode_list.len());
        let mut x = inner.x;

        for &ep in &state.episode_list[start..end] {
            if x + pill_width > inner.x + inner.width {
                break;
            }
            let is_selected = ep == selected_ep;
            let is_watched = state.watched_episodes.contains(&ep);
            let label = format!(" E{:<3}", ep);
            let style = if is_selected {
                // Warm accent for the active cursor position
                Style::default()
                    .fg(theme::BG)
                    .bg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD)
            } else if is_watched {
                // Dimmed to show the episode is already watched
                Style::default().fg(theme::SUCCESS).bg(theme::SURFACE)
            } else {
                Style::default().fg(theme::TEXT).bg(theme::SURFACE)
            };

            let pill = Paragraph::new(Span::styled(label, style)).style(style);
            frame.render_widget(
                pill,
                Rect {
                    x,
                    y,
                    width: pill_width,
                    height: 1,
                },
            );
            x += pill_width;
        }
    }

    // Scrollbar if episodes exceed visible area
    if rows_needed > visible_rows {
        let mut scrollbar_state = ScrollbarState::new(rows_needed).position(offset_rows);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            inner,
            &mut scrollbar_state,
        );
    }
}
