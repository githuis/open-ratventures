use color_eyre::{
    Result,
    eyre::{WrapErr, eyre},
};
use ratatui::{
    Frame,
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};

extern crate ratback;
use ratback::{data::Character, data::User, quest_data::Quest};

use crate::client::Rattp;

mod client;
mod tui;

const MOB_HP: u8 = 5;

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = tui::init()?;
    let app_result = { App { ..App::default() } }.run(&mut terminal);

    if let Err(err) = tui::restore() {
        eprintln!(
            "failed to restore terminal. Run `reset` or restart your terminal to recover: {err}"
        );
    }
    app_result
}

#[derive(Debug, Default)]
pub struct App {
    exit: bool,
    state: AppState,
    active_user: Option<User>,
    active_character: Option<Character>,
    active_quest: Option<Quest>,
    text_input: Option<String>,
    client: Rattp,
}

#[derive(Debug, Default)]
pub enum AppState {
    #[default]
    Main,
    TextInput(Reason),
    FinishInput(Reason),
    Party,
    Combat,
}

#[derive(Debug, Default)]
pub enum Reason {
    #[default]
    Register,
    CreateCharacter,
}

impl App {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events().wrap_err("handle events failed")?;
        }
        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    /// updates the application's state based on user input
    fn handle_events(&mut self) -> Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => self
                .handle_key_event(key_event)
                .wrap_err_with(|| format!("handling key event failed:\n{key_event:#?}")),
            _ => Ok(()),
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        match self.state {
            AppState::TextInput(_) => match key_event.code {
                KeyCode::Enter => self.finish_register_user(),
                KeyCode::Char(value) => match self.text_input.as_mut() {
                    Some(current) => {
                        current.push(value);
                    }
                    _ => {}
                },
                KeyCode::Backspace => match self.text_input.as_mut() {
                    Some(current) => {
                        current.pop();
                    }
                    _ => {}
                },
                KeyCode::Esc => self.state = AppState::Main,
                _ => {}
            },

            _ => match key_event.code {
                KeyCode::Char('q') => self.exit(),
                KeyCode::Char('r') => self.start_register_user(),
                KeyCode::Char('c') => self.register_character(),
                KeyCode::Char('a') => self.start_quest(),
                _ => {}
            },
        }

        Ok(())
    }

    fn get_and_clear_text_input(&mut self) -> Option<String> {
        let value = self.text_input.clone();
        self.text_input = None;
        value
    }

    fn toggle_text_input(&mut self, why: Option<Reason>) {
        self.state = match self.state {
            AppState::Main => match why {
                Some(reason) => {
                    self.text_input = Some("".to_string());
                    AppState::TextInput(reason)
                }
                None => AppState::TextInput(Reason::Register),
            },
            _ => AppState::Main,
        };
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn start_register_user(&mut self) {
        self.toggle_text_input(Some(Reason::Register));
    }

    fn finish_register_user(&mut self) {
        self.toggle_text_input(None);
        self.active_user = match self.get_and_clear_text_input() {
            Some(name) => self.register_user(name),
            _ => None,
        };
    }

    fn register_user(&self, username: String) -> Option<User> {
        match self.client.post_register_user(username) {
            Ok(x) => Some(x),
            _ => None,
        }
    }

    fn register_character(&mut self) {
        self.active_character = match self.client.post_new_character() {
            Ok(new_char) => Some(new_char),
            _ => None,
        };
    }

    fn start_quest(&mut self) {
        self.active_quest = match self.client.post_new_quest() {
            Ok(new_q) => Some(new_q),
            _ => None,
        }
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let text_style = Style::default()
            .fg(Color::Rgb(247, 255, 174))
            .add_modifier(Modifier::BOLD);

        match self.state {
            AppState::TextInput(_) => {
                self.render_input(buf, text_style);
            }
            _ => {
                self.render_main(area, buf, text_style);
                self.render_stats(buf, text_style);
                self.render_user(buf, text_style);
                self.render_quest(buf, text_style);
            }
        }
    }
}

impl App {
    fn render_main(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let title = Line::from(" Open Ratventures ".bold());

        let instructions = Line::from(vec![
            " Register: ".into(),
            Span::styled("<R>", text_style),
            " New Character: ".into(),
            Span::styled("<C>", text_style),
            " Quit: ".into(),
            Span::styled("<Q>", text_style),
        ]);

        let block = Block::default()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let title = Text::from(vec![Line::from(vec!["Welcome".into()])]);

        Paragraph::new(title)
            .centered()
            .block(block)
            //.bg(Color::Rgb(116,86,116))
            .render(area, buf);
    }

    fn render_stats(&self, buf: &mut Buffer, text_style: Style) {
        let stats_block = Block::default()
            .title(Line::from(" Stats ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK);

        // Only Render Stats if you have any stats
        let chr = match &self.active_character {
            Some(c) => c,
            None => return,
        };

        let mut health_text = vec![];

        health_text.push(Line::from(vec![
            "Health: ".into(),
            Span::styled(chr.unit.stats.health.to_string(), text_style),
            "/".into(),
            Span::styled(chr.unit.max_stats.health.to_string(), text_style),
        ]));

        health_text.push(Line::from(vec![
            "Energy: ".into(),
            Span::styled(chr.unit.stats.energy.to_string(), text_style),
            "/".into(),
            Span::styled(chr.unit.max_stats.energy.to_string(), text_style),
        ]));

        health_text.push(Line::from(vec![
            "Coins: ".into(),
            Span::styled(chr.coins.to_string(), text_style),
        ]));

        health_text.push(Line::from(vec![
            "Experience: ".into(),
            Span::styled(chr.experience.to_string(), text_style),
            //Span::styled(self.experience.to_string(), text_style,),
        ]));
        let stats_rect = Rect::new(5, 6, 50, 7);

        Paragraph::new(health_text)
            //.scroll((1,0))
            //.centered()
            .block(stats_block)
            .bg(Color::Rgb(116, 86, 116))
            .render(stats_rect, buf);
    }

    fn render_user(&self, buf: &mut Buffer, text_style: Style) {
        let user_block = Block::default()
            .title(Line::from(" User: ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let current_user = match &self.active_user {
            Some(x) => Line::from(vec![
                "Username: ".into(),
                Span::styled(&x.username, text_style),
            ]),
            None => Line::from(vec!["No active user".into()]),
        };
        let user_text = Text::from(vec![current_user]);
        let user_rect = Rect::new(5, 2, 50, 3);

        Paragraph::new(user_text)
            .block(user_block)
            .bg(Color::Rgb(116, 86, 116))
            .render(user_rect, buf);
    }

    fn render_input(&self, buf: &mut Buffer, text_style: Style) {
        let block = Block::default()
            .title(Line::from(
                " Input username - Enter to Finish, Esc to stop ".bold(),
            ))
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let current_text = match &self.text_input {
            Some(x) => Line::from(vec![Span::styled(x, text_style)]),
            None => Line::from(vec!["Type a username".into()]),
        };
        let text = Text::from(vec![current_text]);
        let rect = Rect::new(40, 15, 100, 3);

        Paragraph::new(text)
            .block(block)
            .bg(Color::Rgb(116, 86, 116))
            .render(rect, buf);
    }

    fn render_quest(&self, buf: &mut Buffer, text_style: Style) {
        let block = Block::default()
            .title(Line::from(" Quest: ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let current_quest = match &self.active_quest {
            Some(x) => Line::from(vec![
                " Quest started! ".into(),
                //Span::styled(& , text_style),
            ]),
            None => return,
        };
        let text = Text::from(vec![current_quest]);
        let user_rect = Rect::new(60, 2, 50, 3);

        Paragraph::new(text)
            .block(block)
            .bg(Color::Rgb(116, 86, 116))
            .render(user_rect, buf);
    }
}
