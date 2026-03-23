use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::app::{App, TavernState};
use crate::ui::{C_TEXT, C_ALERT, C_ACCENT, C_PANEL};

impl App {
    pub(crate) fn render_tavern(&self, area: Rect, buf: &mut Buffer, text_style: Style, sub: &TavernState) {
        match sub {
            TavernState::Main => {
                let has_char = self.active_character.is_some();
                let dim = Style::default().fg(C_ACCENT);

                let opt = |key: &'static str, label: &'static str, enabled: bool| -> Line<'static> {
                    if enabled {
                        Line::from(vec![
                            "  ".into(),
                            Span::styled(key, text_style),
                            format!("  {label}").into(),
                        ])
                    } else {
                        Line::from(Span::styled(
                            format!("  {key}  {label}  (no character)"),
                            dim,
                        ))
                    }
                };

                let block = Block::default()
                    .title(Line::from(" The Rusty Rat Tavern ".bold()))
                    .borders(Borders::ALL)
                    .border_set(border::THICK)
                    .border_style(Style::default().fg(C_ACCENT))
                    .bg(C_PANEL);

                let lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "  The tavern is warm and dimly lit. The smell of ale and wood smoke",
                        text_style,
                    )),
                    Line::from(Span::styled(
                        "  fills the air. A few weathered adventurers nurse their drinks",
                        text_style,
                    )),
                    Line::from(Span::styled(
                        "  in the corners. The barkeep eyes you with a knowing grin.",
                        text_style,
                    )),
                    Line::from(""),
                    Line::from(""),
                    opt("[S]", "Shop — browse goods from the barkeep", has_char),
                    opt("[A]", "Adventure — seek a quest", has_char),
                    opt("[O]", "Options — change character", has_char),
                    Line::from(vec![
                        "  ".into(),
                        Span::styled("[Q]", text_style),
                        "  Quit".into(),
                    ]),
                ];

                Paragraph::new(lines)
                    .block(block)
                    .wrap(Wrap { trim: false })
                    .render(area, buf);
            }
            TavernState::Shop => {
                let coins = self.active_character.as_ref().map(|c| c.character.coins).unwrap_or(0);
                let can_afford = coins >= 5;

                let block = Block::default()
                    .title(Line::from(" Barkeep's Wares ".bold()))
                    .borders(Borders::ALL)
                    .border_set(border::THICK)
                    .border_style(Style::default().fg(C_ACCENT))
                    .bg(C_PANEL);

                let gem_line = if can_afford {
                    Line::from(vec![
                        "  ".into(),
                        Span::styled("[1]", text_style),
                        "  Gem of Resurrection — restore full health  ".into(),
                        Span::styled("(5 gold)", Style::default().fg(C_ALERT)),
                    ])
                } else {
                    Line::from(Span::styled(
                        "  [1]  Gem of Resurrection — restore full health  (5 gold)  [not enough gold]",
                        Style::default().fg(C_ACCENT),
                    ))
                };

                let lines = vec![
                    Line::from(""),
                    Line::from(Span::styled("  \"What'll it be, traveller?\"", text_style)),
                    Line::from(""),
                    gem_line,
                    Line::from(""),
                    Line::from(vec![
                        "  ".into(),
                        Span::styled("[Esc]", text_style),
                        "  Back to the tavern".into(),
                    ]),
                ];

                Paragraph::new(lines)
                    .block(block)
                    .wrap(Wrap { trim: false })
                    .render(area, buf);
            }
        }
    }
}
