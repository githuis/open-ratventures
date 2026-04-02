use super::*;

impl App {
    pub(super) async fn start_quest(&mut self, area: &str) {
        let uid = self.active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            self.active_quest = self.client.post_new_quest(uid, area).await.ok();
            self.fetch_party_members().await;
            self.state = AppState::Main;
            self.check_current_encounter().await;
        }
    }

    pub(super) async fn sync_quest_state(&mut self) {
        let node = match &self.state {
            AppState::Encounter(EncounterState::Dialogue { current_node, .. }) => Some(current_node.clone()),
            _ => None,
        };
        if let Some(q) = &self.active_quest {
            let quest_id = q.id;
            let current_encounter = q.current_encounter;
            let encounters = q.encounters.clone();
            let _ = self.client.put_encounters(quest_id, current_encounter, node.as_deref(), &encounters).await;
        }
    }

    pub(super) async fn refresh_quest_state(&mut self) {
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
                    if let AppState::Encounter(EncounterState::Dialogue { current_node, .. }) = &mut self.state {
                        *current_node = node_id;
                    }
                }
                self.fetch_party_members().await;
                if matches!(self.state, AppState::GameOver) {
                    let someone_alive = self.party_members.iter().any(|m| m.unit.health > 0);
                    if someone_alive {
                        self.check_current_encounter().await;
                    }
                }
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

    pub(super) async fn check_current_encounter(&mut self) {
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
                    self.state = AppState::Encounter(EncounterState::Dialogue { dialogue, current_node: start, cleared: false });
                }
            }
            Some(Encounter::CombatEncounter(_)) => {
                self.last_combat_damage = None;
                self.state = AppState::Encounter(EncounterState::Combat { cleared: false });
            }
            _ => {
                self.state = AppState::Main;
            }
        }
    }

    pub(super) async fn complete_quest(&mut self) {
        let (quest_id, user_id) = match (&self.active_quest, &self.active_user) {
            (Some(q), Some(u)) => (q.id, u.id),
            _ => return,
        };
        let stats = self.active_character.as_ref().map(|c| (c.character.coins, c.character.renown));
        if let Some((coins, renown)) = stats {
            let _ = self.client.save_character_stats(user_id, coins, renown).await;
        }
        if let Ok(updated) = self.client.post_complete_quest(quest_id, user_id).await {
            let cid = updated.character.id;
            self.active_character = Some(updated);
            // Refresh missions and check for victory
            let missions = self.client.get_missions(cid).await.unwrap_or_default();
            let won = !missions.is_empty() && missions.iter().all(|m| m.state == MissionState::Complete);
            self.missions = missions;
            if won {
                self.active_quest = None;
                self.state = AppState::Victory;
                return;
            }
        }
        self.active_quest = None;
        self.state = AppState::Tavern(TavernState::Main);
    }

    pub(super) async fn regen_energy(&mut self) {
        if let Some(c) = self.active_character.as_mut() {
            c.unit.energy = (c.unit.energy + 7).min(c.unit.max_energy);
        }
        let update = self.active_user.as_ref()
            .zip(self.active_character.as_ref())
            .map(|(u, c)| (u.id, c.unit));
        if let Some((uid, unit)) = update {
            let _ = self.client.update_character_unit(uid, &unit).await;
        }
    }

    pub(super) async fn open_missions(&mut self) {
        let char_id = self.active_character.as_ref().map(|c| c.character.id);
        if let Some(cid) = char_id {
            let missions = self.client.get_missions(cid).await.unwrap_or_default();
            self.missions = missions.clone();
            self.state = AppState::MissionSelect { missions, selected: 0 };
        }
    }

    pub(super) async fn start_mission_quest(&mut self, mission_id: &str) {
        let uid = self.active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            self.active_quest = self.client.post_new_quest_mission(uid, mission_id).await.ok();
            self.fetch_party_members().await;
            self.state = AppState::Main;
            self.check_current_encounter().await;
        }
    }

    // ── WASM counterparts ─────────────────────────────────────────────────────

    #[cfg(target_arch = "wasm32")]
    pub(super) async fn wasm_start_quest(
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
    pub(super) async fn wasm_sync_quest_state(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
    ) {
        let (quest_id, current_encounter, encounters, node) = {
            let a = app.borrow();
            let node = if let AppState::Encounter(EncounterState::Dialogue { current_node, .. }) = &a.state {
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
    pub(super) async fn wasm_regen_energy(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
    ) {
        let update = {
            let mut a = app.borrow_mut();
            if let Some(c) = a.active_character.as_mut() {
                c.unit.energy = (c.unit.energy + 7).min(c.unit.max_energy);
            }
            a.active_user.as_ref().zip(a.active_character.as_ref()).map(|(u, c)| (u.id, c.unit))
        };
        if let Some((uid, unit)) = update {
            let _ = client.update_character_unit(uid, &unit).await;
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) async fn wasm_complete_quest(
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
        let char_id = updated.as_ref().map(|c| c.character.id);
        {
            let mut a = app.borrow_mut();
            a.active_character = updated;
            a.active_quest = None;
        }
        // Refresh missions and check for victory (all missions complete)
        if let Some(cid) = char_id {
            let missions = client.get_missions(cid).await.unwrap_or_default();
            let won = !missions.is_empty() && missions.iter().all(|m| m.state == MissionState::Complete);
            app.borrow_mut().missions = missions;
            if won {
                app.borrow_mut().state = AppState::Victory;
                return;
            }
        }
        app.borrow_mut().state = AppState::Tavern(TavernState::Main);
    }

    /// Periodic poll — mirrors the native `refresh_party_state` / `refresh_quest_state` loop.
    #[cfg(target_arch = "wasm32")]
    pub(super) async fn wasm_poll(app: std::rc::Rc<std::cell::RefCell<Self>>) {
        let client = app.borrow().client.clone();
        let (has_quest, has_party, uid) = {
            let a = app.borrow();
            (a.active_quest.is_some(), a.active_party.is_some(), a.active_user.as_ref().map(|u| u.id))
        };
        let Some(uid) = uid else { return };

        if has_quest {
            // Sync quest state with server
            let quest_id = app.borrow().active_quest.as_ref().map(|q| q.id);
            if let Some(qid) = quest_id {
                match client.get_quest(qid).await {
                    Ok(updated) => {
                        let (enc_changed, enc_type_changed) = {
                            let a = app.borrow();
                            let enc_changed = a.active_quest.as_ref()
                                .map(|q| q.current_encounter != updated.current_encounter)
                                .unwrap_or(false);
                            let idx = updated.current_encounter as usize;
                            let old_is_npc = a.active_quest.as_ref()
                                .and_then(|q| q.encounters.get(idx))
                                .map(|e| matches!(e, ratback_types::quest_data::Encounter::NpcEncounter(_)))
                                .unwrap_or(false);
                            let new_is_combat = updated.encounters.get(idx)
                                .map(|e| matches!(e, ratback_types::quest_data::Encounter::CombatEncounter(_)))
                                .unwrap_or(false);
                            (enc_changed, old_is_npc && new_is_combat)
                        };
                        {
                            let mut a = app.borrow_mut();
                            if let Some(q) = a.active_quest.as_mut() {
                                q.current_encounter = updated.current_encounter;
                                q.encounters = updated.encounters;
                                q.current_node_id = updated.current_node_id.clone();
                            }
                        }
                        if enc_changed || enc_type_changed {
                            App::wasm_check_current_encounter(&app, &client).await;
                        }
                        // Refresh party HP
                        App::wasm_fetch_party_members(&app, &client).await;
                        if matches!(app.borrow().state, AppState::GameOver) {
                            let someone_alive = app.borrow().party_members.iter().any(|m| m.unit.health > 0);
                            if someone_alive {
                                App::wasm_check_current_encounter(&app, &client).await;
                            }
                        }
                    }
                    Err(_) => {
                        // Quest ended — fall back to party or tavern
                        if let Ok(updated) = client.get_character(uid).await {
                            app.borrow_mut().active_character = Some(updated);
                        }
                        let party_id = app.borrow().active_party.as_ref().map(|p| p.id);
                        app.borrow_mut().active_quest = None;
                        if let Some(pid) = party_id {
                            let members = client.get_party_members_for_party(pid).await.unwrap_or_default();
                            let mut a = app.borrow_mut();
                            a.party_members = members;
                            a.state = AppState::Party;
                        } else {
                            app.borrow_mut().state = AppState::Tavern(TavernState::Main);
                        }
                    }
                }
            }
        } else if has_party {
            // Refresh party members and check if someone started a quest
            let party_id = app.borrow().active_party.as_ref().map(|p| p.id);
            if let Some(pid) = party_id {
                let members = client.get_party_members_for_party(pid).await.unwrap_or_default();
                app.borrow_mut().party_members = members;
            }
            if let Ok(quest) = client.get_active_quest_for_user(uid).await {
                app.borrow_mut().active_quest = Some(quest);
                App::wasm_fetch_party_members(&app, &client).await;
                App::wasm_check_current_encounter(&app, &client).await;
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) async fn wasm_check_current_encounter(
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
                    app.borrow_mut().state = AppState::Encounter(EncounterState::Dialogue { dialogue, current_node: start, cleared: false });
                }
            }
            Some(Encounter::CombatEncounter(_)) => {
                let mut a = app.borrow_mut();
                a.last_combat_damage = None;
                a.state = AppState::Encounter(EncounterState::Combat { cleared: false });
            }
            _ => { app.borrow_mut().state = AppState::Main; }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) async fn wasm_open_missions(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
    ) {
        let char_id = app.borrow().active_character.as_ref().map(|c| c.character.id);
        if let Some(cid) = char_id {
            let missions = client.get_missions(cid).await.unwrap_or_default();
            app.borrow_mut().missions = missions.clone();
            app.borrow_mut().state = AppState::MissionSelect { missions, selected: 0 };
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) async fn wasm_start_mission_quest(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
        mission_id: String,
    ) {
        let uid = app.borrow().active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            let quest = client.post_new_quest_mission(uid, &mission_id).await.ok();
            app.borrow_mut().active_quest = quest;
            App::wasm_fetch_party_members(app, client).await;
            app.borrow_mut().state = AppState::Main;
            App::wasm_check_current_encounter(app, client).await;
        }
    }
}
