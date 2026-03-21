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
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
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
    pub last_combat_damage: Option<i32>,
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
                KeyCode::Char('5') => self.join_quest_from_lobby(4),
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
                KeyCode::Char('5') => self.pick_dialogue_choice(4),
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

    fn sync_quest_state(&mut self) {
        let node = match &self.state {
            AppState::Dialogue { current_node, .. } => Some(current_node.clone()),
            _ => None,
        };
        if let Some(q) = &self.active_quest {
            let node_ref = node.as_deref();
            let _ = self.client.put_encounters(q.id, q.current_encounter, node_ref, &q.encounters);
        }
    }

    fn refresh_quest_state(&mut self) {
        let quest_id = match &self.active_quest {
            Some(q) => q.id,
            None => return,
        };
        match self.client.get_quest(quest_id) {
            Ok(updated) => {
                let enc_changed = self.active_quest.as_ref()
                    .map(|q| q.current_encounter != updated.current_encounter)
                    .unwrap_or(false);

                // Detect if the encounter type changed at the same index (e.g. NPC → Combat)
                let enc_type_changed = {
                    let idx = updated.current_encounter as usize;
                    let old_is_npc = self.active_quest.as_ref()
                        .and_then(|q| q.encounters.get(idx))
                        .map(|e| matches!(e, Encounter::NpcEncounter(_)))
                        .unwrap_or(false);
                    let new_is_combat = updated.encounters.get(idx)
                        .map(|e| matches!(e, Encounter::CombatEncounter(_)))
                        .unwrap_or(false);
                    old_is_npc && new_is_combat
                };

                if let Some(q) = self.active_quest.as_mut() {
                    q.current_encounter = updated.current_encounter;
                    q.encounters = updated.encounters;
                    q.current_node_id = updated.current_node_id.clone();
                }
                if enc_changed || enc_type_changed {
                    self.check_current_encounter();
                } else if let Some(node_id) = updated.current_node_id {
                    if let AppState::Dialogue { current_node, .. } = &mut self.state {
                        *current_node = node_id;
                    }
                }
                self.fetch_party_members();
            }
            Err(_) => {
                // Quest is no longer active (completed by another client)
                self.active_quest = None;
                self.party_members.clear();
                self.state = AppState::Main;
                if let Some(user) = &self.active_user {
                    if let Ok(updated) = self.client.get_character(user.id) {
                        self.active_character = Some(updated);
                    }
                }
            }
        }
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
                    let start = self.active_quest.as_ref()
                        .and_then(|q| q.current_node_id.clone())
                        .unwrap_or_else(|| dialogue.start.clone());
                    self.state = AppState::Dialogue { dialogue, current_node: start };
                }
            }
            Some(Encounter::CombatEncounter(_)) => {
                self.last_combat_damage = None;
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
                    *current_node = node_id.clone();
                }
                if let Some(q) = self.active_quest.as_mut() {
                    q.current_node_id = Some(node_id);
                }
                self.sync_quest_state();
            }
            (None, Some(outcome)) => {
                self.apply_dialogue_outcome(outcome);
            }
            (None, None) => {
                self.state = AppState::Main;
                if let Some(q) = self.active_quest.as_mut() {
                    q.current_encounter += 1;
                }
                self.sync_quest_state();
            }
        }
    }

    fn apply_dialogue_outcome(&mut self, outcome: DialogueOutcome) {
        self.state = AppState::Main;
        match outcome {
            DialogueOutcome::Reward { coins, experience, heal } => {
                if let Some(c) = self.active_character.as_mut() {
                    c.character.coins = (c.character.coins as i32 + coins).max(0) as u32;
                    c.character.experience = (c.character.experience as i32 + experience).max(0) as u32;
                    if heal != 0 {
                        c.unit.health = (c.unit.health + heal).clamp(0, c.unit.max_health);
                    }
                }
                if let Some(q) = self.active_quest.as_mut() {
                    q.current_encounter += 1;
                }
            }
            DialogueOutcome::Damage { amount } => {
                if let Some(c) = self.active_character.as_mut() {
                    c.unit.health = (c.unit.health - amount).max(0);
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
            DialogueOutcome::Combat(combat) => {
                let idx = self.active_quest.as_ref().map(|q| q.current_encounter as usize).unwrap_or(0);
                if let Some(q) = self.active_quest.as_mut() {
                    if let Some(enc) = q.encounters.get_mut(idx) {
                        *enc = Encounter::CombatEncounter(combat);
                    }
                    q.current_node_id = None;
                }
                self.state = AppState::Combat; // set before sync so node_id is sent as null
                self.sync_quest_state();
                return;
            }
            DialogueOutcome::Escape => {}
        }
        self.sync_quest_state();
        self.check_current_encounter();
    }

    fn attack_first_enemy(&mut self, damage: i32) {
        // Phase 1: player attacks, count surviving monsters
        let (encounter_cleared, monsters_alive) = {
            let quest = match self.active_quest.as_mut() {
                Some(q) => q,
                None => return,
            };
            let idx = quest.current_encounter as usize;
            let combat = match quest.encounters.get_mut(idx) {
                Some(Encounter::CombatEncounter(c)) => c,
                _ => return,
            };
            if let Some(target) = combat.monsters.iter_mut().find(|m| m.health > 0) {
                target.health = (target.health - damage).max(0);
            }
            let alive = combat.monsters.iter().filter(|m| m.health > 0).count();
            combat.turn += 1;
            if alive == 0 {
                quest.current_encounter += 1;
            }
            (alive == 0, alive)
        };

        // Phase 2: monster retaliation
        if !encounter_cleared && monsters_alive > 0 {
            let monster_damage = monsters_alive as i32 * 3;
            self.last_combat_damage = Some(monster_damage);
            if let Some(c) = self.active_character.as_mut() {
                c.unit.health = (c.unit.health - monster_damage).max(0);
            }
            if let (Some(user), Some(c)) = (&self.active_user, &self.active_character) {
                let uid = user.id;
                let unit = c.unit;
                let _ = self.client.update_character_unit(uid, &unit);
            }
        } else if encounter_cleared {
            self.last_combat_damage = None;
        }

        // push updated encounter state to backend
        if let Some(q) = &self.active_quest {
            let _ = self.client.put_encounters(q.id, q.current_encounter, None, &q.encounters);
        }

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
            .constraints([Constraint::Min(4), Constraint::Min(6)])
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

        let block = Block::default()
            .title(Line::from(" Character ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let mut lines: Vec<Line> = Vec::new();

        if let Some(user) = &self.active_user {
            lines.push(Line::from(vec![
                "User: ".into(),
                Span::styled(&user.username, text_style),
            ]));
        }

        if let Some(c) = &self.active_character {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(vec![
                "Name:  ".into(),
                Span::styled(&c.character.name, text_style),
            ]));
            lines.push(Line::from(vec![
                "HP:    ".into(),
                Span::styled(c.unit.health.to_string(), text_style),
                "/".into(),
                Span::styled(c.unit.max_health.to_string(), text_style),
            ]));
            lines.push(Line::from(vec![
                "EP:    ".into(),
                Span::styled(c.unit.energy.to_string(), text_style),
                "/".into(),
                Span::styled(c.unit.max_energy.to_string(), text_style),
            ]));
            lines.push(Line::from(vec![
                "Gold:  ".into(),
                Span::styled(c.character.coins.to_string(), text_style),
            ]));
            lines.push(Line::from(vec![
                "Exp:   ".into(),
                Span::styled(c.character.experience.to_string(), text_style),
            ]));
        }

        if let Some(quest) = &self.active_quest {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            let enc_type = match quest.encounters.get(quest.current_encounter as usize) {
                Some(Encounter::CombatEncounter(_)) => "Combat",
                Some(Encounter::NpcEncounter(_)) => "NPC",
                _ => "—",
            };
            lines.push(Line::from(vec![
                "Quest: #".into(),
                Span::styled(quest.current_encounter.to_string(), text_style),
                " | ".into(),
                enc_type.into(),
            ]));
        }

        Paragraph::new(lines)
            .block(block)
            .bg(Color::Rgb(116, 86, 116))
            .render(area, buf);
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

        block.render(area, buf);
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
            .wrap(Wrap { trim: false })
            .bg(Color::Rgb(60, 50, 80))
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

        let mut lines = vec![
            Line::from(vec![
                " Turn ".into(),
                Span::styled(combat.turn.to_string(), text_style),
            ]),
            Line::from(""),
            Line::from(" Enemies:".bold()),
            Line::from(""),
        ];

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
        if let Some(dmg) = self.last_combat_damage {
            lines.push(Line::from(vec![
                " Monsters dealt ".into(),
                Span::styled(dmg.to_string(), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                " damage!".into(),
            ]));
        }
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
