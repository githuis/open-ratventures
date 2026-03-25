use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use ratback::quest_data::PartySummary;

use crate::app::App;
use crate::ui::{C_ACCENT, C_PANEL};

impl App {
    pub(crate) fn render_party_lobby(&self, area: Rect, buf: &mut Buffer, text_style: Style, parties: &[PartySummary]) {
        let block = Block::default()
            .title(Line::from(" Find a Party ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT));

        let mut lines = vec![Line::from(""), Line::from(" Open parties:".bold()), Line::from("")];

        if parties.is_empty() {
            lines.push(Line::from("  No open parties."));
        } else {
            for (i, p) in parties.iter().enumerate() {
                lines.push(Line::from(vec![
                    Span::styled(format!(" [{}] ", i + 1), text_style),
                    format!("Party #{} — {} member(s)", p.id, p.member_count).into(),
                ]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" [N] ", text_style),
            "Create new party".into(),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" [R] ", text_style),
            "Refresh".into(),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" [Esc] ", text_style),
            "Back".into(),
        ]));

        Paragraph::new(lines)
            .block(block)
            .bg(C_PANEL)
            .render(area, buf);
    }
}
