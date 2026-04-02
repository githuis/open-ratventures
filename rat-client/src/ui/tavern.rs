use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use ratback_types::{RENOWN_SEWER_DEPTHS, RENOWN_FUNGAL_WARRENS, RENOWN_ABYSS};
use crate::app::{App, AppState, TavernState, Reason};


impl App {
    pub(crate) fn render_adventure_menu(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let renown = self.active_character.as_ref().map(|c| c.character.renown).unwrap_or(0);
        let dim = Style::default().fg(self.c_accent());

        let block = Block::default()
            .title(Line::from(" Adventure — Choose a Destination ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(self.c_accent()))
            .bg(self.c_panel());

        let zone = |key: &'static str, label: &'static str, note: Option<&'static str>, enabled: bool, required_renown: u32| -> Line<'static> {
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
                Line::from(Span::styled(format!("  {key}  {label}  — requires {required_renown} renown"), dim))
            }
        };

        let abyss_label = if renown >= RENOWN_ABYSS { "The Abyss" } else { "????" };
        let abyss_note: Option<&'static str> = if renown >= RENOWN_ABYSS { Some("something stirs below") } else { None };

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled("  Where do you venture?", text_style)),
            Line::from(""),
            zone("[1]", "Top-level Sewers", Some("common rats and ruffians"), true, 0),
            zone("[2]", "Sewer Depths", Some("darker, more dangerous"), renown >= RENOWN_SEWER_DEPTHS, RENOWN_SEWER_DEPTHS),
            zone("[3]", "The Fungal Warrens", Some("bioluminescent caverns below the sewers"), renown >= RENOWN_FUNGAL_WARRENS, RENOWN_FUNGAL_WARRENS),
            zone("[4]", abyss_label, abyss_note, renown >= RENOWN_ABYSS, RENOWN_ABYSS),
            Line::from(Span::styled("  [5]  Follow Clues  — (coming soon)", dim)),
            Line::from(""),
            Line::from(vec![
                "  ".into(),
                Span::styled("[Q]", text_style),
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
                let dim = Style::default().fg(self.c_accent());

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
                    .border_style(Style::default().fg(self.c_accent()))
                    .bg(self.c_panel());

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
                    opt("[O]", "Options", has_char),
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
                let dim = Style::default().fg(self.c_accent());
                let selected_bg = Style::default().bg(self.c_accent()).fg(ratatui::style::Color::White);

                let block = Block::default()
                    .title(Line::from(" Barkeep's Wares ".bold()))
                    .borders(Borders::ALL)
                    .border_set(border::THICK)
                    .border_style(Style::default().fg(self.c_accent()))
                    .bg(self.c_panel());

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
                            Span::styled(format!("  {} gold", entry.cost), Style::default().fg(self.c_alert())),
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
                    Span::styled("[Q]", text_style),
                    " Back".into(),
                ]));

                Paragraph::new(lines)
                    .block(block)
                    .wrap(Wrap { trim: false })
                    .render(area, buf);
            }
            TavernState::Options => {
                let char_name = self.active_character.as_ref().map(|c| c.character.name.as_str()).unwrap_or("—");
                let is_renaming = matches!(self.state, AppState::TextInput(Reason::Rename));

                let block = Block::default()
                    .title(Line::from(" Options ".bold()))
                    .borders(Borders::ALL)
                    .border_set(border::THICK)
                    .border_style(Style::default().fg(self.c_accent()))
                    .bg(self.c_panel());

                let mut lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(format!("  Character: {}", char_name), text_style)),
                    Line::from(""),
                    Line::from(vec![
                        "  ".into(),
                        Span::styled("[R]", text_style),
                        "  Rename character".into(),
                    ]),
                    Line::from(vec![
                        "  ".into(),
                        Span::styled("[C]", text_style),
                        "  Change account".into(),
                    ]),
                    Line::from(vec![
                        "  ".into(),
                        Span::styled("[P]", text_style),
                        format!("  Palette — {}", crate::ui::PALETTES[self.palette_index].name).into(),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        "  ".into(),
                        Span::styled("[Q]", text_style),
                        "  Back".into(),
                    ]),
                ];

                if is_renaming {
                    let input = self.text_input.as_deref().unwrap_or("");
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        "  New name: ".into(),
                        Span::styled(format!("{}_", input), text_style),
                    ]));
                }

                Paragraph::new(lines)
                    .block(block)
                    .wrap(Wrap { trim: false })
                    .render(area, buf);
            }
            TavernState::PaletteSelect { selected } => {
                self.render_palette_select(area, buf, text_style, *selected);
            }
        }
    }

    pub(crate) fn render_palette_select(&self, area: Rect, buf: &mut Buffer, text_style: Style, selected: usize) {
        use ratatui::text::Span;
        let dim = Style::default().fg(self.c_accent());

        let block = ratatui::widgets::Block::default()
            .title(ratatui::text::Line::from(" Choose Palette ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(self.c_accent()))
            .bg(self.c_panel());

        let mut lines = vec![
            ratatui::text::Line::from(""),
            ratatui::text::Line::from(Span::styled("  Palettes from lospec.com/palette-list", dim)),
            ratatui::text::Line::from(""),
        ];

        for (i, pal) in crate::ui::PALETTES.iter().enumerate() {
            let cursor = if i == selected { "▶ " } else { "  " };
            let is_current = i == self.palette_index;
            let swatch = ratatui::text::Line::from(vec![
                Span::raw(format!("{}", cursor)),
                Span::styled("█", Style::default().fg(pal.text)),
                Span::styled("█", Style::default().fg(pal.alert)),
                Span::styled("█", Style::default().fg(pal.accent)),
                Span::styled("█", Style::default().fg(pal.panel)),
                Span::styled("█", Style::default().fg(pal.bg)),
                Span::raw(format!("  {}{}", pal.name, if is_current { "  ←" } else { "" })),
            ]);
            lines.push(swatch);
        }

        lines.push(ratatui::text::Line::from(""));
        lines.push(ratatui::text::Line::from(vec![
            "  ".into(),
            Span::styled("[↑/↓]", text_style),
            " Navigate  ".into(),
            Span::styled("[Enter]", text_style),
            " Apply  ".into(),
            Span::styled("[Q]", text_style),
            " Back".into(),
        ]));

        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}
