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
            let can_use = match &inv.item.effect {
                ItemEffect::Damage(_) => in_combat,
                ItemEffect::Heal(_) | ItemEffect::FullHeal => true,
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
}
