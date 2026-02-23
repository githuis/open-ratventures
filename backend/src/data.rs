use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::quest_data::Quest;

pub const MAX_PARTY_SIZE: usize = 3;
pub const MAX_COMBAT_ENEMIES: usize = 5;
pub const MAX_ENCOUNTER_LENGTH: usize = 3;


#[derive(Clone, Debug)]
pub struct ServerState {
    pub users: [Option<User>; 100],
    pub characters: [Option<Character>; 100],
    pub quests: [Option<Quest>; 100],
}

pub type SharedState = Arc<RwLock<ServerState>>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub characters: [Character; 1],
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Character {
    pub unit: Unit,
    pub experience: u32,
    pub coins: u32,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Unit {
    pub stats: Stats,
    pub max_stats: Stats,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub health: i32,
    pub energy: i32,
}

pub struct Item {
    pub name: String,
}

impl Unit {
    pub fn new_lvl_one() -> Unit {
        Unit {
            stats: Stats {
                health: 10,
                energy: 10,
            },
            max_stats: Stats {
                health: 15,
                energy: 15,
            },
        }
    }
}

impl Character {
    pub fn new() -> Character {
        Character {
            unit: Unit::new_lvl_one(),
            experience: 0,
            coins: 0,
        }
    }
}
