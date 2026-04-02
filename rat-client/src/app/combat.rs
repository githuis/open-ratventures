use super::*;

impl App {
    pub(super) async fn attack_first_enemy(&mut self, damage: i32) {
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
            self.state = AppState::Encounter(EncounterState::Combat { cleared: true });
        } else {
            let all_dead = self.active_character.as_ref().map_or(false, |c| c.unit.health <= 0)
                && self.party_members.iter().all(|m| m.unit.health <= 0);
            if all_dead {
                self.state = AppState::GameOver;
            }
        }
    }

    pub(super) async fn use_item(&mut self, index: usize) {
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
            self.state = AppState::Encounter(EncounterState::Combat { cleared: true });
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

    pub(super) async fn apply_full_heal_to_target(&mut self, item_index: usize, target_user_id: i32) {
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

    // ── WASM counterparts ─────────────────────────────────────────────────────

    #[cfg(target_arch = "wasm32")]
    pub(super) async fn wasm_attack(
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
            app.borrow_mut().state = AppState::Encounter(EncounterState::Combat { cleared: true });
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
    pub(super) async fn wasm_use_item(
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
            app.borrow_mut().state = AppState::Encounter(EncounterState::Combat { cleared: true });
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) async fn wasm_apply_full_heal(
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
}
