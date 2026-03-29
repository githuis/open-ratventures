use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};
use ratback::data::ItemEffect;

use crate::app::App;
use crate::ui::{C_ACCENT, C_PANEL};

impl App {
    pub(crate) fn render_inventory_popup(&self, area: Rect, buf: &mut Buffer, text_style: Style, scroll: usize, selected: usize, in_combat: bool) {
        let block = Block::default()
            .title(Line::from(" Inventory ").centered())
            .title_bottom(Line::from(" [↑/↓] Navigate  [Enter] Use  [V/Esc] Close ").centered())
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT))
            .bg(C_PANEL);

        let inner = block.inner(area);
        block.render(area, buf);

        if self.inventory.is_empty() {
            Paragraph::new(Line::from(vec![
                Span::styled(" Your pack is empty.", text_style),
            ]))
            .bg(C_PANEL)
            .render(inner, buf);
            return;
        }

        let dim = Style::default().fg(C_ACCENT);
        let selected_style = Style::default().bg(C_ACCENT).fg(ratatui::style::Color::White);

        const PAGE: usize = 5;
        let mut lines: Vec<Line> = vec![Line::from("")];

        for (abs_idx, inv) in self.inventory.iter().enumerate().skip(scroll).take(PAGE) {
            let is_sel = abs_idx == selected;
            let cursor = if is_sel { "▶ " } else { "  " };
            let is_dead = self.active_character.as_ref().map_or(false, |c| c.unit.health <= 0)
                || self.party_members.iter().any(|m| m.unit.health <= 0);
            let can_use = match &inv.item.effect {
                ItemEffect::Damage(_) => in_combat,
                ItemEffect::Heal(_) | ItemEffect::MaxHpUp(_) => true,
                ItemEffect::FullHeal => is_dead,
            };
            let charges_str = if inv.charges_remaining == -1 {
                "∞".to_string()
            } else {
                format!("x{}", inv.charges_remaining)
            };
            let effect_str = match &inv.item.effect {
                ItemEffect::Damage(d) => format!("{} dmg", d),
                ItemEffect::Heal(h) => format!("heal {}", h),
                ItemEffect::FullHeal => "full heal".to_string(),
                ItemEffect::MaxHpUp(n) => format!("+{} max hp", n),
            };

            if is_sel {
                lines.push(Line::from(Span::styled(
                    format!("{}{} {}  [{}]", cursor, inv.item.name, charges_str, effect_str),
                    selected_style,
                )));
            } else if can_use {
                lines.push(Line::from(vec![
                    Span::styled(format!("{}{} ", cursor, inv.item.name), text_style),
                    format!("{}  ", charges_str).into(),
                    Span::styled(format!("[{}]", effect_str), dim),
                ]));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("{}{}  {}  [{}]  (combat only)", cursor, inv.item.name, charges_str, effect_str),
                    dim,
                )));
            }
            lines.push(Line::from(vec![
                "    ".into(),
                Span::raw(inv.item.description.clone()),
            ]));
            lines.push(Line::from(""));
        }

        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .bg(C_PANEL)
            .render(inner, buf);
    }

    pub(crate) fn render_game_over(&self, area: Rect, buf: &mut Buffer, _text_style: Style) {
        let has_revive = self.inventory.iter().any(|i| {
            matches!(i.item.effect, ItemEffect::FullHeal) && i.charges_remaining != 0
        });

        let hint = if has_revive {
            " [I] Use revive item  [Q] Give up "
        } else {
            " [Q] Give up "
        };

        let block = Block::default()
            .title(Line::from(" Party wiped! ").centered())
            .title_bottom(Line::from(hint).centered())
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT))
            .bg(C_PANEL);

        let inner = block.inner(area);
        block.render(area, buf);

        let body = if has_revive {
            "Everyone is down.\nYou have revive items remaining."
        } else {
            "Everyone is down.\nNo revive items remain.\nThe adventure ends here."
        };

        Paragraph::new(body)
            .wrap(Wrap { trim: false })
            .bg(C_PANEL)
            .render(inner, buf);
    }

    pub(crate) fn render_target_select(&self, area: Rect, buf: &mut Buffer, text_style: Style, selected: usize) {
        let block = Block::default()
            .title(Line::from(" Revive who? ").centered())
            .title_bottom(Line::from(" [↑/↓] Navigate  [Enter] Confirm  [Esc] Back ").centered())
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT))
            .bg(C_PANEL);

        let inner = block.inner(area);
        block.render(area, buf);

        let selected_style = Style::default().bg(C_ACCENT).fg(ratatui::style::Color::White);
        let targets = self.dead_targets();

        let mut lines: Vec<Line> = vec![Line::from("")];
        for (idx, (_, name)) in targets.iter().enumerate() {
            let cursor = if idx == selected { "▶ " } else { "  " };
            let line = format!("{}{}", cursor, name);
            if idx == selected {
                lines.push(Line::from(Span::styled(line, selected_style)));
            } else {
                lines.push(Line::from(Span::styled(line, text_style)));
            }
        }

        Paragraph::new(lines)
            .bg(C_PANEL)
            .render(inner, buf);
    }
}
