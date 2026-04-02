use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use ratback_types::quest_data::{MissionState, MissionStatus};

use crate::app::App;
use crate::ui::{C_ACCENT, C_BG, C_PANEL, C_ALERT};

impl App {
    pub(crate) fn render_mission_select(
        &self,
        area: Rect,
        buf: &mut Buffer,
        text_style: Style,
        missions: &[MissionStatus],
        selected: usize,
    ) {
        let block = Block::default()
            .title(Line::from(" Follow Clues ".bold()))
            .title_bottom(Line::from(" [↑/↓] Navigate  [Enter] Begin  [Q] Back ").centered())
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT))
            .bg(C_PANEL);

        let inner = block.inner(area);
        block.render(area, buf);

        let selected_style = Style::default().bg(C_ACCENT).fg(ratatui::style::Color::White).add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(ratatui::style::Color::DarkGray);

        let mut lines = vec![Line::from(""), Line::from(" Missions:".bold()), Line::from("")];

        if missions.is_empty() {
            lines.push(Line::from("  No missions available yet."));
            lines.push(Line::from("  Find clues by talking to NPCs during quests."));
        } else {
            for (i, m) in missions.iter().enumerate() {
                let cursor = if i == selected { "▶ " } else { "  " };
                let (state_label, style) = match m.state {
                    MissionState::Locked => ("[locked]", dim),
                    MissionState::Ready => ("[ready]", Style::default().fg(C_ACCENT)),
                    MissionState::InProgress => ("[in progress]", Style::default().fg(C_ALERT)),
                    MissionState::Complete => ("[complete]", text_style),
                };
                let row_style = if i == selected { selected_style } else { Style::default() };
                lines.push(Line::styled(
                    format!("{}{}", cursor, m.title),
                    row_style,
                ));
                lines.push(Line::from(vec![
                    "    ".into(),
                    Span::styled(state_label, if i == selected { row_style } else { style }),
                ]));
                if i == selected && m.state != MissionState::Locked {
                    lines.push(Line::from(vec![
                        "    ".into(),
                        Span::styled(&m.description, if i == selected { row_style } else { text_style }),
                    ]));
                }
                lines.push(Line::from(""));
            }
        }

        Paragraph::new(lines)
            .wrap(ratatui::widgets::Wrap { trim: false })
            .bg(C_BG)
            .render(inner, buf);
    }

    pub(crate) fn render_victory(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let block = Block::default()
            .title(Line::from(" Victory! ".bold()))
            .title_bottom(Line::from(" [Q] Return to Menu ").centered())
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT))
            .bg(C_PANEL);

        let inner = block.inner(area);
        block.render(area, buf);

        let name = self.active_character.as_ref().map(|c| c.character.name.as_str()).unwrap_or("Adventurer");
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(" The Abyss has been sealed.", text_style)),
            Line::from(""),
            Line::from(format!(" {name} is a legend.", )),
            Line::from(""),
            Line::from(Span::styled(" The sewer underworld will tell stories", text_style)),
            Line::from(Span::styled(" of this day for generations.", text_style)),
            Line::from(""),
        ];

        Paragraph::new(lines)
            .wrap(ratatui::widgets::Wrap { trim: false })
            .bg(C_BG)
            .render(inner, buf);
    }
}
