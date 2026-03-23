use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use ratback::quest_data::QuestSummary;

use crate::app::App;
use crate::ui::{C_TEXT, C_ACCENT, C_PANEL};

impl App {
    pub(crate) fn render_quest_lobby(&self, area: Rect, buf: &mut Buffer, text_style: Style, quests: &[QuestSummary]) {
        let block = Block::default()
            .title(Line::from(" Quest Lobby ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT));

        let mut lines = vec![Line::from(""), Line::from(" Open quests:".bold()), Line::from("")];

        if quests.is_empty() {
            lines.push(Line::from("  No open quests."));
        } else {
            for (i, q) in quests.iter().enumerate() {
                lines.push(Line::from(vec![
                    Span::styled(format!(" [{}] ", i + 1), text_style),
                    format!("Quest #{} — {} member(s)", q.id, q.member_count).into(),
                ]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" [N] ", text_style),
            "Create new quest".into(),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" [R] ", text_style),
            "Refresh".into(),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" [Esc] ", text_style),
            "Cancel".into(),
        ]));

        Paragraph::new(lines)
            .block(block)
            .bg(C_PANEL)
            .render(area, buf);
    }
}
