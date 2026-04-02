use super::*;
use color_eyre::{Result, eyre::WrapErr};
use std::time::{Duration, Instant};
#[cfg(not(target_arch = "wasm32"))]
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
#[cfg(not(target_arch = "wasm32"))]
use futures::StreamExt;

impl App {
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn run(&mut self, terminal: &mut tui::Tui) -> Result<()> {
        self.backend_version = self.client.get_backend_version().await;
        const REFRESH: Duration = Duration::from_millis(300);
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

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
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
                                ItemEffect::Damage(_) => matches!(*previous, AppState::Encounter(EncounterState::Combat { .. })),
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
                    KeyCode::Char('o') if has_char => self.state = AppState::Tavern(TavernState::Options),
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

            AppState::Tavern(TavernState::Options) => match key_event.code {
                KeyCode::Char('c') => self.start_register_user(),
                KeyCode::Char('r') => {
                    self.text_input = Some("".to_string());
                    self.state = AppState::TextInput(Reason::Rename);
                }
                KeyCode::Char('p') => {
                    let sel = self.palette_index;
                    self.state = AppState::Tavern(TavernState::PaletteSelect { selected: sel });
                }
                KeyCode::Esc | KeyCode::Char('q') => self.state = AppState::Tavern(TavernState::Main),
                _ => {}
            },

            AppState::Tavern(TavernState::PaletteSelect { selected }) => {
                let selected = *selected;
                match key_event.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.state = AppState::Tavern(TavernState::PaletteSelect { selected: selected.saturating_sub(1) });
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let s = (selected + 1).min(crate::ui::PALETTES.len() - 1);
                        self.state = AppState::Tavern(TavernState::PaletteSelect { selected: s });
                    }
                    KeyCode::Enter => {
                        self.palette_index = selected;
                        self.state = AppState::Tavern(TavernState::Options);
                    }
                    KeyCode::Esc | KeyCode::Char('q') => self.state = AppState::Tavern(TavernState::Options),
                    _ => {}
                }
            }

            AppState::TextInput(_) => {
                let reason = match &self.state { AppState::TextInput(r) => *r, _ => unreachable!() };
                match key_event.code {
                    KeyCode::Enter => match reason {
                        Reason::Rename => self.finish_rename_character().await,
                        _ => self.finish_register_user().await,
                    },
                    KeyCode::Char(value) => {
                        if let Some(current) = self.text_input.as_mut() { current.push(value); }
                    }
                    KeyCode::Backspace => {
                        if let Some(current) = self.text_input.as_mut() { current.pop(); }
                    }
                    KeyCode::Esc => self.state = AppState::Tavern(TavernState::Main),
                    _ => {}
                }
            }

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
                    KeyCode::Char('2') if renown >= RENOWN_SEWER_DEPTHS => self.start_quest(AREA_SEWER_DEPTHS).await,
                    KeyCode::Char('3') if renown >= RENOWN_FUNGAL_WARRENS => self.start_quest(AREA_FUNGAL_WARRENS).await,
                    KeyCode::Char('4') if renown >= RENOWN_ABYSS => self.start_quest(AREA_ABYSS).await,
                    KeyCode::Char('5') => self.open_missions().await,
                    KeyCode::Esc | KeyCode::Char('q') => self.state = AppState::Tavern(TavernState::Main),
                    _ => {}
                }
            }

            AppState::MissionSelect { .. } => {
                let (_selected, count) = if let AppState::MissionSelect { selected, missions } = &self.state {
                    (*selected, missions.len())
                } else { (0, 0) };
                match key_event.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if let AppState::MissionSelect { selected, .. } = &mut self.state {
                            *selected = selected.saturating_sub(1);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if let AppState::MissionSelect { selected, .. } = &mut self.state {
                            if *selected + 1 < count { *selected += 1; }
                        }
                    }
                    KeyCode::Enter => {
                        let mission_id = if let AppState::MissionSelect { missions, selected } = &self.state {
                            missions.get(*selected)
                                .filter(|m| m.state == MissionState::Ready)
                                .map(|m| m.mission_id.clone())
                        } else { None };
                        if let Some(mid) = mission_id {
                            self.start_mission_quest(&mid).await;
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('q') => self.state = AppState::AdventureMenu,
                    _ => {}
                }
            }

            AppState::Encounter(EncounterState::Combat { cleared: true })
            | AppState::Encounter(EncounterState::Dialogue { cleared: true, .. }) => match key_event.code {
                KeyCode::Char('g') | KeyCode::Enter => {
                    if let Some(q) = self.active_quest.as_mut() { q.current_encounter += 1; }
                    self.regen_energy().await;
                    self.sync_quest_state().await;
                    self.check_current_encounter().await;
                }
                _ => {}
            },

            AppState::GameOver => match key_event.code {
                KeyCode::Char('i') => {
                    let has_revive = self.inventory.iter().any(|i| {
                        matches!(i.item.effect, ItemEffect::FullHeal) && i.charges_remaining != 0
                    });
                    if has_revive {
                        self.state = AppState::Inventory {
                            scroll: 0,
                            selected: 0,
                            previous: Box::new(AppState::Encounter(EncounterState::Combat { cleared: false })),
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

            AppState::Victory => match key_event.code {
                KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => {
                    self.state = AppState::Tavern(TavernState::Main);
                }
                _ => {}
            },

            AppState::Encounter(EncounterState::Combat { cleared: false }) => match key_event.code {
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

            AppState::Encounter(EncounterState::Dialogue { cleared: false, .. }) => match key_event.code {
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
}
