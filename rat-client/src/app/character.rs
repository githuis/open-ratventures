use super::*;

impl App {
    pub(super) fn start_register_user(&mut self) {
        self.toggle_text_input(Some(Reason::Register));
    }

    pub(super) async fn finish_register_user(&mut self) {
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

    pub(super) async fn register_user(&self, username: String) -> Option<ratback_types::data::User> {
        self.client.post_register_user(username).await.ok()
    }

    pub(super) async fn finish_rename_character(&mut self) {
        self.state = AppState::Tavern(TavernState::Main);
        let name = self.get_and_clear_text_input();
        if let Some(name) = name {
            let uid = self.active_user.as_ref().map(|u| u.id);
            if let Some(uid) = uid {
                self.client.put_rename_character(uid, name.clone()).await.ok();
                if let Some(ref mut cw) = self.active_character {
                    cw.character.name = name;
                }
            }
        }
    }

    pub(super) async fn register_character(&mut self) {
        let uid = self.active_user.as_ref().map(|u| u.id);
        if let Some(uid) = uid {
            self.active_character = self.client.post_new_character(&uid).await.ok();
            self.inventory = self.client.get_character_items(uid).await.unwrap_or_default();
        }
    }

    pub(super) async fn tavern_buy_item(&mut self, item_name: &str, cost: u32) {
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

    pub(super) fn open_inventory(&mut self) {
        let previous = std::mem::replace(&mut self.state, AppState::Main);
        self.state = AppState::Inventory { scroll: 0, selected: 0, previous: Box::new(previous) };
    }

    pub(super) fn exit(&mut self) {
        self.exit = true;
    }

    pub(super) fn get_and_clear_text_input(&mut self) -> Option<String> {
        let value = self.text_input.clone();
        self.text_input = None;
        value
    }

    pub(super) fn toggle_text_input(&mut self, why: Option<Reason>) {
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
}
