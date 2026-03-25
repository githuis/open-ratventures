use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use ratback::quest_data::Encounter;

use crate::app::App;
use crate::ui::{C_TEXT, C_ACCENT, C_PANEL};

impl App {
    pub(crate) fn render_left_panel(&self, area: Rect, buf: &mut Buffer) {
        let text_style = Style::default()
            .fg(C_TEXT)
            .add_modifier(Modifier::BOLD);

        let block = Block::default()
            .title(Line::from(" Character ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT));

        let mut lines: Vec<Line> = Vec::new();

        if let Some(user) = &self.active_user {
            lines.push(Line::from(vec![
                "User: ".into(),
                Span::styled(&user.username, text_style),
            ]));
        }

        if let Some(c) = &self.active_character {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(vec![
                "Name:  ".into(),
                Span::styled(&c.character.name, text_style),
            ]));
            lines.push(Line::from(vec![
                "HP:    ".into(),
                Span::styled(c.unit.health.to_string(), text_style),
                "/".into(),
                Span::styled(c.unit.max_health.to_string(), text_style),
            ]));
            lines.push(Line::from(vec![
                "EP:    ".into(),
                Span::styled(c.unit.energy.to_string(), text_style),
                "/".into(),
                Span::styled(c.unit.max_energy.to_string(), text_style),
            ]));
            lines.push(Line::from(vec![
                "Gold:  ".into(),
                Span::styled(c.character.coins.to_string(), text_style),
            ]));
            lines.push(Line::from(vec![
                "Renown:".into(),
                Span::styled(c.character.renown.to_string(), text_style),
            ]));
        }

        if let Some(quest) = &self.active_quest {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            let enc_type = match quest.encounters.get(quest.current_encounter as usize) {
                Some(Encounter::CombatEncounter(_)) => "Combat",
                Some(Encounter::NpcEncounter(_)) => "NPC",
                _ => "—",
            };
            lines.push(Line::from(vec![
                "Quest: #".into(),
                Span::styled(quest.current_encounter.to_string(), text_style),
                " | ".into(),
                enc_type.into(),
            ]));
        }



        Paragraph::new(lines)
            .block(block)
            .bg(C_PANEL)
            .render(area, buf);
    }
}
