use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::app::{App, TavernState};
use crate::ui::{C_ALERT, C_ACCENT, C_PANEL};

const DEPTHS_RENOWN: u32 = 5;
const WARRENS_RENOWN: u32 = 10;
const ABYSS_RENOWN: u32 = 20;

impl App {
    pub(crate) fn render_adventure_menu(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let renown = self.active_character.as_ref().map(|c| c.character.renown).unwrap_or(0);
        let dim = Style::default().fg(C_ACCENT);

        let block = Block::default()
            .title(Line::from(" Adventure — Choose a Destination ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT))
            .bg(C_PANEL);

        let zone = |key: &'static str, label: &'static str, note: Option<&'static str>, enabled: bool| -> Line<'static> {
            if enabled {
                let mut spans = vec![
                    "  ".into(),
                    Span::styled(key, text_style),
                    format!("  {label}").into(),
                ];
                if let Some(n) = note {
                    spans.push(Span::styled(format!("  — {n}"), dim));
                }
                Line::from(spans)
            } else {
                Line::from(Span::styled(format!("  {key}  {label}"), dim))
            }
        };

        let abyss_label = if renown >= ABYSS_RENOWN { "The Abyss" } else { "????" };
        let abyss_note: Option<&'static str> = if renown >= ABYSS_RENOWN { Some("something stirs below") } else { None };

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled("  Where do you venture?", text_style)),
            Line::from(""),
            zone("[1]", "Top-level Sewers", Some("common rats and ruffians"), true),
            zone("[2]", "Sewer Depths", Some("darker, more dangerous"), renown >= DEPTHS_RENOWN),
            zone("[3]", "The Fungal Warrens", Some("bioluminescent caverns below the sewers"), renown >= WARRENS_RENOWN),
            zone("[4]", abyss_label, abyss_note, renown >= ABYSS_RENOWN),
            Line::from(Span::styled("  [5]  Follow Clues  — (coming soon)", dim)),
            Line::from(""),
            Line::from(vec![
                "  ".into(),
                Span::styled("[Esc]", text_style),
                "  Back".into(),
            ]),
        ];

        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

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
                    opt("[A]", "Adventure — seek out a quest", has_char),
                    opt("[S]", "Shop — browse goods from the barkeep", has_char),
                    opt("[G]", "Group — group up with a new or existing adventuring party", has_char),
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
            TavernState::Shop { items, selected, scroll } => {
                const PAGE: usize = 5;
                let coins = self.active_character.as_ref().map(|c| c.character.coins).unwrap_or(0);
                let dim = Style::default().fg(C_ACCENT);
                let selected_bg = Style::default().bg(C_ACCENT).fg(ratatui::style::Color::White);

                let block = Block::default()
                    .title(Line::from(" Barkeep's Wares ".bold()))
                    .borders(Borders::ALL)
                    .border_set(border::THICK)
                    .border_style(Style::default().fg(C_ACCENT))
                    .bg(C_PANEL);

                let mut lines = vec![
                    Line::from(""),
                    Line::from(Span::styled("  \"What'll it be, traveller?\"", text_style)),
                    Line::from(""),
                ];

                let visible: Vec<_> = items.iter().skip(*scroll).take(PAGE).collect();
                for (i, entry) in visible.iter().enumerate() {
                    let abs = scroll + i;
                    let can_afford = coins >= entry.cost as u32;
                    let is_selected = abs == *selected;
                    let cursor = if is_selected { "▶ " } else { "  " };
                    if can_afford {
                        let style = if is_selected { selected_bg } else { text_style };
                        lines.push(Line::from(vec![
                            Span::styled(format!("{}{}", cursor, entry.item.name), style),
                            Span::styled(format!("  {} gold", entry.cost), Style::default().fg(C_ALERT)),
                        ]));
                    } else {
                        lines.push(Line::from(Span::styled(
                            format!("{}{}  {} gold  [not enough gold]", cursor, entry.item.name, entry.cost),
                            dim,
                        )));
                    }
                    lines.push(Line::from(Span::styled(
                        format!("    {}", entry.item.description),
                        dim,
                    )));
                }

                if items.is_empty() {
                    lines.push(Line::from(Span::styled("  The shelves are bare.", dim)));
                }

                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    "  ".into(),
                    Span::styled("[↑/↓]", text_style),
                    " Navigate  ".into(),
                    Span::styled("[Enter]", text_style),
                    " Buy  ".into(),
                    Span::styled("[Esc]", text_style),
                    " Back".into(),
                ]));

                Paragraph::new(lines)
                    .block(block)
                    .wrap(Wrap { trim: false })
                    .render(area, buf);
            }
        }
    }
}
