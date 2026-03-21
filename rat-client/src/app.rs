use color_eyre::{Result, eyre::WrapErr};
use std::time::{Duration, Instant};
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
    quest_data::{Dialogue, DialogueOutcome, Encounter, Quest, QuestSummary},
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
    pub party_members: Vec<CharacterWrapper>,
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
    QuestLobby { quests: Vec<QuestSummary> },
}

#[derive(Debug, Default)]
pub enum Reason {
    #[default]
    Register,
    CreateCharacter,
}

impl App {
    pub fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        const TICK: Duration = Duration::from_millis(100);
        const REFRESH: Duration = Duration::from_secs(3);
        let mut last_refresh = Instant::now();

        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events(TICK).wrap_err("handle events failed")?;

            if last_refresh.elapsed() >= REFRESH {
                self.refresh_quest_state();
                last_refresh = Instant::now();
            }
        }
        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self, timeout: Duration) -> Result<()> {
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => self
                    .handle_key_event(key_event)
                    .wrap_err_with(|| format!("handling key event failed:\n{key_event:#?}")),
                _ => Ok(()),
            }
        } else {
            Ok(())
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

            AppState::QuestLobby { .. } => match key_event.code {
                KeyCode::Char('1') => self.join_quest_from_lobby(0),
                KeyCode::Char('2') => self.join_quest_from_lobby(1),
                KeyCode::Char('3') => self.join_quest_from_lobby(2),
                KeyCode::Char('4') => self.join_quest_from_lobby(3),
                KeyCode::Char('n') => self.create_new_quest(),
                KeyCode::Char('r') | KeyCode::Char('a') => self.start_quest(),
                KeyCode::Char('q') => self.exit(),
                KeyCode::Esc => self.state = AppState::Main,
                _ => {}
            },

            AppState::Dialogue { .. } => match key_event.code {
                KeyCode::Char('1') => self.pick_dialogue_choice(0),
                KeyCode::Char('2') => self.pick_dialogue_choice(1),
                KeyCode::Char('3') => self.pick_dialogue_choice(2),
                KeyCode::Char('4') => self.pick_dialogue_choice(3),
                KeyCode::Char('q') => self.exit(),
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
            Ok(new_char) => Some(new_char),
            _ => None,
        };
    }

    fn start_quest(&mut self) {
        if self.active_user.is_none() {
            return;
        }
        let quests = self.client.get_open_quests().unwrap_or_default();
        self.state = AppState::QuestLobby { quests };
    }

    fn join_quest_from_lobby(&mut self, index: usize) {
        let (quest_id, user_id) = match &self.state {
            AppState::QuestLobby { quests } => match quests.get(index) {
                Some(q) => (q.id, self.active_user.as_ref().map(|u| u.id).unwrap_or(0)),
                None => return,
            },
            _ => return,
        };
        if let Ok(quest) = self.client.post_join_quest(quest_id, user_id) {
            self.active_quest = Some(quest);
            self.fetch_party_members();
            self.state = AppState::Main;
            self.check_current_encounter();
        }
    }

    fn create_new_quest(&mut self) {
        if let Some(user) = &self.active_user {
            let user_id = user.id;
            self.active_quest = match self.client.post_new_quest(user_id) {
                Ok(new_q) => Some(new_q),
                _ => None,
            };
            self.fetch_party_members();
            self.state = AppState::Main;
            self.check_current_encounter();
        }
    }

    fn refresh_quest_state(&mut self) {
        let quest_id = match &self.active_quest {
            Some(q) => q.id,
            None => return,
        };
        if let Ok(updated) = self.client.get_quest(quest_id) {
            if let Some(q) = self.active_quest.as_mut() {
                q.current_encounter = updated.current_encounter;
            }
            self.check_current_encounter();
        }
        self.fetch_party_members();
    }

    fn fetch_party_members(&mut self) {
        if let Some(quest) = &self.active_quest {
            self.party_members = self.client.get_quest_members(quest.id).unwrap_or_default();
        }
    }

    fn check_current_encounter(&mut self) {
        let (enc, quest_done) = match &self.active_quest {
            Some(q) => {
                let idx = q.current_encounter as usize;
                (q.encounters.get(idx).cloned(), idx >= q.encounters.len())
            }
            None => (None, false),
        };

        if quest_done {
            self.complete_quest();
            return;
        }

        match enc {
            Some(Encounter::NpcEncounter(id)) => {
                if let Ok(dialogue) = self.client.get_dialogue(&id) {
                    let start = dialogue.start.clone();
                    self.state = AppState::Dialogue { dialogue, current_node: start };
                }
            }
            Some(Encounter::CombatEncounter(_)) => {
                self.state = AppState::Combat;
            }
            _ => {
                self.state = AppState::Main;
            }
        }
    }

    fn complete_quest(&mut self) {
        let (quest_id, user_id) = match (&self.active_quest, &self.active_user) {
            (Some(q), Some(u)) => (q.id, u.id),
            _ => return,
        };
        if let Ok(updated) = self.client.post_complete_quest(quest_id, user_id) {
            self.active_character = Some(updated);
        }
        self.active_quest = None;
        self.state = AppState::Main;
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

        let parent_layout = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        let right_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
            .split(parent_layout[1]);

        self.render_main(area, buf, text_style);
        self.render_left_panel(parent_layout[0], buf);
        self.render_party(right_layout[1], buf, text_style);

        match &self.state {
            AppState::TextInput(_) => {
                let rect = Rect::new(40, 15, 100, 3);
                self.render_input(rect, buf, text_style);
            }
            AppState::Dialogue { dialogue, current_node } => {
                self.render_dialogue(right_layout[0], buf, text_style, dialogue, current_node);
            }
            AppState::Combat => {
                self.render_combat(right_layout[0], buf, text_style);
            }
            AppState::QuestLobby { quests } => {
                self.render_quest_lobby(right_layout[0], buf, text_style, quests);
            }
            _ => {}
        }
    }
}

impl App {
    fn render_left_panel(&self, area: Rect, buf: &mut Buffer) {
        let text_style = Style::default()
            .fg(Color::Rgb(247, 255, 174))
            .add_modifier(Modifier::BOLD);

        let lhs_layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![
                Constraint::Min(3),
                Constraint::Min(7),
                Constraint::Min(20),
            ])
            .split(area);

        self.render_user(lhs_layout[0], buf, text_style);
        self.render_stats(lhs_layout[1], buf, text_style);
        self.render_quest(lhs_layout[2], buf, text_style);
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

    fn render_quest(&self, area: Rect, buf: &mut Buffer, _text_style: Style) {
        let block = Block::default()
            .title(Line::from(" Quest ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let quest = match &self.active_quest {
            Some(q) => q,
            None => return,
        };

        let enc_type = match quest.encounters.get(quest.current_encounter as usize) {
            Some(Encounter::CombatEncounter(_)) => "Combat",
            Some(Encounter::NpcEncounter(_)) => "NPC",
            _ => "—",
        };

        let lines = vec![Line::from(format!(
            " #{} | {}",
            quest.current_encounter, enc_type
        ))];

        Paragraph::new(lines)
            .block(block)
            .bg(Color::Rgb(116, 86, 116))
            .render(area, buf);
    }

    fn render_quest_lobby(&self, area: Rect, buf: &mut Buffer, text_style: Style, quests: &[QuestSummary]) {
        let block = Block::default()
            .title(Line::from(" Quest Lobby ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let mut lines = vec![Line::from(""), Line::from(" Open quests:".bold()), Line::from("")];

        if quests.is_empty() {
            lines.push(Line::from("  No open quests."));
        } else {
            for (i, q) in quests.iter().enumerate() {
                lines.push(Line::from(vec![
                    Span::styled(format!(" [{}] ", i + 1), text_style),
                    format!("Quest #{} — {} member(s)", q.id, q.member_count).into(),
                ]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" [N] ", text_style),
            "Create new quest".into(),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" [R] ", text_style),
            "Refresh".into(),
        ]));
        lines.push(Line::from(vec![
            Span::styled(" [Esc] ", text_style),
            "Cancel".into(),
        ]));

        Paragraph::new(lines)
            .block(block)
            .bg(Color::Rgb(40, 60, 80))
            .render(area, buf);
    }

    fn render_combat(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let quest = match &self.active_quest {
            Some(q) => q,
            None => return,
        };
        let combat = match quest.encounters.get(quest.current_encounter as usize) {
            Some(Encounter::CombatEncounter(c)) => c,
            _ => return,
        };

        let block = Block::default()
            .title(Line::from(" Combat ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let mut lines = vec![Line::from(""), Line::from(" Enemies:".bold()), Line::from("")];

        for (i, m) in combat.monsters.iter().enumerate() {
            let label = if m.health <= 0 {
                format!("  Enemy {} [DEAD]", i + 1)
            } else {
                format!("  Enemy {}", i + 1)
            };
            lines.push(Line::from(vec![
                label.into(),
                "  ".into(),
                Span::styled(format!("{}/{} hp", m.health, m.max_health), text_style),
                format!("  {}/{} ep", m.energy, m.max_energy).into(),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            " [F] Attack — ".into(),
            Span::styled("5 dmg to first living enemy", text_style),
        ]));

        Paragraph::new(lines)
            .block(block)
            .bg(Color::Rgb(80, 30, 30))
            .render(area, buf);
    }

    fn render_party(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let block = Block::default()
            .title(Line::from(" Party ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let characters: Vec<&CharacterWrapper> = if self.party_members.is_empty() {
            self.active_character.iter().collect()
        } else {
            self.party_members.iter().collect()
        };

        if characters.is_empty() {
            Paragraph::new(" No party members")
                .block(block)
                .bg(Color::Rgb(116, 86, 116))
                .render(area, buf);
            return;
        }

        let inner = block.inner(area);
        block.render(area, buf);

        let card_width = (inner.width / characters.len() as u16).max(1);
        for (i, c) in characters.iter().enumerate() {
            let card_area = Rect::new(
                inner.x + i as u16 * card_width,
                inner.y,
                card_width,
                inner.height,
            );
            let card_block = Block::default()
                .title(Line::from(format!(" {} ", c.character.name)))
                .borders(Borders::ALL)
                .border_set(border::PLAIN);

            let lines = vec![
                Line::from(vec![
                    "HP ".into(),
                    Span::styled(
                        format!("{}/{}", c.unit.health, c.unit.max_health),
                        text_style,
                    ),
                ]),
                Line::from(vec![
                    "EP ".into(),
                    Span::styled(
                        format!("{}/{}", c.unit.energy, c.unit.max_energy),
                        text_style,
                    ),
                ]),
            ];

            Paragraph::new(lines)
                .block(card_block)
                .bg(Color::Rgb(116, 86, 116))
                .render(card_area, buf);
        }
    }
}
