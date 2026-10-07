//! Search overlay with portrait result cards and keyboard selection.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::{
    db::cache::Anime,
    state::AppState,
    ui::{components::posters::PosterCache, theme},
};

const CARD_WIDTH: u16 = 22;
const GAP: u16 = 2;

pub fn render_overlay(frame: &mut Frame, state: &AppState, posters: &mut PosterCache) {
    let area = theme::popup(frame.area(), 110, 42);
    frame.render_widget(Clear, area);
    let panel = theme::panel("Search anime");
    let inner = panel.inner(area);
    frame.render_widget(panel, area);
    posters.poll();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner);
    let input = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme::BG).bg(theme::ACCENT)),
            Span::styled(
                format!("  {}▏", state.search_query),
                Style::default().fg(theme::TEXT),
            ),
        ]),
        Line::from(Span::styled(
            "Type to search  ·  Arrows previous / next  ·  Enter open  ·  Esc close",
            Style::default().fg(theme::MUTED),
        )),
    ])
    .style(Style::default().bg(theme::SURFACE));
    frame.render_widget(input, chunks[0]);

    if let Some(anime) = state.search_results.get(state.search_cursor) {
        frame.render_widget(
            Paragraph::new(Span::styled(
                anime.display_title(),
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ))
            .style(Style::default().bg(theme::SURFACE)),
            chunks[1],
        );
    }
    render_results(frame, chunks[2], state, posters);
    let position = if state.search_results.is_empty() {
        "Search your collections and AniList".to_string()
    } else {
        format!(
            "{} / {} results",
            state.search_cursor + 1,
            state.search_results.len()
        )
    };
    frame.render_widget(
        Paragraph::new(position).style(Style::default().fg(theme::MUTED).bg(theme::SURFACE)),
        chunks[3],
    );
}

/// Fit complete portrait cards, shrinking their width on short terminals.
fn grid_shape(area: Rect, card_height: impl Fn(u16) -> u16) -> Option<(u16, u16, usize, usize)> {
    let mut width = CARD_WIDTH.min(area.width);
    while width >= 6 && card_height(width) > area.height {
        width -= 1;
    }
    if width < 6 {
        return None;
    }
    let height = card_height(width);
    let columns = (area.width.saturating_add(GAP) / (width + GAP)).max(1) as usize;
    let rows = (area.height.saturating_add(GAP) / (height + GAP)).max(1) as usize;
    Some((width, height, columns, rows))
}

fn page_start(cursor: usize, capacity: usize) -> usize {
    cursor / capacity.max(1) * capacity.max(1)
}

fn render_results(frame: &mut Frame, area: Rect, state: &AppState, posters: &mut PosterCache) {
    let Some((width, height, columns, rows)) = grid_shape(area, |width| posters.card_height(width))
    else {
        frame.render_widget(
            Paragraph::new("Enlarge the terminal to view posters.")
                .style(Style::default().fg(theme::MUTED).bg(theme::SURFACE)),
            area,
        );
        return;
    };
    if state.search_results.is_empty() {
        let message = if state.search_query.is_empty() {
            "Find your next watch. Type an anime title above."
        } else {
            "No matching titles yet. Try another search."
        };
        frame.render_widget(
            Paragraph::new(message).style(Style::default().fg(theme::MUTED).bg(theme::SURFACE)),
            area,
        );
        return;
    }
    let capacity = columns * rows;
    let start = page_start(state.search_cursor, capacity);
    for (slot, anime) in state
        .search_results
        .iter()
        .skip(start)
        .take(capacity)
        .enumerate()
    {
        let card = Rect::new(
            area.x + (slot % columns) as u16 * (width + GAP),
            area.y + (slot / columns) as u16 * (height + GAP),
            width,
            height,
        );
        render_card(
            frame,
            card,
            anime,
            start + slot == state.search_cursor,
            posters,
        );
    }
}

fn render_card(
    frame: &mut Frame,
    area: Rect,
    anime: &Anime,
    selected: bool,
    posters: &mut PosterCache,
) {
    let background = if selected {
        theme::PANEL
    } else {
        theme::SURFACE
    };
    let border = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if selected {
            theme::ACCENT
        } else {
            theme::BORDER
        }))
        .style(Style::default().bg(background));
    let inner = border.inner(area);
    frame.render_widget(border, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    posters.render(frame, chunks[0], anime);
    frame.render_widget(
        Paragraph::new(anime.display_title()).style(
            Style::default()
                .fg(theme::TEXT)
                .bg(background)
                .add_modifier(Modifier::BOLD),
        ),
        chunks[1],
    );
    let score = anime
        .score
        .map(|score| format!("{:.1}", score as f32 / 10.0))
        .unwrap_or_else(|| "--".into());
    let metadata = format!("★ {score}  ·  {}", anime.format.as_deref().unwrap_or("TV"));
    frame.render_widget(
        Paragraph::new(metadata).style(Style::default().fg(theme::MUTED).bg(background)),
        chunks[2],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portrait_grid_fits_after_resize_without_clipping() {
        for (width, height) in [(104, 32), (45, 15), (12, 10), (0, 0), (5, 50)] {
            let area = Rect::new(3, 4, width, height);
            if let Some((card_width, card_height, columns, rows)) = grid_shape(area, |w| w + 4) {
                assert!((columns as u16 * (card_width + GAP)).saturating_sub(GAP) <= width);
                assert!((rows as u16 * (card_height + GAP)).saturating_sub(GAP) <= height);
                assert!(card_width >= 6);
            } else {
                assert!(width < 6 || height < 10);
            }
        }
    }

    #[test]
    fn result_page_stays_still_until_selection_crosses_edge() {
        for cursor in 0..8 {
            assert_eq!(page_start(cursor, 8), 0);
        }
        assert_eq!(page_start(8, 8), 8);
        assert_eq!(page_start(15, 8), 8);
        assert_eq!(page_start(7, 8), 0);
    }
}
