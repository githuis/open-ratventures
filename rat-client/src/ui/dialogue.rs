use ratatui::{
    buffer::Buffer,
    layout::{Margin, Rect},
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};
use ratback::quest_data::{Dialogue, DialogueOutcome};

use crate::app::App;
use crate::ui::{C_ACCENT, C_BG};

impl App {
    pub(crate) fn render_dialogue(&self, area: Rect, buf: &mut Buffer, text_style: Style, dialogue: &Dialogue, current_node: &str) {
        let node = match dialogue.nodes.get(current_node) {
            Some(n) => n,
            None => return,
        };

        let block = Block::default()
            .title(Line::from(" Conversation ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT));

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(node.text.clone(), text_style)),
            Line::from(""),
        ];

        let coins = self.active_character.as_ref().map(|c| c.character.coins as i32).unwrap_or(0);

        let choice_locked = |choice: &ratback::quest_data::DialogueChoice| -> bool {
            match &choice.outcome {
                Some(DialogueOutcome::GiveItem { cost, .. }) => coins < *cost,
                None => {
                    // read ahead: if every choice in the next node is locked, this path is a dead end
                    if let Some(next_id) = &choice.next {
                        if let Some(next_node) = dialogue.nodes.get(next_id.as_str()) {
                            !next_node.choices.is_empty()
                                && next_node.choices.iter().all(|c| match &c.outcome {
                                    Some(DialogueOutcome::GiveItem { cost, .. }) => coins < *cost,
                                    _ => false,
                                })
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            }
        };

        for (i, choice) in node.choices.iter().enumerate() {
            let affordable = !choice_locked(choice);
            if affordable {
                lines.push(Line::from(vec![
                    Span::styled(format!(" [{}] ", i + 1), text_style),
                    choice.text.clone().into(),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!(" [{}] {} (not enough gold)", i + 1, choice.text),
                        Style::default().fg(C_ACCENT),
                    ),
                ]));
            }
        }

        let inner = area.inner(Margin { horizontal: 2, vertical: 0 });
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .bg(C_BG)
            .render(inner, buf);
    }
}
