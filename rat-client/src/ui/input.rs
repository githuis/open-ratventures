use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::app::App;


impl App {
    pub(crate) fn render_input(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let block = Block::default()
            .title(Line::from(
                " Input username - Enter to Finish, Esc to stop ".bold(),
            ))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(self.c_accent()));

        let current_text = match &self.text_input {
            Some(x) => Line::from(vec![Span::styled(x, text_style)]),
            None => Line::from(vec!["Type a username".into()]),
        };
        let text = Text::from(vec![current_text]);

        Paragraph::new(text)
            .block(block)
            .bg(self.c_panel())
            .render(area, buf);
    }
}
