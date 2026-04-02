use super::*;

impl App {
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

        // Fetch backend version once on startup
        {
            let app_v = app.clone();
            let term_v = terminal.clone();
            spawn_local(async move {
                let client = app_v.borrow().client.clone();
                let v = client.get_backend_version().await;
                app_v.borrow_mut().backend_version = v;
                term_v.borrow_mut().draw(|f| app_v.borrow().render_frame(f)).ok();
            });
        }

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

        // Poll server state every 3 seconds (mirrors native refresh loop)
        let app_p = app.clone();
        let term_p = terminal.clone();
        spawn_local(async move {
            use gloo_timers::future::IntervalStream;
            use futures::StreamExt;
            let mut stream = IntervalStream::new(300);
            while stream.next().await.is_some() {
                { let mut a = app_p.borrow_mut(); a.spinner_tick = a.spinner_tick.wrapping_add(1); }
                App::wasm_poll(app_p.clone()).await;
                term_p.borrow_mut().draw(|f| app_p.borrow().render_frame(f)).ok();
            }
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

        {
            let mut a = app.borrow_mut();
            if a.is_processing { return; }
            a.is_processing = true;
        }

        App::handle_key_wasm_inner(app.clone(), key).await;
        app.borrow_mut().is_processing = false;
    }

    #[cfg(target_arch = "wasm32")]
    async fn handle_key_wasm_inner(
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
                                ItemEffect::Damage(_) => matches!(*previous, AppState::Encounter(EncounterState::Combat { .. })),
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
            let reason = if let AppState::TextInput(r) = app.borrow().state { r } else { unreachable!() };
            match key.code {
                KeyCode::Enter => match reason {
                    Reason::Rename => {
                        let name = {
                            let mut a = app.borrow_mut();
                            a.state = AppState::Tavern(TavernState::Main);
                            a.get_and_clear_text_input()
                        };
                        if let Some(name) = name {
                            let uid = app.borrow().active_user.as_ref().map(|u| u.id);
                            if let Some(uid) = uid {
                                client.put_rename_character(uid, name.clone()).await.ok();
                                if let Some(ref mut cw) = app.borrow_mut().active_character {
                                    cw.character.name = name;
                                }
                            }
                        }
                    }
                    _ => {
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
                },
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
                KeyCode::Char('o') if has_char => { app.borrow_mut().state = AppState::Tavern(TavernState::Options); }
                KeyCode::Char('v') => { app.borrow_mut().open_inventory(); }
                KeyCode::Char('q') => App::wasm_reload(),
                _ => {}
            }
            return;
        }

        // ── Tavern(Options) ───────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::Tavern(TavernState::Options)) {
            match key.code {
                KeyCode::Char('c') => App::wasm_reload(),
                KeyCode::Char('r') => {
                    let mut a = app.borrow_mut();
                    a.state = AppState::TextInput(Reason::Rename);
                    a.text_input = Some("".to_string());
                }
                KeyCode::Char('p') => {
                    let sel = app.borrow().palette_index;
                    app.borrow_mut().state = AppState::Tavern(TavernState::PaletteSelect { selected: sel });
                }
                KeyCode::Esc | KeyCode::Char('q') => { app.borrow_mut().state = AppState::Tavern(TavernState::Main); }
                _ => {}
            }
            return;
        }

        // ── Tavern(PaletteSelect) ─────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::Tavern(TavernState::PaletteSelect { .. })) {
            let selected = if let AppState::Tavern(TavernState::PaletteSelect { selected }) = app.borrow().state { selected } else { 0 };
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    let s = selected.saturating_sub(1);
                    app.borrow_mut().state = AppState::Tavern(TavernState::PaletteSelect { selected: s });
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let s = (selected + 1).min(crate::ui::PALETTES.len() - 1);
                    app.borrow_mut().state = AppState::Tavern(TavernState::PaletteSelect { selected: s });
                }
                KeyCode::Enter => {
                    let mut a = app.borrow_mut();
                    a.palette_index = selected;
                    a.state = AppState::Tavern(TavernState::Options);
                }
                KeyCode::Esc | KeyCode::Char('q') => { app.borrow_mut().state = AppState::Tavern(TavernState::Options); }
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
                KeyCode::Char('2') if renown >= RENOWN_SEWER_DEPTHS => App::wasm_start_quest(&app, &client, AREA_SEWER_DEPTHS).await,
                KeyCode::Char('3') if renown >= RENOWN_FUNGAL_WARRENS => App::wasm_start_quest(&app, &client, AREA_FUNGAL_WARRENS).await,
                KeyCode::Char('4') if renown >= RENOWN_ABYSS => App::wasm_start_quest(&app, &client, AREA_ABYSS).await,
                KeyCode::Char('5') => App::wasm_open_missions(&app, &client).await,
                KeyCode::Esc | KeyCode::Char('q') => { app.borrow_mut().state = AppState::Tavern(TavernState::Main); }
                _ => {}
            }
            return;
        }

        // ── MissionSelect ─────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::MissionSelect { .. }) {
            let (selected, count) = {
                let a = app.borrow();
                if let AppState::MissionSelect { selected, missions } = &a.state {
                    (*selected, missions.len())
                } else { (0, 0) }
            };
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if let AppState::MissionSelect { selected, .. } = &mut app.borrow_mut().state {
                        *selected = selected.saturating_sub(1);
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let AppState::MissionSelect { selected, .. } = &mut app.borrow_mut().state {
                        if *selected + 1 < count { *selected += 1; }
                    }
                }
                KeyCode::Enter => {
                    let mission_id = {
                        let a = app.borrow();
                        if let AppState::MissionSelect { missions, selected } = &a.state {
                            missions.get(*selected)
                                .filter(|m| m.state == MissionState::Ready)
                                .map(|m| m.mission_id.clone())
                        } else { None }
                    };
                    if let Some(mid) = mission_id {
                        App::wasm_start_mission_quest(&app, &client, mid).await;
                    }
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    app.borrow_mut().state = AppState::AdventureMenu;
                }
                _ => {}
            }
            return;
        }

        // ── EncounterCleared ──────────────────────────────────────────────────
        let encounter_cleared = matches!(app.borrow().state,
            AppState::Encounter(EncounterState::Combat { cleared: true })
            | AppState::Encounter(EncounterState::Dialogue { cleared: true, .. })
        );
        if encounter_cleared {
            match key.code {
                KeyCode::Char('g') | KeyCode::Enter => {
                    { let mut a = app.borrow_mut(); if let Some(q) = a.active_quest.as_mut() { q.current_encounter += 1; } }
                    App::wasm_regen_energy(app, client).await;
                    App::wasm_sync_quest_state(app, client).await;
                    App::wasm_check_current_encounter(app, client).await;
                }
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
                            previous: Box::new(AppState::Encounter(EncounterState::Combat { cleared: false })),
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

        // ── Victory ───────────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::Victory) {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => {
                    app.borrow_mut().state = AppState::Tavern(TavernState::Main);
                }
                _ => {}
            }
            return;
        }

        // ── Combat ────────────────────────────────────────────────────────────
        if matches!(app.borrow().state, AppState::Encounter(EncounterState::Combat { cleared: false })) {
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
        if matches!(app.borrow().state, AppState::Encounter(EncounterState::Dialogue { cleared: false, .. })) {
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
    pub(super) fn wasm_reload() {
        if let Some(w) = ratzilla::web_sys::window() {
            let _ = w.location().reload();
        }
    }
}
