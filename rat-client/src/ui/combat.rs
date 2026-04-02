use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use ratback_types::data::ItemEffect;
use ratback_types::quest_data::Encounter;

use crate::app::App;


impl App {
    pub(crate) fn render_combat(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let quest = match &self.active_quest {
            Some(q) => q,
            None => return,
        };
        let combat = match quest.encounters.get(quest.current_encounter as usize) {
            Some(Encounter::CombatEncounter(c)) => c,
            _ => return,
        };

        let block = Block::default()
            .title(Line::from(" Combat ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(self.c_accent()))
            .bg(self.c_panel());

        let inner = block.inner(area);
        block.render(area, buf);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(6)])
            .split(inner);

        let mut lines = vec![
            Line::from(vec![
                " Turn ".into(),
                Span::styled(combat.turn.to_string(), text_style),
            ]),
            Line::from(""),
            Line::from(" Enemies:".bold()),
            Line::from(""),
        ];

        for (_i, m) in combat.monsters.iter().enumerate() {
            let label = if m.unit.health <= 0 {
                format!("  {} [DEAD]", m.name)
            } else {
                format!("  {}", m.name)
            };
            lines.push(Line::from(vec![
                label.into(),
                "  ".into(),
                Span::styled(format!("{}/{} hp", m.unit.health, m.unit.max_health), text_style),
                format!("  atk {}", m.attack).into(),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            " ".into(),
            Span::styled("[F]", text_style),
            " Attack — ".into(),
            Span::styled("5 dmg to first living enemy", text_style),
        ]));
        lines.push(Line::from(vec![
            " ".into(),
            Span::styled("[V]", text_style),
            " View inventory".into(),
        ]));

        if !self.inventory.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(" Items:".bold()));
            for (i, inv) in self.inventory.iter().enumerate() {
                let effect_str = match &inv.item.effect {
                    ItemEffect::FullHeal => "full heal".to_string(),
                    ItemEffect::Damage(d) => format!("{} dmg", d),
                    ItemEffect::Heal(h) => format!("heal {}", h),
                    ItemEffect::MaxHpUp(n) => format!("+{} max hp", n),
                };
                lines.push(Line::from(vec![
                    " ".into(),
                    Span::styled(format!("[{}]", i + 1), text_style),
                    format!(" {} ({}) — ", inv.item.name, if inv.charges_remaining == -1 { "∞".to_string() } else { format!("x{}", inv.charges_remaining) }).into(),
                    Span::styled(effect_str, text_style),
                ]));
            }
        }

        Paragraph::new(lines)
            .bg(self.c_panel())
            .render(layout[0], buf);

        // Combat log
        let log_block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(self.c_accent()))
            .bg(self.c_panel());
        let log_inner = log_block.inner(layout[1]);
        log_block.render(layout[1], buf);

        let log_height = log_inner.height as usize;
        let log_lines: Vec<Line> = self.combat_log.iter()
            .rev()
            .take(log_height)
            .rev()
            .map(|entry| {
                Line::from(entry.iter().map(|(text, highlighted)| {
                    if *highlighted {
                        Span::styled(text.as_str(), text_style)
                    } else {
                        Span::raw(text.as_str())
                    }
                }).collect::<Vec<_>>())
            })
            .collect();

        Paragraph::new(log_lines)
            .bg(self.c_panel())
            .render(log_inner, buf);
    }
}
