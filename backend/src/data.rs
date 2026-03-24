use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use crate::quest_data::Quest;

pub const MAX_PARTY_SIZE: usize = 3;
pub const MAX_COMBAT_ENEMIES: usize = 5;
pub const MAX_ENCOUNTER_LENGTH: usize = 10;

pub const FANTASY_NAMES: &[&str] = &[
    "Aelindra", "Borruk", "Caelith", "Draveth", "Eldwyn", "Faelorn", "Gorgrond", "Halvir",
    "Ithara", "Jorvak", "Kaelthas", "Lyndrel", "Morthak", "Naevris", "Orvyn", "Pyrath",
    "Quellyn", "Rhovast", "Sylvara", "Thundrek", "Ulvara", "Vexmor", "Wyndrel", "Xalvir",
    "Ysolde", "Zephrak", "Aevorn", "Bryndis", "Corvath", "Duskmere", "Eryndel", "Fjalrik",
    "Graeven", "Heldrak", "Ivrath", "Jyndra", "Kolveth", "Liraeth", "Mordwyn", "Nyravel",
    "Orvindel", "Praeven", "Quelrath", "Ryndara", "Selvorn", "Tavrek", "Ulindra", "Vormath",
    "Wyrndel", "Xevrath", "Yldren", "Zorvak", "Aldrath", "Belvara", "Cryndel", "Durmorak",
    "Evelorn", "Fyrveth", "Galdrak", "Hyndrel", "Isvorn", "Jyrrath", "Kaevrik", "Lyndrak",
    "Molveth", "Naeldris", "Olvarak", "Pryndel", "Quelvor", "Raldris", "Sylvrek", "Thorveth",
    "Ulvrak", "Vyndara", "Wrolveth", "Xyndrel", "Yvrath", "Zaldorn", "Aevrath", "Brolvek",
    "Cyndrel", "Dravorn", "Elrath", "Fyldrak", "Golveth", "Hryndra", "Iveldris", "Jolvak",
    "Keldrath", "Lyrveth", "Morvara", "Nyldrek", "Orveth", "Praldrak", "Quyndra", "Ryldren",
    "Selvrath", "Tyndrek",
];

pub fn random_fantasy_name() -> &'static str {
    use rand::Rng;
    let idx = rand::thread_rng().gen_range(0..FANTASY_NAMES.len());
    FANTASY_NAMES[idx]
}

#[derive(Clone, Debug)]
pub struct ServerState {
    pub users: Vec<Option<User>>,
    pub characters: Vec<Option<Character>>,
    pub quests: Vec<Option<Quest>>,
}

pub type SharedState = Arc<RwLock<ServerState>>;
//pub type SharedDb = Arc<RwLock<SqliteConnection>>;

#[derive(Clone, Debug, Default, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i32,
    pub username: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, sqlx::FromRow, sqlx::Type)]
pub struct Character {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub experience: u32,
    pub coins: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CharacterWrapper {
    pub character: Character,
    pub unit: Unit,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, sqlx::FromRow, sqlx::Type)]
pub struct Unit {
    pub id: i32,
    pub ref_id: i32,
    pub health: i32,
    pub energy: i32,
    pub max_health: i32,
    pub max_energy: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ItemEffect {
    Damage(i32),
    Heal(i32),
    FullHeal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub effect: ItemEffect,
    pub charges: i32, // -1 = infinite, >0 = limited uses
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventoryItem {
    pub item: Item,
    pub charges_remaining: i32, // -1 = infinite
}

impl Unit {
    pub fn new_lvl_one(parent: &i32) -> Unit {
        Unit {
            id: 0,
            ref_id: *parent,
            health: 10,
            energy: 10,
            max_health: 15,
            max_energy: 15,
        }
    }
}

impl Character {
    pub fn new(user_id: &i32) -> Character {
        let id_value = 0;
        Character {
            id: id_value,
            user_id: *user_id,
            name: String::new(),
            experience: 0,
            coins: 0,
        }
    }
}
