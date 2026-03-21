use std::{collections::HashMap, error::Error};

use ratback::{data::{Character, CharacterWrapper, User}, quest_data::{Dialogue, Quest}};
use reqwest::blocking::Client;

const HOST: &str = "http://localhost:3000/api/";

#[derive(Debug, Default)]
pub struct Rattp {
    pub http: Client,
}

impl Rattp {

    fn destination(path: &str) -> String {
        let mut destination = HOST.to_string();
        destination.push_str(&path);

        destination
    }

    pub fn get_hello(&self) -> Result<String, Box<dyn Error>> {
        let response: String = self.http.get(Self::destination("hello-world")).send()?.text()?;

        Ok(response)
    }

    pub fn post_register_user(&self, username: String) -> Result<User, Box<dyn Error>> {
        let response = self.http.post(Self::destination("register")).body(username).send()?.text()?;

        let usr: User = serde_json::from_str(&response)?;

        Ok(usr)
    }

    pub fn post_new_character(&self, user_id: &i32) -> Result<CharacterWrapper, Box<dyn Error>> {

        let response = self.http.post(Self::destination("character")).body(user_id.to_string()).send()?.text()?;

        let character: CharacterWrapper = serde_json::from_str(&response)?;

        Ok(character)
    }
    
    pub fn get_dialogue(&self, id: &str) -> Result<Dialogue, Box<dyn Error>> {
        let response = self.http.get(Self::destination(&format!("dialogue/{id}"))).send()?.text()?;
        let dialogue: Dialogue = serde_json::from_str(&response)?;
        Ok(dialogue)
    }

    pub(crate) fn post_new_quest(&self, user_id: i32) -> Result<Quest, Box<dyn Error>> {
        let response = self.http.post(Self::destination("quest")).json(&user_id).send()?.text()?;

        let quest: Quest = serde_json::from_str(&response)?;

        Ok(quest)
    }

    /*
    
    pub(crate) fn post_new<'a, T>(&self, typ: &'a T, path: String) -> Result<T, Box<dyn Error>> 
        where T: serde::Deserialize<'a>
    {
        let response = self.http.post(Self::destination(&path)).send()?.text()?;

        let quest: T = serde_json::from_str(&response)?;

        Ok(quest)
    }
     */
}
