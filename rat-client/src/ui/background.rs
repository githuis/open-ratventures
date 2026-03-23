use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Widget},
};

use crate::app::App;
use crate::ui::{C_TEXT, C_ACCENT, C_BG};

impl App {
    pub(crate) fn render_main(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let title = Line::from(" Open Ratventures ".bold());

        let _instructions = Line::from(vec![
            " Register: ".into(),
            Span::styled("<R>", text_style),
            " New Character: ".into(),
            Span::styled("<C>", text_style),
            " New Quest: ".into(),
            Span::styled("<A>", text_style),
            " Quit: ".into(),
            Span::styled("<Q> ", text_style),
        ]);

        let block = Block::default()
            .title(title.centered())
            //.title_bottom(instructions.centered())
            .bg(C_BG);

        block.render(area, buf);
    }
}
