//! Help overlay — keybinding reference, shown with ? key.

use crate::ui::theme;

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

/// Render the help overlay centered on screen.
pub fn render_overlay(frame: &mut Frame) {
    let area = theme::popup(frame.area(), 94, 36);
    frame.render_widget(Clear, area);

    let keybindings = vec![
        (
            "Navigation",
            vec![
                ("j / ↓", "Move down (rows / menus)"),
                ("k / ↑", "Move up (rows / menus)"),
                ("h / ←", "Scroll left (cards / episodes / related)"),
                ("l / →", "Scroll right (cards / episodes / related)"),
                ("Tab", "Toggle Detail focus: Episodes / More Like This"),
            ],
        ),
        (
            "Actions",
            vec![
                ("Enter", "Open detail (Home) / start or continue (Detail)"),
                ("d", "Open detail from Home"),
                ("r", "Resume from Home / next episode"),
                ("Esc", "Back / close (restores prior related detail)"),
                ("/", "Open search"),
                ("s", "Open settings"),
                ("!", "Open setup / dependency checks"),
                ("+", "Add to / remove from watchlist"),
                ("n", "Play next episode from Detail"),
                ("q", "Quit / Stop playback"),
            ],
        ),
        (
            "Other",
            vec![
                ("?", "Toggle this help"),
                ("Shift+R", "Refresh home screen"),
                (
                    "Search",
                    "Type normally, use ↑/↓ to pick results and preview",
                ),
                ("Settings", "Use j/k to focus rows, h/l or Enter to change"),
                ("Setup", "Use r to refresh dependency checks"),
                ("Ctrl+C", "Force quit"),
            ],
        ),
    ];

    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "Make yourself at home",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (section, bindings) in &keybindings {
        lines.push(Line::from(Span::styled(
            format!("  {} ", section),
            Style::default()
                .fg(theme::MUTED)
                .add_modifier(Modifier::BOLD),
        )));
        for (key, desc) in bindings {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:>12}  ", key),
                    Style::default()
                        .fg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(*desc, Style::default().fg(theme::MUTED)),
            ]));
        }
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "  Press ? or Esc to close",
        Style::default().fg(theme::MUTED),
    )));

    let help = Paragraph::new(lines).block(theme::panel("Keyboard shortcuts"));
    frame.render_widget(help, area);
}

/// Render the toast notification bar at the bottom of the screen.
pub fn render_toast(frame: &mut Frame, message: &str) {
    let area = frame.area();
    let height = 1u16;
    let toast_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(height),
        width: area.width,
        height,
    };

    let toast = Paragraph::new(Span::styled(
        format!("  ✓ {}", message),
        Style::default()
            .fg(theme::TEXT)
            .bg(theme::PANEL)
            .add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(toast, toast_area);
}
