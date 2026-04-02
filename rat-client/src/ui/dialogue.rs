use ratatui::{
    buffer::Buffer,
    layout::{Margin, Rect},
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};
use ratback_types::quest_data::{Dialogue, DialogueOutcome};

use crate::app::App;


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
            .border_style(Style::default().fg(self.c_accent()));

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(node.text.clone(), text_style)),
            Line::from(""),
        ];

        let coins = self.active_character.as_ref().map(|c| c.character.coins as i32).unwrap_or(0);

        let too_expensive = |o: &DialogueOutcome| match o {
            DialogueOutcome::GiveItem { cost, .. } => coins < *cost,
            DialogueOutcome::Reward { coins: c, .. } => *c < 0 && coins < c.unsigned_abs() as i32,
            _ => false,
        };

        let choice_locked = |choice: &ratback_types::quest_data::DialogueChoice| -> bool {
            match &choice.outcome {
                Some(outcome) => too_expensive(outcome),
                None => {
                    // read ahead: if every choice in the next node is locked, this path is a dead end
                    if let Some(next_id) = &choice.next {
                        if let Some(next_node) = dialogue.nodes.get(next_id.as_str()) {
                            !next_node.choices.is_empty()
                                && next_node.choices.iter().all(|c| {
                                    c.outcome.as_ref().map_or(false, too_expensive)
                                })
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
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
                        Style::default().fg(self.c_accent()),
                    ),
                ]));
            }
        }

        let inner = area.inner(Margin { horizontal: 2, vertical: 0 });
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .bg(self.c_bg())
            .render(inner, buf);
    }
}
