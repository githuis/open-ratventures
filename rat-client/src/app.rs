use color_eyre::{Result, eyre::WrapErr};
use ratatui::{
    Frame,
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};
use ratback::{
    data::{CharacterWrapper, User},
    quest_data::{Dialogue, DialogueOutcome, Encounter, Quest},
};

use crate::client::Rattp;
use crate::tui;

#[derive(Debug, Default)]
pub struct App {
    pub exit: bool,
    pub state: AppState,
    pub active_user: Option<User>,
    pub active_character: Option<CharacterWrapper>,
    pub active_quest: Option<Quest>,
    pub text_input: Option<String>,
    pub client: Rattp,
}

#[derive(Debug, Default)]
pub enum AppState {
    #[default]
    Main,
    TextInput(Reason),
    FinishInput(Reason),
    Party,
    Combat,
    Dialogue { dialogue: Dialogue, current_node: String },
}

#[derive(Debug, Default)]
pub enum Reason {
    #[default]
    Register,
    CreateCharacter,
}

impl App {
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

    fn handle_events(&mut self) -> Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => self
                .handle_key_event(key_event)
                .wrap_err_with(|| format!("handling key event failed:\n{key_event:#?}")),
            _ => Ok(()),
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        match &self.state {
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

            AppState::Dialogue { .. } => match key_event.code {
                KeyCode::Char('1') => self.pick_dialogue_choice(0),
                KeyCode::Char('2') => self.pick_dialogue_choice(1),
                KeyCode::Char('3') => self.pick_dialogue_choice(2),
                KeyCode::Char('4') => self.pick_dialogue_choice(3),
                _ => {}
            },

            _ => match key_event.code {
                KeyCode::Char('q') => self.exit(),
                KeyCode::Char('r') => self.start_register_user(),
                KeyCode::Char('c') => self.register_character(),
                KeyCode::Char('a') => self.start_quest(),
                KeyCode::Char('f') => self.attack_first_enemy(5),
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
        self.active_character = match self
            .client
            .post_new_character(&self.active_user.as_mut().unwrap().id)
        {
            Ok(new_char) => {
                print!("Found char");
                Some(new_char)
            }
            _ => None,
        };
    }

    fn start_quest(&mut self) {
        if let Some(user) = &self.active_user {
            self.active_quest = match self.client.post_new_quest(user.id) {
                Ok(new_q) => Some(new_q),
                _ => None,
            };
            self.check_current_encounter();
        }
    }

    fn check_current_encounter(&mut self) {
        let dialogue_id = match &self.active_quest {
            Some(q) => match q.encounters.get(q.current_encounter as usize) {
                Some(Encounter::NpcEncounter(id)) => Some(id.clone()),
                _ => None,
            },
            None => None,
        };
        if let Some(id) = dialogue_id {
            if let Ok(dialogue) = self.client.get_dialogue(&id) {
                let start = dialogue.start.clone();
                self.state = AppState::Dialogue { dialogue, current_node: start };
            }
        }
    }

    fn pick_dialogue_choice(&mut self, index: usize) {
        let (next, outcome) = match &self.state {
            AppState::Dialogue { dialogue, current_node } => {
                match dialogue.nodes.get(current_node) {
                    Some(node) => match node.choices.get(index) {
                        Some(choice) => (choice.next.clone(), choice.outcome.clone()),
                        None => return,
                    },
                    None => return,
                }
            }
            _ => return,
        };

        match (next, outcome) {
            (Some(node_id), _) => {
                if let AppState::Dialogue { current_node, .. } = &mut self.state {
                    *current_node = node_id;
                }
            }
            (None, Some(outcome)) => {
                self.apply_dialogue_outcome(outcome);
            }
            (None, None) => {
                self.state = AppState::Main;
                if let Some(q) = self.active_quest.as_mut() {
                    q.current_encounter += 1;
                }
            }
        }
    }

    fn apply_dialogue_outcome(&mut self, outcome: DialogueOutcome) {
        self.state = AppState::Main;
        match outcome {
            DialogueOutcome::Reward { coins, experience } => {
                if let Some(c) = self.active_character.as_mut() {
                    c.character.coins += coins;
                    c.character.experience += experience;
                }
                if let Some(q) = self.active_quest.as_mut() {
                    q.current_encounter += 1;
                }
            }
            DialogueOutcome::NextEncounter => {
                if let Some(q) = self.active_quest.as_mut() {
                    q.current_encounter += 1;
                }
            }
            DialogueOutcome::Combat => {
                // TODO: spawn combat encounter
            }
            DialogueOutcome::Escape => {}
        }
        self.check_current_encounter();
    }

    fn attack_first_enemy(&mut self, damage: i32) {
        let encounter_cleared = {
            let quest = match self.active_quest.as_mut() {
                Some(q) => q,
                None => return,
            };
            let idx = quest.current_encounter as usize;
            let all_dead = match quest.encounters.get_mut(idx) {
                Some(Encounter::CombatEncounter(c)) => {
                    if let Some(target) = c.monsters.iter_mut().find(|m| m.health > 0) {
                        target.health = (target.health - damage).max(0);
                    }
                    c.monsters.iter().all(|m| m.health <= 0)
                }
                _ => return,
            };
            if all_dead {
                quest.current_encounter += 1;
            }
            all_dead
        };
        if encounter_cleared {
            self.check_current_encounter();
        }
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let text_style = Style::default()
            .fg(Color::Rgb(247, 255, 174))
            .add_modifier(Modifier::BOLD);

        let parentLayout = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints(vec![Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        match &self.state {
            AppState::TextInput(_) => {
                let rect = Rect::new(40, 15, 100, 3);
                self.render_main(area, buf, text_style);
                self.render_left_panel(parentLayout[0], buf);
                self.render_input(rect, buf, text_style);
            }
            AppState::Dialogue { dialogue, current_node } => {
                self.render_main(area, buf, text_style);
                self.render_left_panel(parentLayout[0], buf);
                self.render_dialogue(parentLayout[1], buf, text_style, dialogue, current_node);
            }
            _ => {
                self.render_main(area, buf, text_style);
                self.render_left_panel(parentLayout[0], buf);
            }
        }
    }
}

impl App {
    fn render_left_panel(&self, area: Rect, buf: &mut Buffer) {
        let text_style = Style::default()
            .fg(Color::Rgb(247, 255, 174))
            .add_modifier(Modifier::BOLD);

        let lhsLayout = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![
                Constraint::Min(3),
                Constraint::Min(7),
                Constraint::Min(20),
            ])
            .split(area);

        self.render_user(lhsLayout[0], buf, text_style);
        self.render_stats(lhsLayout[1], buf, text_style);
        self.render_quest(lhsLayout[2], buf, text_style);
    }

    fn render_main(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let title = Line::from(" Open Ratventures ".bold());

        let instructions = Line::from(vec![
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
            .title_bottom(instructions.centered())
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let title = Text::from(vec![Line::from(vec!["Welcome".into()])]);

        Paragraph::new(title)
            .centered()
            .block(block)
            .render(area, buf);
    }

    fn render_stats(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let stats_block = Block::default()
            .title(Line::from(" Stats ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let wrapper = match &self.active_character {
            Some(c) => c,
            None => return,
        };

        let mut health_text = vec![];

        health_text.push(Line::from(vec![
            "Health: ".into(),
            Span::styled(wrapper.unit.health.to_string(), text_style),
            "/".into(),
            Span::styled(wrapper.unit.max_health.to_string(), text_style),
        ]));

        health_text.push(Line::from(vec![
            "Energy: ".into(),
            Span::styled(wrapper.unit.energy.to_string(), text_style),
            "/".into(),
            Span::styled(wrapper.unit.max_energy.to_string(), text_style),
        ]));

        health_text.push(Line::from(vec![
            "Coins: ".into(),
            Span::styled(wrapper.character.coins.to_string(), text_style),
        ]));

        health_text.push(Line::from(vec![
            "Experience: ".into(),
            Span::styled(wrapper.character.experience.to_string(), text_style),
        ]));

        Paragraph::new(health_text)
            .block(stats_block)
            .bg(Color::Rgb(116, 86, 116))
            .render(area, buf);
    }

    fn render_user(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
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

        Paragraph::new(user_text)
            .block(user_block)
            .bg(Color::Rgb(116, 86, 116))
            .render(area, buf);
    }

    fn render_input(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
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

        Paragraph::new(text)
            .block(block)
            .bg(Color::Rgb(116, 86, 116))
            .render(area, buf);
    }

    fn render_dialogue(&self, area: Rect, buf: &mut Buffer, text_style: Style, dialogue: &Dialogue, current_node: &str) {
        let node = match dialogue.nodes.get(current_node) {
            Some(n) => n,
            None => return,
        };

        let block = Block::default()
            .title(Line::from(" Conversation ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(node.text.clone(), text_style)),
            Line::from(""),
        ];

        for (i, choice) in node.choices.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(format!(" [{}] ", i + 1), text_style),
                choice.text.clone().into(),
            ]));
        }

        Paragraph::new(lines)
            .block(block)
            .bg(Color::Rgb(60, 50, 80))
            .render(area, buf);
    }

    fn render_quest(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let block = Block::default()
            .title(Line::from(" Quest: ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let quest = match &self.active_quest {
            Some(q) => q,
            None => return,
        };

        let mut lines = vec![Line::from(format!(" Encounter: {}", quest.current_encounter))];

        match quest.encounters.get(quest.current_encounter as usize) {
            Some(Encounter::CombatEncounter(combat)) => {
                lines.push(Line::from(" Combat!".bold()));
                for (i, monster) in combat.monsters.iter().enumerate() {
                    lines.push(Line::from(format!(
                        "  Enemy {}: {}/{} hp  {}/{} ep",
                        i + 1,
                        monster.health,
                        monster.max_health,
                        monster.energy,
                        monster.max_energy
                    )));
                }
            }
            Some(Encounter::NpcEncounter(_)) => {
                lines.push(Line::from(" NPC Encounter"));
            }
            _ => {}
        }

        let text = Text::from(lines);

        Paragraph::new(text)
            .block(block)
            .bg(Color::Rgb(116, 86, 116))
            .render(area, buf);
    }
}
