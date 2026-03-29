pub use ratback_types::data::*;

use std::sync::{Arc, RwLock};
use ratback_types::quest_data::Quest;

#[derive(Clone, Debug)]
pub struct ServerState {
    pub users: Vec<Option<User>>,
    pub characters: Vec<Option<Character>>,
    pub quests: Vec<Option<Quest>>,
}

pub type SharedState = Arc<RwLock<ServerState>>;
