use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use ratback_types::data::CharacterWrapper;

use crate::app::App;
use crate::ui::{C_ACCENT, C_ALERT, C_PANEL};

impl App {
    pub(crate) fn render_party(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let block = Block::default()
            .title(Line::from(" Party ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT));

        let my_char_id = self.active_character.as_ref().map(|c| c.character.id);
        let characters: Vec<&CharacterWrapper> = self.party_members.iter()
            .filter(|c| Some(c.character.id) != my_char_id)
            .collect();

        let bg = C_PANEL;

        if characters.is_empty() {
            let msg = if self.active_party.is_some() || self.active_quest.is_some() {
                " Waiting for others..."
            } else {
                " Not in a party"
            };
            Paragraph::new(msg)
                .block(block)
                .bg(bg)
                .render(area, buf);
            return;
        }

        let inner = block.inner(area);
        // fill background so uncovered space matches cards
        buf.set_style(inner, Style::default().bg(bg));
        block.bg(bg).render(area, buf);

        let card_height = (inner.height / characters.len() as u16).max(1);
        for (i, c) in characters.iter().enumerate() {
            let card_area = Rect::new(
                inner.x,
                inner.y + i as u16 * card_height,
                inner.width,
                card_height,
            );
            let card_block = Block::default()
                .title(Line::from(format!(" {} ", c.character.name)))
                .borders(Borders::ALL)
                .border_set(border::PLAIN)
                .border_style(Style::default().fg(C_ACCENT));

            let lines = vec![
                Line::from(vec![
                    "HP ".into(),
                    Span::styled(
                        format!("{}/{}", c.unit.health, c.unit.max_health),
                        text_style,
                    ),
                ]),
                Line::from(vec![
                    "EP ".into(),
                    Span::styled(
                        format!("{}/{}", c.unit.energy, c.unit.max_energy),
                        text_style,
                    ),
                ]),
            ];

            Paragraph::new(lines)
                .block(card_block)
                .bg(C_PANEL)
                .render(card_area, buf);
        }
    }

    pub(crate) fn render_party_screen(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let my_char_id = self.active_character.as_ref().map(|c| c.character.id);

        let block = Block::default()
            .title(Line::from(" Party ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT));

        let in_party = self.active_party.is_some();
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                if in_party { " You are in a party" } else { " You are not in a party" },
                Style::default().fg(if in_party { C_ACCENT } else { C_ALERT }),
            )),
            Line::from(""),
            Line::from(" Members:".bold()),
            Line::from(""),
        ];

        if self.party_members.is_empty() {
            lines.push(Line::from(Span::styled(
                "  Waiting for others to join...",
                Style::default().fg(C_ALERT),
            )));
        } else {
            for c in &self.party_members {
                let is_you = Some(c.character.id) == my_char_id;
                let leader_mark = if self.active_party.as_ref().map(|p| p.leader_id == c.character.id).unwrap_or(false) {
                    " ★"
                } else {
                    ""
                };
                lines.push(Line::from(Span::styled(
                    format!("  {}{}{}", c.character.name, leader_mark, if is_you { " (you)" } else { "" }),
                    text_style,
                )));
                lines.push(Line::from(Span::styled(
                    format!("    HP {}/{}", c.unit.health, c.unit.max_health),
                    Style::default().fg(C_ACCENT),
                )));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Return to the tavern to start an adventure.",
            Style::default().fg(C_ALERT),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" [L] ", text_style),
            "Leave party".into(),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" [Q] ", text_style),
            "Back to tavern (stay in party)".into(),
        ]));

        Paragraph::new(lines)
            .block(block)
            .bg(C_PANEL)
            .render(area, buf);
    }
}
