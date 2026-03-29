use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use ratback_types::data::ItemEffect;
use ratback_types::quest_data::Encounter;

use crate::app::App;
use crate::ui::{C_ALERT, C_ACCENT, C_BG};

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
            .border_style(Style::default().fg(C_ACCENT));

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
        if let Some((dmg, target)) = &self.last_combat_damage {
            lines.push(Line::from(vec![
                " Monsters dealt ".into(),
                Span::styled(dmg.to_string(), Style::default().fg(C_ALERT).add_modifier(Modifier::BOLD)),
                format!(" damage to {}!", target).into(),
            ]));
        }
        lines.push(Line::from(vec![
            " [F] Attack — ".into(),
            Span::styled("5 dmg to first living enemy", text_style),
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
                    format!(" [{}] {} ({}) — ", i + 1, inv.item.name, if inv.charges_remaining == -1 { "∞".to_string() } else { format!("x{}", inv.charges_remaining) }).into(),
                    Span::styled(effect_str, text_style),
                ]));
            }
        }

        Paragraph::new(lines)
            .block(block)
            .bg(C_BG)
            .render(area, buf);
    }
}
