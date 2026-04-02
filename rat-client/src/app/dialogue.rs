use super::*;

impl App {
    pub(super) async fn pick_dialogue_choice(&mut self, index: usize) {
        let (next, outcome) = match &self.state {
            AppState::Encounter(EncounterState::Dialogue { dialogue, current_node, .. }) => {
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
                if let AppState::Encounter(EncounterState::Dialogue { current_node, .. }) = &mut self.state {
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
                if let AppState::Encounter(EncounterState::Dialogue { cleared, .. }) = &mut self.state {
                    *cleared = true;
                }
            }
        }
    }

    pub(super) async fn apply_dialogue_outcome(&mut self, outcome: DialogueOutcome) {
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
            }
            DialogueOutcome::Damage { amount } => {
                if let Some(c) = self.active_character.as_mut() {
                    c.unit.health = (c.unit.health - amount).max(0);
                }
            }
            DialogueOutcome::NextEncounter => {}
            DialogueOutcome::Combat(combat) => {
                let idx = self.active_quest.as_ref().map(|q| q.current_encounter as usize).unwrap_or(0);
                if let Some(q) = self.active_quest.as_mut() {
                    if let Some(enc) = q.encounters.get_mut(idx) {
                        *enc = Encounter::CombatEncounter(combat);
                    }
                    q.current_node_id = None;
                }
                self.state = AppState::Encounter(EncounterState::Combat { cleared: false });
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
            }
            DialogueOutcome::Escape => {
                return;
            }
            DialogueOutcome::GiveClue { clue_id } => {
                let char_id = self.active_character.as_ref().map(|c| c.character.id);
                if let Some(cid) = char_id {
                    if let Ok(Some(unlocked)) = self.client.post_clue(cid, &clue_id).await {
                        self.clue_notification = Some(format!("Clue found! \"{}\" is now available.", unlocked.title));
                        self.missions = self.client.get_missions(cid).await.unwrap_or_default();
                    }
                }
            }
        }
        if let AppState::Encounter(EncounterState::Dialogue { cleared, .. }) = &mut self.state {
            *cleared = true;
        }
    }

    // ── WASM counterparts ─────────────────────────────────────────────────────

    #[cfg(target_arch = "wasm32")]
    pub(super) async fn wasm_pick_dialogue_choice(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
        index: usize,
    ) {
        use ratback_types::quest_data::DialogueOutcome;
        let (next, outcome) = {
            let a = app.borrow();
            match &a.state {
                AppState::Encounter(EncounterState::Dialogue { dialogue, current_node, .. }) => {
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
                    if let AppState::Encounter(EncounterState::Dialogue { current_node, .. }) = &mut a.state {
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
                if let AppState::Encounter(EncounterState::Dialogue { cleared, .. }) = &mut app.borrow_mut().state {
                    *cleared = true;
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) async fn wasm_apply_dialogue_outcome(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
        outcome: ratback_types::quest_data::DialogueOutcome,
    ) {
        use ratback_types::quest_data::{DialogueOutcome, Encounter};
        match outcome {
            DialogueOutcome::Reward { coins, renown, heal } => {
                let uid_stats = {
                    let mut a = app.borrow_mut();
                    if let Some(c) = a.active_character.as_mut() {
                        c.character.coins = (c.character.coins as i32 + coins).max(0) as u32;
                        c.character.renown = (c.character.renown as i32 + renown).max(0) as u32;
                        if heal != 0 { c.unit.health = (c.unit.health + heal).clamp(0, c.unit.max_health); }
                    }
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
            }
            DialogueOutcome::NextEncounter => {}
            DialogueOutcome::Combat(combat) => {
                {
                    let mut a = app.borrow_mut();
                    let idx = a.active_quest.as_ref().map(|q| q.current_encounter as usize).unwrap_or(0);
                    if let Some(q) = a.active_quest.as_mut() {
                        if let Some(enc) = q.encounters.get_mut(idx) { *enc = Encounter::CombatEncounter(combat); }
                        q.current_node_id = None;
                    }
                    a.state = AppState::Encounter(EncounterState::Combat { cleared: false });
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
            DialogueOutcome::GiveClue { clue_id } => {
                let char_id = app.borrow().active_character.as_ref().map(|c| c.character.id);
                if let Some(cid) = char_id {
                    if let Ok(Some(unlocked)) = client.post_clue(cid, &clue_id).await {
                        app.borrow_mut().clue_notification = Some(format!("Clue found! \"{}\" is now available.", unlocked.title));
                        let missions = client.get_missions(cid).await.unwrap_or_default();
                        app.borrow_mut().missions = missions;
                    }
                }
            }
        }
        if let AppState::Encounter(EncounterState::Dialogue { cleared, .. }) = &mut app.borrow_mut().state {
            *cleared = true;
        }
    }
}
