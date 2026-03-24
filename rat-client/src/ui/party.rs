use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use ratback::data::CharacterWrapper;

use crate::app::App;
use crate::ui::{C_ACCENT, C_PANEL};

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
            Paragraph::new(" No other party members")
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
}
