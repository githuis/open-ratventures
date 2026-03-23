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
use crate::ui::{C_TEXT, C_ACCENT, C_PANEL};

impl App {
    pub(crate) fn render_inventory_popup(&self, area: Rect, buf: &mut Buffer, text_style: Style, scroll: usize) {
        let block = Block::default()
            .title(Line::from(" Inventory ").centered())
            .title_bottom(Line::from(" [V/Esc] Close  [J/K] Scroll ").centered())
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

        let mut lines: Vec<Line> = vec![Line::from("")];
        for inv in &self.inventory {
            let effect_str = match &inv.item.effect {
                ItemEffect::Damage(d) => format!("{} dmg", d),
                ItemEffect::Heal(h) => format!("heal {}", h),
                ItemEffect::FullHeal => "full heal".to_string(),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", inv.item.name), text_style),
                if inv.charges_remaining == -1 { "∞  ".into() } else { format!("x{}  ", inv.charges_remaining).into() },
                Span::styled(format!("[{}]", effect_str), Style::default().fg(C_ACCENT)),
            ]));
            lines.push(Line::from(vec![
                "    ".into(),
                Span::raw(inv.item.description.clone()),
            ]));
            lines.push(Line::from(""));
        }

        Paragraph::new(lines)
            .scroll((scroll as u16, 0))
            .wrap(Wrap { trim: false })
            .bg(C_PANEL)
            .render(inner, buf);
    }
}
