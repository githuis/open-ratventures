use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use crate::app::App;
use crate::ui::{C_ACCENT, C_BG};

impl App {
    pub(crate) fn render_welcome(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        Block::default().bg(C_BG).render(area, buf);

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Min(14),
                Constraint::Percentage(20),
            ])
            .split(area)[1];

        let center = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Min(40),
                Constraint::Percentage(20),
            ])
            .split(inner)[1];

        let client_ver = env!("CARGO_PKG_VERSION");
        let backend_ver = self.backend_version.as_deref().unwrap_or("...");
        let version_line = format!("v{client_ver}c  v{backend_ver}b");

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled("O P E N   R A T V E N T U R E S", text_style)),
            Line::from(Span::styled(version_line, Style::default().fg(C_ACCENT))),
            Line::from(""),
            Line::from(Span::styled("────────────────────────────────", Style::default().fg(C_ACCENT))),
            Line::from(""),
            Line::from("  Deep beneath the cobblestones, something stirs."),
            Line::from("  Rats with ambition. Dungeons with loot. Cheese"),
            Line::from("  of dubious origin. You have been warned."),
            Line::from(""),
            Line::from("  Gather your party. Choose your words carefully."),
            Line::from("  Not all battles are won with a blade."),
            Line::from(""),
            Line::from(Span::styled("────────────────────────────────", Style::default().fg(C_ACCENT))),
            Line::from(""),
            Line::from(vec![
                "  ".into(),
                Span::styled("[R]", text_style),
                " Enter your name to begin".into(),
            ]),
            Line::from(vec![
                "  ".into(),
                Span::styled("[Q]", text_style),
                " Quit".into(),
            ]),
            Line::from(""),
        ];

        Paragraph::new(lines)
            .centered()
            .render(center, buf);
    }
}
