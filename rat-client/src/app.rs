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
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};
use ratback::{
    data::{CharacterWrapper, InventoryItem, ItemEffect, User},
    quest_data::{Dialogue, DialogueOutcome, Encounter, Quest, QuestSummary},
};

use crate::client::Rattp;
use crate::tui;

// Palette
// #fbbbad — warm salmon: primary highlighted/styled text
// #ee8695 — rose pink:   damage, warnings, enemy info, alerts
// #4a7a96 — muted blue:  borders, labels, secondary UI chrome
// #333f58 — dark slate:  panel backgrounds (character, party, lobby)
// #292831 — near-black:  main window background, combat/dialogue
const C_TEXT: Color    = Color::Rgb(251, 187, 173); // #fbbbad
const C_ALERT: Color   = Color::Rgb(238, 134, 149); // #ee8695
const C_ACCENT: Color  = Color::Rgb(74, 122, 150);  // #4a7a96
const C_PANEL: Color   = Color::Rgb(51, 63, 88);    // #333f58
const C_BG: Color      = Color::Rgb(41, 40, 49);    // #292831

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
    pub inventory: Vec<InventoryItem>,
}

#[derive(Debug, Default)]
pub enum TavernState {
    #[default]
    Main,
    Shop,
}

#[derive(Debug)]
pub enum AppState {
    Tavern(TavernState),
    Main,
    TextInput(Reason),
    FinishInput(Reason),
    Party,
    Combat,
    Dialogue { dialogue: Dialogue, current_node: String },
    QuestLobby { quests: Vec<QuestSummary> },
    Inventory { scroll: usize, previous: Box<AppState> },
}

impl Default for AppState {
    fn default() -> Self {
        AppState::Tavern(TavernState::Main)
    }
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
        if matches!(&self.state, AppState::Inventory { .. }) {
            if let AppState::Inventory { scroll, previous } =
                std::mem::replace(&mut self.state, AppState::Main)
            {
                match key_event.code {
                    KeyCode::Char('j') | KeyCode::Char('s') | KeyCode::Down => {
                        self.state = AppState::Inventory { scroll: scroll.saturating_add(1), previous };
                    }
                    KeyCode::Char('k') | KeyCode::Char('w') | KeyCode::Up => {
                        self.state = AppState::Inventory { scroll: scroll.saturating_sub(1), previous };
                    }
                    KeyCode::Char('v') | KeyCode::Esc | KeyCode::Char('q') => {
                        self.state = *previous;
                    }
                    _ => {
                        self.state = AppState::Inventory { scroll, previous };
                    }
                }
            }
            return Ok(());
        }

        match &self.state {
            AppState::Tavern(TavernState::Main) => {
                let has_char = self.active_character.is_some();
                match key_event.code {
                    KeyCode::Char('s') if has_char => self.state = AppState::Tavern(TavernState::Shop),
                    KeyCode::Char('a') if has_char => self.start_quest(),
                    KeyCode::Char('o') => self.start_register_user(),
                    KeyCode::Char('v') => self.open_inventory(),
                    KeyCode::Char('q') => self.exit(),
                    _ => {}
                }
            }

            AppState::Tavern(TavernState::Shop) => match key_event.code {
                KeyCode::Char('1') => self.tavern_buy_item("Gem of Resurrection", 5),
                KeyCode::Esc | KeyCode::Char('q') => self.state = AppState::Tavern(TavernState::Main),
                _ => {}
            },

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
                KeyCode::Esc => self.state = AppState::Tavern(TavernState::Main),
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
                KeyCode::Esc => self.state = AppState::Tavern(TavernState::Main),
                _ => {}
            },

            AppState::Combat => match key_event.code {
                KeyCode::Char('f') => self.attack_first_enemy(5),
                KeyCode::Char('1') => self.use_item(0),
                KeyCode::Char('2') => self.use_item(1),
                KeyCode::Char('3') => self.use_item(2),
                KeyCode::Char('4') => self.use_item(3),
                KeyCode::Char('5') => self.use_item(4),
                KeyCode::Char('v') => self.open_inventory(),
                KeyCode::Char('q') => self.exit(),
                _ => {}
            },

            AppState::Dialogue { .. } => match key_event.code {
                KeyCode::Char('1') => self.pick_dialogue_choice(0),
                KeyCode::Char('2') => self.pick_dialogue_choice(1),
                KeyCode::Char('3') => self.pick_dialogue_choice(2),
                KeyCode::Char('4') => self.pick_dialogue_choice(3),
                KeyCode::Char('5') => self.pick_dialogue_choice(4),
                KeyCode::Char('v') => self.open_inventory(),
                KeyCode::Char('q') => self.exit(),
                _ => {}
            },

            _ => match key_event.code {
                KeyCode::Char('q') => self.exit(),
                KeyCode::Char('r') => self.start_register_user(),
                KeyCode::Char('c') => self.register_character(),
                KeyCode::Char('a') => self.start_quest(),
                KeyCode::Char('f') => self.attack_first_enemy(5),
                KeyCode::Char('v') => self.open_inventory(),
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
            AppState::Main | AppState::Tavern(_) => match why {
                Some(reason) => {
                    self.text_input = Some("".to_string());
                    AppState::TextInput(reason)
                }
                None => AppState::TextInput(Reason::Register),
            },
            _ => AppState::Tavern(TavernState::Main),
        };
    }

    fn tavern_buy_item(&mut self, item_name: &str, cost: u32) {
        let coins = self.active_character.as_ref().map(|c| c.character.coins).unwrap_or(0);
        if coins < cost {
            return;
        }
        if let (Some(user), Some(c)) = (&self.active_user, &mut self.active_character) {
            let user_id = user.id;
            c.character.coins -= cost;
            if self.client.post_give_item(user_id, item_name).is_ok() {
                self.inventory = self.client.get_character_items(user_id).unwrap_or_default();
            }
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn open_inventory(&mut self) {
        let previous = std::mem::replace(&mut self.state, AppState::Main);
        self.state = AppState::Inventory { scroll: 0, previous: Box::new(previous) };
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
        if let Some(user) = &self.active_user {
            let user_id = user.id;
            self.active_character = self.client.post_new_character(&user_id).ok();
            self.inventory = self.client.get_character_items(user_id).unwrap_or_default();
            self.state = AppState::Tavern(TavernState::Main);
        }
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
        if let Some(user) = &self.active_user {
            self.inventory = self.client.get_character_items(user.id).unwrap_or_default();
        }
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
                self.state = AppState::Tavern(TavernState::Main);
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
                        .filter(|node| dialogue.nodes.contains_key(node.as_str()))
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
        self.state = AppState::Tavern(TavernState::Main);
    }

    fn pick_dialogue_choice(&mut self, index: usize) {
        let (next, outcome) = match &self.state {
            AppState::Dialogue { dialogue, current_node } => {
                match dialogue.nodes.get(current_node) {
                    Some(node) => match node.choices.get(index) {
                        Some(choice) => {
                            let coins = self.active_character.as_ref()
                                .map(|c| c.character.coins as i32)
                                .unwrap_or(0);
                            // block if this choice is locked (direct outcome or read-ahead)
                            let locked = match &choice.outcome {
                                Some(DialogueOutcome::GiveItem { cost, .. }) => coins < *cost,
                                None => {
                                    if let Some(next_id) = &choice.next {
                                        if let Some(next_node) = dialogue.nodes.get(next_id.as_str()) {
                                            !next_node.choices.is_empty()
                                                && next_node.choices.iter().all(|c| match &c.outcome {
                                                    Some(DialogueOutcome::GiveItem { cost, .. }) => coins < *cost,
                                                    _ => false,
                                                })
                                        } else { false }
                                    } else { false }
                                }
                                _ => false,
                            };
                            if locked { return; }
                            (choice.next.clone(), choice.outcome.clone())
                        }
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
                self.state = AppState::Tavern(TavernState::Main);
                if let Some(q) = self.active_quest.as_mut() {
                    q.current_encounter += 1;
                }
                self.sync_quest_state();
            }
        }
    }

    fn apply_dialogue_outcome(&mut self, outcome: DialogueOutcome) {
        self.state = AppState::Tavern(TavernState::Main);
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
            DialogueOutcome::GiveItem { item_name, cost } => {
                if let Some(c) = self.active_character.as_mut() {
                    c.character.coins = (c.character.coins as i32 - cost).max(0) as u32;
                }
                if let Some(user) = &self.active_user {
                    let uid = user.id;
                    let _ = self.client.post_give_item(uid, &item_name);
                    self.inventory = self.client.get_character_items(uid).unwrap_or_default();
                }
                if let Some(q) = self.active_quest.as_mut() {
                    q.current_encounter += 1;
                }
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

    fn use_item(&mut self, index: usize) {
        let inv_item = match self.inventory.get(index) {
            Some(i) => i.clone(),
            None => return,
        };

        let mut encounter_cleared = false;
        match &inv_item.item.effect {
            ItemEffect::Damage(dmg) => {
                let dmg = *dmg;
                if let Some(quest) = self.active_quest.as_mut() {
                    let idx = quest.current_encounter as usize;
                    if let Some(Encounter::CombatEncounter(c)) = quest.encounters.get_mut(idx) {
                        if let Some(target) = c.monsters.iter_mut().find(|m| m.health > 0) {
                            target.health = (target.health - dmg).max(0);
                        }
                        if c.monsters.iter().all(|m| m.health <= 0) {
                            quest.current_encounter += 1;
                            encounter_cleared = true;
                        }
                    }
                }
            }
            ItemEffect::Heal(heal) => {
                let heal = *heal;
                if let Some(c) = self.active_character.as_mut() {
                    c.unit.health = (c.unit.health + heal).clamp(0, c.unit.max_health);
                }
                if let (Some(user), Some(c)) = (&self.active_user, &self.active_character) {
                    let uid = user.id;
                    let unit = c.unit;
                    let _ = self.client.update_character_unit(uid, &unit);
                }
            }
            ItemEffect::FullHeal => {
                if let Some(c) = self.active_character.as_mut() {
                    c.unit.health = c.unit.max_health;
                }
                if let (Some(user), Some(c)) = (&self.active_user, &self.active_character) {
                    let uid = user.id;
                    let unit = c.unit;
                    let _ = self.client.update_character_unit(uid, &unit);
                }
            }
        }

        if inv_item.item.consumable {
            if let Some(user) = &self.active_user {
                let _ = self.client.delete_character_item(user.id, inv_item.item.id);
            }
            self.inventory.remove(index);
        }

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
            .fg(C_TEXT)
            .add_modifier(Modifier::BOLD);

        let parent_layout = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        let left_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Min(6)])
            .split(parent_layout[0]);

        self.render_main(area, buf, text_style);
        self.render_left_panel(left_layout[0], buf);
        self.render_party(left_layout[1], buf, text_style);

        match &self.state {
            AppState::Tavern(sub) => {
                self.render_tavern(parent_layout[1], buf, text_style, sub);
            }
            AppState::TextInput(_) => {
                let popup_width = 100.min(area.width.saturating_sub(4));
                let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
                let popup_y = area.y + area.height / 2;
                let rect = Rect::new(popup_x, popup_y, popup_width, 3);
                Clear::default().render(rect, buf);
                self.render_input(rect, buf, text_style);
            }
            AppState::Dialogue { dialogue, current_node } => {
                self.render_dialogue(parent_layout[1], buf, text_style, dialogue, current_node);
            }
            AppState::Combat => {
                self.render_combat(parent_layout[1], buf, text_style);
            }
            AppState::QuestLobby { quests } => {
                self.render_quest_lobby(parent_layout[1], buf, text_style, quests);
            }
            AppState::Inventory { scroll, .. } => {
                let popup_width = 60.min(area.width.saturating_sub(4));
                let popup_height = 20.min(area.height.saturating_sub(4));
                let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
                let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
                let rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
                Clear::default().render(rect, buf);
                self.render_inventory_popup(rect, buf, text_style, *scroll);
            }
            _ => {}
        }
    }
}

impl App {
    fn render_left_panel(&self, area: Rect, buf: &mut Buffer) {
        let text_style = Style::default()
            .fg(C_TEXT)
            .add_modifier(Modifier::BOLD);

        let block = Block::default()
            .title(Line::from(" Character ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT));

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

        if !self.inventory.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(" Items:".bold()));
            for inv in &self.inventory {
                let effect_str = match &inv.item.effect {
                    ItemEffect::Damage(d) => format!("{}dmg", d),
                    ItemEffect::Heal(h) => format!("heal {}", h),
                    ItemEffect::FullHeal => "full heal".to_string(),
                };
                lines.push(Line::from(format!("  {} x{} ({})", inv.item.name, inv.quantity, effect_str)));
            }
        }

        Paragraph::new(lines)
            .block(block)
            .bg(C_PANEL)
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
            .bg(C_BG);

        block.render(area, buf);
    }

    fn render_tavern(&self, area: Rect, buf: &mut Buffer, text_style: Style, sub: &TavernState) {
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
                    Line::from(vec![
                        "  ".into(),
                        Span::styled("[O]", text_style),
                        "  Options — change character".into(),
                    ]),
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

    fn render_input(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let block = Block::default()
            .title(Line::from(
                " Input username - Enter to Finish, Esc to stop ".bold(),
            ))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT));

        let current_text = match &self.text_input {
            Some(x) => Line::from(vec![Span::styled(x, text_style)]),
            None => Line::from(vec!["Type a username".into()]),
        };
        let text = Text::from(vec![current_text]);

        Paragraph::new(text)
            .block(block)
            .bg(C_PANEL)
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
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT));

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(node.text.clone(), text_style)),
            Line::from(""),
        ];

        let coins = self.active_character.as_ref().map(|c| c.character.coins as i32).unwrap_or(0);

        let choice_locked = |choice: &ratback::quest_data::DialogueChoice| -> bool {
            match &choice.outcome {
                Some(DialogueOutcome::GiveItem { cost, .. }) => coins < *cost,
                None => {
                    // read ahead: if every choice in the next node is locked, this path is a dead end
                    if let Some(next_id) = &choice.next {
                        if let Some(next_node) = dialogue.nodes.get(next_id.as_str()) {
                            !next_node.choices.is_empty()
                                && next_node.choices.iter().all(|c| match &c.outcome {
                                    Some(DialogueOutcome::GiveItem { cost, .. }) => coins < *cost,
                                    _ => false,
                                })
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            }
        };

        for (i, choice) in node.choices.iter().enumerate() {
            let affordable = !choice_locked(choice);
            if affordable {
                lines.push(Line::from(vec![
                    Span::styled(format!(" [{}] ", i + 1), text_style),
                    choice.text.clone().into(),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!(" [{}] {} (not enough gold)", i + 1, choice.text),
                        Style::default().fg(C_ACCENT),
                    ),
                ]));
            }
        }

        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .bg(C_BG)
            .render(area, buf);
    }


    fn render_quest_lobby(&self, area: Rect, buf: &mut Buffer, text_style: Style, quests: &[QuestSummary]) {
        let block = Block::default()
            .title(Line::from(" Quest Lobby ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT));

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
            .bg(C_PANEL)
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
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT));

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
                Span::styled(dmg.to_string(), Style::default().fg(C_ALERT).add_modifier(Modifier::BOLD)),
                " damage!".into(),
            ]));
        }
        lines.push(Line::from(vec![
            " [F] Attack — ".into(),
            Span::styled("5 dmg to first living enemy", text_style),
        ]));

        if !self.inventory.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(" Items:".bold()));
            for (i, inv) in self.inventory.iter().enumerate() {
                let effect_str = match &inv.item.effect {
                    ItemEffect::FullHeal => "full heal".to_string(),
                    ItemEffect::Damage(d) => format!("{} dmg", d),
                    ItemEffect::Heal(h) => format!("heal {}", h),
                };
                lines.push(Line::from(vec![
                    format!(" [{}] {} (x{}) — ", i + 1, inv.item.name, inv.quantity).into(),
                    Span::styled(effect_str, text_style),
                ]));
            }
        }

        Paragraph::new(lines)
            .block(block)
            .bg(C_BG)
            .render(area, buf);
    }

    fn render_party(&self, area: Rect, buf: &mut Buffer, text_style: Style) {
        let block = Block::default()
            .title(Line::from(" Party ".bold()))
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT));

        let my_char_id = self.active_character.as_ref().map(|c| c.character.id);
        let characters: Vec<&CharacterWrapper> = self.party_members.iter()
            .filter(|c| Some(c.character.id) != my_char_id)
            .collect();

        let bg = C_PANEL;

        if characters.is_empty() {
            Paragraph::new(" No other party members")
                .block(block)
                .bg(bg)
                .render(area, buf);
            return;
        }

        let inner = block.inner(area);
        // fill background so uncovered space matches cards
        buf.set_style(inner, Style::default().bg(bg));
        block.bg(bg).render(area, buf);

        let card_height = (inner.height / characters.len() as u16).max(1);
        for (i, c) in characters.iter().enumerate() {
            let card_area = Rect::new(
                inner.x,
                inner.y + i as u16 * card_height,
                inner.width,
                card_height,
            );
            let card_block = Block::default()
                .title(Line::from(format!(" {} ", c.character.name)))
                .borders(Borders::ALL)
                .border_set(border::PLAIN)
                .border_style(Style::default().fg(C_ACCENT));

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
                .bg(C_PANEL)
                .render(card_area, buf);
        }
    }

    fn render_inventory_popup(&self, area: Rect, buf: &mut Buffer, text_style: Style, scroll: usize) {
        let block = Block::default()
            .title(Line::from(" Inventory ").centered())
            .title_bottom(Line::from(" [V/Esc] Close  [J/K] Scroll ").centered())
            .borders(Borders::ALL)
            .border_set(border::THICK)
            .border_style(Style::default().fg(C_ACCENT))
            .bg(C_PANEL);

        let inner = block.inner(area);
        block.render(area, buf);

        if self.inventory.is_empty() {
            Paragraph::new(Line::from(vec![
                Span::styled(" Your pack is empty.", text_style),
            ]))
            .bg(C_PANEL)
            .render(inner, buf);
            return;
        }

        let mut lines: Vec<Line> = vec![Line::from("")];
        for inv in &self.inventory {
            let effect_str = match &inv.item.effect {
                ItemEffect::Damage(d) => format!("{} dmg", d),
                ItemEffect::Heal(h) => format!("heal {}", h),
                ItemEffect::FullHeal => "full heal".to_string(),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", inv.item.name), text_style),
                format!("x{}  ", inv.quantity).into(),
                Span::styled(format!("[{}]", effect_str), Style::default().fg(C_ACCENT)),
            ]));
            lines.push(Line::from(vec![
                "    ".into(),
                Span::raw(inv.item.description.clone()),
            ]));
            lines.push(Line::from(""));
        }

        Paragraph::new(lines)
            .scroll((scroll as u16, 0))
            .wrap(Wrap { trim: false })
            .bg(C_PANEL)
            .render(inner, buf);
    }
}
