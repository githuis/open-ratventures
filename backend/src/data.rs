use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::quest_data::Quest;

pub const MAX_PARTY_SIZE: usize = 3;
pub const MAX_COMBAT_ENEMIES: usize = 5;
pub const MAX_ENCOUNTER_LENGTH: usize = 3;

#[derive(Clone, Debug)]
pub struct ServerState {
    pub users: Vec<Option<User>>,
    pub characters: Vec<Option<Character>>,
    pub quests: Vec<Option<Quest>>,
}

pub type SharedState = Arc<RwLock<ServerState>>;
//pub type SharedDb = Arc<RwLock<SqliteConnection>>;


#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[derive(sqlx::FromRow)]
pub struct User {
    pub id: i32,
    pub username: String,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Character {
    pub id: i32,
    pub user_id: i32,
    pub unit: Unit,
    pub experience: u32,
    pub coins: u32,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Unit {
    pub id: i32,
    pub stats: Stats,
    pub max_stats: Stats,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub id: i32,
    pub health: i32,
    pub energy: i32,
}

pub struct Item {
    pub id: i32,
    pub name: String,
}

impl Unit {
    pub fn new_lvl_one() -> Unit {
        Unit {
            id: 0,
            stats: Stats {
                id: 0,
                health: 10,
                energy: 10,
            },
            max_stats: Stats {
                id: 0,
                health: 15,
                energy: 15,
            },
        }
    }
}

impl Character {
    pub fn new(user_id: &i32) -> Character {
        Character{
            id: 0,
            user_id: *user_id,
            unit: Unit::new_lvl_one(),
            experience: 0,
            coins: 0,
        }
    }
}
