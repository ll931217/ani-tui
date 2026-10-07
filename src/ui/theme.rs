//! Shared visual language: the user's Alacritty Amp palette.
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Padding},
};

pub const BG: Color = Color::Rgb(15, 15, 15);
pub const SURFACE: Color = Color::Rgb(22, 22, 22);
pub const PANEL: Color = Color::Rgb(58, 38, 20);
pub const TEXT: Color = Color::Rgb(242, 236, 221);
pub const MUTED: Color = Color::Rgb(122, 115, 103);
pub const ACCENT: Color = Color::Rgb(231, 137, 76);
pub const BORDER: Color = Color::Rgb(26, 26, 26);
pub const WARNING: Color = Color::Rgb(227, 162, 90);
pub const SUCCESS: Color = Color::Rgb(124, 155, 150);

pub fn panel(title: &str) -> Block<'_> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().fg(TEXT).bg(SURFACE))
        .padding(Padding::new(2, 2, 1, 1))
}

pub fn popup(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

/// Older cached synopses can contain HTML even when AniList requests plain text.
pub fn plain_text(value: &str) -> String {
    let mut text = String::new();
    let mut chars = value.chars().peekable();
    let mut in_tag = false;
    while let Some(ch) = chars.next() {
        if ch == '<' && chars.peek().is_some_and(|c| c.is_ascii_alphabetic() || *c == '/') {
            in_tag = true;
        } else if ch == '>' && in_tag {
            in_tag = false;
            text.push(' ');
        } else if !in_tag {
            text.push(ch);
        }
    }
    for (entity, decoded) in [
        ("&nbsp;", " "), ("&quot;", "\""), ("&#39;", "'"),
        ("&lt;", "<"), ("&gt;", ">"), ("&amp;", "&"),
    ] {
        text = text.replace(entity, decoded);
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_synopses_are_readable_without_losing_plain_comparisons() {
        assert_eq!(plain_text("<b>One</b><br><br>Two &amp; three"), "One Two & three");
        assert_eq!(plain_text("A < B &gt; C"), "A < B > C");
    }
}
