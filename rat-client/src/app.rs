use color_eyre::{Result, eyre::WrapErr};
use std::time::{Duration, Instant};
use ratatui::{
    Frame,
    buffer::Buffer,
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Clear, Widget},
};
use ratback::{
    data::{CharacterWrapper, InventoryItem, ItemEffect, User},
    quest_data::{Dialogue, DialogueOutcome, Encounter, Party, PartySummary, Quest},
};

use crate::client::Rattp;
use crate::tui;
use crate::ui::C_TEXT;

pub(crate) const AREA_SEWERS: &str = "sewers";
pub(crate) const AREA_SEWER_DEPTHS: &str = "sewer_depths";
pub(crate) const AREA_FUNGAL_WARRENS: &str = "fungal_warrens";
pub(crate) const AREA_ABYSS: &str = "abyss";

#[derive(Debug, Default)]
pub struct App {
    pub exit: bool,
    pub state: AppState,
    pub active_user: Option<User>,
    pub active_character: Option<CharacterWrapper>,
    pub active_quest: Option<Quest>,
    pub active_party: Option<Party>,
    pub party_members: Vec<CharacterWrapper>,
    pub text_input: Option<String>,
    pub client: Rattp,
    pub last_combat_damage: Option<(i32, String)>,
    pub inventory: Vec<InventoryItem>,
}

#[derive(Debug, Default)]
pub enum TavernState {
    #[default]
    Main,
    Shop { items: Vec<ratback::data::ShopItem>, selected: usize, scroll: usize },
}

#[derive(Debug)]
pub enum AppState {
    Welcome,
    Tavern(TavernState),
    Main,
    TextInput(Reason),
    FinishInput(Reason),
    Party,
    Combat,
    Dialogue { dialogue: Dialogue, current_node: String },
    PartyLobby { parties: Vec<PartySummary> },
    AdventureMenu,
    Inventory { scroll: usize, selected: usize, previous: Box<AppState> },
}

impl Default for AppState {
    fn default() -> Self {
        AppState::Welcome
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
                // Poll for quest start whenever in a party but not yet on a quest
                if self.active_party.is_some() && self.active_quest.is_none() {
                    self.refresh_party_state();
                } else {
                    self.refresh_quest_state();
                }
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
            if let AppState::Inventory { scroll, selected, previous } =
                std::mem::replace(&mut self.state, AppState::Main)
            {
                let item_count = self.inventory.len();
                const PAGE: usize = 5;
                match key_event.code {
                    KeyCode::Char('j') | KeyCode::Char('s') | KeyCode::Down => {
                        let new_sel = (selected + 1).min(item_count.saturating_sub(1));
                        let new_scroll = if new_sel >= scroll + PAGE { scroll + 1 } else { scroll };
                        self.state = AppState::Inventory { scroll: new_scroll, selected: new_sel, previous };
                    }
                    KeyCode::Char('k') | KeyCode::Char('w') | KeyCode::Up => {
                        let new_sel = selected.saturating_sub(1);
                        let new_scroll = if new_sel < scroll { scroll.saturating_sub(1) } else { scroll };
                        self.state = AppState::Inventory { scroll: new_scroll, selected: new_sel, previous };
                    }
                    KeyCode::Enter => {
                        let can_use = self.inventory.get(selected).map(|i| {
                            match &i.item.effect {
                                ItemEffect::Damage(_) => matches!(*previous, AppState::Combat),
                                ItemEffect::Heal(_) | ItemEffect::FullHeal => true,
                            }
                        }).unwrap_or(false);
                        if can_use {
                            self.state = *previous;
                            self.use_item(selected);
                        } else {
                            self.state = AppState::Inventory { scroll, selected, previous };
                        }
                    }
                    KeyCode::Char('v') | KeyCode::Esc | KeyCode::Char('q') => {
                        self.state = *previous;
                    }
                    _ => {
                        self.state = AppState::Inventory { scroll, selected, previous };
                    }
                }
            }
            return Ok(());
        }

        match &self.state {
            AppState::Welcome => match key_event.code {
                KeyCode::Char('r') => self.start_register_user(),
                KeyCode::Char('q') => self.exit(),
                _ => {}
            },

            AppState::Tavern(TavernState::Main) => {
                let has_char = self.active_character.is_some();
                match key_event.code {
                    KeyCode::Char('s') if has_char => {
                        let items = self.client.get_shop_items().unwrap_or_default();
                        self.state = AppState::Tavern(TavernState::Shop { items, selected: 0, scroll: 0 });
                    }
                    KeyCode::Char('a') if has_char => self.state = AppState::AdventureMenu,
                    KeyCode::Char('g') if has_char => self.open_party(),
                    KeyCode::Char('o') | KeyCode::Char('r') if has_char => self.start_register_user(),
                    KeyCode::Char('v') => self.open_inventory(),
                    KeyCode::Char('q') => self.exit(),
                    _ => {}
                }
            }

            AppState::Tavern(TavernState::Shop { .. }) => {
                let (selected, scroll, item_count) =
                    if let AppState::Tavern(TavernState::Shop { selected, scroll, items }) = &self.state {
                        (*selected, *scroll, items.len())
                    } else { unreachable!() };

                const PAGE: usize = 5;
                match key_event.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        let new_sel = (selected + 1).min(item_count.saturating_sub(1));
                        let new_scroll = if new_sel >= scroll + PAGE { scroll + 1 } else { scroll };
                        if let AppState::Tavern(TavernState::Shop { selected: s, scroll: sc, .. }) = &mut self.state {
                            *s = new_sel; *sc = new_scroll;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        let new_sel = selected.saturating_sub(1);
                        let new_scroll = if new_sel < scroll { scroll.saturating_sub(1) } else { scroll };
                        if let AppState::Tavern(TavernState::Shop { selected: s, scroll: sc, .. }) = &mut self.state {
                            *s = new_sel; *sc = new_scroll;
                        }
                    }
                    KeyCode::Enter => {
                        let (name, cost) =
                            if let AppState::Tavern(TavernState::Shop { items, .. }) = &self.state {
                                items.get(selected).map(|i| (i.item.name.clone(), i.cost as u32))
                            } else { None }.unwrap_or_default();
                        if !name.is_empty() { self.tavern_buy_item(&name, cost); }
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.state = AppState::Tavern(TavernState::Main);
                    }
                    _ => {}
                }
            }

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

            AppState::PartyLobby { .. } => match key_event.code {
                KeyCode::Char('1') => self.join_party_from_lobby(0),
                KeyCode::Char('2') => self.join_party_from_lobby(1),
                KeyCode::Char('3') => self.join_party_from_lobby(2),
                KeyCode::Char('4') => self.join_party_from_lobby(3),
                KeyCode::Char('5') => self.join_party_from_lobby(4),
                KeyCode::Char('n') => self.create_new_party(),
                KeyCode::Char('r') => self.open_party(),
                KeyCode::Char('q') => self.exit(),
                KeyCode::Esc => self.state = AppState::Tavern(TavernState::Main),
                _ => {}
            },

            AppState::Party => match key_event.code {
                KeyCode::Char('l') => self.leave_party(),
                KeyCode::Char('v') => self.open_inventory(),
                KeyCode::Char('q') | KeyCode::Esc => self.state = AppState::Tavern(TavernState::Main),
                _ => {}
            },

            AppState::AdventureMenu => {
                let renown = self.active_character.as_ref().map(|c| c.character.renown).unwrap_or(0);
                match key_event.code {
                    KeyCode::Char('1') => self.start_quest(AREA_SEWERS),
                    KeyCode::Char('2') if renown >= 5 => self.start_quest(AREA_SEWER_DEPTHS),
                    KeyCode::Char('3') if renown >= 10 => self.start_quest(AREA_FUNGAL_WARRENS),
                    KeyCode::Char('4') if renown >= 20 => self.start_quest(AREA_ABYSS),
                    KeyCode::Esc | KeyCode::Char('q') => self.state = AppState::Tavern(TavernState::Main),
                    _ => {}
                }
            }

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
            AppState::Welcome | AppState::Main | AppState::Tavern(_) => match why {
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
        self.state = AppState::Inventory { scroll: 0, selected: 0, previous: Box::new(previous) };
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
            self.active_party = self.client.get_party_for_user(user_id).ok();
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

    fn open_party(&mut self) {
        if self.active_user.is_none() {
            return;
        }
        // If already in a party, go to the waiting room
        if self.active_party.is_some() {
            if let Some(p) = &self.active_party {
                let pid = p.id;
                self.party_members = self.client.get_party_members_for_party(pid).unwrap_or_default();
            }
            self.state = AppState::Party;
        } else {
            let parties = self.client.get_open_parties().unwrap_or_default();
            self.state = AppState::PartyLobby { parties };
        }
    }

    fn join_party_from_lobby(&mut self, index: usize) {
        let (party_id, user_id) = match &self.state {
            AppState::PartyLobby { parties } => match parties.get(index) {
                Some(p) => (p.id, self.active_user.as_ref().map(|u| u.id).unwrap_or(0)),
                None => return,
            },
            _ => return,
        };
        if let Ok(party) = self.client.post_join_party(party_id, user_id) {
            self.active_party = Some(party);
            if let Some(p) = &self.active_party {
                let pid = p.id;
                self.party_members = self.client.get_party_members_for_party(pid).unwrap_or_default();
            }
            self.state = AppState::Party;
        }
    }

    fn create_new_party(&mut self) {
        if let Some(user) = &self.active_user {
            let user_id = user.id;
            if let Ok(party) = self.client.post_create_party(user_id) {
                self.active_party = Some(party);
                if let Some(p) = &self.active_party {
                    let pid = p.id;
                    self.party_members = self.client.get_party_members_for_party(pid).unwrap_or_default();
                }
                self.state = AppState::Party;
            }
        }
    }

    fn leave_party(&mut self) {
        if let Some(user) = &self.active_user {
            let _ = self.client.delete_leave_party(user.id);
        }
        self.active_party = None;
        self.party_members.clear();
        self.state = AppState::Tavern(TavernState::Main);
    }

    fn start_quest(&mut self, area: &str) {
        if let Some(user) = &self.active_user {
            let user_id = user.id;
            self.active_quest = match self.client.post_new_quest(user_id, area) {
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
                if let Some(user) = &self.active_user {
                    if let Ok(updated) = self.client.get_character(user.id) {
                        self.active_character = Some(updated);
                    }
                }
                if self.active_party.is_some() {
                    if let Some(p) = &self.active_party {
                        let pid = p.id;
                        self.party_members = self.client.get_party_members_for_party(pid).unwrap_or_default();
                    }
                    self.state = AppState::Party;
                } else {
                    self.party_members.clear();
                    self.state = AppState::Tavern(TavernState::Main);
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
        if let Some(c) = &self.active_character {
            let _ = self.client.save_character_stats(user_id, c.character.coins, c.character.renown);
        }
        if let Ok(updated) = self.client.post_complete_quest(quest_id, user_id) {
            self.active_character = Some(updated);
        }
        self.active_quest = None;
        if self.active_party.is_some() {
            if let Some(p) = &self.active_party {
                let pid = p.id;
                self.party_members = self.client.get_party_members_for_party(pid).unwrap_or_default();
            }
            self.state = AppState::Party;
        } else {
            self.state = AppState::Tavern(TavernState::Main);
        }
    }

    fn refresh_party_state(&mut self) {
        // Refresh party member list
        if let Some(p) = &self.active_party {
            let pid = p.id;
            self.party_members = self.client.get_party_members_for_party(pid).unwrap_or_default();
        }
        // Check if someone else started a quest for this party
        if let Some(user) = &self.active_user {
            if let Ok(quest) = self.client.get_active_quest_for_user(user.id) {
                self.active_quest = Some(quest);
                self.fetch_party_members();
                self.check_current_encounter();
            }
        }
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
            DialogueOutcome::Reward { coins, renown, heal } => {
                if let Some(c) = self.active_character.as_mut() {
                    c.character.coins = (c.character.coins as i32 + coins).max(0) as u32;
                    c.character.renown = (c.character.renown as i32 + renown).max(0) as u32;
                    if heal != 0 {
                        c.unit.health = (c.unit.health + heal).clamp(0, c.unit.max_health);
                    }
                }
                if let (Some(user), Some(c)) = (&self.active_user, &self.active_character) {
                    let _ = self.client.save_character_stats(user.id, c.character.coins, c.character.renown);
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
            if let Some(target) = combat.monsters.iter_mut().find(|m| m.unit.health > 0) {
                target.unit.health = (target.unit.health - damage).max(0);
            }
            let alive = combat.monsters.iter().filter(|m| m.unit.health > 0).count();
            combat.turn += 1;
            if alive == 0 {
                quest.current_encounter += 1;
            }
            (alive == 0, alive)
        };

        // Phase 2: monster retaliation — each living monster attacks or uses an item
        if !encounter_cleared && monsters_alive > 0 {
            let monster_damage: i32 = {
                let quest = self.active_quest.as_mut().unwrap();
                let idx = quest.current_encounter as usize;
                let combat = match quest.encounters.get_mut(idx) {
                    Some(Encounter::CombatEncounter(c)) => c,
                    _ => return,
                };
                combat.monsters.iter_mut()
                    .filter(|m| m.unit.health > 0)
                    .map(|m| {
                        // find usable item (charges > 0 or infinite)
                        if let Some(item) = m.items.iter_mut().find(|it| it.charges != 0) {
                            let dmg = match item.effect { ratback::data::ItemEffect::Damage(d) => d, _ => m.attack };
                            if item.charges > 0 { item.charges -= 1; }
                            dmg
                        } else {
                            m.attack
                        }
                    })
                    .sum()
            };
            let target_name = self.active_character.as_ref().map(|c| c.character.name.clone()).unwrap_or_default();
            self.last_combat_damage = Some((monster_damage, target_name));
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
                        if let Some(target) = c.monsters.iter_mut().find(|m| m.unit.health > 0) {
                            target.unit.health = (target.unit.health - dmg).max(0);
                        }
                        if c.monsters.iter().all(|m| m.unit.health <= 0) {
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

        // consume one charge and re-sync from server
        if inv_item.item.charges != -1 {
            if let Some(user) = &self.active_user {
                let uid = user.id;
                let _ = self.client.delete_character_item(uid, inv_item.item.id);
                self.inventory = self.client.get_character_items(uid).unwrap_or_default();
            }
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

        if matches!(self.state, AppState::Welcome) {
            self.render_welcome(area, buf, text_style);
            return;
        }

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
        if self.active_character.is_some() {
            self.render_left_panel(left_layout[0], buf);
            self.render_party(left_layout[1], buf, text_style);
        }

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
            AppState::AdventureMenu => {
                self.render_adventure_menu(parent_layout[1], buf, text_style);
            }
            AppState::PartyLobby { parties } => {
                self.render_party_lobby(parent_layout[1], buf, text_style, parties);
            }
            AppState::Party => {
                self.render_party_screen(parent_layout[1], buf, text_style);
            }
            AppState::Inventory { scroll, selected, previous } => {
                let popup_width = 60.min(area.width.saturating_sub(4));
                let popup_height = 20.min(area.height.saturating_sub(4));
                let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
                let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
                let rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
                Clear::default().render(rect, buf);
                let in_combat = matches!(**previous, AppState::Combat);
                self.render_inventory_popup(rect, buf, text_style, *scroll, *selected, in_combat);
            }
            _ => {}
        }
    }
}

