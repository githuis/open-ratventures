use std::error::Error;

use ratback::{data::{CharacterWrapper, InventoryItem, ShopItem, User}, quest_data::{Dialogue, Party, PartySummary, Quest, QuestSummary}};
use reqwest::blocking::Client;

const DEFAULT_HOST: &str = "http://localhost:3000/api/";

#[derive(Debug)]
pub struct Rattp {
    pub http: Client,
    host: String,
}

impl Default for Rattp {
    fn default() -> Self {
        let host = std::env::var("RATQUEST_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
        Self { http: Client::new(), host }
    }
}

impl Rattp {

    fn destination(&self, path: &str) -> String {
        format!("{}{}", self.host, path)
    }

    /***********
     * Users
     ***********/

    pub fn get_hello(&self) -> Result<String, Box<dyn Error>> {
        let response: String = self.http.get(self.destination("hello-world")).send()?.text()?;
        Ok(response)
    }

    pub fn post_register_user(&self, username: String) -> Result<User, Box<dyn Error>> {
        let response = self.http.post(self.destination("register")).body(username).send()?.text()?;
        let usr: User = serde_json::from_str(&response)?;
        Ok(usr)
    }

    /***********
     * Characters
     ***********/

    pub fn post_new_character(&self, user_id: &i32) -> Result<CharacterWrapper, Box<dyn Error>> {
        let response = self.http.post(self.destination("character")).body(user_id.to_string()).send()?.text()?;
        let character: CharacterWrapper = serde_json::from_str(&response)?;
        Ok(character)
    }

    pub fn get_character(&self, user_id: i32) -> Result<CharacterWrapper, Box<dyn Error>> {
        let response = self.http.get(self.destination(&format!("character/{user_id}"))).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    pub fn update_character_unit(&self, user_id: i32, unit: &ratback::data::Unit) -> Result<(), Box<dyn Error>> {
        self.http.put(self.destination(&format!("character/{user_id}/unit"))).json(unit).send()?;
        Ok(())
    }

    pub fn save_character_stats(&self, user_id: i32, coins: u32, renown: u32) -> Result<(), Box<dyn Error>> {
        let body = serde_json::json!({ "coins": coins, "renown": renown });
        self.http.put(self.destination(&format!("character/{user_id}/stats"))).json(&body).send()?;
        Ok(())
    }

    /***********
     * Inventory
     ***********/

    pub fn post_give_item(&self, user_id: i32, item_name: &str) -> Result<(), Box<dyn Error>> {
        self.http.post(self.destination(&format!("character/{user_id}/items"))).json(item_name).send()?;
        Ok(())
    }

    pub fn get_character_items(&self, user_id: i32) -> Result<Vec<InventoryItem>, Box<dyn Error>> {
        let response = self.http.get(self.destination(&format!("character/{user_id}/items"))).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    pub fn delete_character_item(&self, user_id: i32, item_id: i32) -> Result<(), Box<dyn Error>> {
        self.http.delete(self.destination(&format!("character/{user_id}/items/{item_id}"))).send()?;
        Ok(())
    }

    /***********
     * Shop
     ***********/

    pub fn get_shop_items(&self) -> Result<Vec<ShopItem>, Box<dyn Error>> {
        let response = self.http.get(self.destination("shop")).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    /***********
     * Quests
     ***********/

    pub(crate) fn post_new_quest(&self, user_id: i32) -> Result<Quest, Box<dyn Error>> {
        let response = self.http.post(self.destination("quest")).json(&user_id).send()?.text()?;
        let quest: Quest = serde_json::from_str(&response)?;
        Ok(quest)
    }

    pub fn get_quest(&self, quest_id: i32) -> Result<Quest, Box<dyn Error>> {
        let response = self.http.get(self.destination(&format!("quest/{quest_id}"))).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    pub fn get_active_quest_for_user(&self, user_id: i32) -> Result<Quest, Box<dyn Error>> {
        let response = self.http.get(self.destination(&format!("quest/active/{user_id}"))).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    pub fn get_open_quests(&self) -> Result<Vec<QuestSummary>, Box<dyn Error>> {
        let response = self.http.get(self.destination("quest/open")).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    pub fn post_join_quest(&self, quest_id: i32, user_id: i32) -> Result<Quest, Box<dyn Error>> {
        let body = serde_json::json!({ "quest_id": quest_id, "user_id": user_id });
        let response = self.http.post(self.destination("quest/join")).json(&body).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    pub fn post_complete_quest(&self, quest_id: i32, user_id: i32) -> Result<CharacterWrapper, Box<dyn Error>> {
        let body = serde_json::json!({ "quest_id": quest_id, "user_id": user_id });
        let response = self.http.post(self.destination("quest/complete")).json(&body).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    pub fn get_quest_members(&self, quest_id: i32) -> Result<Vec<CharacterWrapper>, Box<dyn Error>> {
        let response = self.http.get(self.destination(&format!("quest/{quest_id}/members"))).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    pub fn put_encounters(&self, quest_id: i32, current_encounter: i32, current_node_id: Option<&str>, encounters: &Vec<ratback::quest_data::Encounter>) -> Result<(), Box<dyn Error>> {
        let body = serde_json::json!({ "current_encounter": current_encounter, "current_node_id": current_node_id, "encounters": encounters });
        self.http.put(self.destination(&format!("quest/{quest_id}/encounters"))).json(&body).send()?;
        Ok(())
    }

    /***********
     * Dialogue
     ***********/

    pub fn get_dialogue(&self, id: &str) -> Result<Dialogue, Box<dyn Error>> {
        let response = self.http.get(self.destination(&format!("dialogue/{id}"))).send()?.text()?;
        let dialogue: Dialogue = serde_json::from_str(&response)?;
        Ok(dialogue)
    }

    /***********
     * Parties
     ***********/

    pub fn post_create_party(&self, user_id: i32) -> Result<Party, Box<dyn Error>> {
        let response = self.http.post(self.destination("party")).json(&user_id).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    pub fn get_open_parties(&self) -> Result<Vec<PartySummary>, Box<dyn Error>> {
        let response = self.http.get(self.destination("party/open")).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    pub fn post_join_party(&self, party_id: i32, user_id: i32) -> Result<Party, Box<dyn Error>> {
        let body = serde_json::json!({ "party_id": party_id, "user_id": user_id });
        let response = self.http.post(self.destination("party/join")).json(&body).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    pub fn delete_leave_party(&self, user_id: i32) -> Result<(), Box<dyn Error>> {
        self.http.delete(self.destination("party/leave")).json(&user_id).send()?;
        Ok(())
    }

    pub fn get_party_for_user(&self, user_id: i32) -> Result<Party, Box<dyn Error>> {
        let response = self.http.get(self.destination(&format!("party/active/{user_id}"))).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

    pub fn get_party_members_for_party(&self, party_id: i32) -> Result<Vec<CharacterWrapper>, Box<dyn Error>> {
        let response = self.http.get(self.destination(&format!("party/{party_id}/members"))).send()?.text()?;
        Ok(serde_json::from_str(&response)?)
    }

}
