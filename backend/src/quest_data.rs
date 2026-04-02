pub use ratback_types::quest_data::*;

use ratback_types::data::Item;
use serde::Deserialize;

pub enum CombatAction {
    WeaponAttack,
    UseItem(Item),
}

#[derive(Deserialize)]
pub struct JoinPartyRequest {
    pub party_id: i32,
    pub user_id: i32,
}

#[derive(Deserialize)]
pub struct JoinQuestRequest {
    pub quest_id: i32,
    pub user_id: i32,
}

#[derive(Deserialize)]
pub struct CompleteQuestRequest {
    pub quest_id: i32,
    pub user_id: i32,
}

#[derive(Deserialize)]
pub struct UpdateEncountersRequest {
    pub current_encounter: i32,
    pub current_node_id: Option<String>,
    pub encounters: Vec<Encounter>,
}

#[derive(Deserialize)]
pub struct GiveClueRequest {
    pub character_id: i32,
    pub clue_id: String,
}
