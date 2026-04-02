use super::*;

impl App {
    pub(super) async fn open_party(&mut self) {
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

    pub(super) async fn join_party_from_lobby(&mut self, index: usize) {
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

    pub(super) async fn create_new_party(&mut self) {
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

    pub(super) async fn leave_party(&mut self) {
        let uid = self.active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            let _ = self.client.delete_leave_party(uid).await;
        }
        self.active_party = None;
        self.party_members.clear();
        self.state = AppState::Tavern(TavernState::Main);
    }

    pub(super) async fn fetch_party_members(&mut self) {
        let quest_id = self.active_quest.as_ref().map(|q| q.id);
        if let Some(qid) = quest_id {
            self.party_members = self.client.get_quest_members(qid).await.unwrap_or_default();
        }
    }

    pub(super) async fn refresh_party_state(&mut self) {
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

    // ── WASM counterparts ─────────────────────────────────────────────────────

    #[cfg(target_arch = "wasm32")]
    pub(super) async fn wasm_open_party(
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
    pub(super) async fn wasm_join_party_from_lobby(
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
    pub(super) async fn wasm_fetch_party_members(
        app: &std::rc::Rc<std::cell::RefCell<Self>>,
        client: &crate::client::Rattp,
    ) {
        let quest_id = app.borrow().active_quest.as_ref().map(|q| q.id);
        if let Some(qid) = quest_id {
            let members = client.get_quest_members(qid).await.unwrap_or_default();
            app.borrow_mut().party_members = members;
        }
    }
}
