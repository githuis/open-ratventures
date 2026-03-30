use color_eyre::{Result, eyre::WrapErr};
use std::time::{Duration, Instant};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Clear, Widget},
};
#[cfg(not(target_arch = "wasm32"))]
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
#[cfg(not(target_arch = "wasm32"))]
use futures::StreamExt;
use ratback_types::{
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
    Shop { items: Vec<ratback_types::data::ShopItem>, selected: usize, scroll: usize },
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
    TargetSelect { item_index: usize, target_selected: usize, inv_scroll: usize, inv_item_selected: usize, return_to: Box<AppState> },
    GameOver,
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
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        const REFRESH: Duration = Duration::from_secs(3);
        let mut last_refresh = Instant::now();
        let mut event_reader = EventStream::new();

        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                maybe_event = event_reader.next() => {
                    if let Some(Ok(Event::Key(key_event))) = maybe_event {
                        if key_event.kind == KeyEventKind::Press {
                            self.handle_key_event(key_event).await
                                .wrap_err_with(|| format!("handling key event failed:\n{key_event:#?}"))?;
                        }
                    }
                }
            }

            if last_refresh.elapsed() >= REFRESH {
                if self.active_party.is_some() && self.active_quest.is_none() {
                    self.refresh_party_state().await;
                } else {
                    self.refresh_quest_state().await;
                }
                last_refresh = Instant::now();
            }
        }
        Ok(())
    }

    /// WASM entry point — called from `#[wasm_bindgen(start)]` in main.rs.
    #[cfg(target_arch = "wasm32")]
    pub fn start_wasm() {
        use std::{cell::RefCell, rc::Rc};
        use ratzilla::WebRenderer;
        use wasm_bindgen_futures::spawn_local;

        let app = Rc::new(RefCell::new(App::default()));
        let terminal = Rc::new(RefCell::new(tui::init().expect("tui init")));

        terminal
            .borrow_mut()
            .draw(|frame| app.borrow().render_frame(frame))
            .expect("initial draw");

        let app_c = app.clone();
        let term_c = terminal.clone();
        terminal.borrow().on_key_event(move |key_event| {
            let app2 = app_c.clone();
            let term2 = term_c.clone();
            spawn_local(async move {
                App::handle_key_wasm(app2.clone(), key_event).await;
                term2.borrow_mut().draw(|f| app2.borrow().render_frame(f)).ok();
            });
        });
    }

    /// Full async WASM key dispatcher.  All network actions are wired via
    /// spawn_local — borrows are always dropped before each .await.
    #[cfg(target_arch = "wasm32")]
    async fn handle_key_wasm(
        app: std::rc::Rc<std::cell::RefCell<Self>>,
        key: ratzilla::event::KeyEvent,
    ) {
        use ratzilla::event::KeyCode;

        // ── Inventory ────────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::Inventory { .. }) {
            let state = std::mem::replace(&mut app.borrow_mut().state, AppState::Main);
            if let AppState::Inventory { scroll, selected, previous } = state {
                let item_count = app.borrow().inventory.len();
                const PAGE: usize = 5;
                match key.code {
                    KeyCode::Char('j') | KeyCode::Char('s') | KeyCode::Down => {
                        let new_sel = (selected + 1).min(item_count.saturating_sub(1));
                        let new_scroll = if new_sel >= scroll + PAGE { scroll + 1 } else { scroll };
                        app.borrow_mut().state = AppState::Inventory { scroll: new_scroll, selected: new_sel, previous };
                    }
                    KeyCode::Char('k') | KeyCode::Char('w') | KeyCode::Up => {
                        let new_sel = selected.saturating_sub(1);
                        let new_scroll = if new_sel < scroll { scroll.saturating_sub(1) } else { scroll };
                        app.borrow_mut().state = AppState::Inventory { scroll: new_scroll, selected: new_sel, previous };
                    }
                    KeyCode::Char('v') | KeyCode::Esc | KeyCode::Char('q') => {
                        app.borrow_mut().state = *previous;
                    }
                    KeyCode::Enter => {
                        let (can_use, is_full_heal) = {
                            let a = app.borrow();
                            let is_dead = a.active_character.as_ref().map_or(false, |c| c.unit.health <= 0)
                                || a.party_members.iter().any(|m| m.unit.health <= 0);
                            let can_use = a.inventory.get(selected).map(|i| match &i.item.effect {
                                ItemEffect::Damage(_) => matches!(*previous, AppState::Combat),
                                ItemEffect::Heal(_) | ItemEffect::MaxHpUp(_) => true,
                                ItemEffect::FullHeal => is_dead,
                            }).unwrap_or(false);
                            let is_full_heal = a.inventory.get(selected)
                                .map_or(false, |i| matches!(i.item.effect, ItemEffect::FullHeal));
                            (can_use, is_full_heal)
                        };
                        if can_use && is_full_heal {
                            app.borrow_mut().state = AppState::TargetSelect {
                                item_index: selected, target_selected: 0,
                                inv_scroll: scroll, inv_item_selected: selected,
                                return_to: previous,
                            };
                        } else if can_use {
                            app.borrow_mut().state = *previous;
                            let client = app.borrow().client.clone();
                            App::wasm_use_item(&app, &client, selected).await;
                        } else {
                            app.borrow_mut().state = AppState::Inventory { scroll, selected, previous };
                        }
                    }
                    _ => { app.borrow_mut().state = AppState::Inventory { scroll, selected, previous }; }
                }
            }
            return;
        }

        // ── TargetSelect ─────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::TargetSelect { .. }) {
            let state = std::mem::replace(&mut app.borrow_mut().state, AppState::Main);
            if let AppState::TargetSelect { item_index, target_selected, inv_scroll, inv_item_selected, return_to } = state {
                let targets = app.borrow().dead_targets();
                let target_count = targets.len();
                match key.code {
                    KeyCode::Char('j') | KeyCode::Char('s') | KeyCode::Down => {
                        app.borrow_mut().state = AppState::TargetSelect {
                            item_index,
                            target_selected: (target_selected + 1).min(target_count.saturating_sub(1)),
                            inv_scroll, inv_item_selected, return_to,
                        };
                    }
                    KeyCode::Char('k') | KeyCode::Char('w') | KeyCode::Up => {
                        app.borrow_mut().state = AppState::TargetSelect {
                            item_index,
                            target_selected: target_selected.saturating_sub(1),
                            inv_scroll, inv_item_selected, return_to,
                        };
                    }
                    KeyCode::Esc => {
                        app.borrow_mut().state = AppState::Inventory {
                            scroll: inv_scroll, selected: inv_item_selected, previous: return_to,
                        };
                    }
                    KeyCode::Enter => {
                        let target_user_id = targets.get(target_selected).map(|(id, _)| *id);
                        if let Some(target_user_id) = target_user_id {
                            let client = app.borrow().client.clone();
                            App::wasm_apply_full_heal(&app, &client, item_index, target_user_id).await;
                        }
                        app.borrow_mut().state = *return_to;
                    }
                    _ => {
                        app.borrow_mut().state = AppState::TargetSelect {
                            item_index, target_selected, inv_scroll, inv_item_selected, return_to,
                        };
                    }
                }
            }
            return;
        }

        let client = app.borrow().client.clone();

        // ── Welcome ───────────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::Welcome) {
            match key.code {
                KeyCode::Char('r') => { app.borrow_mut().start_register_user(); }
                KeyCode::Char('q') => App::wasm_reload(),
                _ => {}
            }
            return;
        }

        // ── TextInput ─────────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::TextInput(_)) {
            match key.code {
                KeyCode::Enter => {
                    let username = {
                        let mut a = app.borrow_mut();
                        a.toggle_text_input(None);
                        a.get_and_clear_text_input()
                    };
                    if let Some(name) = username {
                        if let Ok(user) = client.post_register_user(name).await {
                            let uid = user.id;
                            app.borrow_mut().active_user = Some(user);
                            let character = client.post_new_character(&uid).await.ok();
                            let inventory = client.get_character_items(uid).await.unwrap_or_default();
                            let party = client.get_party_for_user(uid).await.ok();
                            let mut a = app.borrow_mut();
                            a.active_character = character;
                            a.inventory = inventory;
                            a.active_party = party;
                            a.state = AppState::Tavern(TavernState::Main);
                        }
                    }
                }
                KeyCode::Char(value) => {
                    if let Some(current) = app.borrow_mut().text_input.as_mut() {
                        current.push(value);
                    }
                }
                KeyCode::Backspace => {
                    if let Some(current) = app.borrow_mut().text_input.as_mut() {
                        current.pop();
                    }
                }
                KeyCode::Esc => { app.borrow_mut().state = AppState::Tavern(TavernState::Main); }
                _ => {}
            }
            return;
        }

        // ── Tavern(Main) ──────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::Tavern(TavernState::Main)) {
            let has_char = app.borrow().active_character.is_some();
            match key.code {
                KeyCode::Char('s') if has_char => {
                    let items = client.get_shop_items().await.unwrap_or_default();
                    app.borrow_mut().state = AppState::Tavern(TavernState::Shop { items, selected: 0, scroll: 0 });
                }
                KeyCode::Char('a') if has_char => { app.borrow_mut().state = AppState::AdventureMenu; }
                KeyCode::Char('g') if has_char => { App::wasm_open_party(&app, &client).await; }
                KeyCode::Char('o') | KeyCode::Char('r') if has_char => { app.borrow_mut().start_register_user(); }
                KeyCode::Char('v') => { app.borrow_mut().open_inventory(); }
                KeyCode::Char('q') => App::wasm_reload(),
                _ => {}
            }
            return;
        }

        // ── Tavern(Shop) ──────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::Tavern(TavernState::Shop { .. })) {
            let (selected, scroll, item_count) = {
                let a = app.borrow();
                if let AppState::Tavern(TavernState::Shop { selected, scroll, items }) = &a.state {
                    (*selected, *scroll, items.len())
                } else { unreachable!() }
            };
            const PAGE: usize = 5;
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    let new_sel = (selected + 1).min(item_count.saturating_sub(1));
                    let new_scroll = if new_sel >= scroll + PAGE { scroll + 1 } else { scroll };
                    if let AppState::Tavern(TavernState::Shop { selected: s, scroll: sc, .. }) = &mut app.borrow_mut().state {
                        *s = new_sel; *sc = new_scroll;
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    let new_sel = selected.saturating_sub(1);
                    let new_scroll = if new_sel < scroll { scroll.saturating_sub(1) } else { scroll };
                    if let AppState::Tavern(TavernState::Shop { selected: s, scroll: sc, .. }) = &mut app.borrow_mut().state {
                        *s = new_sel; *sc = new_scroll;
                    }
                }
                KeyCode::Enter => {
                    let (name, cost) = {
                        let a = app.borrow();
                        if let AppState::Tavern(TavernState::Shop { items, .. }) = &a.state {
                            items.get(selected).map(|i| (i.item.name.clone(), i.cost as u32))
                                .unwrap_or_default()
                        } else { (String::new(), 0) }
                    };
                    if !name.is_empty() {
                        let coins = app.borrow().active_character.as_ref().map(|c| c.character.coins).unwrap_or(0);
                        if coins >= cost {
                            let uid = app.borrow().active_user.as_ref().map(|u| u.id);
                            if let Some(uid) = uid {
                                let (new_coins, renown) = {
                                    let mut a = app.borrow_mut();
                                    if let Some(c) = a.active_character.as_mut() {
                                        c.character.coins -= cost;
                                    }
                                    a.active_character.as_ref()
                                        .map(|c| (c.character.coins, c.character.renown))
                                        .unwrap_or_default()
                                };
                                let _ = client.save_character_stats(uid, new_coins, renown).await;
                                if client.post_give_item(uid, &name).await.is_ok() {
                                    let inventory = client.get_character_items(uid).await.unwrap_or_default();
                                    app.borrow_mut().inventory = inventory;
                                }
                            }
                        }
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => { app.borrow_mut().state = AppState::Tavern(TavernState::Main); }
                _ => {}
            }
            return;
        }

        // ── PartyLobby ────────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::PartyLobby { .. }) {
            match key.code {
                KeyCode::Char('1') => App::wasm_join_party_from_lobby(&app, &client, 0).await,
                KeyCode::Char('2') => App::wasm_join_party_from_lobby(&app, &client, 1).await,
                KeyCode::Char('3') => App::wasm_join_party_from_lobby(&app, &client, 2).await,
                KeyCode::Char('4') => App::wasm_join_party_from_lobby(&app, &client, 3).await,
                KeyCode::Char('5') => App::wasm_join_party_from_lobby(&app, &client, 4).await,
                KeyCode::Char('n') => {
                    let uid = app.borrow().active_user.as_ref().map(|u| u.id);
                    if let Some(uid) = uid {
                        if let Ok(party) = client.post_create_party(uid).await {
                            let pid = party.id;
                            app.borrow_mut().active_party = Some(party);
                            let members = client.get_party_members_for_party(pid).await.unwrap_or_default();
                            let mut a = app.borrow_mut();
                            a.party_members = members;
                            a.state = AppState::Party;
                        }
                    }
                }
                KeyCode::Char('r') => App::wasm_open_party(&app, &client).await,
                KeyCode::Char('q') => App::wasm_reload(),
                KeyCode::Esc => { app.borrow_mut().state = AppState::Tavern(TavernState::Main); }
                _ => {}
            }
            return;
        }

        // ── Party ─────────────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::Party) {
            match key.code {
                KeyCode::Char('l') => {
                    let uid = app.borrow().active_user.as_ref().map(|u| u.id);
                    if let Some(uid) = uid {
                        let _ = client.delete_leave_party(uid).await;
                    }
                    let mut a = app.borrow_mut();
                    a.active_party = None;
                    a.party_members.clear();
                    a.state = AppState::Tavern(TavernState::Main);
                }
                KeyCode::Char('v') => { app.borrow_mut().open_inventory(); }
                KeyCode::Char('q') | KeyCode::Esc => { app.borrow_mut().state = AppState::Tavern(TavernState::Main); }
                _ => {}
            }
            return;
        }

        // ── AdventureMenu ─────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::AdventureMenu) {
            let renown = app.borrow().active_character.as_ref().map(|c| c.character.renown).unwrap_or(0);
            match key.code {
                KeyCode::Char('1') => App::wasm_start_quest(&app, &client, crate::app::AREA_SEWERS).await,
                KeyCode::Char('2') if renown >= 5 => App::wasm_start_quest(&app, &client, crate::app::AREA_SEWER_DEPTHS).await,
                KeyCode::Char('3') if renown >= 10 => App::wasm_start_quest(&app, &client, crate::app::AREA_FUNGAL_WARRENS).await,
                KeyCode::Char('4') if renown >= 20 => App::wasm_start_quest(&app, &client, crate::app::AREA_ABYSS).await,
                KeyCode::Esc | KeyCode::Char('q') => { app.borrow_mut().state = AppState::Tavern(TavernState::Main); }
                _ => {}
            }
            return;
        }

        // ── GameOver ──────────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::GameOver) {
            match key.code {
                KeyCode::Char('i') => {
                    let has_revive = app.borrow().inventory.iter()
                        .any(|i| matches!(i.item.effect, ItemEffect::FullHeal) && i.charges_remaining != 0);
                    if has_revive {
                        app.borrow_mut().state = AppState::Inventory {
                            scroll: 0, selected: 0,
                            previous: Box::new(AppState::Combat),
                        };
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    {
                        let mut a = app.borrow_mut();
                        a.active_quest = None;
                        if a.active_user.is_some() {
                            if let Some(c) = a.active_character.as_mut() {
                                c.unit.health = c.unit.max_health;
                                c.character.coins = 0;
                                c.character.renown = 0;
                            }
                        }
                        a.inventory.clear();
                    }
                    let reset_self = {
                        let a = app.borrow();
                        a.active_user.as_ref().zip(a.active_character.as_ref()).map(|(u, c)| (u.id, c.unit))
                    };
                    if let Some((uid, unit)) = reset_self {
                        let _ = client.update_character_unit(uid, &unit).await;
                        let _ = client.save_character_stats(uid, 0, 0).await;
                        let _ = client.clear_character_items(uid).await;
                    }
                    let resets: Vec<(i32, ratback_types::data::Unit)> = {
                        let mut a = app.borrow_mut();
                        a.party_members.iter_mut().map(|m| {
                            m.unit.health = m.unit.max_health;
                            m.character.coins = 0;
                            m.character.renown = 0;
                            (m.character.user_id, m.unit.clone())
                        }).collect()
                    };
                    for (uid, unit) in resets {
                        let _ = client.update_character_unit(uid, &unit).await;
                        let _ = client.save_character_stats(uid, 0, 0).await;
                        let _ = client.clear_character_items(uid).await;
                    }
                    app.borrow_mut().state = AppState::Tavern(TavernState::Main);
                }
                _ => {}
            }
            return;
        }

        // ── Combat ────────────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::Combat) {
            match key.code {
                KeyCode::Char('f') => App::wasm_attack(&app, &client, 5).await,
                KeyCode::Char('1') => App::wasm_use_item(&app, &client, 0).await,
                KeyCode::Char('2') => App::wasm_use_item(&app, &client, 1).await,
                KeyCode::Char('3') => App::wasm_use_item(&app, &client, 2).await,
                KeyCode::Char('4') => App::wasm_use_item(&app, &client, 3).await,
                KeyCode::Char('5') => App::wasm_use_item(&app, &client, 4).await,
                KeyCode::Char('6') => App::wasm_use_item(&app, &client, 5).await,
                KeyCode::Char('7') => App::wasm_use_item(&app, &client, 6).await,
                KeyCode::Char('8') => App::wasm_use_item(&app, &client, 7).await,
                KeyCode::Char('9') => App::wasm_use_item(&app, &client, 8).await,
                KeyCode::Char('v') => { app.borrow_mut().open_inventory(); }
                KeyCode::Char('q') => App::wasm_reload(),
                _ => {}
            }
            return;
        }

        // ── Dialogue ──────────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::Dialogue { .. }) {
            match key.code {
                KeyCode::Char('1') => App::wasm_pick_dialogue_choice(&app, &client, 0).await,
                KeyCode::Char('2') => App::wasm_pick_dialogue_choice(&app, &client, 1).await,
                KeyCode::Char('3') => App::wasm_pick_dialogue_choice(&app, &client, 2).await,
                KeyCode::Char('4') => App::wasm_pick_dialogue_choice(&app, &client, 3).await,
                KeyCode::Char('5') => App::wasm_pick_dialogue_choice(&app, &client, 4).await,
                KeyCode::Char('v') => { app.borrow_mut().open_inventory(); }
                KeyCode::Char('q') => App::wasm_reload(),
                _ => {}
            }
            return;
        }

        // ── catch-all ─────────────────────────────────────────────────────────
        match key.code {
            KeyCode::Char('q') => App::wasm_reload(),
            KeyCode::Char('r') => { app.borrow_mut().start_register_user(); }
            KeyCode::Char('c') => {
                let uid = app.borrow().active_user.as_ref().map(|u| u.id);
                if let Some(uid) = uid {
                    let character = client.post_new_character(&uid).await.ok();
                    let inventory = client.get_character_items(uid).await.unwrap_or_default();
                    let mut a = app.borrow_mut();
                    a.active_character = character;
                    a.inventory = inventory;
                }
            }
            KeyCode::Char('f') => App::wasm_attack(&app, &client, 5).await,
            KeyCode::Char('v') => { app.borrow_mut().open_inventory(); }
            _ => {}
        }
    }

    // ── WASM async helpers ────────────────────────────────────────────────────

    #[cfg(target_arch = "wasm32")]
    fn wasm_reload() {
        if let Some(w) = ratzilla::web_sys::window() {
            let _ = w.location().reload();
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_open_party(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
    ) {
        if app.borrow().active_user.is_none() { return; }
        let party_id = app.borrow().active_party.as_ref().map(|p| p.id);
        if let Some(pid) = party_id {
            let members = client.get_party_members_for_party(pid).await.unwrap_or_default();
            let mut a = app.borrow_mut();
            a.party_members = members;
            a.state = AppState::Party;
        } else {
            let parties = client.get_open_parties().await.unwrap_or_default();
            app.borrow_mut().state = AppState::PartyLobby { parties };
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_join_party_from_lobby(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
        index: usize,
    ) {
        let (party_id, user_id) = {
            let a = app.borrow();
            match &a.state {
                AppState::PartyLobby { parties } => match parties.get(index) {
                    Some(p) => (p.id, a.active_user.as_ref().map(|u| u.id).unwrap_or(0)),
                    None => return,
                },
                _ => return,
            }
        };
        if let Ok(party) = client.post_join_party(party_id, user_id).await {
            let pid = party.id;
            app.borrow_mut().active_party = Some(party);
            let members = client.get_party_members_for_party(pid).await.unwrap_or_default();
            let mut a = app.borrow_mut();
            a.party_members = members;
            a.state = AppState::Party;
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_start_quest(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
        area: &str,
    ) {
        let uid = app.borrow().active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            let quest = client.post_new_quest(uid, area).await.ok();
            app.borrow_mut().active_quest = quest;
            App::wasm_fetch_party_members(app, client).await;
            app.borrow_mut().state = AppState::Main;
            App::wasm_check_current_encounter(app, client).await;
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_fetch_party_members(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
    ) {
        let quest_id = app.borrow().active_quest.as_ref().map(|q| q.id);
        if let Some(qid) = quest_id {
            let members = client.get_quest_members(qid).await.unwrap_or_default();
            app.borrow_mut().party_members = members;
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_sync_quest_state(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
    ) {
        let (quest_id, current_encounter, encounters, node) = {
            let a = app.borrow();
            let node = if let AppState::Dialogue { current_node, .. } = &a.state {
                Some(current_node.clone())
            } else { None };
            match &a.active_quest {
                Some(q) => (q.id, q.current_encounter, q.encounters.clone(), node),
                None => return,
            }
        };
        let _ = client.put_encounters(quest_id, current_encounter, node.as_deref(), &encounters).await;
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_regen_energy(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
    ) {
        let update = {
            let mut a = app.borrow_mut();
            if let Some(c) = a.active_character.as_mut() {
                c.unit.energy = (c.unit.energy + 5).min(c.unit.max_energy);
            }
            a.active_user.as_ref().zip(a.active_character.as_ref()).map(|(u, c)| (u.id, c.unit))
        };
        if let Some((uid, unit)) = update {
            let _ = client.update_character_unit(uid, &unit).await;
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_complete_quest(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
    ) {
        let (quest_id, user_id, stats) = {
            let a = app.borrow();
            match (&a.active_quest, &a.active_user) {
                (Some(q), Some(u)) => {
                    let stats = a.active_character.as_ref().map(|c| (c.character.coins, c.character.renown));
                    (q.id, u.id, stats)
                }
                _ => return,
            }
        };
        if let Some((coins, renown)) = stats {
            let _ = client.save_character_stats(user_id, coins, renown).await;
        }
        let updated = client.post_complete_quest(quest_id, user_id).await.ok();
        {
            let mut a = app.borrow_mut();
            a.active_character = updated;
            a.active_quest = None;
        }
        let party_id = app.borrow().active_party.as_ref().map(|p| p.id);
        if let Some(pid) = party_id {
            let members = client.get_party_members_for_party(pid).await.unwrap_or_default();
            let mut a = app.borrow_mut();
            a.party_members = members;
            a.state = AppState::Party;
        } else {
            app.borrow_mut().state = AppState::Tavern(TavernState::Main);
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_check_current_encounter(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
    ) {
        use ratback_types::quest_data::Encounter;
        let (enc, quest_done) = {
            let a = app.borrow();
            match &a.active_quest {
                Some(q) => {
                    let idx = q.current_encounter as usize;
                    (q.encounters.get(idx).cloned(), idx >= q.encounters.len())
                }
                None => (None, false),
            }
        };
        if quest_done {
            App::wasm_complete_quest(app, client).await;
            return;
        }
        match enc {
            Some(Encounter::NpcEncounter(id)) => {
                if let Ok(dialogue) = client.get_dialogue(&id).await {
                    let start = {
                        let a = app.borrow();
                        a.active_quest.as_ref()
                            .and_then(|q| q.current_node_id.clone())
                            .filter(|n| dialogue.nodes.contains_key(n.as_str()))
                            .unwrap_or_else(|| dialogue.start.clone())
                    };
                    app.borrow_mut().state = AppState::Dialogue { dialogue, current_node: start };
                }
            }
            Some(Encounter::CombatEncounter(_)) => {
                let mut a = app.borrow_mut();
                a.last_combat_damage = None;
                a.state = AppState::Combat;
            }
            _ => { app.borrow_mut().state = AppState::Main; }
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_pick_dialogue_choice(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
        index: usize,
    ) {
        use ratback_types::quest_data::DialogueOutcome;
        let (next, outcome) = {
            let a = app.borrow();
            match &a.state {
                AppState::Dialogue { dialogue, current_node } => {
                    match dialogue.nodes.get(current_node) {
                        Some(node) => match node.choices.get(index) {
                            Some(choice) => {
                                let coins = a.active_character.as_ref().map(|c| c.character.coins as i32).unwrap_or(0);
                                let too_expensive = |o: &DialogueOutcome| match o {
                                    DialogueOutcome::GiveItem { cost, .. } => coins < *cost,
                                    DialogueOutcome::Reward { coins: c, .. } => *c < 0 && coins < c.unsigned_abs() as i32,
                                    _ => false,
                                };
                                let locked = match &choice.outcome {
                                    Some(o) => too_expensive(o),
                                    None => choice.next.as_ref()
                                        .and_then(|nid| dialogue.nodes.get(nid.as_str()))
                                        .map(|nn| !nn.choices.is_empty() && nn.choices.iter().all(|c| c.outcome.as_ref().map_or(false, too_expensive)))
                                        .unwrap_or(false),
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
            }
        };
        match (next, outcome) {
            (Some(node_id), _) => {
                {
                    let mut a = app.borrow_mut();
                    if let AppState::Dialogue { current_node, .. } = &mut a.state {
                        *current_node = node_id.clone();
                    }
                    if let Some(q) = a.active_quest.as_mut() {
                        q.current_node_id = Some(node_id);
                    }
                }
                App::wasm_sync_quest_state(app, client).await;
            }
            (None, Some(outcome)) => {
                App::wasm_apply_dialogue_outcome(app, client, outcome).await;
            }
            (None, None) => {
                {
                    let mut a = app.borrow_mut();
                    a.state = AppState::Tavern(TavernState::Main);
                    if let Some(q) = a.active_quest.as_mut() { q.current_encounter += 1; }
                }
                App::wasm_regen_energy(app, client).await;
                App::wasm_sync_quest_state(app, client).await;
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_apply_dialogue_outcome(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
        outcome: ratback_types::quest_data::DialogueOutcome,
    ) {
        use ratback_types::quest_data::{DialogueOutcome, Encounter};
        app.borrow_mut().state = AppState::Tavern(TavernState::Main);
        match outcome {
            DialogueOutcome::Reward { coins, renown, heal } => {
                let uid_stats = {
                    let mut a = app.borrow_mut();
                    if let Some(c) = a.active_character.as_mut() {
                        c.character.coins = (c.character.coins as i32 + coins).max(0) as u32;
                        c.character.renown = (c.character.renown as i32 + renown).max(0) as u32;
                        if heal != 0 { c.unit.health = (c.unit.health + heal).clamp(0, c.unit.max_health); }
                    }
                    if let Some(q) = a.active_quest.as_mut() { q.current_encounter += 1; }
                    a.active_user.as_ref().zip(a.active_character.as_ref())
                        .map(|(u, c)| (u.id, c.character.coins, c.character.renown))
                };
                if let Some((uid, c, r)) = uid_stats {
                    let _ = client.save_character_stats(uid, c, r).await;
                }
            }
            DialogueOutcome::Damage { amount } => {
                let mut a = app.borrow_mut();
                if let Some(c) = a.active_character.as_mut() { c.unit.health = (c.unit.health - amount).max(0); }
                if let Some(q) = a.active_quest.as_mut() { q.current_encounter += 1; }
            }
            DialogueOutcome::NextEncounter => {
                if let Some(q) = app.borrow_mut().active_quest.as_mut() { q.current_encounter += 1; }
            }
            DialogueOutcome::Combat(combat) => {
                {
                    let mut a = app.borrow_mut();
                    let idx = a.active_quest.as_ref().map(|q| q.current_encounter as usize).unwrap_or(0);
                    if let Some(q) = a.active_quest.as_mut() {
                        if let Some(enc) = q.encounters.get_mut(idx) { *enc = Encounter::CombatEncounter(combat); }
                        q.current_node_id = None;
                    }
                    a.state = AppState::Combat;
                }
                App::wasm_sync_quest_state(app, client).await;
                return;
            }
            DialogueOutcome::GiveItem { item_name, cost } => {
                let (uid, new_coins, renown) = {
                    let mut a = app.borrow_mut();
                    if let Some(c) = a.active_character.as_mut() {
                        c.character.coins = (c.character.coins as i32 - cost).max(0) as u32;
                    }
                    if let Some(q) = a.active_quest.as_mut() { q.current_encounter += 1; }
                    let uid = a.active_user.as_ref().map(|u| u.id);
                    let (coins, renown) = a.active_character.as_ref()
                        .map(|c| (c.character.coins, c.character.renown))
                        .unwrap_or_default();
                    (uid, coins, renown)
                };
                if let Some(uid) = uid {
                    let _ = client.save_character_stats(uid, new_coins, renown).await;
                    let _ = client.post_give_item(uid, &item_name).await;
                    let inventory = client.get_character_items(uid).await.unwrap_or_default();
                    app.borrow_mut().inventory = inventory;
                }
            }
            DialogueOutcome::Escape => return,
        }
        App::wasm_regen_energy(app, client).await;
        App::wasm_sync_quest_state(app, client).await;
        App::wasm_check_current_encounter(app, client).await;
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_attack(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
        damage: i32,
    ) {
        use ratback_types::quest_data::Encounter;
        // Spend 1 EP
        let update = {
            let mut a = app.borrow_mut();
            if let Some(c) = a.active_character.as_mut() {
                if c.unit.energy <= 0 { return; }
                c.unit.energy -= 1;
            }
            a.active_user.as_ref().zip(a.active_character.as_ref()).map(|(u, c)| (u.id, c.unit))
        };
        if let Some((uid, unit)) = update { let _ = client.update_character_unit(uid, &unit).await; }

        // Player attacks
        let (encounter_cleared, monsters_alive) = {
            let mut a = app.borrow_mut();
            let quest = match a.active_quest.as_mut() { Some(q) => q, None => return };
            let idx = quest.current_encounter as usize;
            let combat = match quest.encounters.get_mut(idx) {
                Some(Encounter::CombatEncounter(c)) => c, _ => return,
            };
            if let Some(t) = combat.monsters.iter_mut().find(|m| m.unit.health > 0) {
                t.unit.health = (t.unit.health - damage).max(0);
            }
            let alive = combat.monsters.iter().filter(|m| m.unit.health > 0).count();
            combat.turn += 1;
            if alive == 0 { quest.current_encounter += 1; }
            (alive == 0, alive)
        };

        // Monsters retaliate
        if !encounter_cleared && monsters_alive > 0 {
            let (monster_damage, target_name) = {
                let mut a = app.borrow_mut();
                let quest = match a.active_quest.as_mut() { Some(q) => q, None => return };
                let idx = quest.current_encounter as usize;
                let combat = match quest.encounters.get_mut(idx) {
                    Some(Encounter::CombatEncounter(c)) => c, _ => return,
                };
                let dmg: i32 = combat.monsters.iter_mut().filter(|m| m.unit.health > 0).map(|m| {
                    if let Some(item) = m.items.iter_mut().find(|it| it.charges != 0) {
                        let d = match item.effect { ratback_types::data::ItemEffect::Damage(d) => d, _ => m.attack };
                        if item.charges > 0 { item.charges -= 1; }
                        d
                    } else { m.attack }
                }).sum();
                (dmg, a.active_character.as_ref().map(|c| c.character.name.clone()).unwrap_or_default())
            };
            app.borrow_mut().last_combat_damage = Some((monster_damage, target_name));
            let update = {
                let mut a = app.borrow_mut();
                if let Some(c) = a.active_character.as_mut() { c.unit.health = (c.unit.health - monster_damage).max(0); }
                a.active_user.as_ref().zip(a.active_character.as_ref()).map(|(u, c)| (u.id, c.unit))
            };
            if let Some((uid, unit)) = update { let _ = client.update_character_unit(uid, &unit).await; }
        } else if encounter_cleared {
            app.borrow_mut().last_combat_damage = None;
        }

        // Push encounter state
        let enc_snapshot = {
            let a = app.borrow();
            a.active_quest.as_ref().map(|q| (q.id, q.current_encounter, q.encounters.clone()))
        };
        if let Some((qid, ce, encs)) = enc_snapshot {
            let _ = client.put_encounters(qid, ce, None, &encs).await;
        }

        if encounter_cleared {
            App::wasm_regen_energy(app, client).await;
            App::wasm_check_current_encounter(app, client).await;
        } else {
            let all_dead = {
                let a = app.borrow();
                a.active_character.as_ref().map_or(false, |c| c.unit.health <= 0)
                    && a.party_members.iter().all(|m| m.unit.health <= 0)
            };
            if all_dead { app.borrow_mut().state = AppState::GameOver; }
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_use_item(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
        index: usize,
    ) {
        use ratback_types::quest_data::Encounter;
        let inv_item = match app.borrow().inventory.get(index).cloned() {
            Some(i) => i, None => return,
        };
        let mut encounter_cleared = false;
        match &inv_item.item.effect {
            ItemEffect::Damage(dmg) => {
                let dmg = *dmg;
                let mut a = app.borrow_mut();
                if let Some(q) = a.active_quest.as_mut() {
                    let idx = q.current_encounter as usize;
                    if let Some(Encounter::CombatEncounter(c)) = q.encounters.get_mut(idx) {
                        if let Some(t) = c.monsters.iter_mut().find(|m| m.unit.health > 0) {
                            t.unit.health = (t.unit.health - dmg).max(0);
                        }
                        if c.monsters.iter().all(|m| m.unit.health <= 0) {
                            q.current_encounter += 1;
                            encounter_cleared = true;
                        }
                    }
                }
            }
            ItemEffect::Heal(heal) => {
                let heal = *heal;
                let update = {
                    let mut a = app.borrow_mut();
                    if let Some(c) = a.active_character.as_mut() {
                        c.unit.health = (c.unit.health + heal).clamp(0, c.unit.max_health);
                    }
                    a.active_user.as_ref().zip(a.active_character.as_ref()).map(|(u, c)| (u.id, c.unit))
                };
                if let Some((uid, unit)) = update { let _ = client.update_character_unit(uid, &unit).await; }
            }
            ItemEffect::FullHeal => {
                let update = {
                    let mut a = app.borrow_mut();
                    if let Some(c) = a.active_character.as_mut() { c.unit.health = c.unit.max_health; }
                    a.active_user.as_ref().zip(a.active_character.as_ref()).map(|(u, c)| (u.id, c.unit))
                };
                if let Some((uid, unit)) = update { let _ = client.update_character_unit(uid, &unit).await; }
            }
            ItemEffect::MaxHpUp(amount) => {
                let amount = *amount;
                let update = {
                    let mut a = app.borrow_mut();
                    if let Some(c) = a.active_character.as_mut() {
                        c.unit.max_health += amount;
                        c.unit.health = (c.unit.health + amount).min(c.unit.max_health);
                    }
                    a.active_user.as_ref().zip(a.active_character.as_ref()).map(|(u, c)| (u.id, c.unit))
                };
                if let Some((uid, unit)) = update { let _ = client.update_character_unit(uid, &unit).await; }
            }
        }
        if inv_item.item.charges != -1 {
            let uid = app.borrow().active_user.as_ref().map(|u| u.id);
            if let Some(uid) = uid {
                let _ = client.delete_character_item(uid, inv_item.item.id).await;
                let inventory = client.get_character_items(uid).await.unwrap_or_default();
                app.borrow_mut().inventory = inventory;
            }
        }
        let enc_snapshot = {
            let a = app.borrow();
            a.active_quest.as_ref().map(|q| (q.id, q.current_encounter, q.encounters.clone()))
        };
        if let Some((qid, ce, encs)) = enc_snapshot {
            let _ = client.put_encounters(qid, ce, None, &encs).await;
        }
        if encounter_cleared {
            App::wasm_regen_energy(app, client).await;
            App::wasm_check_current_encounter(app, client).await;
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn wasm_apply_full_heal(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
        item_index: usize,
        target_user_id: i32,
    ) {
        let active_user_id = app.borrow().active_user.as_ref().map(|u| u.id);
        if active_user_id == Some(target_user_id) {
            let unit = {
                let mut a = app.borrow_mut();
                a.active_character.as_mut().map(|c| { c.unit.health = c.unit.max_health; c.unit })
            };
            if let Some(unit) = unit { let _ = client.update_character_unit(target_user_id, &unit).await; }
        } else {
            let unit = {
                let mut a = app.borrow_mut();
                a.party_members.iter_mut().find(|m| m.character.user_id == target_user_id)
                    .map(|m| { m.unit.health = m.unit.max_health; m.unit.clone() })
            };
            if let Some(unit) = unit { let _ = client.update_character_unit(target_user_id, &unit).await; }
        }
        let inv_item = app.borrow().inventory.get(item_index).cloned();
        if let Some(inv_item) = inv_item {
            if inv_item.item.charges != -1 {
                let uid = app.borrow().active_user.as_ref().map(|u| u.id);
                if let Some(uid) = uid {
                    let _ = client.delete_character_item(uid, inv_item.item.id).await;
                    let inventory = client.get_character_items(uid).await.unwrap_or_default();
                    app.borrow_mut().inventory = inventory;
                }
            }
        }
    }

    fn render_frame(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
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
                        let is_dead = self.active_character.as_ref().map_or(false, |c| c.unit.health <= 0)
                            || self.party_members.iter().any(|m| m.unit.health <= 0);
                        let can_use = self.inventory.get(selected).map(|i| {
                            match &i.item.effect {
                                ItemEffect::Damage(_) => matches!(*previous, AppState::Combat),
                                ItemEffect::Heal(_) | ItemEffect::MaxHpUp(_) => true,
                                ItemEffect::FullHeal => is_dead,
                            }
                        }).unwrap_or(false);
                        let is_full_heal = self.inventory.get(selected)
                            .map_or(false, |i| matches!(i.item.effect, ItemEffect::FullHeal));
                        if can_use && is_full_heal {
                            self.state = AppState::TargetSelect {
                                item_index: selected,
                                target_selected: 0,
                                inv_scroll: scroll,
                                inv_item_selected: selected,
                                return_to: previous,
                            };
                        } else if can_use {
                            self.state = *previous;
                            self.use_item(selected).await;
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

        if matches!(&self.state, AppState::TargetSelect { .. }) {
            if let AppState::TargetSelect { item_index, target_selected, inv_scroll, inv_item_selected, return_to } =
                std::mem::replace(&mut self.state, AppState::Main)
            {
                let targets = self.dead_targets();
                let target_count = targets.len();
                match key_event.code {
                    KeyCode::Char('j') | KeyCode::Char('s') | KeyCode::Down => {
                        self.state = AppState::TargetSelect {
                            item_index,
                            target_selected: (target_selected + 1).min(target_count.saturating_sub(1)),
                            inv_scroll,
                            inv_item_selected,
                            return_to,
                        };
                    }
                    KeyCode::Char('k') | KeyCode::Char('w') | KeyCode::Up => {
                        self.state = AppState::TargetSelect {
                            item_index,
                            target_selected: target_selected.saturating_sub(1),
                            inv_scroll,
                            inv_item_selected,
                            return_to,
                        };
                    }
                    KeyCode::Enter => {
                        if let Some((user_id, _)) = targets.get(target_selected) {
                            let user_id = *user_id;
                            self.apply_full_heal_to_target(item_index, user_id).await;
                        }
                        self.state = *return_to;
                    }
                    KeyCode::Esc => {
                        self.state = AppState::Inventory {
                            scroll: inv_scroll,
                            selected: inv_item_selected,
                            previous: return_to,
                        };
                    }
                    _ => {
                        self.state = AppState::TargetSelect {
                            item_index,
                            target_selected,
                            inv_scroll,
                            inv_item_selected,
                            return_to,
                        };
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
                        let items = self.client.get_shop_items().await.unwrap_or_default();
                        self.state = AppState::Tavern(TavernState::Shop { items, selected: 0, scroll: 0 });
                    }
                    KeyCode::Char('a') if has_char => self.state = AppState::AdventureMenu,
                    KeyCode::Char('g') if has_char => self.open_party().await,
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
                        if !name.is_empty() { self.tavern_buy_item(&name, cost).await; }
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.state = AppState::Tavern(TavernState::Main);
                    }
                    _ => {}
                }
            }

            AppState::TextInput(_) => match key_event.code {
                KeyCode::Enter => self.finish_register_user().await,
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
                KeyCode::Char('1') => self.join_party_from_lobby(0).await,
                KeyCode::Char('2') => self.join_party_from_lobby(1).await,
                KeyCode::Char('3') => self.join_party_from_lobby(2).await,
                KeyCode::Char('4') => self.join_party_from_lobby(3).await,
                KeyCode::Char('5') => self.join_party_from_lobby(4).await,
                KeyCode::Char('n') => self.create_new_party().await,
                KeyCode::Char('r') => self.open_party().await,
                KeyCode::Char('q') => self.exit(),
                KeyCode::Esc => self.state = AppState::Tavern(TavernState::Main),
                _ => {}
            },

            AppState::Party => match key_event.code {
                KeyCode::Char('l') => self.leave_party().await,
                KeyCode::Char('v') => self.open_inventory(),
                KeyCode::Char('q') | KeyCode::Esc => self.state = AppState::Tavern(TavernState::Main),
                _ => {}
            },

            AppState::AdventureMenu => {
                let renown = self.active_character.as_ref().map(|c| c.character.renown).unwrap_or(0);
                match key_event.code {
                    KeyCode::Char('1') => self.start_quest(AREA_SEWERS).await,
                    KeyCode::Char('2') if renown >= 5 => self.start_quest(AREA_SEWER_DEPTHS).await,
                    KeyCode::Char('3') if renown >= 10 => self.start_quest(AREA_FUNGAL_WARRENS).await,
                    KeyCode::Char('4') if renown >= 20 => self.start_quest(AREA_ABYSS).await,
                    KeyCode::Esc | KeyCode::Char('q') => self.state = AppState::Tavern(TavernState::Main),
                    _ => {}
                }
            }

            AppState::GameOver => match key_event.code {
                KeyCode::Char('i') => {
                    let has_revive = self.inventory.iter().any(|i| {
                        matches!(i.item.effect, ItemEffect::FullHeal) && i.charges_remaining != 0
                    });
                    if has_revive {
                        self.state = AppState::Inventory {
                            scroll: 0,
                            selected: 0,
                            previous: Box::new(AppState::Combat),
                        };
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.active_quest = None;
                    // Mutate character state first, then extract copies for client calls
                    if let (Some(_), Some(c)) = (&self.active_user, &mut self.active_character) {
                        c.unit.health = c.unit.max_health;
                        c.character.coins = 0;
                        c.character.renown = 0;
                    }
                    let reset_self = self.active_user.as_ref()
                        .zip(self.active_character.as_ref())
                        .map(|(u, c)| (u.id, c.unit));
                    if let Some((uid, unit)) = reset_self {
                        let _ = self.client.update_character_unit(uid, &unit).await;
                        let _ = self.client.save_character_stats(uid, 0, 0).await;
                        let _ = self.client.clear_character_items(uid).await;
                    }
                    self.inventory.clear();
                    let resets: Vec<(i32, ratback_types::data::Unit)> = self.party_members.iter_mut().map(|m| {
                        m.unit.health = m.unit.max_health;
                        m.character.coins = 0;
                        m.character.renown = 0;
                        (m.character.user_id, m.unit.clone())
                    }).collect();
                    for (uid, unit) in resets {
                        let _ = self.client.update_character_unit(uid, &unit).await;
                        let _ = self.client.save_character_stats(uid, 0, 0).await;
                        let _ = self.client.clear_character_items(uid).await;
                    }
                    self.state = AppState::Tavern(TavernState::Main);
                }
                _ => {}
            },

            AppState::Combat => match key_event.code {
                KeyCode::Char('f') => self.attack_first_enemy(5).await,
                KeyCode::Char('1') => self.use_item(0).await,
                KeyCode::Char('2') => self.use_item(1).await,
                KeyCode::Char('3') => self.use_item(2).await,
                KeyCode::Char('4') => self.use_item(3).await,
                KeyCode::Char('5') => self.use_item(4).await,
                KeyCode::Char('6') => self.use_item(5).await,
                KeyCode::Char('7') => self.use_item(6).await,
                KeyCode::Char('8') => self.use_item(7).await,
                KeyCode::Char('9') => self.use_item(8).await,
                KeyCode::Char('v') => self.open_inventory(),
                KeyCode::Char('q') => self.exit(),
                _ => {}
            },

            AppState::Dialogue { .. } => match key_event.code {
                KeyCode::Char('1') => self.pick_dialogue_choice(0).await,
                KeyCode::Char('2') => self.pick_dialogue_choice(1).await,
                KeyCode::Char('3') => self.pick_dialogue_choice(2).await,
                KeyCode::Char('4') => self.pick_dialogue_choice(3).await,
                KeyCode::Char('5') => self.pick_dialogue_choice(4).await,
                KeyCode::Char('v') => self.open_inventory(),
                KeyCode::Char('q') => self.exit(),
                _ => {}
            },

            _ => match key_event.code {
                KeyCode::Char('q') => self.exit(),
                KeyCode::Char('r') => self.start_register_user(),
                KeyCode::Char('c') => self.register_character().await,
                KeyCode::Char('f') => self.attack_first_enemy(5).await,
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

    async fn tavern_buy_item(&mut self, item_name: &str, cost: u32) {
        let coins = self.active_character.as_ref().map(|c| c.character.coins).unwrap_or(0);
        if coins < cost {
            return;
        }
        let uid = self.active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            if let Some(c) = self.active_character.as_mut() {
                c.character.coins -= cost;
            }
            let (new_coins, renown) = self.active_character.as_ref()
                .map(|c| (c.character.coins, c.character.renown))
                .unwrap_or_default();
            let _ = self.client.save_character_stats(uid, new_coins, renown).await;
            if self.client.post_give_item(uid, item_name).await.is_ok() {
                self.inventory = self.client.get_character_items(uid).await.unwrap_or_default();
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

    async fn finish_register_user(&mut self) {
        self.toggle_text_input(None);
        self.active_user = match self.get_and_clear_text_input() {
            Some(name) => self.register_user(name).await,
            _ => None,
        };
        let uid = self.active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            self.active_character = self.client.post_new_character(&uid).await.ok();
            self.inventory = self.client.get_character_items(uid).await.unwrap_or_default();
            self.active_party = self.client.get_party_for_user(uid).await.ok();
            self.state = AppState::Tavern(TavernState::Main);
        }
    }

    async fn register_user(&self, username: String) -> Option<User> {
        self.client.post_register_user(username).await.ok()
    }

    async fn register_character(&mut self) {
        let uid = self.active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            self.active_character = self.client.post_new_character(&uid).await.ok();
            self.inventory = self.client.get_character_items(uid).await.unwrap_or_default();
        }
    }

    async fn open_party(&mut self) {
        if self.active_user.is_none() {
            return;
        }
        let party_id = self.active_party.as_ref().map(|p| p.id);
        if let Some(pid) = party_id {
            self.party_members = self.client.get_party_members_for_party(pid).await.unwrap_or_default();
            self.state = AppState::Party;
        } else {
            let parties = self.client.get_open_parties().await.unwrap_or_default();
            self.state = AppState::PartyLobby { parties };
        }
    }

    async fn join_party_from_lobby(&mut self, index: usize) {
        let (party_id, user_id) = match &self.state {
            AppState::PartyLobby { parties } => match parties.get(index) {
                Some(p) => (p.id, self.active_user.as_ref().map(|u| u.id).unwrap_or(0)),
                None => return,
            },
            _ => return,
        };
        if let Ok(party) = self.client.post_join_party(party_id, user_id).await {
            self.active_party = Some(party);
            let pid = self.active_party.as_ref().map(|p| p.id);
            if let Some(pid) = pid {
                self.party_members = self.client.get_party_members_for_party(pid).await.unwrap_or_default();
            }
            self.state = AppState::Party;
        }
    }

    async fn create_new_party(&mut self) {
        let uid = self.active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            if let Ok(party) = self.client.post_create_party(uid).await {
                self.active_party = Some(party);
                let pid = self.active_party.as_ref().map(|p| p.id);
                if let Some(pid) = pid {
                    self.party_members = self.client.get_party_members_for_party(pid).await.unwrap_or_default();
                }
                self.state = AppState::Party;
            }
        }
    }

    async fn leave_party(&mut self) {
        let uid = self.active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            let _ = self.client.delete_leave_party(uid).await;
        }
        self.active_party = None;
        self.party_members.clear();
        self.state = AppState::Tavern(TavernState::Main);
    }

    async fn start_quest(&mut self, area: &str) {
        let uid = self.active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            self.active_quest = self.client.post_new_quest(uid, area).await.ok();
            self.fetch_party_members().await;
            self.state = AppState::Main;
            self.check_current_encounter().await;
        }
    }

    async fn sync_quest_state(&mut self) {
        let node = match &self.state {
            AppState::Dialogue { current_node, .. } => Some(current_node.clone()),
            _ => None,
        };
        if let Some(q) = &self.active_quest {
            let quest_id = q.id;
            let current_encounter = q.current_encounter;
            let encounters = q.encounters.clone();
            let _ = self.client.put_encounters(quest_id, current_encounter, node.as_deref(), &encounters).await;
        }
    }

    async fn refresh_quest_state(&mut self) {
        let quest_id = match &self.active_quest {
            Some(q) => q.id,
            None => return,
        };
        match self.client.get_quest(quest_id).await {
            Ok(updated) => {
                let enc_changed = self.active_quest.as_ref()
                    .map(|q| q.current_encounter != updated.current_encounter)
                    .unwrap_or(false);

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
                    self.check_current_encounter().await;
                } else if let Some(node_id) = updated.current_node_id {
                    if let AppState::Dialogue { current_node, .. } = &mut self.state {
                        *current_node = node_id;
                    }
                }
                self.fetch_party_members().await;
            }
            Err(_) => {
                self.active_quest = None;
                let uid = self.active_user.as_ref().map(|u| u.id);
                if let Some(uid) = uid {
                    if let Ok(updated) = self.client.get_character(uid).await {
                        self.active_character = Some(updated);
                    }
                }
                let party_id = self.active_party.as_ref().map(|p| p.id);
                if let Some(pid) = party_id {
                    self.party_members = self.client.get_party_members_for_party(pid).await.unwrap_or_default();
                    self.state = AppState::Party;
                } else {
                    self.party_members.clear();
                    self.state = AppState::Tavern(TavernState::Main);
                }
            }
        }
    }

    async fn fetch_party_members(&mut self) {
        let quest_id = self.active_quest.as_ref().map(|q| q.id);
        if let Some(qid) = quest_id {
            self.party_members = self.client.get_quest_members(qid).await.unwrap_or_default();
        }
    }

    async fn check_current_encounter(&mut self) {
        let (enc, quest_done) = match &self.active_quest {
            Some(q) => {
                let idx = q.current_encounter as usize;
                (q.encounters.get(idx).cloned(), idx >= q.encounters.len())
            }
            None => (None, false),
        };

        if quest_done {
            self.complete_quest().await;
            return;
        }

        match enc {
            Some(Encounter::NpcEncounter(id)) => {
                if let Ok(dialogue) = self.client.get_dialogue(&id).await {
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

    async fn complete_quest(&mut self) {
        let (quest_id, user_id) = match (&self.active_quest, &self.active_user) {
            (Some(q), Some(u)) => (q.id, u.id),
            _ => return,
        };
        let stats = self.active_character.as_ref().map(|c| (c.character.coins, c.character.renown));
        if let Some((coins, renown)) = stats {
            let _ = self.client.save_character_stats(user_id, coins, renown).await;
        }
        if let Ok(updated) = self.client.post_complete_quest(quest_id, user_id).await {
            self.active_character = Some(updated);
        }
        self.active_quest = None;
        let party_id = self.active_party.as_ref().map(|p| p.id);
        if let Some(pid) = party_id {
            self.party_members = self.client.get_party_members_for_party(pid).await.unwrap_or_default();
            self.state = AppState::Party;
        } else {
            self.state = AppState::Tavern(TavernState::Main);
        }
    }

    async fn refresh_party_state(&mut self) {
        let party_id = self.active_party.as_ref().map(|p| p.id);
        if let Some(pid) = party_id {
            self.party_members = self.client.get_party_members_for_party(pid).await.unwrap_or_default();
        }
        let uid = self.active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            if let Ok(quest) = self.client.get_active_quest_for_user(uid).await {
                self.active_quest = Some(quest);
                self.fetch_party_members().await;
                self.check_current_encounter().await;
            }
        }
    }

    async fn pick_dialogue_choice(&mut self, index: usize) {
        let (next, outcome) = match &self.state {
            AppState::Dialogue { dialogue, current_node } => {
                match dialogue.nodes.get(current_node) {
                    Some(node) => match node.choices.get(index) {
                        Some(choice) => {
                            let coins = self.active_character.as_ref()
                                .map(|c| c.character.coins as i32)
                                .unwrap_or(0);
                            let outcome_costs_more_than = |outcome: &DialogueOutcome| match outcome {
                                DialogueOutcome::GiveItem { cost, .. } => coins < *cost,
                                DialogueOutcome::Reward { coins: c, .. } => *c < 0 && coins < c.unsigned_abs() as i32,
                                _ => false,
                            };
                            let locked = match &choice.outcome {
                                Some(outcome) => outcome_costs_more_than(outcome),
                                None => {
                                    if let Some(next_id) = &choice.next {
                                        if let Some(next_node) = dialogue.nodes.get(next_id.as_str()) {
                                            !next_node.choices.is_empty()
                                                && next_node.choices.iter().all(|c| match &c.outcome {
                                                    Some(outcome) => outcome_costs_more_than(outcome),
                                                    _ => false,
                                                })
                                        } else { false }
                                    } else { false }
                                }
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
                self.sync_quest_state().await;
            }
            (None, Some(outcome)) => {
                self.apply_dialogue_outcome(outcome).await;
            }
            (None, None) => {
                self.state = AppState::Tavern(TavernState::Main);
                if let Some(q) = self.active_quest.as_mut() {
                    q.current_encounter += 1;
                }
                self.regen_energy().await;
                self.sync_quest_state().await;
            }
        }
    }

    async fn apply_dialogue_outcome(&mut self, outcome: DialogueOutcome) {
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
                let stats = self.active_user.as_ref()
                    .zip(self.active_character.as_ref())
                    .map(|(u, c)| (u.id, c.character.coins, c.character.renown));
                if let Some((uid, coins, renown)) = stats {
                    let _ = self.client.save_character_stats(uid, coins, renown).await;
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
                self.state = AppState::Combat;
                self.sync_quest_state().await;
                return;
            }
            DialogueOutcome::GiveItem { item_name, cost } => {
                if let Some(c) = self.active_character.as_mut() {
                    c.character.coins = (c.character.coins as i32 - cost).max(0) as u32;
                }
                let uid = self.active_user.as_ref().map(|u| u.id);
                let (new_coins, renown) = self.active_character.as_ref()
                    .map(|c| (c.character.coins, c.character.renown))
                    .unwrap_or_default();
                if let Some(uid) = uid {
                    let _ = self.client.save_character_stats(uid, new_coins, renown).await;
                    let _ = self.client.post_give_item(uid, &item_name).await;
                    self.inventory = self.client.get_character_items(uid).await.unwrap_or_default();
                }
                if let Some(q) = self.active_quest.as_mut() {
                    q.current_encounter += 1;
                }
            }
            DialogueOutcome::Escape => {
                return;
            }
        }
        self.regen_energy().await;
        self.sync_quest_state().await;
        self.check_current_encounter().await;
    }

    async fn attack_first_enemy(&mut self, damage: i32) {
        // Check and spend 1 EP
        if let Some(c) = self.active_character.as_mut() {
            if c.unit.energy <= 0 {
                return;
            }
            c.unit.energy -= 1;
        }
        let update = self.active_user.as_ref()
            .zip(self.active_character.as_ref())
            .map(|(u, c)| (u.id, c.unit));
        if let Some((uid, unit)) = update {
            let _ = self.client.update_character_unit(uid, &unit).await;
        }

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
                        if let Some(item) = m.items.iter_mut().find(|it| it.charges != 0) {
                            let dmg = match item.effect { ratback_types::data::ItemEffect::Damage(d) => d, _ => m.attack };
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
            let update = self.active_user.as_ref()
                .zip(self.active_character.as_ref())
                .map(|(u, c)| (u.id, c.unit));
            if let Some((uid, unit)) = update {
                let _ = self.client.update_character_unit(uid, &unit).await;
            }
        } else if encounter_cleared {
            self.last_combat_damage = None;
        }

        // push updated encounter state to backend
        if let Some(q) = &self.active_quest {
            let quest_id = q.id;
            let current_encounter = q.current_encounter;
            let encounters = q.encounters.clone();
            let _ = self.client.put_encounters(quest_id, current_encounter, None, &encounters).await;
        }

        if encounter_cleared {
            self.regen_energy().await;
            self.check_current_encounter().await;
        } else {
            let all_dead = self.active_character.as_ref().map_or(false, |c| c.unit.health <= 0)
                && self.party_members.iter().all(|m| m.unit.health <= 0);
            if all_dead {
                self.state = AppState::GameOver;
            }
        }
    }

    async fn use_item(&mut self, index: usize) {
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
                let update = self.active_user.as_ref()
                    .zip(self.active_character.as_ref())
                    .map(|(u, c)| (u.id, c.unit));
                if let Some((uid, unit)) = update {
                    let _ = self.client.update_character_unit(uid, &unit).await;
                }
            }
            ItemEffect::FullHeal => {
                if let Some(c) = self.active_character.as_mut() {
                    c.unit.health = c.unit.max_health;
                }
                let update = self.active_user.as_ref()
                    .zip(self.active_character.as_ref())
                    .map(|(u, c)| (u.id, c.unit));
                if let Some((uid, unit)) = update {
                    let _ = self.client.update_character_unit(uid, &unit).await;
                }
            }
            ItemEffect::MaxHpUp(amount) => {
                let amount = *amount;
                if let Some(c) = self.active_character.as_mut() {
                    c.unit.max_health += amount;
                    c.unit.health = (c.unit.health + amount).min(c.unit.max_health);
                }
                let update = self.active_user.as_ref()
                    .zip(self.active_character.as_ref())
                    .map(|(u, c)| (u.id, c.unit));
                if let Some((uid, unit)) = update {
                    let _ = self.client.update_character_unit(uid, &unit).await;
                }
            }
        }

        // consume one charge and re-sync from server
        if inv_item.item.charges != -1 {
            let uid = self.active_user.as_ref().map(|u| u.id);
            if let Some(uid) = uid {
                let _ = self.client.delete_character_item(uid, inv_item.item.id).await;
                self.inventory = self.client.get_character_items(uid).await.unwrap_or_default();
            }
        }

        if let Some(q) = &self.active_quest {
            let quest_id = q.id;
            let current_encounter = q.current_encounter;
            let encounters = q.encounters.clone();
            let _ = self.client.put_encounters(quest_id, current_encounter, None, &encounters).await;
        }
        if encounter_cleared {
            self.regen_energy().await;
            self.check_current_encounter().await;
        }
    }

    async fn regen_energy(&mut self) {
        if let Some(c) = self.active_character.as_mut() {
            c.unit.energy = (c.unit.energy + 5).min(c.unit.max_energy);
        }
        let update = self.active_user.as_ref()
            .zip(self.active_character.as_ref())
            .map(|(u, c)| (u.id, c.unit));
        if let Some((uid, unit)) = update {
            let _ = self.client.update_character_unit(uid, &unit).await;
        }
    }

    pub fn dead_targets(&self) -> Vec<(i32, String)> {
        let mut targets = vec![];
        if let (Some(user), Some(c)) = (&self.active_user, &self.active_character) {
            if c.unit.health <= 0 {
                targets.push((user.id, c.character.name.clone()));
            }
        }
        for m in &self.party_members {
            if m.unit.health <= 0 {
                targets.push((m.character.user_id, m.character.name.clone()));
            }
        }
        targets
    }

    async fn apply_full_heal_to_target(&mut self, item_index: usize, target_user_id: i32) {
        let active_user_id = self.active_user.as_ref().map(|u| u.id);
        if active_user_id == Some(target_user_id) {
            let unit = self.active_character.as_mut().map(|c| {
                c.unit.health = c.unit.max_health;
                c.unit
            });
            if let Some(unit) = unit {
                let _ = self.client.update_character_unit(target_user_id, &unit).await;
            }
        } else {
            let unit = self.party_members.iter_mut()
                .find(|m| m.character.user_id == target_user_id)
                .map(|m| {
                    m.unit.health = m.unit.max_health;
                    m.unit
                });
            if let Some(unit) = unit {
                let _ = self.client.update_character_unit(target_user_id, &unit).await;
            }
        }
        let inv_item = self.inventory.get(item_index).cloned();
        if let Some(inv_item) = inv_item {
            if inv_item.item.charges != -1 {
                let uid = self.active_user.as_ref().map(|u| u.id);
                if let Some(uid) = uid {
                    let _ = self.client.delete_character_item(uid, inv_item.item.id).await;
                    self.inventory = self.client.get_character_items(uid).await.unwrap_or_default();
                }
            }
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
            AppState::GameOver => {
                self.render_combat(parent_layout[1], buf, text_style);
                let popup_width = 40.min(area.width.saturating_sub(4));
                let popup_height = 8.min(area.height.saturating_sub(4));
                let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
                let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
                let rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
                Clear::default().render(rect, buf);
                self.render_game_over(rect, buf, text_style);
            }
            AppState::TargetSelect { target_selected, .. } => {
                let popup_width = 40.min(area.width.saturating_sub(4));
                let popup_height = 10.min(area.height.saturating_sub(4));
                let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
                let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
                let rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
                Clear::default().render(rect, buf);
                self.render_target_select(rect, buf, text_style, *target_selected);
            }
            _ => {}
        }
    }
}
