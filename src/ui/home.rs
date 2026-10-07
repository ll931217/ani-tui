//! Home screen — Netflix-style category rows.
//! Renders: Featured banner + Continue Watching + Trending + Popular + Top Rated + Seasonal.

use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};
use std::collections::HashMap;

use crate::{
    db::cache::Anime,
    state::{AppState, CategoryRow},
    ui::{components::posters::PosterCache, theme},
};

/// Width of each anime card in the row (chars)
const CARD_WIDTH: u16  = 22;
/// Gap between cards
const CARD_GAP: u16    = 2;

/// Render the full home screen.
pub fn render(frame: &mut Frame, state: &mut AppState, categories: &HomeData, posters: &mut PosterCache) {
    frame.render_widget(Block::default().style(Style::default().bg(theme::BG)), frame.area());
    let area = frame.area().inner(Margin { horizontal: 2, vertical: 1 });
    posters.set_overlay(state.screen != crate::state::Screen::Home);
    posters.poll();
    state.visible_cards = ((area.width + CARD_GAP) / (CARD_WIDTH + CARD_GAP)).max(1) as usize;
    categories.normalize_selection(state);
    let chunks = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(2), Constraint::Length(9), Constraint::Length(1),
        Constraint::Min(0), Constraint::Length(2),
    ]).split(area);
    let header = Line::from(vec![
        Span::styled("ani", Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("  /  Discover", Style::default().fg(theme::TEXT)),
    ]);
    frame.render_widget(Paragraph::new(header).style(Style::default().bg(theme::BG)), chunks[0]);
    let anime = active_banner_anime(state, categories).or(categories.featured.as_ref());
    render_featured(frame, chunks[1], state, anime, categories, posters);
    render_rows(frame, chunks[3], state, categories, posters);
    frame.render_widget(Paragraph::new(Line::from(vec![
        Span::styled("h l", Style::default().fg(theme::TEXT)), Span::styled(" select    ", Style::default().fg(theme::MUTED)),
        Span::styled("j k", Style::default().fg(theme::TEXT)), Span::styled(" collections    ", Style::default().fg(theme::MUTED)),
        Span::styled("Enter", Style::default().fg(theme::TEXT)), Span::styled(" details    ", Style::default().fg(theme::MUTED)),
        Span::styled("/", Style::default().fg(theme::ACCENT)), Span::styled(" search    s settings    ? help    q quit", Style::default().fg(theme::MUTED)),
    ])).style(Style::default().bg(theme::BG)), chunks[4]);
}

fn render_featured(frame: &mut Frame, area: Rect, state: &mut AppState, anime: Option<&Anime>, data: &HomeData, posters: &mut PosterCache) {
    let Some(anime) = anime else {
        let empty = theme::popup(area, 70, 7);
        frame.render_widget(Paragraph::new("Search for an anime to start watching.").block(theme::panel("Your next watch")), empty);
        return;
    };
    frame.render_widget(Block::default().style(Style::default().bg(theme::SURFACE)), area);
    let inner = area.inner(Margin { horizontal: 2, vertical: 1 });
    let columns = Layout::default().direction(Direction::Horizontal).constraints([
        Constraint::Length(10), Constraint::Length(3), Constraint::Min(0),
    ]).split(inner);
    posters.render(frame, columns[0], anime);
    let score = anime.score.map(|s| format!("{:.1}", s as f32 / 10.0)).unwrap_or_else(|| "--".into());
    let metadata = format!("   {}  ·  {}  ·  {}", anime.format.as_deref().unwrap_or("TV"),
        anime.season_year.map(|year| year.to_string()).unwrap_or_else(|| "TBA".into()),
        anime.episodes.map(|episodes| format!("{episodes} episodes")).unwrap_or_else(|| "Airing".into()));
    let next = data.resume_next.get(&anime.id).map(|ep| format!("Resume E{ep}")).unwrap_or_else(|| "Start watching".into());
    let watchlist = if data.watchlist.iter().any(|a| a.id == anime.id) { "+ Remove from list" } else { "+ Watchlist" };
    let watched = state.banner_progress.filter(|(id, _)| *id == anime.id).map(|(_, n)| n).unwrap_or(0);
    let status = if state.last_played_anime_id == Some(anime.id) { "  Playing in your player".into() }
        else if watched > 0 { format!("  {watched} watched") } else { String::new() };
    let lines = [
        Line::from(Span::styled(anime.display_title(), Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD))),
        Line::from(vec![Span::styled(format!("★ {score}"), Style::default().fg(theme::WARNING)), Span::styled(metadata, Style::default().fg(theme::MUTED))]),
        Line::from(Span::styled(anime.genre_list().join("  /  "), Style::default().fg(theme::MUTED))),
        Line::from(""),
        Line::from(Span::styled(truncate_reason(&theme::plain_text(anime.description.as_deref().unwrap_or("No synopsis available.")), columns[2].width as usize * 2), Style::default().fg(theme::TEXT))),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!(" r  {next} "), Style::default().fg(theme::BG).bg(theme::ACCENT).add_modifier(Modifier::BOLD)),
            Span::styled(format!("    {watchlist}    d Details"), Style::default().fg(theme::TEXT)),
            Span::styled(status, Style::default().fg(theme::SUCCESS)),
        ]),
    ];
    let text_areas = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(3), Constraint::Length(1), Constraint::Min(0), Constraint::Length(1),
    ]).split(columns[2]);
    frame.render_widget(Paragraph::new(lines[..3].to_vec()).style(Style::default().bg(theme::SURFACE)), text_areas[0]);
    frame.render_widget(Paragraph::new(lines[4].clone()).style(Style::default().bg(theme::SURFACE)).wrap(Wrap { trim: true }), text_areas[2]);
    frame.render_widget(Paragraph::new(lines[6].clone()).style(Style::default().bg(theme::SURFACE)), text_areas[3]);
}

fn active_banner_anime<'a>(state: &AppState, data: &'a HomeData) -> Option<&'a Anime> {
    let (row_key, items) = match state.active_row {
        CategoryRow::ContinueWatching => ("continue_watching", &data.continue_watching),
        CategoryRow::Watchlist        => ("watchlist", &data.watchlist),
        CategoryRow::Recommended      => ("recommended", &data.recommended),
        CategoryRow::Trending         => ("trending", &data.trending),
        CategoryRow::Popular          => ("popular", &data.popular),
        CategoryRow::TopRated         => ("top_rated", &data.top_rated),
        CategoryRow::Seasonal         => ("seasonal", &data.seasonal),
    };

    items.get(state.row_cursor(row_key))
}

/// Render all category rows.
fn render_rows(frame: &mut Frame, area: Rect, state: &mut AppState, data: &HomeData, posters: &mut PosterCache) {
    let rows: Vec<(String, &str, &[Anime])> = vec![
        ("Continue watching".to_string(), "continue_watching", &data.continue_watching),
        ("Your watchlist".to_string(),      "watchlist",         &data.watchlist),
        (recommended_label(data), "recommended",    &data.recommended),
        ("Trending now".to_string(),         "trending",          &data.trending),
        ("Popular picks".to_string(),          "popular",           &data.popular),
        ("Top rated".to_string(),        "top_rated",         &data.top_rated),
        ("This season".to_string(),         "seasonal",          &data.seasonal),
    ];

    // Filter out empty rows
    let visible: Vec<_> = rows.into_iter().filter(|(_, _, items)| !items.is_empty()).collect();
    let row_count        = visible.len() as u16;

    if row_count == 0 {
        return;
    }

    // Poster height follows the terminal cell aspect ratio.
    let row_height  = posters.card_height(CARD_WIDTH) + 3;
    let max_rows_that_fit = (area.height / row_height).max(1);
    let visible_rows      = row_count.min(max_rows_that_fit);
    let constraints: Vec<Constraint> = (0..visible_rows)
        .map(|_| Constraint::Length(row_height))
        .collect();

    let row_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Determine which row key is currently active for highlight
    let active_key = match state.active_row {
        CategoryRow::ContinueWatching => "continue_watching",
        CategoryRow::Watchlist        => "watchlist",
        CategoryRow::Recommended      => "recommended",
        CategoryRow::Trending         => "trending",
        CategoryRow::Popular          => "popular",
        CategoryRow::TopRated         => "top_rated",
        CategoryRow::Seasonal         => "seasonal",
    };

    let visible_rows = visible_rows as usize;
    let active_idx   = visible
        .iter()
        .position(|(_, key, _)| *key == active_key)
        .unwrap_or(0);
    let start_idx    = active_idx.saturating_sub(visible_rows.saturating_sub(1));

    for (i, (label, key, items)) in visible.iter().skip(start_idx).take(visible_rows).enumerate() {
        if i < row_areas.len() {
            render_row(
                frame,
                row_areas[i],
                state,
                (label, key, items),
                *key == active_key,
                data,
                posters,
            );
        }
    }
}

/// Render a single horizontal category row with a label and cards.
fn render_row(
    frame:     &mut Frame,
    area:      Rect,
    state:     &mut AppState,
    row: (&str, &str, &[Anime]),
    is_active: bool,
    data: &HomeData,
    posters: &mut PosterCache,
) {
    let (label, key, items) = row;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(area);

    // A quiet section label and position counter orient navigation.
    let label_style = if is_active {
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(theme::MUTED)
            .add_modifier(Modifier::BOLD)
    };
    let label_widget = Paragraph::new(Line::from(Span::styled(
        format!("{}   {}/{}", label, state.row_cursor(key) + 1, items.len()),
        label_style,
    ))).style(Style::default().bg(theme::BG));
    frame.render_widget(label_widget, chunks[0]);

    // Selection moves inside the viewport; scrolling starts at its edges.
    let card_area   = chunks[1];
    let offset      = state.row_offset(key);
    let visible_n   = ((card_area.width + CARD_GAP) / (CARD_WIDTH + CARD_GAP)).max(1) as usize;
    let visible_items: Vec<&Anime> = items.iter().skip(offset).take(visible_n).collect();

    for (i, anime) in visible_items.iter().enumerate() {
        let x    = card_area.x + i as u16 * (CARD_WIDTH + CARD_GAP);
        let rect = Rect {
            x,
            y:      card_area.y,
            width:  CARD_WIDTH,
            height: posters.card_height(CARD_WIDTH).min(card_area.height),
        };
        if rect.x + rect.width <= card_area.x + card_area.width {
            let reason = if key == "recommended" {
                data.recommended_reasons.get(&anime.id).map(String::as_str)
            } else {
                None
            };
            let progress = data.progress_labels.get(&anime.id).map(String::as_str);
            render_card(frame, rect, anime, is_active && offset + i == state.row_cursor(key), reason, progress, posters);
        }
    }
}

/// Render a single anime card (cover + title + score).
/// `selected` draws an Amp accent border around the card to indicate it's the active selection.
fn render_card(
    frame: &mut Frame,
    area: Rect,
    anime: &Anime,
    selected: bool,
    reason: Option<&str>,
    progress: Option<&str>,
    posters: &mut PosterCache,
) {
    if area.height < 3 {
        return;
    }

    let border = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if selected { theme::ACCENT } else { theme::BG }))
        .style(Style::default().bg(if selected { theme::PANEL } else { theme::BG }));
    let content_area = border.inner(area);
    frame.render_widget(border, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // cover art
            Constraint::Length(1), // title
            Constraint::Length(1), // score + format
        ])
        .split(content_area);

    posters.render(frame, chunks[0], anime);

    // Title
    let title = Paragraph::new(Span::styled(
        anime.short_title(),
        Style::default()
            .fg(theme::TEXT)
            .add_modifier(Modifier::BOLD),
    ))
    .style(Style::default().bg(if selected {
        theme::PANEL
    } else {
        theme::BG
    }));
    frame.render_widget(title, chunks[1]);

    // Score + format
    let meta_text = match (progress, reason) {
        (Some(progress), Some(label)) => format!(
            "{} · {}",
            truncate_reason(progress, 10),
            truncate_reason(label, 7)
        ),
        (Some(progress), None) => truncate_reason(progress, 18),
        (None, Some(label)) => format!("• {}", truncate_reason(label, 18)),
        (None, None) => {
            let score_str = anime
                .score
                .map(|s| format!("★{:.1}", s as f32 / 10.0))
                .unwrap_or_else(|| "★ N/A".to_string());
            let fmt_str   = anime.format.as_deref().unwrap_or("TV");
            format!("{} · {}", score_str, fmt_str)
        }
    };
    let meta_color = if selected {
        theme::TEXT
    } else if reason.is_some() || progress.is_some() {
        theme::ACCENT
    } else {
        theme::MUTED
    };
    let meta      = Paragraph::new(Span::styled(
        meta_text,
        Style::default().fg(meta_color),
    ))
    .style(Style::default().bg(if selected {
        theme::PANEL
    } else {
        theme::BG
    }));
    frame.render_widget(meta, chunks[2]);
}

fn truncate_reason(reason: &str, max_chars: usize) -> String {
    let chars: Vec<char> = reason.chars().collect();
    if chars.len() <= max_chars {
        reason.to_string()
    } else {
        let truncated: String = chars[..max_chars.saturating_sub(1)].iter().collect();
        format!("{}…", truncated)
    }
}

fn recommended_label(data: &HomeData) -> String {
    if let Some(seed) = data.continue_watching.first() {
        format!("Because you watched {}", truncate_reason(seed.short_title().as_str(), 18))
    } else {
        "Recommended for you".to_string()
    }
}

/// All data needed to render the home screen.
pub struct HomeData {
    pub featured:          Option<Anime>,
    pub continue_watching: Vec<Anime>,
    pub watchlist:         Vec<Anime>,
    pub recommended:      Vec<Anime>,
    pub recommended_reasons: HashMap<i64, String>,
    pub progress_labels:  HashMap<i64, String>,
    pub resume_next:      HashMap<i64, u32>,
    pub trending:          Vec<Anime>,
    pub popular:           Vec<Anime>,
    pub top_rated:         Vec<Anime>,
    pub seasonal:          Vec<Anime>,
}

impl HomeData {
    /// Clamp selection before banner loading when refreshed lists change size.
    pub fn normalize_selection(&self, state: &mut AppState) {
        for (key, total) in [
            ("continue_watching", self.continue_watching.len()),
            ("watchlist", self.watchlist.len()),
            ("recommended", self.recommended.len()),
            ("trending", self.trending.len()),
            ("popular", self.popular.len()),
            ("top_rated", self.top_rated.len()),
            ("seasonal", self.seasonal.len()),
        ] {
            state.normalize_row(key, total);
        }
    }

    pub fn empty() -> Self {
        Self {
            featured:          None,
            continue_watching: Vec::new(),
            watchlist:         Vec::new(),
            recommended:      Vec::new(),
            recommended_reasons: HashMap::new(),
            progress_labels:  HashMap::new(),
            resume_next:      HashMap::new(),
            trending:          Vec::new(),
            popular:           Vec::new(),
            top_rated:         Vec::new(),
            seasonal:          Vec::new(),
        }
    }
}
